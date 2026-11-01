//! 播放引擎：rodio 实现 + 线程通道封装
//!
//! rodio 的 OutputStream/Sink 在 Windows 上非 Send（COM STA 绑定），
//! 因此引擎运行在专用线程，PlayerHandle 通过 mpsc 通道发送命令。

use std::f64::consts::PI;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::error::{AppError, AppResult};
use crate::library::sacd::{cached_dsf_path, ensure_sacd_cache_fresh, SacdDisc};
use crate::player::analyser::{AnalyserProcessor, AnalyserSource};

/// symphonia 原生可解码的扩展名；其余格式走 ffmpeg 子进程解码为 PCM。
///
/// 平台差异（Linux，rodio 0.19 原生解码器实际构成）：
///  - mp3  → symphonia-bundle-mp3；
///  - flac → claxon；wav → hound；ogg/oga(vorbis) → lewton；
///  - **aac / m4a / opus 在 rodio 0.19 中没有对应解码器**（symphonia 仅启用
///    mp3 bundle；lewton 仅支持 Vorbis）。这三个扩展名留在原生表会直达
///    `rodio::Decoder::new` 并解码失败（"不支持的音频格式或文件损坏"），
///    连累封装于 .m4a 的 Apple Lossless（ALAC）无损曲目也无法播放。
///
/// 因此 Linux 将 aac/m4a/opus 分流到 ffmpeg 管道：ffmpeg 可用时秒级出声、
/// 可 seek、支持预加载（与 ape/dsd 等冷门格式同一链路）；不可用时不进原生
/// 引擎（避免必败探测），由 `track_needs_compat` 交前端 WASM 兜底。
///
/// Windows 侧保持原表不变——其 aac/m4a/opus 走既有链路（原生失败后由前端
/// 兼容层兜底），行为零变化。
#[cfg(target_os = "linux")]
const NATIVE_EXT: &[&str] = &["mp3", "flac", "wav", "ogg", "oga"];

#[cfg(not(target_os = "linux"))]
const NATIVE_EXT: &[&str] = &["mp3", "flac", "wav", "ogg", "oga", "opus", "aac", "m4a"];

/// 实验室·音频可视化开关（与主程序完全隔离的原子标志）。
///
/// 关闭时播放链中 `AnalyserSource` 仅做直通（一次原子读），零成本；开启时由
/// `lab_set_feature` 原子翻转，即时生效，无需重建播放链、无爆音。
pub static AUDIO_VISUALIZER: AtomicBool = AtomicBool::new(false);

/// 启动期由 `run` 注入的全局 `AppHandle`，供播放链各 Source 发射实验室事件
/// （如 `audio-analyser`）。以 `OnceLock` 持有，避免改动 `spawn_engine` 签名。
pub static ENGINE_APP: OnceLock<AppHandle> = OnceLock::new();

/// ffmpeg 可解码的补充格式（ape/dsd/dts/wma/wv/tta/tak/aiff 等）。
/// Linux 追加 aac/m4a/opus：rodio 无对应原生解码器，改由 ffmpeg 管道解码
/// （含 Apple Lossless 封装于 .m4a 的无损曲目）；ffmpeg 可用时这些格式
/// 计入 `player_capabilities` 的 supportedFormats。
#[cfg(target_os = "linux")]
const FFMPEG_EXT: &[&str] = &[
    "wma", "aif", "aiff", "ape", "wv", "tta", "tak", "dsf", "dff", "dts", "aac", "m4a", "opus",
    "iso-dsd",
];

#[cfg(not(target_os = "linux"))]
const FFMPEG_EXT: &[&str] = &["wma", "aif", "aiff", "ape", "wv", "tta", "tak", "dsf", "dff", "dts", "iso-dsd"];

/// DSD（SACD / .dsf / .dff）解码时追加的低通截止频率（Hz）。
///
/// 背景：部分 SACD 母带的 DSD 噪声底偏高（4-24kHz 有台阶式能量，实测比干净重制 DSF
/// 高数倍，且与音量无关 = 固定噪声底），ffmpeg 的 `dsd_lsbf` 多级抽取又会把 >20kHz 的
/// 超声噪声混叠回可闻段，二者叠加表现为盖住音乐的"嘶声/噪音"。在抽取前加一级 lowpass
/// 抑制。取值在"抑制嘶声"与"保留高频细节"间权衡：
///   10k 更干净（默认， SACD 噪声底主要在 4-10k 以上）
///   16k 更亮/细节更多但噪声更明显
///   8k  极干净但偏闷（损失更多高频空气感）
/// 不同 SACD 噪声强度不同，可依听感调整本常量。
const DSD_LOWPASS_HZ: u32 = 10000;

/// DTS 位流同步字表（与 ffmpeg dca_parser 一致，按大端 32-bit 读入后比较）
const DTS_SYNC_WORDS: &[u32] = &[
    0x7FFE8001, // 16-bit big-endian（DTS-CD / 常规裸流最常见）
    0xFF1F00E8, // 14-bit big-endian
    0xE8001FFF, // 14-bit little-endian
    0x0180FE7F, // 16-bit little-endian
    0x64582025, // DTS-HD（XLL / 扩展子流 "dX %"）
];

/// 解析 RIFF 头，返回 data chunk 数据区在 head 内的偏移；无法解析返回 None
fn riff_data_offset(head: &[u8]) -> Option<usize> {
    if head.len() < 12 || &head[0..4] != b"RIFF" {
        return None;
    }
    let mut off = 12usize;
    while off + 8 <= head.len() {
        let id = &head[off..off + 4];
        let size = u32::from_le_bytes([head[off + 4], head[off + 5], head[off + 6], head[off + 7]])
            as usize;
        if id == b"data" {
            return Some(off + 8);
        }
        // chunk 数据区 + 2 字节对齐填充
        off += 8 + size + (size % 2);
    }
    None
}

/// data chunk 起始小窗口内扫描 DTS 同步字（容忍极端帧对齐偏移）
fn head_data_has_dts(head: &[u8]) -> bool {
    let Some(data_off) = riff_data_offset(head) else {
        return false;
    };
    let end = head.len().min(data_off + 16);
    let mut i = data_off;
    while i + 4 <= end {
        let w = u32::from_be_bytes([head[i], head[i + 1], head[i + 2], head[i + 3]]);
        if DTS_SYNC_WORDS.contains(&w) {
            return true;
        }
        i += 1;
    }
    false
}

/// 读取文件头判断是否为「WAV 容器封装 DTS」（DTS-CD/DTS-ES 伪装 WAV）。
/// 读失败按非 DTS 处理（保持原路由，由后续解码错误链路兜底）。
fn wav_head_contains_dts(path: &Path) -> bool {
    read_head_4k(path).is_some_and(|head| head_data_has_dts(&head))
}

/// 读取文件前 4KiB（WAV 头解析用，足以覆盖 fmt / LIST / data 前置 chunk）
fn read_head_4k(path: &Path) -> Option<Vec<u8>> {
    const HEAD_LEN: usize = 4096;
    let mut file = std::fs::File::open(path).ok()?;
    let mut head = vec![0u8; HEAD_LEN];
    let mut total = 0usize;
    while total < HEAD_LEN {
        match file.read(&mut head[total..]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(_) => break,
        }
    }
    head.truncate(total);
    Some(head)
}

/// 解析 RIFF 头，返回 fmt chunk 数据区在 head 内的偏移；无法解析返回 None
fn riff_fmt_offset(head: &[u8]) -> Option<usize> {
    if head.len() < 12 || &head[0..4] != b"RIFF" {
        return None;
    }
    let mut off = 12usize;
    while off + 8 <= head.len() {
        let id = &head[off..off + 4];
        let size = u32::from_le_bytes([head[off + 4], head[off + 5], head[off + 6], head[off + 7]])
            as usize;
        if id == b"fmt " {
            return Some(off + 8);
        }
        off += 8 + size + (size % 2);
    }
    None
}

/// 读取 WAV fmt chunk 声明的声道数（兼容 WAVEFORMATEXTENSIBLE）。
///
/// 用于识别「多声道 PCM WAV」（如 6.1 环绕 7ch）：rodio 0.19 的混音器假定
/// 源声道与输出设备声道一致，直接把 7ch 交错样本按 2ch 步长写入设备 buffer，
/// 结果是把相邻声道的采样点错误重排 → 播放为宽带白噪音。这类文件必须绕开
/// 原生解码，交给 ffmpeg 管道 + `PcmStreamSource` 做显式下混。
fn wav_head_channels(path: &Path) -> Option<u16> {
    let head = read_head_4k(path)?;
    let fmt = riff_fmt_offset(&head)?;
    // fmt 数据区：wFormatTag(2) nChannels(2) ...
    if fmt + 4 > head.len() {
        return None;
    }
    Some(u16::from_le_bytes([head[fmt + 2], head[fmt + 3]]))
}

/// 读取 FLAC STREAMINFO 声明的声道数。
///
/// 同上：多声道 FLAC（5.1 环绕母带常见）经 rodio/claxon 原生解码后同样会被
/// 混音器按设备 2ch 步长重排成白噪音，需改走 ffmpeg 管道显式下混。
///
/// STREAMINFO 布局（`fLaC` 后第一个 metadata block，类型 0，数据 34 字节）：
/// 偏移 12（block 内）= sample_rate 低 4 位 + (channels-1) 3 位 + bps-1 最高位。
fn flac_head_channels(path: &Path) -> Option<u16> {
    let head = read_head_4k(path)?;
    if head.len() < 21 || &head[0..4] != b"fLaC" {
        return None;
    }
    // metadata block header：bit7 = last-block，低 7 位 = 类型；类型 0 = STREAMINFO
    if head[4] & 0x7F != 0 {
        return None;
    }
    // 数据区从 offset 8 起；channels-1 位于数据区第 12 字节的 bits 3..1
    Some((((head[8 + 12] >> 1) & 0x07) as u16) + 1)
}

fn is_native_format(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let ext = ext.to_lowercase();
    if !NATIVE_EXT.contains(&ext.as_str()) {
        return false;
    }
    // WAV 容器深检：fmt 声称 PCM 但数据实为 DTS 位流（如 DTS-ES 封装在 .wav 中，
    // symphonia 会按 PCM 打开并输出噪声）——识别后路由到 ffmpeg 管道解码。
    // 反向误判（真 PCM 数据开头恰好命中同步字）代价仅为换 ffmpeg 通道，播放结果一致。
    if ext == "wav" {
        if wav_head_contains_dts(path) {
            return false;
        }
        // 多声道 PCM WAV（如 6.1 环绕 7ch 无损抓轨）：rodio 混音器按设备声道步长
        // 直接重排交错样本会把环绕声拆成白噪音。强制走 ffmpeg 管道，由
        // `PcmStreamSource` 按 src_channels 显式下混到设备声道（与 5.1 ape/wv/tta
        // /dts-in-wav 同一语义）。ffmpeg 不可用时由前端兼容层兜底。
        if wav_head_channels(path).is_some_and(|ch| ch > 2) {
            return false;
        }
    }
    // FLAC 同类深检：多声道 FLAC 原生解码同样会被混音器错位成白噪音。
    if ext == "flac" && flac_head_channels(path).is_some_and(|ch| ch > 2) {
        return false;
    }
    true
}

/// 外部工具定位统一委托给 `crate::tools`（查找顺序：公共依赖目录 → 安装目录 →
/// 系统固定路径 → PATH）。此处保留薄封装，避免改动全部调用点。
pub(crate) use crate::tools::resolve_bin_tool;

/// 定位 FFmpeg
pub fn resolve_ffmpeg() -> Option<PathBuf> {
    crate::tools::resolve_ffmpeg()
}

/// 定位 FFprobe
pub fn resolve_ffprobe() -> Option<PathBuf> {
    crate::tools::resolve_ffprobe()
}

/// 探测音频流参数（是否 DSD、采样率、声道数），用于动态构建 ffmpeg 解码命令，
/// 避免写死 `-ar/-ac`（不同格式/文件的采样率与声道数各异）。ffprobe 缺失或解析
/// 失败返回 None，由调用方回退到扩展名判定与安全默认值。
fn probe_audio_stream(path: &Path) -> Option<(bool, u32, u16)> {
    // 缓存键含文件大小与修改时间：文件被替换/重新编码后自动失效。
    // 命中缓存可完全避免 fork ffprobe（Windows 冷启动可达 100ms 级），
    // 这是引擎线程在切歌/预加载期间保持响应的关键——探测与解码准备都在
    // 引擎线程内同步执行，任何多余子进程都会延长 player 锁的持有时间。
    let key = std::fs::metadata(path)
        .ok()
        .map(|m| {
            let mtime = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            (path.to_path_buf(), m.len(), mtime)
        });
    if let Some(key) = &key {
        if let Ok(cache) = probe_cache().lock() {
            if let Some(hit) = cache.get(key) {
                return Some(*hit);
            }
        }
    }

    let ffprobe = resolve_ffprobe()?;
    let out = crate::util::hidden_command(ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=codec_name,sample_rate,channels",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    // 只取首行并按逗号切分：多流/带 side_data 的输出会有多行，若整体 split
    // 会让字段错位，进而推出错误的声道数（→ ffmpeg 输出与 PcmStreamSource
    // 假设的声道不一致 → 逐帧错位成白噪音）。
    let first = s.lines().next().unwrap_or("").trim();
    let f: Vec<&str> = first.split(',').map(|x| x.trim()).collect();
    if f.len() < 3 {
        return None;
    }
    let is_dsd = f[0].starts_with("dsd");
    let rate: u32 = f[1].parse().ok()?;
    let ch: u16 = f[2].parse().ok()?;
    if let Some(key) = key {
        if let Ok(mut cache) = probe_cache().lock() {
            if cache.len() > 4096 {
                cache.clear();
            }
            cache.insert(key, (is_dsd, rate, ch));
        }
    }
    Some((is_dsd, rate, ch))
}

/// ffprobe 探测缓存（路径 + 大小 + 修改时间 → 参数）。
fn probe_cache(
) -> &'static std::sync::Mutex<std::collections::HashMap<(std::path::PathBuf, u64, u64), (bool, u32, u16)>>
{
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<(std::path::PathBuf, u64, u64), (bool, u32, u16)>>,
    > = std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// ffprobe 取到的音频流编码信息（用于 DTS 子类型识别与 lofty 未解析格式的补充）。
#[derive(Debug, Clone)]
pub struct AudioCodecInfo {
    pub codec_name: String,
    pub profile: Option<String>,
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
}

/// 用 ffprobe 取音频流的 `codec_name` / `profile` / `bit_rate` / `sample_rate`。
///
/// 用于：
///  - DTS 子类型识别（Core / HD HRA / HD MA）——lofty 无法解析 DTS 属性，必须靠 ffprobe；
///  - lofty 未给出码率 / 采样率的格式（dts/tta/tak 等）补充这些字段，使前端能正确分级。
///
/// 输出顺序与 `-show_entries` 字段顺序一致：`codec_name,profile,bit_rate,sample_rate`
/// （profile 缺失时为空串，按空处理）。工具缺失 / 解析失败返回 None。
///
/// 注：扫描阶段调用，非播放引擎热路径，不做缓存（DTS 等稀有格式占比极低）。
pub fn probe_audio_codec(path: &Path) -> Option<AudioCodecInfo> {
    let ffprobe = resolve_ffprobe()?;
    let out = crate::util::hidden_command(ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=codec_name,profile,bit_rate,sample_rate",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let first = s.lines().next().unwrap_or("").trim();
    // 字段顺序：codec_name, profile, bit_rate, sample_rate（profile 可能为空串）
    let f: Vec<&str> = first.split(',').map(|x| x.trim()).collect();
    if f.is_empty() || f[0].is_empty() {
        return None;
    }
    let codec_name = f[0].to_string();
    let profile = f.get(1).and_then(|p| {
        let p = p.to_string();
        if p.is_empty() {
            None
        } else {
            Some(p)
        }
    });
    let bitrate = f.get(2).and_then(|b| b.parse::<u32>().ok());
    let sample_rate = f.get(3).and_then(|r| r.parse::<u32>().ok());
    Some(AudioCodecInfo {
        codec_name,
        profile,
        bitrate,
        sample_rate,
    })
}

pub fn ffmpeg_available() -> bool {
    resolve_ffmpeg().is_some()
}

/// 用 ffprobe 探测音频时长（秒）。
/// 用于 lofty 无法解析的格式（dsf/dts/tta/tak/dff 等）在扫描与播放时的兜底；
/// 失败（工具缺失/输出异常/时长非正）返回 None。
///
/// 探测顺序（跨平台一致）：
///  1. ffprobe（机器可读输出、开销最小）——随包分发或系统安装时优先；
///  2. 回退 `ffmpeg -i` 解析 stderr 的 `Duration:` 行（依赖 ffmpeg 含对应解复用器，
///     如 dff/dsf 的 DSD 时长也能探测）。ffprobe 缺失或解析失败时自动走此路径，
///     使仅随包分发 ffmpeg（如 arm64 无 ffprobe 二进制）或精简版 ffprobe 的场景
///     也能完成时长探测。
pub fn probe_duration(path: &Path) -> Option<f64> {
    // Linux：曲库路径可能是非 UTF-8 文件名的容错编码，先还原真实路径
    // （合法路径零成本直连），否则 dsf/dff/tta/tak 等时长探测全部失败、
    // 进度条不推进、自动连播失效。
    #[cfg(target_os = "linux")]
    let path = &crate::library::scan_linux::resolve_path_str(&path.to_string_lossy());

    // 1) ffprobe 首选
    if let Some(ffprobe) = resolve_ffprobe() {
        let mut cmd = crate::util::hidden_command(ffprobe);
        cmd.arg("-v")
            .arg("error")
            .arg("-show_entries")
            .arg("format=duration")
            .arg("-of")
            .arg("csv=p=0")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            // CREATE_NO_WINDOW：避免探测时弹出控制台窗口
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        if let Ok(output) = cmd.output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                if let Some(secs) = text.trim().parse::<f64>().ok().filter(|s| *s > 0.0) {
                    return Some(secs);
                }
            }
        }
    }

    // 2) 回退：ffprobe 不存在或解析失败时，经 `ffmpeg -i` 解析 stderr 的 `Duration:` 行。
    // 跨平台生效（依赖 ffmpeg 含对应解复用器，如 dff/dsf 的 DSD 时长也能探测）。
    probe_duration_via_ffmpeg(path)
}

/// 回退探测：`ffmpeg -i <file>` 的 stderr 含输入信息（含 `Duration:` 行），跨平台生效。
/// 解析之得到时长。未指定输出时 ffmpeg 以非零码退出属预期，忽略退出码；
/// `-t 0.001` 使 ffmpeg 读够输入信息后立即退出，避免大文件全量解码。
fn probe_duration_via_ffmpeg(path: &Path) -> Option<f64> {
    let ffmpeg = resolve_ffmpeg()?;
    let mut cmd = crate::util::hidden_command(ffmpeg);
    cmd.arg("-hide_banner")
        .arg("-i")
        .arg(path)
        .arg("-t")
        .arg("0.001")
        .arg("-f")
        .arg("null")
        .arg("-")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let output = cmd.output().ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_duration_line(&stderr)
}

/// 从 `ffmpeg -i` 的 stderr 中解析 `Duration: HH:MM:SS.ss`。
/// 行格式：`  Duration: 00:02:00.65, start: 0.000000, bitrate: ...`；
/// 无 Duration 行、值为 N/A 或段数异常时返回 None。
fn parse_duration_line(stderr: &str) -> Option<f64> {
    for line in stderr.lines() {
        let Some(rest) = line.trim().strip_prefix("Duration:") else {
            continue;
        };
        // 取逗号前的时间值（"00:02:00.65"）
        let value = rest.trim().split(',').next()?.trim();
        let mut parts = value.split(':');
        let hours: f64 = parts.next()?.trim().parse().ok()?;
        let minutes: f64 = parts.next()?.trim().parse().ok()?;
        let seconds: f64 = parts.next()?.trim().parse().ok()?;
        if parts.next().is_some() {
            return None; // 段数异常，防御
        }
        let secs = hours * 3600.0 + minutes * 60.0 + seconds;
        return (secs > 0.0).then_some(secs);
    }
    None
}

/// ffmpeg 解码输出的后端支持格式清单（供 capabilities 使用）
pub fn ffmpeg_formats() -> Vec<String> {
    FFMPEG_EXT.iter().map(|s| s.to_string()).collect()
}

/// 播放器能力声明
#[derive(Debug, Clone, Serialize)]
pub struct EngineCapabilities {
    pub seek: bool,
    #[serde(rename = "volumeControl")]
    pub volume_control: bool,
    #[serde(rename = "rateControl")]
    pub rate_control: bool,
    #[serde(rename = "supportedFormats")]
    pub supported_formats: Vec<String>,
}

/// 发往引擎线程的命令
enum EngineCommand {
    /// 播放（附回执通道：打开/解码是否成功同步回传，失败可转为 error 事件）
    /// 末参为可选 CUE 分轨区间 (start_sec, duration_sec)：非 None 时按偏移定位播放
    Play(PathBuf, Option<(f32, f32)>, Option<(f64, f64)>, Option<u32>, mpsc::Sender<AppResult<()>>),
    /// 播放并定位到指定毫秒（断点续播专用：等混音线程起流后再 seek，规避启动竞态）
    PlayAt(PathBuf, u64, Option<(f32, f32)>, mpsc::Sender<AppResult<()>>),
    Pause,
    Resume,
    Stop,
    Seek(u64),
    SetVolume(f32),
    SetMuted(bool),
    /// 当前 sink 是否已播完（无音源在播）：供状态机兜底时长未知（0）时的结束检测
    IsEnded(mpsc::Sender<bool>),
    /// 更新交叉淡化配置（启用 + 时长 ms）
    SetCrossfade(bool, u64),
    /// 预解码下一曲到待命链路（不发声；切歌时 SwapToNext 无缝接管）
    /// 末参为可选 CUE 分轨区间 (start_sec, duration_sec)
    Preload(PathBuf, Option<(f32, f32)>, Option<(f64, f64)>, Option<u32>, mpsc::Sender<AppResult<()>>),
    /// 把预加载链路提升为当前曲目（旧曲目转入淡出）
    SwapToNext(mpsc::Sender<AppResult<()>>),
    /// 更新音量归一化 ReplayGain 开关（开启时当前曲目立即应用增益）
    SetReplayGain(bool),
    /// 更新播放倍速（1.0 = 原速）；立即作用于当前曲目与待命预加载链路
    SetRate(f32),
    /// 更新实验室均衡器配置（频段增益 + 总开关）；新建/接管链路时应用，已在播的链路
    /// 通过 EqProcessor::set_config 热更新（下一帧即生效）。
    SetEq(EqConfig),
}

/// 引擎句柄（Send）：PlayerHandle 通过它控制播放
#[derive(Clone)]
pub struct EngineHandle {
    tx: mpsc::Sender<EngineCommand>,
}

impl EngineHandle {
    pub fn play(
        &self,
        path: PathBuf,
        gain: Option<(f32, f32)>,
        range: Option<(f64, f64)>,
        sacd_track_index: Option<u32>,
    ) -> AppResult<()> {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.tx
            .send(EngineCommand::Play(path, gain, range, sacd_track_index, ack_tx))
            .map_err(|_| AppError::Player("引擎线程已退出".into()))?;
        ack_rx
            .recv()
            .map_err(|_| AppError::Player("引擎线程已退出".into()))?
    }
    pub fn play_at(&self, path: PathBuf, position_ms: u64, gain: Option<(f32, f32)>) -> AppResult<()> {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.tx
            .send(EngineCommand::PlayAt(path, position_ms, gain, ack_tx))
            .map_err(|_| AppError::Player("引擎线程已退出".into()))?;
        ack_rx
            .recv()
            .map_err(|_| AppError::Player("引擎线程已退出".into()))?
    }
    pub fn pause(&self) -> AppResult<()> {
        self.tx
            .send(EngineCommand::Pause)
            .map_err(|_| AppError::Player("引擎线程已退出".into()))
    }
    pub fn resume(&self) -> AppResult<()> {
        self.tx
            .send(EngineCommand::Resume)
            .map_err(|_| AppError::Player("引擎线程已退出".into()))
    }
    pub fn stop(&self) -> AppResult<()> {
        self.tx
            .send(EngineCommand::Stop)
            .map_err(|_| AppError::Player("引擎线程已退出".into()))
    }
    pub fn seek(&self, ms: u64) -> AppResult<()> {
        self.tx
            .send(EngineCommand::Seek(ms))
            .map_err(|_| AppError::Player("引擎线程已退出".into()))
    }
    pub fn set_volume(&self, v: f32) -> AppResult<()> {
        self.tx
            .send(EngineCommand::SetVolume(v))
            .map_err(|_| AppError::Player("引擎线程已退出".into()))
    }
    pub fn set_muted(&self, m: bool) -> AppResult<()> {
        self.tx
            .send(EngineCommand::SetMuted(m))
            .map_err(|_| AppError::Player("引擎线程已退出".into()))
    }
    /// 当前曲目是否已在引擎侧播完（sink 无音源）
    pub fn is_current_ended(&self) -> bool {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.tx
            .send(EngineCommand::IsEnded(ack_tx))
            .map(|_| ack_rx.recv().unwrap_or(false))
            .unwrap_or(false)
    }

    /// 更新交叉淡化参数（启用 + 时长 ms）
    pub fn set_crossfade(&self, enabled: bool, duration_ms: u64) {
        let _ = self.tx.send(EngineCommand::SetCrossfade(enabled, duration_ms));
    }

    /// 预解码下一曲到待命链路（不发声；切歌时 swap_to_next 无缝接管）
    pub fn preload(
        &self,
        path: std::path::PathBuf,
        gain: Option<(f32, f32)>,
        range: Option<(f64, f64)>,
        sacd_track_index: Option<u32>,
    ) -> AppResult<()> {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.tx
            .send(EngineCommand::Preload(path, gain, range, sacd_track_index, ack_tx))
            .map_err(|_| AppError::Player("引擎线程已退出".into()))?;
        ack_rx
            .recv()
            .map_err(|_| AppError::Player("引擎线程已退出".into()))?
    }

    /// 更新音量归一化 ReplayGain 开关（开启时当前曲目立即应用增益）
    pub fn set_replaygain(&self, enabled: bool) {
        let _ = self.tx.send(EngineCommand::SetReplayGain(enabled));
    }

    /// 更新播放倍速（1.0 = 原速）；新曲目装载时也会自动沿用该速率
    pub fn set_rate(&self, rate: f32) {
        let _ = self.tx.send(EngineCommand::SetRate(rate));
    }

    /// 更新实验室均衡器配置（频段增益 + 总开关）；新曲目装载时沿用新配置。
    pub fn set_eq(&self, cfg: EqConfig) {
        let _ = self.tx.send(EngineCommand::SetEq(cfg));
    }

    /// 把预加载链路提升为当前曲目（旧曲目转入淡出）
    pub fn swap_to_next(&self) -> AppResult<()> {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.tx
            .send(EngineCommand::SwapToNext(ack_tx))
            .map_err(|_| AppError::Player("引擎线程已退出".into()))?;
        ack_rx
            .recv()
            .map_err(|_| AppError::Player("引擎线程已退出".into()))?
    }
}

/// 启动引擎线程，返回可发送命令的句柄
pub fn spawn_engine() -> EngineHandle {
    let (tx, rx) = mpsc::channel::<EngineCommand>();
    std::thread::Builder::new()
    .name("audio-engine".into())
    .spawn(move || {
        let mut engine = RodioEngine::new();
            // 引擎级交叉淡化配置：是否启用 + 时长（ms）
            // 初始值与 `RodioEngine::fade_ms` 及 config 默认一致（1000ms），
            // 避免 set_crossfade 下发前两者不同步导致淡出起点按错误时长计算。
            let mut fade_enabled = true;
            let mut fade_duration_ms: u64 = 1000;
            // 带超时轮询：无命令期间也能步进驱动淡入淡出。
            // 淡变进行中用 5ms 步进（与 rodio `periodic_access` 同频，音量阶跃
            // 足够细，听感不再有 20ms 阶梯造成的"沙沙"调制噪声）；
            // 空闲时退回 20ms，避免空转唤醒。
            loop {
                let tick_ms = if engine.fading_active() { 5 } else { 20 };
                match rx.recv_timeout(Duration::from_millis(tick_ms)) {
                    Ok(cmd) => match cmd {
                        EngineCommand::Play(path, gain, range, sacd, ack) => {
                            let _ = ack.send(engine.play(&path, gain, range, sacd));
                        }
                        EngineCommand::PlayAt(path, pos_ms, gain, ack) => {
                            let _ = ack.send(engine.play_at(&path, pos_ms, gain));
                        }
                        EngineCommand::Pause => {
                            let _ = engine.pause();
                        }
                        EngineCommand::Resume => {
                            let _ = engine.resume();
                        }
                        EngineCommand::Stop => {
                            let _ = engine.stop();
                        }
                        EngineCommand::Seek(ms) => {
                            let _ = engine.seek(ms);
                        }
                        EngineCommand::SetVolume(v) => {
                            let _ = engine.set_volume(v);
                        }
                        EngineCommand::SetMuted(m) => {
                            let _ = engine.set_muted(m);
                        }
                        EngineCommand::IsEnded(ack) => {
                            let _ = ack.send(engine.current_ended());
                        }
                        EngineCommand::SetCrossfade(enabled, duration_ms) => {
                            fade_enabled = enabled;
                            fade_duration_ms = duration_ms;
                            engine.fade_ms = duration_ms;
                        }
                        EngineCommand::Preload(path, gain, range, sacd, ack) => {
                            let _ = ack.send(engine.preload(&path, gain, range, sacd));
                        }
                        EngineCommand::SwapToNext(ack) => {
                            let _ = ack.send(engine.swap_to_next());
                        }
                        EngineCommand::SetReplayGain(enabled) => {
                            engine.replaygain_enabled = enabled;
                            // 开关变化时当前曲目立即按新策略重算目标音量
                            if engine.fade_in_started.is_none() {
                                engine.apply_volume();
                            }
                        }
                        EngineCommand::SetRate(rate) => {
                            let _ = engine.set_rate(rate);
                        }
                        EngineCommand::SetEq(cfg) => {
                            engine.set_eq(cfg);
                        }
                    },
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
                if fade_enabled {
                    engine.tick_fades(fade_duration_ms);
                } else {
                    engine.tick_fades(0);
                }
            }
        })
        .expect("failed to spawn audio engine thread");
    EngineHandle { tx }
}

/// 一条播放链路：sink + 其专属 ffmpeg 子进程（非原生格式）。
/// 交叉淡化期间新旧两条链路同时存活。
struct Playback {
    sink: rodio::Sink,
    ffmpeg_child: Option<Child>,
    /// 本曲目生效的 ReplayGain 音量倍率（用户音量之外的乘数）
    gain_scale: f32,
    /// SACD 解码导出的临时 DSF 路径；seek 时直接对该 DSF 重启 ffmpeg 进程定位
    temp_file: Option<PathBuf>,
    /// CUE 分轨范围 (起始毫秒, 时长毫秒)；seek 时用于叠加基址并以剩余时长限长，
    /// 避免越界到下一轨
    range: Option<(u64, u64)>,
}

impl Playback {
    /// 立即终止播放（sink 停止拉流，ffmpeg 子进程随后退出；Drop 再 kill 兜底）
    fn halt(mut self) {
        self.sink.stop();
        if let Some(child) = self.ffmpeg_child.take() {
            reap_child(child);
        }
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        if let Some(child) = self.ffmpeg_child.take() {
            reap_child(child);
        }
    }
}

/// 结束 ffmpeg 子进程并对回收动作设上限。
///
/// 旧实现 kill 后直接 `wait()`：若 ffmpeg 正卡在磁盘 I/O（大文件/网络盘），
/// 引擎线程会被无限期挂住，期间所有播放命令（暂停/音量/seek/连播判定）
/// 全部排队，前端表现为界面无响应。此处改为最多轮询 50ms，超时即放手
/// 交给 `Drop`/系统回收。
fn reap_child(mut child: Child) {
    let _ = child.kill();
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(50);
    loop {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => break,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
    }
}

/// 正在淡出的旧曲目
struct FadingOut {
    playback: Playback,
    started: Instant,
    from: f32,
}

/// rodio 引擎本体（非 Send，仅在引擎线程内使用）
struct RodioEngine {
    /// 输出设备流：整个引擎生命周期复用一条（多个 sink 可挂在同一 handle 上），
    /// 避免每次切歌重建输出设备造成的爆音
    stream: Option<rodio::OutputStream>,
    handle: Option<rodio::OutputStreamHandle>,
    /// 当前曲目
    current: Option<Playback>,
    /// 切歌时正在淡出的上一曲目
    fading_out: Option<FadingOut>,
    /// 当前曲目淡入起点（播放 / seek 后短暂存在）
    fade_in_started: Option<Instant>,
    volume: f32,
    muted: bool,
    /// 当前曲目路径（ffmpeg 后端 seek 需带 -ss 重启进程）
    current_path: Option<PathBuf>,
    /// 当前曲目是否走 ffmpeg 管道（决定 seek 方式）
    current_is_ffmpeg: bool,
    /// 预加载的下一曲链路（sink 暂停待命；SwapToNext 时接管）
    next: Option<Playback>,
    /// 预加载链路对应的路径（play 时据此判断是否复用）
    next_path: Option<PathBuf>,
    /// 当前交叉淡化时长（ms）；SetCrossfade 更新，切歌淡出起点计算使用
    fade_ms: u64,
    /// 音量归一化 ReplayGain 开关（默认开启，与设置默认一致）
    replaygain_enabled: bool,
    /// 当前播放倍速（1.0 = 原速；新建/接管链路时下发到 sink）
    rate: f32,
    /// 是否处于用户暂停态（seek 时据此决定是否恢复出声，避免"暂停中拖进度条却开始播放"）
    paused: bool,
    /// 实验室均衡器处理器（**与所有音源共享**）。
    ///
    /// `PcmStreamSource` / `EqSource` 各持一份 `Arc` 引用，故 `set_eq` 更新它即可
    /// 让**正在播放**的曲目立即生效；滤波器状态不重置、系数走插值，不会爆音。
    /// 旧实现是每轨独占一份 `EqProcessor`，改动只能等下一首装载时生效。
    eq: Arc<Mutex<EqProcessor>>,
    /// 实验室·音频可视化处理器（与所有音源共享，按帧算频谱）。仅在可视化开启时
    /// 由 `AnalyserSource` 喂样，关闭时为空转 pass-through，对输出零影响。
    viz_proc: Arc<Mutex<AnalyserProcessor>>,
}

impl RodioEngine {
    fn new() -> Self {
        // 尽早打开输出设备；失败（如无音频设备）则延迟到首次 play 重试
        let (stream, handle) = match rodio::OutputStream::try_default() {
            Ok((stream, handle)) => (Some(stream), Some(handle)),
            Err(_) => (None, None),
        };
        Self {
            stream,
            handle,
            current: None,
            fading_out: None,
            fade_in_started: None,
            volume: 1.0,
            muted: false,
            current_path: None,
            current_is_ffmpeg: false,
            next: None,
            next_path: None,
            fade_ms: 1000,
            replaygain_enabled: true,
            rate: 1.0,
            paused: false,
            // 初始为默认（关闭）配置；启动时由 lib.rs 用 config.json 的 eq 段覆盖
            eq: Arc::new(Mutex::new(EqProcessor::new(
                44100,
                2,
                &EqConfig::default(),
            ))),
            // 可视化处理器：关闭时不被喂样，为空转 pass-through
            viz_proc: Arc::new(Mutex::new(AnalyserProcessor::new())),
        }
    }

    /// 由 ReplayGain 标签/分析结果计算本曲目音量倍率（用户音量之外）
    /// gain = (增益 dB, 峰值线性)；关闭开关或无数据时返回 1.0（不归一化）。
    /// 规则：放大受峰值防削波限制（scale <= 1/peak），并整体限制在 [0.05, 4.0]
    /// 防极端曲目把音量推到听感失衡。
    fn gain_scale_for(&self, gain: Option<(f32, f32)>) -> f32 {
        if !self.replaygain_enabled {
            return 1.0;
        }
        let Some((gain_db, peak)) = gain else {
            return 1.0;
        };
        let db = gain_db.clamp(-24.0, 24.0);
        let mut scale = 10.0_f32.powf(db / 20.0);
        if peak.is_finite() && peak > 0.0 {
            scale = scale.min(1.0 / peak);
        }
        scale.clamp(0.05, 4.0)
    }

    /// 用户期望的目标音量（静音为 0，乘以当前曲目的 ReplayGain 倍率）
    fn target_volume(&self) -> f32 {
        let user = if self.muted {
            0.0
        } else {
            self.volume
        };
        let gain = self.current.as_ref().map(|p| p.gain_scale).unwrap_or(1.0);
        user * gain
    }

    /// 立即把当前 sink 设到目标音量（音量 / 静音变更时调用）
    fn apply_volume(&self) {
        if let Some(playback) = &self.current {
            playback.sink.set_volume(self.target_volume());
        }
    }

    /// 是否有淡入 / 淡出正在进行（引擎线程据此在 5ms / 20ms 步进间切换）
    fn fading_active(&self) -> bool {
        self.fade_in_started.is_some() || self.fading_out.is_some()
    }

    /// smoothstep 缓动，避免线性渐变在端点处可闻的台阶感
    fn ease(t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// 渐变驱动（每 ~20ms）：旧曲目先淡出到静默后回收，随后新曲目再从静默淡入。
    /// 任一时刻只有一个 Sink 发声，彻底绕开 rodio 多 Sink 混音产生的白噪音。
    /// fade_duration 为当前生效的交叉淡化时长（ms）；0 时关闭淡化。
    fn tick_fades(&mut self, fade_duration_ms: u64) {
        let fade_ms = if fade_duration_ms > 0 {
            fade_duration_ms
        } else {
            // 关闭淡入淡出：旧曲直接静默回收，新曲直接设满音量
            if let Some(fading) = &self.fading_out {
                fading.playback.sink.set_volume(0.0);
                if let Some(done) = self.fading_out.take() {
                    done.playback.halt();
                }
            }
            if let Some(_started) = self.fade_in_started.take() {
                if let Some(playback) = &self.current {
                    playback.sink.set_volume(self.target_volume());
                }
            }
            return;
        };

        // 旧曲目淡出：优先处理，淡出完毕后 halt，随后再开始新曲淡入
        if let Some(fading) = &self.fading_out {
            let t = fading.started.elapsed().as_millis() as f32 / fade_ms as f32;
            let gain = fading.from * (1.0 - Self::ease(t));
            fading.playback.sink.set_volume(gain);
            if t >= 1.0 {
                if let Some(done) = self.fading_out.take() {
                    done.playback.halt();
                }
                // 旧曲淡出完毕，开始新曲淡入
                self.fade_in_started = Some(Instant::now());
            }
            return;
        }

        // 新曲目淡入（仅在旧曲淡出完毕后）
        if let Some(started) = self.fade_in_started {
            let t = started.elapsed().as_millis() as f32 / fade_ms as f32;
            if let Some(playback) = &self.current {
                playback.sink.set_volume(self.target_volume() * Self::ease(t));
            }
            if t >= 1.0 {
                self.fade_in_started = None;
                self.apply_volume();
            }
        }
    }

    /// 确保输出设备可用（首次播放或启动时无设备的延迟重试）
    fn ensure_stream(&mut self) -> AppResult<&rodio::OutputStreamHandle> {
        if self.handle.is_none() {
            let (stream, handle) = rodio::OutputStream::try_default()
                .map_err(|e| AppError::Player(format!("无法创建音频输出设备: {}", e)))?;
            self.stream = Some(stream);
            self.handle = Some(handle);
        }
        self.handle
            .as_ref()
            .ok_or_else(|| AppError::Player("音频输出设备不可用".into()))
    }

    fn play(
        &mut self,
        path: &std::path::Path,
        gain: Option<(f32, f32)>,
        range: Option<(f64, f64)>,
        sacd_track_index: Option<u32>,
    ) -> AppResult<()> {
        // 新曲目起播即脱离暂停态（避免上首暂停态残留导致 seek 不出声）
        self.paused = false;
        // Linux：还原非 UTF-8 文件名的容错编码路径（合法路径零成本直连），
        // 保证 NTFS 上的这类曲目可被打开、解码、播放（含后续 ffmpeg 管道）。
        #[cfg(target_os = "linux")]
        let resolved = crate::library::scan_linux::resolve_path_str(&path.to_string_lossy());
        #[cfg(target_os = "linux")]
        let path: &std::path::Path = &resolved;
        let handle = self.ensure_stream()?.clone();
        let gain_scale = self.gain_scale_for(gain);

        // 预加载链路与本曲目一致时直接无缝接管（复用已解码的下一曲）
        if self.next_path.as_deref() == Some(path) {
            return self.swap_to_next();
        }
        // 路径不一致：预加载链路已无用，立即回收
        if let Some(stale) = self.next.take() {
            stale.halt();
        }
        self.next_path = None;

        // 旧曲目转入淡出链路（交叉淡化）；淡出起点取其当前实际增益，
        // 淡入中途切歌也不会出现音量跳变
        if let Some(old) = self.current.take() {
            // 注意：必须用 `old.gain_scale`——此刻 `self.current` 已被 take()，
            // 再调 `target_volume()` 会因取不到 gain_scale 而回落 1.0，
            // 造成淡出起点比实际音量高一截（切换瞬间"炸一下"）。
            let base = if self.muted { 0.0 } else { self.volume } * old.gain_scale;
            let fade = self.fade_ms.max(1) as f32;
            let from = match &self.fade_in_started {
                Some(started) => base * Self::ease(started.elapsed().as_millis() as f32 / fade),
                None => base,
            };
            // 同一时刻只保留一条淡出链路：更早的旧链路立即回收
            if let Some(older) = self.fading_out.take() {
                older.playback.halt();
            }
            self.fading_out = Some(FadingOut {
                playback: old,
                started: Instant::now(),
                from,
            });
        }
        self.fade_in_started = None;

        let sink = rodio::Sink::try_new(&handle)
            .map_err(|e| AppError::Player(format!("无法创建音频 sink: {}", e)))?;

        let start_ms = range.map(|(s, _)| (s * 1000.0) as u64).unwrap_or(0);
        let stored_range = range
            .map(|(s, d)| ((s * 1000.0) as u64, if d > 0.0 { (d * 1000.0) as u64 } else { 0 }));
        let duration_ms = range
            .and_then(|(_, d)| if d > 0.0 { Some((d * 1000.0) as u64) } else { None });

        // 解码源路由：
        // - SACD 轨：导出临时 DSF 后走 ffmpeg 管道（每轨独立 DSF，无需 seek）；
        // - 非原生格式，或 CUE 分轨（range 有值，需从偏移处起播并限定时长）：走 ffmpeg
        //   的输入 seek（-ss 位于 -i 前）——起点/时长确定，规避 rodio try_seek 的竞态/卡死；
        // - 其余整曲（无区间）：rodio 原生解码，低延迟直连。
        let (ffmpeg_child, temp_file): (Option<Child>, Option<PathBuf>) =
            if let Some(idx) = sacd_track_index {
                let (child, source, temp) = spawn_sacd_decoder(path, idx, &self.eq)?;
                sink.append(AnalyserSource::new(source, &self.viz_proc));
                (Some(child), Some(temp))
            } else if !is_native_format(path) || range.is_some() {
                let (child, source) =
                    spawn_ffmpeg_decoder(path, start_ms, duration_ms, &self.eq)?;
                sink.append(AnalyserSource::new(source, &self.viz_proc));
                (Some(child), None)
            } else {
                let file = std::fs::File::open(path).map_err(|e| {
                    AppError::Player(format!("无法打开音频文件 {}: {}", path.display(), e))
                })?;
                let decoder = rodio::Decoder::new(BufReader::new(file))
                    .map_err(|e| AppError::Player(format!("不支持的音频格式或文件损坏: {}", e)))?;
                // 原生解码此前直接进 sink，**完全绕过 EQ**（同一份设置对 DSF 生效、
                // 对 flac 无效）。包一层 EqSource，两条链路共用同一个 EqProcessor。
                sink.append(AnalyserSource::new(EqSource::new(decoder, &self.eq), &self.viz_proc));
                (None, None)
            };

        // 新曲目从静默开始淡入
        sink.set_volume(0.0);
        // 继承当前倍速（切歌/seek 重启后避免倍速丢失）
        sink.set_speed(self.rate);
        self.current = Some(Playback {
            sink,
            ffmpeg_child,
            gain_scale,
            temp_file,
            range: stored_range,
        });
        self.fade_in_started = Some(Instant::now());
        self.current_path = Some(path.to_path_buf());
        self.current_is_ffmpeg = self.current.as_ref().map_or(false, |p| p.ffmpeg_child.is_some());

        // CUE 分轨的起点/时长已由上面的 ffmpeg 输入 seek（-ss/-t）精确定位，无需再 seek；
        // rodio 原生分支仅在无区间（整曲）时进入，天然从 0 播放。
        Ok(())
    }

    /// 预解码下一曲到待命链路：创建 sink + 解码器/ffmpeg 子进程，
    /// 但 sink 立即暂停不发声，等待 SwapToNext 或 play 路径一致时复用。
    fn preload(
        &mut self,
        path: &std::path::Path,
        gain: Option<(f32, f32)>,
        range: Option<(f64, f64)>,
        sacd_track_index: Option<u32>,
    ) -> AppResult<()> {
        // Linux：还原非 UTF-8 文件名的容错编码路径（同 play）
        #[cfg(target_os = "linux")]
        let resolved = crate::library::scan_linux::resolve_path_str(&path.to_string_lossy());
        #[cfg(target_os = "linux")]
        let path: &std::path::Path = &resolved;
        // 新预加载前先回收旧预加载链路
        if let Some(stale) = self.next.take() {
            stale.halt();
        }

        let handle = self.ensure_stream()?.clone();
        let gain_scale = self.gain_scale_for(gain);
        let sink = rodio::Sink::try_new(&handle)
            .map_err(|e| AppError::Player(format!("无法创建音频 sink: {}", e)))?;

        let start_ms = range.map(|(s, _)| (s * 1000.0) as u64).unwrap_or(0);
        let stored_range = range
            .map(|(s, d)| ((s * 1000.0) as u64, if d > 0.0 { (d * 1000.0) as u64 } else { 0 }));
        let duration_ms = range
            .and_then(|(_, d)| if d > 0.0 { Some((d * 1000.0) as u64) } else { None });

        // 解码源路由（同 play）：CUE 分轨/非原生走 ffmpeg 输入 seek，确保起点/时长可靠
        let (ffmpeg_child, temp_file): (Option<Child>, Option<PathBuf>) =
            if let Some(idx) = sacd_track_index {
                let (child, source, temp) = spawn_sacd_decoder(path, idx, &self.eq)?;
                sink.append(AnalyserSource::new(source, &self.viz_proc));
                (Some(child), Some(temp))
            } else if !is_native_format(path) || range.is_some() {
                let (child, source) =
                    spawn_ffmpeg_decoder(path, start_ms, duration_ms, &self.eq)?;
                sink.append(AnalyserSource::new(source, &self.viz_proc));
                (Some(child), None)
            } else {
                let file = std::fs::File::open(path).map_err(|e| {
                    AppError::Player(format!("无法打开音频文件 {}: {}", path.display(), e))
                })?;
                let decoder = rodio::Decoder::new(BufReader::new(file))
                    .map_err(|e| AppError::Player(format!("不支持的音频格式或文件损坏: {}", e)))?;
                // 原生解码此前直接进 sink，**完全绕过 EQ**（同一份设置对 DSF 生效、
                // 对 flac 无效）。包一层 EqSource，两条链路共用同一个 EqProcessor。
                sink.append(AnalyserSource::new(EqSource::new(decoder, &self.eq), &self.viz_proc));
                (None, None)
            };

        // 待命：暂停不发声，避免预加载期间泄漏音频
        sink.pause();
        // 待命链路也继承倍速（swap 接管后延续）
        sink.set_speed(self.rate);
        self.next = Some(Playback {
            sink,
            ffmpeg_child,
            gain_scale,
            temp_file,
            range: stored_range,
        });
        self.next_path = Some(path.to_path_buf());
        Ok(())
    }

    /// 无缝接管预加载链路：旧曲目转入淡出，新曲目从静默淡入并开始发声。
    fn swap_to_next(&mut self) -> AppResult<()> {
        self.paused = false;
        let next = self
            .next
            .take()
            .ok_or_else(|| AppError::Player("没有预加载的曲目".into()))?;
        let next_path = self.next_path.take();

        // 旧曲目转入淡出链路（交叉淡化）；淡出起点取其当前实际增益
        // （同 play：must 用 old.gain_scale，take 之后 target_volume() 会丢失增益）
        if let Some(old) = self.current.take() {
            let base = if self.muted { 0.0 } else { self.volume } * old.gain_scale;
            let fade = self.fade_ms.max(1) as f32;
            let from = match &self.fade_in_started {
                Some(started) => base * Self::ease(started.elapsed().as_millis() as f32 / fade),
                None => base,
            };
            if let Some(older) = self.fading_out.take() {
                older.playback.halt();
            }
            self.fading_out = Some(FadingOut {
                playback: old,
                started: Instant::now(),
                from,
            });
        }
        self.fade_in_started = None;

        // 接管：恢复播放并从静默淡入（sink 已 pause，此处 play 才开始发声）
        let is_ffmpeg = next.ffmpeg_child.is_some();
        next.sink.set_volume(0.0);
        next.sink.set_speed(self.rate);
        next.sink.play();
        self.current = Some(next);
        self.current_is_ffmpeg = is_ffmpeg;
        self.fade_in_started = Some(Instant::now());
        self.current_path = next_path;
        Ok(())
    }

    /// 播放并定位到断点（断点续播专用）
    fn play_at(&mut self, path: &std::path::Path, position_ms: u64, gain: Option<(f32, f32)>) -> AppResult<()> {
        if position_ms == 0 {
            return self.play(path, gain, None, None);
        }
        // 带位置起播时预加载链路不可复用（swap_to_next 只能从 0 开始），先回收
        if let Some(stale) = self.next.take() {
            stale.halt();
        }
        self.next_path = None;

        self.paused = false;
        if is_native_format(path) {
            self.play(path, gain, None, None)?;
            // sink 新建后混音线程可能还没拉到首帧；此时立即 seek 在个别时序下
            // 会让 Sink::try_seek 永久等待（引擎线程卡死、进度冻结）。
            // 先让出 ~150ms 等输出线程开始消费，再定位（原生解码仅此路径可实现）。
            std::thread::sleep(Duration::from_millis(150));
            self.seek(position_ms)
        } else {
            // FFmpeg 管道：解码进程以 -ss 直接从目标位置起流，
            // 无"先播开头再跳转"的听感跳秒，也无 150ms 等待
            self.play(path, gain, Some((position_ms as f64 / 1000.0, 0.0)), None)
        }
    }

    fn pause(&mut self) -> AppResult<()> {
        if let Some(playback) = &self.current {
            playback.sink.pause();
        }
        self.paused = true;
        Ok(())
    }

    fn resume(&mut self) -> AppResult<()> {
        if let Some(playback) = &self.current {
            playback.sink.play();
        }
        self.paused = false;
        Ok(())
    }

    fn stop(&mut self) -> AppResult<()> {
        if let Some(playback) = self.current.take() {
            playback.halt();
        }
        if let Some(fading) = self.fading_out.take() {
            fading.playback.halt();
        }
        if let Some(stale) = self.next.take() {
            stale.halt();
        }
        self.next_path = None;
        self.fade_in_started = None;
        self.current_path = None;
        self.current_is_ffmpeg = false;
        self.paused = false;
        Ok(())
    }

    fn seek(&mut self, position_ms: u64) -> AppResult<()> {
        let Some(playback) = self.current.as_mut() else {
            return Ok(());
        };

        // SACD：临时 DSF 已在磁盘，直接带 -ss 重启 ffmpeg 进程定位到轨内偏移
        // （整轨 DSF，无需叠加基址；音频自然终止于该轨末尾）。
        if let Some(dsf) = playback.temp_file.clone() {
            let (child, source) =
                spawn_ffmpeg_decoder(&dsf, position_ms, None, &self.eq)?;
            if let Some(old) = playback.ffmpeg_child.take() {
                reap_child(old);
            }
            playback.ffmpeg_child = Some(child);
            playback.sink.clear();
            playback.sink.append(AnalyserSource::new(source, &self.viz_proc));
            // 暂停态 seek 只重定位不出声（恢复播放由 resume 负责）
            if !self.paused {
                playback.sink.play();
            }
            playback.sink.set_speed(self.rate);
            self.fade_in_started = Some(Instant::now());
            return Ok(());
        }

        if self.current_is_ffmpeg {
            // ffmpeg 后端：PCM 管道不可 seek，带 -ss 重启解码进程。
            // CUE 分轨需叠加基址（cue_offset）并以剩余时长限长，避免越界到下一轨；
            // 普通文件直接用相对位置（无基址）。
            let path = self
                .current_path
                .clone()
                .ok_or_else(|| AppError::Player("没有正在播放的曲目".into()))?;
            let (abs_ms, dur_ms) = match playback.range {
                Some((base, dur)) => {
                    let abs = base + position_ms;
                    let rem = if dur > position_ms {
                        Some(dur - position_ms)
                    } else {
                        None
                    };
                    (abs, rem)
                }
                None => (position_ms, None),
            };
            let (child, source) =
                spawn_ffmpeg_decoder(&path, abs_ms, dur_ms, &self.eq)?;
            if let Some(old) = playback.ffmpeg_child.take() {
                reap_child(old);
            }
            playback.ffmpeg_child = Some(child);
            playback.sink.clear();
            playback.sink.append(AnalyserSource::new(source, &self.viz_proc));
            // 暂停态 seek 只重定位不出声（恢复播放由 resume 负责）
            if !self.paused {
                playback.sink.play();
            }
            playback.sink.set_speed(self.rate);
        } else {
            playback
                .sink
                .try_seek(std::time::Duration::from_millis(position_ms))
                .map_err(|e| AppError::Player(format!("seek 失败: {}", e)))?;
        }

        // seek 后做一次快速淡入，消除解码重启 / 定位点的爆音
        self.fade_in_started = Some(Instant::now());
        Ok(())
    }

    fn set_volume(&mut self, volume: f32) -> AppResult<()> {
        self.volume = volume.clamp(0.0, 1.0);
        // 淡入进行中由 tick 渐变到新目标；否则立即生效
        if self.fade_in_started.is_none() {
            self.apply_volume();
        }
        Ok(())
    }

    fn set_muted(&mut self, muted: bool) -> AppResult<()> {
        self.muted = muted;
        if self.fade_in_started.is_none() {
            self.apply_volume();
        }
        Ok(())
    }

    /// 设置播放倍速并立即作用于当前曲目与待命预加载链路。
    /// 速率记录在引擎内，后续新建 sink（切歌 / seek 重启后端）会自动沿用。
    fn set_rate(&mut self, rate: f32) -> AppResult<()> {
        let r = rate.clamp(0.25, 4.0);
        self.rate = r;
        if let Some(playback) = &self.current {
            playback.sink.set_speed(r);
        }
        if let Some(playback) = &self.next {
            playback.sink.set_speed(r);
        }
        Ok(())
    }

    /// 更新实验室均衡器配置，**正在播放的曲目立即生效**。
    ///
    /// 所有音源共享同一个 `EqProcessor`（各持 `Arc` 引用），此处更新后下一次取
    /// 样本即按新系数过滤。配合 `Biquad` 的系数插值，拖动滑块时连续无爆音。
    fn set_eq(&mut self, cfg: EqConfig) {
        self.eq.lock().unwrap().set_config(&cfg);
    }

    /// 当前曲目是否已在引擎侧播完（sink 中无音源）。
    /// 用途：ffmpeg 缺失/秒退、时长探测为 0 等无法靠时间判定结束的场景。
    fn current_ended(&self) -> bool {
        self.current
            .as_ref()
            .map(|playback| playback.sink.empty())
            .unwrap_or(false)
    }
}

impl Drop for RodioEngine {
    fn drop(&mut self) {
        if let Some(playback) = self.current.take() {
            playback.halt();
        }
        if let Some(fading) = self.fading_out.take() {
            fading.playback.halt();
        }
        if let Some(stale) = self.next.take() {
            stale.halt();
        }
    }
}

/// SACD ISO → 临时 DSF → ffmpeg 管道。
///
/// 打开 ISO 取出指定音轨，用 ffmpeg 把 DSD 位流（DSF）转成 PCM 送进 rodio；
/// 临时 DSF 缓存在系统临时目录（按 ISO 路径哈希），重复播放不重复导出。
/// 返回（ffmpeg 子进程, PCM 音源, 临时 DSF 路径）。临时 DSF 在 Playback 生命周期
/// 内保留（seek 时用于识别 SACD 链路），缓存本身跨播放持久以加速后续重播。
/// 防白噪兜底：导出 DSF 后解码其头部约 0.5s，确认 ffmpeg 确实把 DSD 解成了 PCM、
/// 且解码结果不是宽带白噪（DST 压缩被误当原始 DSD、或去交错/位序参数错误都会产出白噪）。
///
/// 判据（无需 FFT，廉价可靠）：
/// - ffmpeg 解码失败或无足够输出 → DSD 未被正确解码（DST/格式不支持）→ 拒绝；
/// - `hf = Σ(x_i-x_{i-1})² / Σx_i²`：白噪≈2（相邻样本不相关），音乐远低于 2（相邻相关）；
/// - 配合 `rms > 0.2` 过滤静音前导，避免误伤；两者同时成立才判定为异常音频。
fn guard_sacd_dsf(dsf_path: &Path) -> AppResult<()> {
    let ffmpeg = resolve_ffmpeg().ok_or_else(|| {
        AppError::DependencyMissing(
            "SACD 解码校验需要 FFmpeg。请在「设置 → 解码组件」中安装 FFmpeg 后重试".into(),
        )
    })?;
    let out = crate::util::hidden_command(&ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(dsf_path)
        .args([
            "-t", "0.5", "-af", "aresample=44100", "-f", "f32le", "-acodec", "pcm_f32le", "-",
        ])
        .output()
        .map_err(|e| AppError::Player(format!("SACD 解码校验进程启动失败: {e}")))?;
    // 0.5s @44.1k*2ch*4B(f32le) ≈ 176400 字节；至少要有 ~0.25s 才作数
    if !out.status.success() || out.stdout.len() < 88_200 {
        return Err(AppError::Player(
            "SACD 解码失败：该音轨可能是 DST 压缩或区域格式不支持，已阻止播放".into(),
        ));
    }
    let samples: Vec<f32> = out
        .stdout
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    if samples.len() < 2000 {
        return Err(AppError::Player("SACD 解码输出异常（样本不足）".into()));
    }
    let mut sum_sq = 0.0f64;
    let mut diff_sq = 0.0f64;
    for i in 0..samples.len() {
        let x = samples[i] as f64;
        sum_sq += x * x;
        if i > 0 {
            let prev = samples[i - 1] as f64;
            let d = x - prev;
            diff_sq += d * d;
        }
    }
    let n = samples.len() as f64;
    let rms = (sum_sq / n).sqrt();
    let hf = diff_sq / sum_sq.max(1e-9);
    if rms > 0.2 && hf > 1.3 {
        return Err(AppError::Player(format!(
            "SACD 解码异常（疑似 DST/参数错误导致白噪），已阻止播放以免输出白噪 [rms={:.2} hf={:.2}]",
            rms, hf
        )));
    }
    Ok(())
}

fn spawn_sacd_decoder(
    iso_path: &Path,
    track_index: u32,
    eq: &Arc<Mutex<EqProcessor>>,
) -> AppResult<(Child, PcmStreamSource, PathBuf)> {
    let disc = SacdDisc::open(iso_path).ok_or_else(|| {
        AppError::Player(format!("无法解析 SACD ISO: {}", iso_path.display()))
    })?;
    // 曲库中的 sacd_track_index 是**两区合并索引**：前 n 条为立体声区
    // （n = disc.tracks.len()），其后为多声道区（disc.multich_track_count 条）。
    // disc.tracks 只含立体声区；多声道区曲目与立体声区一一对应（同曲目号/帧数），
    // 元数据直接复用对应立体声轨。
    let stereo_len = disc.tracks.len();
    let idx = track_index as usize;
    let (meta, multich) = if idx < stereo_len {
        (&disc.tracks[idx], false)
    } else {
        let m = idx - stereo_len;
        if m >= disc.multich_track_count {
            return Err(AppError::Player(format!(
                "SACD 曲目索引越界：{}/{}",
                track_index,
                stereo_len + disc.multich_track_count
            )));
        }
        (&disc.tracks[m], true)
    };
    let multich = multich || meta.channels > 2;
    // 导出策略：未压缩立体声区走内置裸 DSD 导出（快、无外部依赖）；
    // DST 压缩区与多声道（5.1）区交给随包分发的 sacd_extract 解码。
    let use_external = multich || disc.unsupported;

    let dsf_path = cached_dsf_path(iso_path, idx).ok_or_else(|| {
        AppError::Player("无法创建 SACD 临时缓存目录".into())
    })?;

    // 解包前确保缓存为当前导出逻辑版本：版本变化则整体清空旧解包文件，
    // 杜绝读取到早期（错误参数）导出的 DSD 噪声数据。
    if let Some(dir) = dsf_path.parent() {
        ensure_sacd_cache_fresh(dir);
    }
    // 缓存校验：按真实音频包抽取后载荷长度无法预先精确预测，改为与文件**自身 DSF 头**
    // 声明的 file_size 比对（自校验，读 20 字节即可，避免每次播放都按估算值误判重导）；
    // 外部解码同样无法预知产出大小，用「大于 DSF 头」判定存在性。
    // 导出前清理可能残留的历史文件。
    let cache_valid = if use_external {
        dsf_path
            .metadata()
            .map(|m| m.len() > crate::library::sacd::DSF_HEADER_BYTES)
            .unwrap_or(false)
    } else {
        crate::library::sacd::dsf_declared_file_size(&dsf_path)
            .and_then(|declared| {
                dsf_path
                    .metadata()
                    .ok()
                    .map(|m| m.len() == declared && m.len() > crate::library::sacd::DSF_HEADER_BYTES)
            })
            .unwrap_or(false)
    };
    if !cache_valid {
        let _ = std::fs::remove_file(&dsf_path);
        if use_external {
            // 外部 sacd_extract 可能因环境缺依赖而不可用（典型：Linux 缺 libxml2.so.2，
            // 进程以退出码 127 报 "error while loading shared libraries"）。此时回退内置
            // 裸 DSD 导出——v11 起内置导出按权威扇区/数据包结构抽取（已与 sacd_extract
            // 在同 ISO 上验证逐字节一致），多数立体声区可直接正确解码；仅当内置也拒绝
            // （真正的 DST 压缩区）才报错。
            if let Err(e) = disc.extract_track_via_external(meta.track_number, multich, &dsf_path) {
                let _ = std::fs::remove_file(&dsf_path);
                disc.export_track_to_dsf(idx, &dsf_path).map_err(|inner| {
                    AppError::Player(format!(
                        "解码 SACD 音轨失败: sacd_extract 不可用（{e}）；内置导出亦失败（{inner}）。\
                         若为 Linux 请安装 libxml2：sudo apt-get install -y libxml2"
                    ))
                })?;
            }
        } else {
            disc.export_track_to_dsf(idx, &dsf_path)
                .map_err(|e| AppError::Player(format!("导出 SACD 分轨为 DSF 失败: {}", e)))?;
        }
    }

    // 防白噪兜底：校验导出 DSF 确为可正常解码的 PCM（非 DST/参数错误导致的白噪），
    // 一旦检出异常立即拒绝播放，绝不把白噪轰到声卡（对应"运行期自检 & 防白噪兜底"）。
    //
    // 内置导出校验失败时不能直接报错：Area TOC 不携带可靠的「该区是否 DST」标志
    // （Energy 的 2ch 区就是被误判为未压缩的 DST 数据），此时改由 sacd_extract 重解，
    // 这是一次性的兜底（成功后会命中缓存），正常碟片零成本。
    if let Err(err) = guard_sacd_dsf(&dsf_path) {
        if use_external {
            return Err(err);
        }
        let _ = std::fs::remove_file(&dsf_path);
        disc.extract_track_via_external(meta.track_number, multich, &dsf_path)
            .map_err(|e| AppError::Player(format!("解码 SACD 音轨失败: {}", e)))?;
        guard_sacd_dsf(&dsf_path)?;
    }
    // 复用 ffmpeg 管道解码 DSF（ffmpeg 原生支持 DSF，由 dsd_lsbf 解码器直接输出 PCM）
    let (child, source) = spawn_ffmpeg_decoder(&dsf_path, 0, None, eq)?;
    Ok((child, source, dsf_path))
}

/// 启动 ffmpeg 将任意格式解码为 PCM s16le（44.1kHz 双声道）并输出到管道，
/// 返回（子进程, rodio 音源）。seek 通过 -ss 在输入端定位实现。
fn spawn_ffmpeg_decoder(
    path: &Path,
    offset_ms: u64,
    duration_ms: Option<u64>,
    eq: &Arc<Mutex<EqProcessor>>,
) -> AppResult<(Child, PcmStreamSource)> {
    let ffmpeg = resolve_ffmpeg().ok_or_else(|| {
        AppError::DependencyMissing(
            "该格式需要 FFmpeg 解码。请在「设置 → 解码组件」中安装 FFmpeg 后重试".into(),
        )
    })?;

    // 动态探测音频流参数，避免写死采样率/声道（不同格式/文件参数各异）。
    let (codec_dsd, src_rate, src_ch) = probe_audio_stream(path).unwrap_or((false, 44100, 2));
    let ext_dsd = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_ascii_lowercase().as_str(), "dsf" | "dff"))
        .unwrap_or(false);
    let is_dsd = codec_dsd || ext_dsd;
    // 输出声道：**按源（文件头）声道数解码**，不强制 2ch——多声道文件（如 5.1 SACD
    // 解出的 6ch DSF）若被硬按 2ch 解释，样本帧会错位成噪声。设备适配（多声道→立体声
    // 下混）由 PcmStreamSource 在输出侧完成，解码链路保持源声道。
    let src_ch = src_ch.clamp(1, 8);
    // 供给 rodio 的声道：rodio 0.19 不做声道转换，设备为立体声（Windows 共享混音常态），
    // 多声道在此下混；单声道复制为双声道，立体声原样。
    let out_ch = 2;
    // 输出采样率：DSD 统一到 44.1kHz 标准率；PCM 透传源速率（探测失败回退 44.1kHz）。
    let out_rate = if is_dsd {
        44100
    } else if src_rate > 0 {
        src_rate
    } else {
        44100
    };
    // DSD：ffmpeg 8.x 对 DSD 直接输出 s16le 存在缺陷（解出的 PCM 为白噪），故先以 f32le 浮点
    // 输出（本机 ffmpeg 此路径正常），再由 Rust 量化回 i16 喂给 rodio。同时先 resample 到 44.1k
    // 再 lowpass，避免"高采样率直接低通后由 -ar 抽取"导致的混叠白噪。
    let audio_filter = if is_dsd {
        Some(format!("aresample=44100,lowpass=f={}", DSD_LOWPASS_HZ))
    } else {
        None
    };

    let mut cmd = crate::util::hidden_command(ffmpeg);
    if offset_ms > 0 {
        cmd.arg("-ss")
            .arg(format!("{:.3}", offset_ms as f64 / 1000.0));
    }
    if let Some(d) = duration_ms {
        if d > 0 {
            cmd.arg("-t")
                .arg(format!("{:.3}", d as f64 / 1000.0));
        }
    }
    cmd.arg("-v")
        .arg("error")
        .arg("-i")
        .arg(path);
    // DSD（dff/dsf）：现代 ffmpeg 的 `dsd_lsbf` 解码器已在解码阶段直接把 DSD 位流转换为
    // PCM（旧版需显式 `-af dsd` 滤镜，该滤镜已在当前 ffmpeg 主线中被移除），因此这里
    // 不再注入 `-af dsd`，交由解码器完成 DSD→PCM；缺 `dff` 解复用器的 DSDIFF 文件
    // 当前无法解码（属格式不支持），SACD 走 .dsf 链路不受影响。
    // DSD 追加 lowpass（见 audio_filter）：抑制抽取过程混叠回可闻段的超声噪声底。
    if let Some(af) = &audio_filter {
        cmd.arg("-af").arg(af);
    }
    if is_dsd {
        // DSD→PCM 经 ffmpeg 解码：ffmpeg 8.x 直接输出 s16le 会解出白噪，必须以 f32le 浮点输出
        // （本机 ffmpeg 此路径正常），再由 PcmStreamSource 量化回 i16 喂给 rodio。
        // 不指定 -ar/-ac：声道数按文件头（源）输出——5.1 DSF 输出 6ch，立体声 2ch，
        // 设备适配（下混到 2ch）由 PcmStreamSource 完成；显式 -ac 2 会让多声道帧错位成噪声。
        cmd.arg("-f")
            .arg("f32le")
            .arg("-acodec")
            .arg("pcm_f32le")
            // 显式与 `PcmStreamSource::src_channels` 对齐：ffprobe 探测失败时会
            // 回退到 2 声道假设，若此时仍让 ffmpeg 按源声道（如 7ch）输出，
            // 取样点会逐帧错位，整轨变成宽带白噪音。显式 `-ac` 让两者永远一致。
            .arg("-ac")
            .arg(src_ch.to_string())
            .arg("-")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
    } else {
        // 非 DSD：同样显式 `-ac src_ch`，与 PcmStreamSource 的 src_channels 对齐
        // （多声道 ape/wv/tta/tak/dts-in-wav 与 7ch PCM WAV 走同一语义）。
        // 设备适配（下混到立体声）仍由 PcmStreamSource 在输出侧完成。
        cmd.arg("-f")
            .arg("s16le")
            .arg("-acodec")
            .arg("pcm_s16le")
            .arg("-ar")
            .arg(out_rate.to_string())
            .arg("-ac")
            .arg(src_ch.to_string())
            .arg("-")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
    }
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW：避免解码期间弹出控制台窗口
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::Player(format!("无法启动 ffmpeg: {}", e)))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Player("无法获取 ffmpeg 输出管道".into()))?;

    Ok((
        child,
        PcmStreamSource::new(
            BufReader::new(stdout),
            src_ch,
            out_ch,
            out_rate,
            is_dsd,
            eq,
        ),
    ))
}

/// 实验室·均衡器：手写双二阶（Biquad）峰值 EQ，作用于 `PcmStreamSource` 输出帧。
///
/// 不引入新 crate，避免依赖链与体积膨胀；峰值滤波器公式源自
/// Robert Bristow-Johnson 的音频 EQ cookbook，对每个频段独立计算系数，
/// 每声道维护一组滤波器状态（避免声道间串扰）。EQ 仅在 `enabled` 时生效，
/// 关闭时原样透传，零开销。
/// 频段定义。
///
/// 序列化用 camelCase（与前端一致，见 converter.rs 的约定）；`gainDb` 保留
/// `gain_db` 别名以兼容早期写入 config.json 的旧数据——否则旧配置缺 `gainDb`
/// 字段会让整个 `AppConfig` 反序列化失败（config.rs 记录过的历史坑）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqBand {
    /// 频段中心频率（Hz）
    pub freq: f32,
    /// 增益（dB，-15 ~ +15）
    #[serde(alias = "gain_db")]
    pub gain_db: f32,
    /// Q 值（带宽，0.1 ~ 10）
    pub q: f32,
}

/// 均衡器配置：频段列表 + 总开关 + 输出增益（make-up gain）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqConfig {
    pub enabled: bool,
    pub bands: Vec<EqBand>,
    /// 输出增益（dB，-12 ~ +12）：提升多段时为防削波可由用户压低，默认 0 保持现状
    #[serde(default, alias = "output_gain")]
    pub output_gain_db: f32,
}

impl Default for EqConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            output_gain_db: 0.0,
            bands: vec![
                EqBand { freq: 60.0, gain_db: 0.0, q: 1.0 },
                EqBand { freq: 170.0, gain_db: 0.0, q: 1.0 },
                EqBand { freq: 310.0, gain_db: 0.0, q: 1.0 },
                EqBand { freq: 600.0, gain_db: 0.0, q: 1.0 },
                EqBand { freq: 1000.0, gain_db: 0.0, q: 1.0 },
                EqBand { freq: 3000.0, gain_db: 0.0, q: 1.0 },
                EqBand { freq: 6000.0, gain_db: 0.0, q: 1.0 },
                EqBand { freq: 12000.0, gain_db: 0.0, q: 1.0 },
                EqBand { freq: 14000.0, gain_db: 0.0, q: 1.0 },
                EqBand { freq: 16000.0, gain_db: 0.0, q: 1.0 },
            ],
        }
    }
}

/// 系数插值长度（样本数）：44.1kHz 下约 23ms——足够平滑到听不出爆音，
/// 又短到拖动滑块时反馈即时。
const EQ_RAMP_SAMPLES: u32 = 1024;

/// 双二阶滤波器（峰值 EQ）：每声道每频段一套状态。
///
/// 系数带**斜坡过渡**：`retune` 只更新目标系数并启动插值，**不重置滤波器状态**
/// （x1/x2/y1/y2）。旧实现 `*self = Self::new(...)` 会把状态清零，滤波器输出从
/// 当前值瞬间跳到 0，高频段表现为明显「啪」声——实时拖动 EQ 滑块时每动一次响一次。
/// 改为在 `EQ_RAMP_SAMPLES` 个样本内线性插值，状态与相位连续，听感平滑。
#[derive(Debug, Clone)]
struct Biquad {
    b0: f32, b1: f32, b2: f32,
    a1: f32, a2: f32,
    /// 目标系数（`retune` 后逐步逼近）
    t_b0: f32, t_b1: f32, t_b2: f32, t_a1: f32, t_a2: f32,
    /// 插值起点系数
    s_b0: f32, s_b1: f32, s_b2: f32, s_a1: f32, s_a2: f32,
    /// 插值进度（已处理样本数）与总长度（0 = 无需插值）
    ramp: u32,
    ramp_len: u32,
    x1: f32, x2: f32,
    y1: f32, y2: f32,
}

impl Biquad {
    fn new(sample_rate: u32, band: &EqBand) -> Self {
        let (b0, b1, b2, a1, a2) = peaking_coeffs(sample_rate, band);
        Self {
            b0, b1, b2, a1, a2,
            t_b0: b0, t_b1: b1, t_b2: b2, t_a1: a1, t_a2: a2,
            s_b0: b0, s_b1: b1, s_b2: b2, s_a1: a1, s_a2: a2,
            ramp: 0, ramp_len: 0,
            x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0,
        }
    }

    /// 更新目标系数并启动插值（保留滤波器状态，避免爆音）
    fn retune(&mut self, sample_rate: u32, band: &EqBand) {
        let (b0, b1, b2, a1, a2) = peaking_coeffs(sample_rate, band);
        self.s_b0 = self.b0;
        self.s_b1 = self.b1;
        self.s_b2 = self.b2;
        self.s_a1 = self.a1;
        self.s_a2 = self.a2;
        self.t_b0 = b0;
        self.t_b1 = b1;
        self.t_b2 = b2;
        self.t_a1 = a1;
        self.t_a2 = a2;
        self.ramp = 0;
        self.ramp_len = EQ_RAMP_SAMPLES;
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        if self.ramp < self.ramp_len {
            self.ramp += 1;
            let t = self.ramp as f32 / self.ramp_len as f32;
            self.b0 = self.s_b0 + (self.t_b0 - self.s_b0) * t;
            self.b1 = self.s_b1 + (self.t_b1 - self.s_b1) * t;
            self.b2 = self.s_b2 + (self.t_b2 - self.s_b2) * t;
            self.a1 = self.s_a1 + (self.t_a1 - self.s_a1) * t;
            self.a2 = self.s_a2 + (self.t_a2 - self.s_a2) * t;
        }
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// 峰值滤波器系数（RBJ cookbook），返回按 a0 归一化后的 (b0, b1, b2, a1, a2)。
fn peaking_coeffs(sample_rate: u32, band: &EqBand) -> (f32, f32, f32, f32, f32) {
    let f0 = band.freq.clamp(20.0, 20000.0) as f64 / sample_rate as f64;
    let gain_db = band.gain_db.clamp(-15.0, 15.0) as f64;
    let q = band.q.clamp(0.1, 10.0) as f64;
    let a = 10.0f64.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * f0;
    // alpha = sin(w0) / (2 * Q)
    let alpha = w0.sin() / (2.0 * q);
    let cos_w0 = w0.cos();

    // 峰值滤波器系数（RBJ cookbook）
    let b0 = 1.0 + alpha * a;
    let b1 = -2.0 * cos_w0;
    let b2 = 1.0 - alpha * a;
    let a0 = 1.0 + alpha / a;
    let a1 = -2.0 * cos_w0;
    let a2 = 1.0 - alpha / a;

    (
        (b0 / a0) as f32,
        (b1 / a0) as f32,
        (b2 / a0) as f32,
        (a1 / a0) as f32,
        (a2 / a0) as f32,
    )
}

/// 干/湿混合斜坡长度（样本数）：~11ms@44.1k，开关 EQ 时交叉淡化无爆音。
const EQ_MIX_RAMP_SAMPLES: u32 = 512;

/// 温和软限幅：|x|≤1 近似线性；|x|>1 做饱和压缩（渐近线 2.0），取代原先的硬 `clamp`，
/// 消除多段提升时临界削波的方波感——这是「稳定完整」的关键音质兜底。
#[inline]
fn soft_clip(x: f32) -> f32 {
    let a = x.abs();
    let v = if a <= 1.0 {
        a
    } else {
        1.0 + (a - 1.0) * 2.0 / (1.0 + 2.0 * (a - 1.0))
    };
    x.signum() * v
}

/// dB → 线性增益（0dB = 1.0），区间夹紧避免极端值。
#[inline]
fn db_to_linear(db: f32) -> f32 {
    10.0f32.powf(db.clamp(-24.0, 24.0) / 20.0)
}

/// 均衡器处理器：对交错 i16 样本帧逐声道应用 biquad EQ。
///
/// 三重稳定性设计：
///  1. **干/湿交叉淡化**：`mix` 随 `config.enabled` 斜坡（开关无爆音）；
///  2. **输出增益（用户可调 make-up gain）**：提升多段时为防削波可由用户压低，默认 0dB 保持现状；
///  3. **软限幅兜底**：任何残余溢出走 `soft_clip` 而非硬削波，避免方波失真。
#[derive(Debug, Clone)]
pub struct EqProcessor {
    config: EqConfig,
    sample_rate: u32,
    channels: u16,
    /// 每声道每频段一组 biquad 状态
    filters: Vec<Vec<Biquad>>,
    /// 输出增益（线性，由 config.output_gain_db 换算），默认 1.0
    output_gain: f32,
    /// 干/湿混合系数（0=直通，1=完全 EQ）；开关时斜坡过渡
    mix: f32,
    /// 混合目标（随 config.enabled 变化）
    mix_target: f32,
}

impl EqProcessor {
    pub fn new(sample_rate: u32, channels: u16, config: &EqConfig) -> Self {
        let mut p = Self {
            config: config.clone(),
            sample_rate,
            channels,
            filters: Vec::new(),
            output_gain: db_to_linear(config.output_gain_db),
            mix: if config.enabled { 1.0 } else { 0.0 },
            mix_target: if config.enabled { 1.0 } else { 0.0 },
        };
        // 新建即立即生效（无前值可插值）
        p.rebuild_filters(false);
        p
    }

    /// 按当前 `sample_rate` / `channels` / `config` 重建（或重新调谐）滤波器组。
    ///
    /// `ramp = true` 走系数插值，用于**实时改参数**（在播曲目不中断、无爆音）；
    /// `ramp = false` 立即生效，用于新建与换轨。
    ///
    /// 注意：旧实现在构造时对所有频段一律用 `config.bands[0]` 调谐——10 个滤波器
    /// 全是同一个 60Hz 峰值，等于整段 EQ 失效。此处按下标取各自频段。
    fn rebuild_filters(&mut self, ramp: bool) {
        let channels = self.channels as usize;
        let bands = self.config.bands.len();
        let shape_ok =
            self.filters.len() == channels && self.filters.iter().all(|c| c.len() == bands);
        if !shape_ok {
            // 结构变化（声道数 / 频段数不同）→ 无从插值，直接重建
            self.filters = (0..channels)
                .map(|_| {
                    (0..bands)
                        .map(|b| Biquad::new(self.sample_rate, &self.config.bands[b]))
                        .collect()
                })
                .collect();
            return;
        }
        for c in 0..channels {
            for b in 0..bands {
                if ramp {
                    self.filters[c][b].retune(self.sample_rate, &self.config.bands[b]);
                } else {
                    self.filters[c][b] = Biquad::new(self.sample_rate, &self.config.bands[b]);
                }
            }
        }
    }

    /// 更新配置（频段参数 / 总开关 / 输出增益变化时调用）。走系数插值，正在播放立即生效。
    pub fn set_config(&mut self, config: &EqConfig) {
        self.config = config.clone();
        self.output_gain = db_to_linear(config.output_gain_db);
        self.mix_target = if config.enabled { 1.0 } else { 0.0 };
        self.rebuild_filters(true);
    }

    /// 采样率 / 声道数变化时重新调谐。
    ///
    /// 原生解码（`rodio::Decoder`）的采样率与声道数**逐文件不同**（48000 / 96000 /
    /// 单声道 / 5.1 皆可能），而滤波器系数必须由实际速率算出，故换轨时必须重调。
    pub fn reconfigure(&mut self, sample_rate: u32, channels: u16, config: &EqConfig) {
        self.sample_rate = sample_rate;
        self.channels = channels;
        self.config = config.clone();
        self.output_gain = db_to_linear(config.output_gain_db);
        self.mix_target = if config.enabled { 1.0 } else { 0.0 };
        self.rebuild_filters(false);
    }

    /// 干/湿混合系数每样本逼近目标，开关 EQ 时平滑无爆音。
    #[inline]
    fn step_mix(&mut self) {
        let diff = self.mix_target - self.mix;
        if diff.abs() < f32::EPSILON {
            self.mix = self.mix_target;
            return;
        }
        let step = 1.0 / EQ_MIX_RAMP_SAMPLES as f32;
        self.mix += diff.signum() * step;
        if (self.mix_target - self.mix).abs() < step {
            self.mix = self.mix_target;
        }
    }

    /// 处理单个样本：滤波 → 输出增益 → 干/湿交叉淡化 → 软限幅 → i16。
    #[inline]
    fn process_one(&mut self, ch: usize, sample: i16) -> i16 {
        let x = sample as f32 / 32768.0;
        self.step_mix();
        if self.mix <= 0.0 {
            // 完全直通：零滤波开销（关闭 EQ 或已淡出时）
            return (x * 32768.0).clamp(-32768.0, 32767.0) as i16;
        }
        let mut y = x;
        if ch < self.filters.len() {
            for f in self.filters[ch].iter_mut() {
                y = f.process(y);
            }
            y *= self.output_gain;
        }
        let out = x * (1.0 - self.mix) + y * self.mix;
        let out = soft_clip(out);
        (out * 32768.0).clamp(-32768.0, 32767.0) as i16
    }

    /// 处理**单个**交错样本（供 `EqSource` 逐样本调用）；`ch` 为声道索引。
    ///
    /// `PcmStreamSource` 用 `process_interleaved`（按帧批量，免逐样本加锁）；
    /// 原生链路是 rodio 的 `Iterator`，只能逐样本，故另开此入口。两者共用同一组
    /// 滤波器状态与系数。
    #[inline]
    pub fn process_sample(&mut self, ch: usize, sample: i16) -> i16 {
        if self.channels == 0 || ch >= self.filters.len() {
            return sample;
        }
        self.process_one(ch, sample)
    }

    /// 对交错 i16 样本切片（长度 = channels * n_frames）就地应用 EQ。
    /// 返回处理后的样本数（与输入相同）。
    pub fn process_interleaved(&mut self, samples: &mut [i16]) {
        if self.channels == 0 {
            return;
        }
        // 完全直通（EQ 关闭且已淡出）：原样透传，零开销
        if self.mix <= 0.0 && self.mix_target <= 0.0 {
            return;
        }
        let ch = self.channels as usize;
        for frame in samples.chunks_mut(ch) {
            for (c, sample) in frame.iter_mut().enumerate() {
                *sample = self.process_one(c, *sample);
            }
        }
    }
}

/// 通用 EQ 包装源：为**任意** `Source<Item = i16>` 套上均衡器。
///
/// 存在的理由：原生解码分支（`rodio::Decoder`，mp3/flac/wav/ogg 等）此前直接进
/// sink，**完全绕过 EQ**——同一份 EQ 设置对 DSF 生效、对 flac 毫无作用，行为不
/// 一致且难以察觉。这里把解码器包一层，在其 `Iterator::next` 里逐样本应用同一组
/// 滤波器，两条链路共用同一个 `EqProcessor`。
///
/// 声道轮换由 `ch_pos` 维护：rodio 的 `Iterator` 产出交错样本，故按 `channels()`
/// 取模决定当前样本属于哪个声道。
pub struct EqSource<S> {
    inner: S,
    eq: Arc<Mutex<EqProcessor>>,
    channels: u16,
    ch_pos: usize,
}

impl<S> EqSource<S>
where
    S: rodio::Source<Item = i16>,
{
    /// 包装一个音源，并按该音源的**实际**采样率 / 声道数重新调谐共享 EQ。
    ///
    /// 原生解码的速率与声道数逐文件不同（48000 / 96000 / 单声道 / 5.1 皆可能），
    /// 而 biquad 系数必须由真实速率算出，故换轨必须重调；ffmpeg 链路同理。
    pub fn new(inner: S, eq: &Arc<Mutex<EqProcessor>>) -> Self {
        let channels = inner.channels();
        let rate = inner.sample_rate();
        let cfg = eq.lock().unwrap().config.clone();
        eq.lock().unwrap().reconfigure(rate, channels, &cfg);
        Self {
            inner,
            eq: Arc::clone(eq),
            channels,
            ch_pos: 0,
        }
    }
}

impl<S> Iterator for EqSource<S>
where
    S: rodio::Source<Item = i16>,
{
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        let s = self.inner.next()?;
        let mut eq = self.eq.lock().unwrap();
        let ch = self.ch_pos;
        self.ch_pos = (self.ch_pos + 1) % self.channels.max(1) as usize;
        Some(eq.process_sample(ch, s))
    }
}

impl<S> rodio::Source for EqSource<S>
where
    S: rodio::Source<Item = i16>,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

/// ffmpeg PCM 管道音源（s16le / f32le 交错样本 → rodio::Source）
///
/// 内部缓冲大块样本（默认 8192 样本），避免逐样本 read 导致的高频 syscall 与混音线程欠载
/// （表现为爆音/杂音）。
///
/// `float_input`：ffmpeg 8.x 对 DSD 直接输出 `s16le` 存在缺陷（解出的 PCM 为白噪），须以
/// `f32le` 浮点输出（本机 ffmpeg 此路径正常），此处再把 f32 量化回 i16 喂给 rodio。
///
/// 声道语义（v2.0.1 修复）：`src_channels` = ffmpeg 实际输出的声道数（来自文件头探测，
/// 5.1 DSF 为 6、立体声为 2）；`channels` = 供给 rodio 的声道数（设备适配后）。
/// 旧实现把多声道流硬按 2ch 解释（帧错位成噪声），现在改为**按源声道解码**，仅在输出侧
/// 按设备声道下混/上混（5.1→立体声走 ITU-R BS.775 系数，单声道复制到双声道）。
struct PcmStreamSource {
    reader: BufReader<ChildStdout>,
    /// ffmpeg 输出（源）声道数
    src_channels: u16,
    /// 供给 rodio 的声道数（设备适配后）
    channels: u16,
    sample_rate: u32,
    /// 解码输出是否为 f32le（DSD 解码走此路径以避免 s16le 白噪缺陷）
    float_input: bool,
    /// 解码输出的样本缓冲（i16，按 src_channels 交错）
    buffer: Vec<i16>,
    /// 当前缓冲读取位置
    pos: usize,
    /// 是否到达流末尾
    eof: bool,
    /// 声道适配后的当前帧样本（长度 = channels）
    out_frame: Vec<i16>,
    /// out_frame 内的读取位置
    out_pos: usize,
    /// 上次 read 不足一个完整样本的残余字节（跨块拼接，保证字节对齐）
    residual: Vec<u8>,
    /// 实验室均衡器处理器（未启用时原样透传，零开销）。
    ///
    /// 与 `RodioEngine` **共享同一份**：改 EQ 时引擎侧更新它，正在播放的曲目
    /// 立即生效（旧实现是每轨独占一份，改动要等到下一首才生效）。
    eq: Arc<Mutex<EqProcessor>>,
}

impl PcmStreamSource {
    const CHUNK_SAMPLES: usize = 8192;

    fn new(
        reader: BufReader<ChildStdout>,
        src_channels: u16,
        channels: u16,
        sample_rate: u32,
        float_input: bool,
        eq: &Arc<Mutex<EqProcessor>>,
    ) -> Self {
        // 按本流的实际速率/声道数重调共享 EQ（ffmpeg 链路速率由探测结果决定）
        let cfg = eq.lock().unwrap().config.clone();
        eq.lock().unwrap().reconfigure(sample_rate, channels, &cfg);
        Self {
            reader,
            src_channels: src_channels.max(1),
            channels: channels.max(1),
            sample_rate,
            float_input,
            buffer: Vec::with_capacity(Self::CHUNK_SAMPLES),
            pos: 0,
            eof: false,
            out_frame: Vec::new(),
            out_pos: 0,
            residual: Vec::new(),
            eq: Arc::clone(eq),
        }
    }

    /// 从管道填充一块样本到 buffer（追加，不清空——保证跨块帧连续）；
    /// 失败/EOF 时标记 eof。
    ///
    /// 管道 `read` 不保证按样本边界返回（部分读常见于 DSD lowpass 重负载与 EOF 附近），
    /// 尾部不足一个样本的字节缓存到 `residual`，与下次读到的数据拼接后再切样本，
    /// 否则该点之后全部样本字节错位 → 整轨噪声。
    fn refill(&mut self) {
        if self.eof {
            return;
        }
        let stride = if self.float_input { 4 } else { 2 };
        let mut bytes = vec![0u8; Self::CHUNK_SAMPLES * stride];
        let n = match self.reader.read(&mut bytes) {
            Ok(0) => {
                self.eof = true;
                return;
            }
            Ok(n) => n,
            Err(_) => {
                self.eof = true;
                return;
            }
        };
        // 拼接上次残余 → 取整样本 → 残余留给下次
        let mut all = std::mem::take(&mut self.residual);
        all.extend_from_slice(&bytes[..n]);
        let usable = all.len() - (all.len() % stride);
        self.residual = all.split_off(usable);
        if self.float_input {
            // f32le：每个样本 4 字节；读到后量化回 i16（rodio 按 i16 消费）。
            self.buffer.extend(all.chunks_exact(4).map(|c| {
                let f = f32::from_le_bytes([c[0], c[1], c[2], c[3]]).clamp(-1.0, 1.0);
                (f * 32767.0) as i16
            }));
        } else {
            self.buffer.extend(
                all.chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]])),
            );
        }
    }

    /// 取下一帧（src_channels 个样本）并适配为 out_frame（channels 个样本）
    fn next_frame(&mut self) -> bool {
        let src = self.src_channels as usize;
        loop {
            if self.pos + src <= self.buffer.len() {
                let frame = &self.buffer[self.pos..self.pos + src];
                self.pos += src;
                self.out_frame.clear();
                self.out_frame
                    .extend(adapt_channels(frame, self.channels as usize));
                // 实验室均衡器：对适配后的输出帧逐样本应用 EQ（未启用时原样透传）。
                // 按帧批量加锁，避免逐样本 lock/unlock 的开销。
                self.eq.lock().unwrap().process_interleaved(&mut self.out_frame);
                self.out_pos = 0;
                return true;
            }
            if self.eof {
                return false;
            }
            // 剩余样本不足一帧：压缩已消费部分后续读，保证帧跨块连续
            if self.pos > 0 {
                self.buffer.drain(..self.pos);
                self.pos = 0;
            }
            let before = self.buffer.len();
            self.refill();
            if self.buffer.len() == before && self.eof {
                return false;
            }
        }
    }
}

/// 把一帧 src 声道样本适配为 dst 声道（输出侧设备适配，不改解码链路）。
///
/// - 源 == 目标：原样拷贝；
/// - 单声道 → 立体声：复制；
/// - 5.1（L R C LFE Ls Rs）→ 立体声：ITU-R BS.775 下混
///   （L' = L + 0.707·C + 0.707·Ls，R' = R + 0.707·C + 0.707·Rs，LFE 不计入）；
/// - 其它多声道 → 立体声：按奇偶索引均分（保留全部声道能量，避免只取前两声丢失环绕）。
fn adapt_channels(frame: &[i16], dst: usize) -> Vec<i16> {
    let src = frame.len();
    if src == dst {
        return frame.to_vec();
    }
    if dst == 2 {
        match src {
            1 => return vec![frame[0], frame[0]],
            6 => {
                // 5.1（FL FR FC LFE BL BR）→ 立体声：中置 / 环绕以 1/√2 并入左右，
                // LFE 丢弃（低频能量并入会明显浑浊，且多数设备无独立低频通道）
                const DOWNMIX: f32 = std::f32::consts::FRAC_1_SQRT_2;
                let l = frame[0] as f32 + DOWNMIX * (frame[2] as f32 + frame[4] as f32);
                let r = frame[1] as f32 + DOWNMIX * (frame[2] as f32 + frame[5] as f32);
                return vec![clamp_i16(l), clamp_i16(r)];
            }
            7 => {
                // 6.1（FL FR FC LFE BL BR FLC）→ 立体声：与 5.1 同规则，
                // 前中左（FLC）以 0.5 权重并入左右；LFE 丢弃。
                const DOWNMIX: f32 = std::f32::consts::FRAC_1_SQRT_2;
                let center_l = 0.5 * frame[6] as f32;
                let l = frame[0] as f32 + DOWNMIX * (frame[2] as f32 + frame[4] as f32) + center_l;
                let r = frame[1] as f32 + DOWNMIX * (frame[2] as f32 + frame[5] as f32) + center_l;
                return vec![clamp_i16(l), clamp_i16(r)];
            }
            8 => {
                // 7.1（FL FR FC LFE BL BR SL SR）→ 立体声：侧环绕并入同侧
                const DOWNMIX: f32 = std::f32::consts::FRAC_1_SQRT_2;
                let l = frame[0] as f32
                    + DOWNMIX * (frame[2] as f32 + frame[4] as f32 + frame[6] as f32);
                let r = frame[1] as f32
                    + DOWNMIX * (frame[2] as f32 + frame[5] as f32 + frame[7] as f32);
                return vec![clamp_i16(l), clamp_i16(r)];
            }
            _ => {}
        }
    }
    // 通用：按目标声道数 round-robin 平均
    let mut out = vec![0f32; dst];
    for (i, &s) in frame.iter().enumerate() {
        out[i % dst] += s as f32;
    }
    let scale = 1.0 / (src as f32 / dst as f32);
    out.iter().map(|&v| clamp_i16(v * scale)).collect()
}

#[inline]
fn clamp_i16(v: f32) -> i16 {
    if v >= 32767.0 {
        32767
    } else if v <= -32768.0 {
        -32768
    } else {
        v as i16
    }
}

impl Iterator for PcmStreamSource {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        loop {
            if self.out_pos < self.out_frame.len() {
                let sample = self.out_frame[self.out_pos];
                self.out_pos += 1;
                return Some(sample);
            }
            if !self.next_frame() {
                return None;
            }
        }
    }
}

impl rodio::Source for PcmStreamSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// 端到端验证 SACD 播放链路（导出 DSF → ffmpeg 解码 → PcmStreamSource f32→i16）不会解出白噪。
    /// 直接用真实播放代码路径，避免只测导出/只测 ffmpeg 命令导致的盲区。
    /// `SACD_TEST_ISO` 指定镜像；`SACD_TEST_TRACK_INDEX` 指定合并轨索引（默认 0；
    /// 立体声区之后的索引为多声道 5.1 轨，走外部 sacd_extract 解码）。
    #[test]
    fn sacd_playback_pipeline_not_white_noise() {
        let iso = match std::env::var("SACD_TEST_ISO") {
            Ok(v) => v,
            Err(_) => {
                eprintln!("SACD_TEST_ISO unset; skipping");
                return;
            }
        };
        let track_index: u32 = std::env::var("SACD_TEST_TRACK_INDEX")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        // EQ 由引擎与各音源共享（Arc），测试里就地建一份默认（关闭）的
        let eq = std::sync::Arc::new(std::sync::Mutex::new(EqProcessor::new(
            44100,
            2,
            &EqConfig::default(),
        )));
        let (mut child, mut source, _dsf) =
            match spawn_sacd_decoder(Path::new(&iso), track_index, &eq) {
            Ok(x) => x,
            Err(e) => {
                // DST 压缩 / 非标准布局的 ISO 应被明确拒绝（而非输出白噪）——同样算通过
                let msg = e.to_string();
                assert!(
                    msg.contains("DST") || msg.contains("无法解码"),
                    "非预期拒绝播放: {msg}"
                );
                eprintln!("SACD ISO 为 DST 压缩/非标准布局，已按预期明确拒绝（无白噪）");
                return;
            }
        };
        // 跳过开头 1s（前导），取之后约 4s 的 i16 样本
        const SKIP: usize = 44_100 * 2 * 1;
        const TAKE: usize = 44_100 * 2 * 4;
        let mut taken: Vec<i16> = Vec::with_capacity(TAKE);
        let mut skipped = 0usize;
        for s in source.by_ref() {
            if skipped < SKIP {
                skipped += 1;
                continue;
            }
            taken.push(s);
            if taken.len() >= TAKE {
                break;
            }
        }
        let _ = child.kill();
        let n = taken.len() as f64;
        assert!(n > 1000.0, "解码样本过少（ffmpeg 未产出）: {}", n);
        let mut sum_sq = 0.0f64;
        let mut diff_sq = 0.0f64;
        for i in 0..taken.len() {
            let x = taken[i] as f64 / 32768.0;
            sum_sq += x * x;
            if i > 0 {
                let d = x - taken[i - 1] as f64 / 32768.0;
                diff_sq += d * d;
            }
        }
        let rms = (sum_sq / n).sqrt();
        let hf = diff_sq / sum_sq.max(1e-9);
        eprintln!("SACD PIPELINE rms={:.3} hf={:.3}（白噪≈2，音乐远低于2）", rms, hf);
        assert!(hf < 1.3, "播放链路解出白噪 hf={:.3}", hf);
    }

    /// 端到端验证「多声道 PCM WAV」播放链路（ffprobe 探测 → `ffmpeg -ac` → PcmStreamSource 下混）：
    /// A) 不再解出白噪；B) 供给 rodio 的声道为设备立体声；C) `src_channels` 与探测值一致
    /// —— C 不一致正是"逐帧错位成宽带白噪"的根因，故此断言直接守住回归。
    ///
    /// `TPLAYER_TEST_MULTICH_WAV` 指定样本文件（如 7 声道 6.1 抓轨 WAV）。
    #[test]
    fn multichannel_wav_pipeline_not_white_noise() {
        let file = match std::env::var("TPLAYER_TEST_MULTICH_WAV") {
            Ok(v) => v,
            Err(_) => {
                eprintln!("TPLAYER_TEST_MULTICH_WAV unset; skipping");
                return;
            }
        };
        let path = Path::new(&file);
        let eq = std::sync::Arc::new(std::sync::Mutex::new(EqProcessor::new(
            44100,
            2,
            &EqConfig::default(),
        )));
        // 从 30s 起播，绕开多数抓轨的静音前导
        let (mut child, mut source) =
            spawn_ffmpeg_decoder(path, 30_000, None, &eq).expect("启动 ffmpeg 解码失败");

        // C) 声道对齐（不一致时 PcmStreamSource 会按错误步长切帧）
        if let Some((_, _, ch)) = probe_audio_stream(path) {
            assert_eq!(
                source.src_channels, ch,
                "ffmpeg 输出声道与 PcmStreamSource 假设不一致（会逐帧错位成白噪）"
            );
        }
        // B) 供给 rodio 的声道应为设备立体声
        assert_eq!(source.channels, 2, "输出声道应适配为立体声");

        const TAKE: usize = 44_100 * 2 * 3;
        let mut taken: Vec<i16> = Vec::with_capacity(TAKE);
        for s in source.by_ref() {
            taken.push(s);
            if taken.len() >= TAKE {
                break;
            }
        }
        let _ = child.kill();
        let _ = child.wait();

        assert!(taken.len() > 44_100, "解码样本过少: {}", taken.len());
        // A) 按声道分组统计高频占比（交错数据的跨声道相邻差无意义）
        for ch in 0..2 {
            let mono: Vec<f64> = taken
                .iter()
                .skip(ch)
                .step_by(2)
                .map(|&s| s as f64 / 32768.0)
                .collect();
            let sum_sq: f64 = mono.iter().map(|x| x * x).sum();
            let diff_sq: f64 = mono.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum();
            let hf = diff_sq / sum_sq.max(1e-9);
            eprintln!("MULTICH WAV ch{ch} hf={hf:.3}（白噪≈2，音乐远低于 2）");
            assert!(hf < 1.3, "声道 {ch} 解出白噪 hf={hf:.3}");
        }
    }

    /// 构造 WAV 文件字节：RIFF + WAVE + fmt(16B, PCM 44.1kHz/16bit/stereo) + 可选前置 chunk + data
    fn wav_bytes(data: &[u8], pre_chunk: Option<(&[u8], &[u8])>) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        let mut body = Vec::new();
        body.extend_from_slice(b"WAVE");
        body.extend_from_slice(b"fmt ");
        body.extend_from_slice(&16u32.to_le_bytes());
        body.extend_from_slice(&1u16.to_le_bytes()); // PCM
        body.extend_from_slice(&2u16.to_le_bytes()); // stereo
        body.extend_from_slice(&44100u32.to_le_bytes());
        body.extend_from_slice(&(44100u32 * 4).to_le_bytes());
        body.extend_from_slice(&4u16.to_le_bytes());
        body.extend_from_slice(&16u16.to_le_bytes());
        if let Some((id, payload)) = pre_chunk {
            body.extend_from_slice(id);
            body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            body.extend_from_slice(payload);
            if payload.len() % 2 != 0 {
                body.push(0);
            }
        }
        body.extend_from_slice(b"data");
        body.extend_from_slice(&(data.len() as u32).to_le_bytes());
        body.extend_from_slice(data);
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(&body);
        v
    }

    /// 构造 WAVEFORMATEXTENSIBLE 头（多声道 PCM 必须用此形式声明 channel mask）
    fn wav_bytes_ext(data: &[u8], channels: u16) -> Vec<u8> {
        let block_align = channels * 2;
        let mask: u32 = match channels {
            6 => 0x3F,  // 5.1：FL FR FC LFE BL BR
            7 => 0x70F, // 6.1：FL FR FC LFE BL BR FLC
            8 => 0x63F, // 7.1：FL FR FC LFE BL BR SL SR
            _ => 0x3,   // 立体声
        };
        let mut body = Vec::new();
        body.extend_from_slice(b"WAVE");
        body.extend_from_slice(b"fmt ");
        body.extend_from_slice(&40u32.to_le_bytes()); // WAVEFORMATEXTENSIBLE
        body.extend_from_slice(&0xFFFEu16.to_le_bytes()); // WAVE_FORMAT_EXTENSIBLE
        body.extend_from_slice(&channels.to_le_bytes());
        body.extend_from_slice(&44100u32.to_le_bytes());
        body.extend_from_slice(&(44100u32 * block_align as u32).to_le_bytes());
        body.extend_from_slice(&block_align.to_le_bytes());
        body.extend_from_slice(&16u16.to_le_bytes()); // wBitsPerSample
        body.extend_from_slice(&22u16.to_le_bytes()); // cbSize
        body.extend_from_slice(&16u16.to_le_bytes()); // wValidBitsPerSample
        body.extend_from_slice(&mask.to_le_bytes());
        // SubFormat：KSDATAFORMAT_SUBTYPE_PCM
        body.extend_from_slice(&[
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38,
            0x9B, 0x71,
        ]);
        body.extend_from_slice(b"data");
        body.extend_from_slice(&(data.len() as u32).to_le_bytes());
        body.extend_from_slice(data);
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(&body);
        v
    }

    fn write_temp(name: &str, bytes: &[u8]) -> PathBuf {
        // name 含扩展名且置于末尾，保证 extension() 解析正确
        let p = std::env::temp_dir().join(format!(
            "tplayer-engine-{}-{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            name
        ));
        std::fs::write(&p, bytes).unwrap();
        p
    }

    /// 均衡器频响：只把第 0 段（60Hz）抬到 +15 dB，其余保持 0 dB。
    ///
    /// 期望：60Hz 正弦显著变响（≈ +15 dB ≈ 5.6 倍），1kHz 正弦基本不变。
    ///
    /// 这条同时是「构造时 10 段一律用 `bands[0]` 调谐」那个 bug 的回归闸：若各频段
    /// 又被误用同一组系数，1kHz 也会被一起放大，第二条断言即失败。
    #[test]
    fn eq_boosts_only_its_own_band() {
        const RATE: u32 = 44100;

        let mut cfg = EqConfig::default();
        cfg.enabled = true;
        for b in cfg.bands.iter_mut() {
            b.gain_db = 0.0;
        }
        cfg.bands[0].gain_db = 15.0;

        /// 送 1 秒正弦过 EQ，丢弃前 0.5 秒让滤波器状态收敛，返回后 0.5 秒的 RMS。
        ///
        /// 每次用**全新**的 EqProcessor，避免上一次测量留下的滤波器状态影响本次结果。
        ///
        /// 幅度取 0.05（-26 dBFS）：+15 dB 后为 0.28 满量程，全程不削波——
        /// 削波会产生谐波并污染 RMS，让「其它频段不受影响」的判断失真。
        fn rms_after_eq(cfg: &EqConfig, freq: f32) -> f64 {
            let mut eq = EqProcessor::new(RATE, 2, cfg);
            let n = RATE as usize;
            let mut frame = Vec::with_capacity(n * 2);
            for i in 0..n {
                let s = (2.0 * PI * freq as f64 * i as f64 / RATE as f64).sin();
                let v = (s * 0.05 * 32767.0) as i16;
                frame.push(v); // L
                frame.push(v); // R
            }
            let half = frame.len() / 2;
            let mut tail = frame[half..].to_vec();
            eq.process_interleaved(&mut tail);
            let mut sum = 0.0f64;
            for s in tail.iter() {
                sum += (*s as f64 / 32768.0).powi(2);
            }
            (sum / tail.len() as f64).sqrt()
        }

        // 基线：EQ 关闭（默认）时原样透传
        let default = EqConfig::default();
        let base_60 = rms_after_eq(&default, 60.0);
        let base_1k = rms_after_eq(&default, 1000.0);

        let boost_60 = rms_after_eq(&cfg, 60.0);
        let boost_1k = rms_after_eq(&cfg, 1000.0);

        let ratio_60 = boost_60 / base_60;
        let ratio_1k = boost_1k / base_1k;

        // 60Hz：理论 +15 dB = 5.62 倍；实测约 5.53（i16 量化与收敛误差 ~1.7%）
        assert!(
            ratio_60 > 5.0 && ratio_60 < 6.0,
            "60Hz 段应被抬升约 5.6 倍，实测 {ratio_60:.2} 倍"
        );
        // 1kHz：理论 1.0098 倍（+0.08 dB）。0 dB 的峰值滤波器数学上严格透明
        // （b 系数与 a 系数完全相同），故其它频段不该有明显影响。
        assert!(
            ratio_1k > 0.95 && ratio_1k < 1.05,
            "1kHz 不应被 60Hz 段影响（理论 1.01 倍），实测 {ratio_1k:.4} 倍"
        );
    }

    #[test]
    fn multichannel_pcm_wav_routes_to_ffmpeg() {
        // 6.1 环绕 WAV（7ch 无损抓轨）：rodio 混音器会把交错样本按 2ch 步长重排成
        // 白噪音，必须识别为非原生 → ffmpeg 管道 + PcmStreamSource 下混。
        for ch in [6u16, 7, 8] {
            let p = write_temp(&format!("multi{ch}.wav"), &wav_bytes_ext(&vec![0u8; 64], ch));
            assert!(
                !is_native_format(&p),
                "{ch} 声道 PCM WAV 应路由到 ffmpeg 而非原生解码"
            );
            let _ = std::fs::remove_file(&p);
        }
    }

    #[test]
    fn stereo_extensible_wav_stays_native() {
        // 立体声 WAVEFORMATEXTENSIBLE（common in 48k 母带）仍走原生低延迟链路
        let p = write_temp("stereo-ext.wav", &wav_bytes_ext(&[0x11, 0x22, 0x33, 0x44], 2));
        assert!(is_native_format(&p));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn wav_head_channels_reads_extensible_field() {
        for ch in [2u16, 6, 7, 8] {
            let p = write_temp(&format!("ch{ch}.wav"), &wav_bytes_ext(&[0u8; 16], ch));
            assert_eq!(wav_head_channels(&p), Some(ch));
            let _ = std::fs::remove_file(&p);
        }
    }

    /// 构造最小 FLAC 头（fLaC + STREAMINFO metadata block）
    fn flac_bytes(channels: u16) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"fLaC");
        v.push(0x00); // last-block=0, type=0 (STREAMINFO)
        v.extend_from_slice(&[0x00, 0x00, 0x22]); // 长度 34
        let mut info = vec![0u8; 34];
        // 偏移 12：sample_rate 低 4 位 | (channels-1) << 1 | bps 最高位
        info[12] = (((channels - 1) & 0x07) << 1) as u8;
        v.extend_from_slice(&info);
        v
    }

    #[test]
    fn flac_head_channels_reads_streaminfo() {
        for ch in [1u16, 2, 6, 8] {
            let p = write_temp(&format!("s{ch}.flac"), &flac_bytes(ch));
            assert_eq!(flac_head_channels(&p), Some(ch));
            let _ = std::fs::remove_file(&p);
        }
    }

    #[test]
    fn multichannel_flac_routes_to_ffmpeg_stereo_stays_native() {
        let p6 = write_temp("surround.flac", &flac_bytes(6));
        assert!(!is_native_format(&p6), "5.1 FLAC 应走 ffmpeg 下混");
        let _ = std::fs::remove_file(&p6);

        let p2 = write_temp("stereo.flac", &flac_bytes(2));
        assert!(is_native_format(&p2), "立体声 FLAC 保持原生低延迟链路");
        let _ = std::fs::remove_file(&p2);
    }

    #[test]
    fn six_point_one_downmix_keeps_stereo_image() {
        // 6.1：FL=8000 FR=-8000 FC=2000 LFE=9999 BL=1000 BR=-1000 FLC=1000
        // 期望：左 > 右（前左主导），LFE 不参与（否则左右都会异常抬高）
        let out = adapt_channels(&[8000, -8000, 2000, 9999, 1000, -1000, 1000], 2);
        assert_eq!(out.len(), 2);
        assert!(out[0] > 0 && out[1] < 0, "左右应保持反相（前左/前右主导）");
        assert!(
            out[0] < 32000 && out[1] > -32000,
            "LFE 不应被并入（合并会把左右都拉向正/负饱和）"
        );
    }

    #[test]
    fn true_pcm_wav_stays_native() {
        let p = write_temp("pcm.wav", &wav_bytes(&[0x00, 0x00, 0x12, 0x34, 0xAB, 0xCD], None));
        assert!(is_native_format(&p));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn dts_in_wav_standard_sync_routes_to_ffmpeg() {
        let dts = [0x7F, 0xFE, 0x80, 0x01, 0x11, 0x22, 0x33, 0x44];
        let p = write_temp("dts.wav", &wav_bytes(&dts, None));
        assert!(!is_native_format(&p));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn dts_in_wav_with_list_chunk_detected() {
        // LIST chunk 将 data 起始推到 64B 之后（旧 64B 读头会 miss 的场景）
        let dts = [0x7F, 0xFE, 0x80, 0x01, 0x00, 0x01, 0x02, 0x03];
        let list_payload = b"INFOIART\050\000\000\000TPlayer test artist padding!";
        let p = write_temp("dts-list.wav", &wav_bytes(&dts, Some((b"LIST", list_payload))));
        assert!(!is_native_format(&p));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn dts_in_wav_hd_and_le_syncs_detected() {
        for sync in [
            [0x64u8, 0x58, 0x20, 0x25], // DTS-HD
            [0x01u8, 0x80, 0xFE, 0x7F], // 16-bit LE
            [0xFFu8, 0x1F, 0x00, 0xE8], // 14-bit BE
            [0xE8u8, 0x00, 0x1F, 0xFF], // 14-bit LE
        ] {
            let mut data = sync.to_vec();
            data.extend_from_slice(&[0x55, 0x66, 0x77, 0x88]);
            let p = write_temp("dts-hd.wav", &wav_bytes(&data, None));
            assert!(!is_native_format(&p), "sync {:02X?} 应识别为 DTS", sync);
            let _ = std::fs::remove_file(&p);
        }
    }

    #[test]
    fn non_riff_and_native_exts_unaffected() {
        // 非 WAV 扩展名不做内容深检
        let flac = write_temp("x.flac", b"fLaC\x00\x00\x00\x22");
        assert!(is_native_format(&flac));
        let _ = std::fs::remove_file(&flac);
        // DTS 裸流扩展名直接走 ffmpeg
        let dts = write_temp("x.dts", &[0x7F, 0xFE, 0x80, 0x01]);
        assert!(!is_native_format(&dts));
        let _ = std::fs::remove_file(&dts);
        // RIFF 但 fmt/data 结构损坏（解析不到 data）：按 wav 原生处理，交由解码错误链路兜底
        let broken = write_temp("broken.wav", b"RIFF\x24\x00\x00\x00WAVEfmt ");
        assert!(is_native_format(&broken));
        let _ = std::fs::remove_file(&broken);
    }

    #[test]
    fn riff_data_offset_walks_chunks() {
        let dts = [0x7Fu8, 0xFE, 0x80, 0x01];
        let bytes = wav_bytes(&dts, None);
        let off = riff_data_offset(&bytes).unwrap();
        // 44 = 12(RIFF头) + 8+16(fmt) + 8(data头)
        assert_eq!(off, 44);
        assert_eq!(&bytes[off..off + 4], &[0x7F, 0xFE, 0x80, 0x01]);
        assert!(head_data_has_dts(&bytes));
    }
}

/// Linux 回退探测的 Duration 行解析测试（仅 Linux 目标编译；
/// parse_duration_line 本身为纯函数，无平台副作用）
#[cfg(test)]
mod probe_tests {
    use super::*;

    #[test]
    fn dsd_extensions_are_routed_to_ffmpeg() {
        // DSD 扩展名需进入 ffmpeg 管道（dsd_lsbf 解码器负责 DSD→PCM）
        assert!(FFMPEG_EXT.contains(&"dsf"));
        assert!(FFMPEG_EXT.contains(&"dff"));
    }

    fn approx(got: Option<f64>, want: f64) {
        let got = got.expect("应解析出时长");
        assert!((got - want).abs() < 1e-6, "got={got} want={want}");
    }

    #[test]
    fn parses_typical_line() {
        let stderr = "ffmpeg version N-127579\nInput #0, flac, from 'x.flac':\n  Duration: 00:02:00.65, start: 0.000000, bitrate: 611 kb/s\n    Stream #0:0: Audio: flac, 44100 Hz\nAt least one output file must be specified\n";
        approx(parse_duration_line(stderr), 2.0 * 60.0 + 0.65);
    }

    #[test]
    fn parses_hours_and_missing_start() {
        // WAV 无 start 字段；跨小时时长
        let stderr = "Input #0, wav, from 'x.wav':\n  Duration: 01:30:05.50, bitrate: 1411 kb/s\n";
        approx(parse_duration_line(stderr), 3600.0 + 30.0 * 60.0 + 5.5);
    }

    #[test]
    fn none_when_missing_or_na_or_malformed() {
        assert_eq!(
            parse_duration_line("Input #0 ...\nAt least one output file must be specified\n"),
            None
        );
        assert_eq!(parse_duration_line("  Duration: N/A, bitrate: N/A\n"), None);
        assert_eq!(parse_duration_line("  Duration: 12:34\n"), None); // 段数不足
        assert_eq!(parse_duration_line("  Duration: 00:00:00.00, start: 0\n"), None); // 零时长
    }

    // ---- 声道适配（多声道解码 → 设备输出）----

    #[test]
    fn stereo_passthrough() {
        assert_eq!(adapt_channels(&[100, -100], 2), vec![100, -100]);
    }

    #[test]
    fn mono_duplicated() {
        assert_eq!(adapt_channels(&[500], 2), vec![500, 500]);
    }

    #[test]
    fn surround_5_1_downmix_bs775() {
        // L R C LFE Ls Rs = 1000 1000 1000 0 1000 1000
        let out = adapt_channels(&[1000, 1000, 1000, 0, 1000, 1000], 2);
        assert_eq!(out.len(), 2);
        let expect = 1000f32 + std::f32::consts::FRAC_1_SQRT_2 * (1000f32 + 1000f32);
        assert!((out[0] as f32 - expect).abs() < 2.0, "L={} expect={}", out[0], expect);
        assert!((out[1] as f32 - expect).abs() < 2.0);
    }

    #[test]
    fn surround_downmix_no_clipping_on_typical_levels() {
        let out = adapt_channels(&[8000, 7000, 5000, 4000, 3000, 2000], 2);
        assert!(out[0] > i16::MIN && out[0] < i16::MAX);
        assert!(out[1] > i16::MIN && out[1] < i16::MAX);
    }

    #[test]
    fn generic_multichannel_round_robin() {
        // 4ch → 2ch：奇偶均分
        assert_eq!(adapt_channels(&[100, 200, 300, 400], 2), vec![200, 300]);
    }

    #[test]
    fn six_channel_frames_not_misread_as_two() {
        // 回归：6ch 帧若被当 2ch 会帧错位成噪声；下混后首帧须等于 BS.775 结果
        let frame: Vec<i16> = vec![10, 20, 30, 40, 50, 60];
        let out = adapt_channels(&frame, 2);
        let expect_l = 10f32 + std::f32::consts::FRAC_1_SQRT_2 * (30f32 + 50f32);
        assert!((out[0] as f32 - expect_l).abs() < 2.0);
    }

    /// 端到端：5.1 解包出的 6ch DSF 走完整播放链路（探测 → ffmpeg 6ch → 下混 2ch）。
    /// 用环境变量 DSF_6CH_TEST 指定文件（本地回归用；CI 无此文件自动跳过）。
    #[test]
    fn six_channel_dsf_downmix_pipeline() {
        let Ok(p) = std::env::var("DSF_6CH_TEST") else {
            eprintln!("DSF_6CH_TEST unset; skipping");
            return;
        };
        let path = Path::new(&p);
        if !path.exists() {
            eprintln!("DSF_6CH_TEST file missing; skipping");
            return;
        }
        // EQ 由引擎与各音源共享（Arc），测试里就地建一份默认（关闭）的
        let eq = std::sync::Arc::new(std::sync::Mutex::new(EqProcessor::new(
            44100,
            2,
            &EqConfig::default(),
        )));
        let (mut child, mut source) =
            spawn_ffmpeg_decoder(path, 0, None, &eq).expect("spawn ffmpeg");
        use rodio::Source as _;
        // 源为 6ch，供给 rodio 的必须是设备声道（2ch），否则 rodio 会帧错位
        assert_eq!(source.channels(), 2, "多声道源须在输出侧适配为设备声道");
        assert_eq!(source.sample_rate(), 44100);
        let skip = 44_100 * 2 * 2;
        let take = 44_100 * 2 * 3;
        let mut taken: Vec<i16> = Vec::with_capacity(take);
        let mut skipped = 0usize;
        for s in source.by_ref() {
            if skipped < skip {
                skipped += 1;
                continue;
            }
            taken.push(s);
            if taken.len() >= take {
                break;
            }
        }
        let _ = child.kill();
        assert!(taken.len() > take / 2, "样本不足: {}", taken.len());
        let rms = (taken.iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>()
            / taken.len() as f64)
            .sqrt();
        eprintln!("6ch DSF 下混后 rms={rms:.3}");
        assert!(rms > 0.001, "下混输出近似静音（rms={rms}）");
    }
}
