//! 播放引擎：rodio 实现 + 线程通道封装
//!
//! rodio 的 OutputStream/Sink 在 Windows 上非 Send（COM STA 绑定），
//! 因此引擎运行在专用线程，PlayerHandle 通过 mpsc 通道发送命令。

use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::error::{AppError, AppResult};

/// symphonia 原生可解码的扩展名；其余格式走 ffmpeg 子进程解码为 PCM
const NATIVE_EXT: &[&str] = &["mp3", "flac", "wav", "ogg", "oga", "opus", "aac", "m4a"];

/// ffmpeg 可解码的补充格式（ape/dsd/dts/wma/wv/tta/tak/aiff 等）
const FFMPEG_EXT: &[&str] = &["wma", "aif", "aiff", "ape", "wv", "tta", "tak", "dsf", "dff", "dts"];

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
    const HEAD_LEN: usize = 4096;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut head = vec![0u8; HEAD_LEN];
    let mut total = 0usize;
    while total < HEAD_LEN {
        match file.read(&mut head[total..]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(_) => break,
        }
    }
    head_data_has_dts(&head[..total])
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
    if ext == "wav" && wav_head_contains_dts(path) {
        return false;
    }
    true
}

/// 定位 bin 目录下的工具（ffmpeg/ffprobe）：覆盖开发期与安装期多种布局。
///
/// - 沿可执行文件祖先目录查找 `<ancestor>/bin/<name>`（开发期 `src-tauri/bin`、
///   构建产物目录下的 `bin/` 等）；
/// - 安装包下 Tauri 把资源放到 `<app>/resources/bin/<name>`，单独补查该路径与
///   同目录 `<app>/<name>`（避免 Release 版本找不到 ffmpeg 导致 DSD/APE/DTS 等
///   依赖第三方解码的格式无法播放、进度与拖动失效）。
fn resolve_bin_tool(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    for ancestor in exe.ancestors().skip(1) {
        let cand = ancestor.join("bin").join(name);
        if cand.is_file() {
            return Some(cand);
        }
    }
    if let Some(parent) = exe.parent() {
        for rel in [
            std::path::Path::new("resources/bin").join(name),
            std::path::Path::new("bin").join(name),
            std::path::Path::new(name).to_path_buf(),
        ] {
            let cand = parent.join(rel);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

pub fn resolve_ffmpeg() -> Option<PathBuf> {
    resolve_bin_tool("ffmpeg.exe")
}

pub fn resolve_ffprobe() -> Option<PathBuf> {
    resolve_bin_tool("ffprobe.exe")
}

pub fn ffmpeg_available() -> bool {
    resolve_ffmpeg().is_some()
}

/// 用 ffprobe 探测音频时长（秒）。
/// 用于 lofty 无法解析的格式（dsf/dff/dts/tta/tak 等）在扫描与播放时的兜底；
/// 失败（工具缺失/输出异常/时长非正）返回 None。
pub fn probe_duration(path: &Path) -> Option<f64> {
    let ffprobe = resolve_ffprobe()?;
    let mut cmd = Command::new(ffprobe);
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
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let secs = text.trim().parse::<f64>().ok()?;
    (secs > 0.0).then_some(secs)
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
    Play(PathBuf, Option<(f32, f32)>, Option<(f64, f64)>, mpsc::Sender<AppResult<()>>),
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
    Preload(PathBuf, Option<(f32, f32)>, Option<(f64, f64)>, mpsc::Sender<AppResult<()>>),
    /// 把预加载链路提升为当前曲目（旧曲目转入淡出）
    SwapToNext(mpsc::Sender<AppResult<()>>),
    /// 更新音量归一化 ReplayGain 开关（开启时当前曲目立即应用增益）
    SetReplayGain(bool),
    /// 更新播放倍速（1.0 = 原速）；立即作用于当前曲目与待命预加载链路
    SetRate(f32),
}

/// 引擎句柄（Send）：PlayerHandle 通过它控制播放
#[derive(Clone)]
pub struct EngineHandle {
    tx: mpsc::Sender<EngineCommand>,
}

impl EngineHandle {
    pub fn play(&self, path: PathBuf, gain: Option<(f32, f32)>, range: Option<(f64, f64)>) -> AppResult<()> {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.tx
            .send(EngineCommand::Play(path, gain, range, ack_tx))
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
    pub fn preload(&self, path: std::path::PathBuf, gain: Option<(f32, f32)>, range: Option<(f64, f64)>) -> AppResult<()> {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.tx
            .send(EngineCommand::Preload(path, gain, range, ack_tx))
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
            let mut fade_enabled = true;
            let mut fade_duration_ms: u64 = 400;
            // recv 改为带超时轮询：无命令期间也能以 ~20ms 步进驱动淡入淡出
            loop {
                match rx.recv_timeout(Duration::from_millis(20)) {
                    Ok(cmd) => match cmd {
                        EngineCommand::Play(path, gain, range, ack) => {
                            let _ = ack.send(engine.play(&path, gain, range));
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
                        EngineCommand::Preload(path, gain, range, ack) => {
                            let _ = ack.send(engine.preload(&path, gain, range));
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
}

impl Playback {
    /// 立即终止播放（sink 停止拉流，ffmpeg 子进程随后退出；Drop 再 kill 兜底）
    fn halt(mut self) {
        self.sink.stop();
        if let Some(child) = self.ffmpeg_child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.ffmpeg_child = None;
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        if let Some(mut child) = self.ffmpeg_child.take() {
            let _ = child.kill();
            let _ = child.wait();
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

    /// smoothstep 缓动，避免线性渐变在端点处可闻的台阶感
    fn ease(t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// 渐变驱动（每 ~20ms）：新曲目淡入到目标音量，旧曲目淡出到静默后回收
    /// fade_duration 为当前生效的交叉淡化时长（ms）；0 时关闭淡化。
    fn tick_fades(&mut self, fade_duration_ms: u64) {
        let fade_ms = if fade_duration_ms > 0 {
            fade_duration_ms
        } else {
            // 关闭淡入淡出：直接设满音量 / 静默
            if let Some(started) = self.fade_in_started {
                let _ = started.elapsed();
                if let Some(playback) = &self.current {
                    playback.sink.set_volume(self.target_volume());
                }
                self.fade_in_started = None;
            }
            if let Some(fading) = &self.fading_out {
                fading.playback.sink.set_volume(0.0);
                if let Some(done) = self.fading_out.take() {
                    done.playback.halt();
                }
            }
            return;
        };

        // 新曲目淡入
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

        // 旧曲目淡出：结束即停止并释放（含 ffmpeg 子进程）
        if let Some(fading) = &self.fading_out {
            let t = fading.started.elapsed().as_millis() as f32 / fade_ms as f32;
            let gain = fading.from * (1.0 - Self::ease(t));
            fading.playback.sink.set_volume(gain);
            if t >= 1.0 {
                if let Some(done) = self.fading_out.take() {
                    done.playback.halt();
                }
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

    fn play(&mut self, path: &std::path::Path, gain: Option<(f32, f32)>, range: Option<(f64, f64)>) -> AppResult<()> {
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
            let from = match &self.fade_in_started {
                Some(started) => {
                    let t = started.elapsed().as_millis() as f32 / self.fade_ms as f32;
                    self.target_volume() * Self::ease(t)
                }
                None => self.target_volume(),
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
        let duration_ms = range
            .and_then(|(_, d)| if d > 0.0 { Some((d * 1000.0) as u64) } else { None });

        let is_ffmpeg = !is_native_format(path);
        let ffmpeg_child = if is_ffmpeg {
            let (child, source) = spawn_ffmpeg_decoder(path, start_ms, duration_ms)?;
            sink.append(source);
            Some(child)
        } else {
            let file = std::fs::File::open(path).map_err(|e| {
                AppError::Player(format!("无法打开音频文件 {}: {}", path.display(), e))
            })?;
            let decoder = rodio::Decoder::new(BufReader::new(file))
                .map_err(|e| AppError::Player(format!("不支持的音频格式或文件损坏: {}", e)))?;
            sink.append(decoder);
            // CUE 分轨：原生解码路径按起始偏移 seek（时长由播放状态机按 per-track duration 推进）
            if let Some((start, _)) = range {
                if start > 0.0 {
                    let _ = sink.try_seek(std::time::Duration::from_secs_f64(start));
                }
            }
            None
        };

        // 新曲目从静默开始淡入
        sink.set_volume(0.0);
        // 继承当前倍速（切歌/seek 重启后避免倍速丢失）
        sink.set_speed(self.rate);
        self.current = Some(Playback {
            sink,
            ffmpeg_child,
            gain_scale,
        });
        self.fade_in_started = Some(Instant::now());
        self.current_path = Some(path.to_path_buf());
        self.current_is_ffmpeg = is_ffmpeg;
        Ok(())
    }

    /// 预解码下一曲到待命链路：创建 sink + 解码器/ffmpeg 子进程，
    /// 但 sink 立即暂停不发声，等待 SwapToNext 或 play 路径一致时复用。
    fn preload(&mut self, path: &std::path::Path, gain: Option<(f32, f32)>, range: Option<(f64, f64)>) -> AppResult<()> {
        // 新预加载前先回收旧预加载链路
        if let Some(stale) = self.next.take() {
            stale.halt();
        }

        let handle = self.ensure_stream()?.clone();
        let gain_scale = self.gain_scale_for(gain);
        let sink = rodio::Sink::try_new(&handle)
            .map_err(|e| AppError::Player(format!("无法创建音频 sink: {}", e)))?;

        let start_ms = range.map(|(s, _)| (s * 1000.0) as u64).unwrap_or(0);
        let duration_ms = range
            .and_then(|(_, d)| if d > 0.0 { Some((d * 1000.0) as u64) } else { None });

        let is_ffmpeg = !is_native_format(path);
        let ffmpeg_child = if is_ffmpeg {
            let (child, source) = spawn_ffmpeg_decoder(path, start_ms, duration_ms)?;
            sink.append(source);
            Some(child)
        } else {
            let file = std::fs::File::open(path).map_err(|e| {
                AppError::Player(format!("无法打开音频文件 {}: {}", path.display(), e))
            })?;
            let decoder = rodio::Decoder::new(BufReader::new(file))
                .map_err(|e| AppError::Player(format!("不支持的音频格式或文件损坏: {}", e)))?;
            sink.append(decoder);
            // CUE 分轨：原生解码路径按起始偏移 seek（时长由播放状态机按 per-track duration 推进）
            if let Some((start, _)) = range {
                if start > 0.0 {
                    let _ = sink.try_seek(std::time::Duration::from_secs_f64(start));
                }
            }
            None
        };

        // 待命：暂停不发声，避免预加载期间泄漏音频
        sink.pause();
        // 待命链路也继承倍速（swap 接管后延续）
        sink.set_speed(self.rate);
        self.next = Some(Playback {
            sink,
            ffmpeg_child,
            gain_scale,
        });
        self.next_path = Some(path.to_path_buf());
        Ok(())
    }

    /// 无缝接管预加载链路：旧曲目转入淡出，新曲目从静默淡入并开始发声。
    fn swap_to_next(&mut self) -> AppResult<()> {
        let next = self
            .next
            .take()
            .ok_or_else(|| AppError::Player("没有预加载的曲目".into()))?;
        let next_path = self.next_path.take();

        // 旧曲目转入淡出链路（交叉淡化）；淡出起点取其当前实际增益
        if let Some(old) = self.current.take() {
            let from = match &self.fade_in_started {
                Some(started) => {
                    let t = started.elapsed().as_millis() as f32 / self.fade_ms as f32;
                    self.target_volume() * Self::ease(t)
                }
                None => self.target_volume(),
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
            return self.play(path, gain, None);
        }
        // 带位置起播时预加载链路不可复用（swap_to_next 只能从 0 开始），先回收
        if let Some(stale) = self.next.take() {
            stale.halt();
        }
        self.next_path = None;

        if is_native_format(path) {
            self.play(path, gain, None)?;
            // sink 新建后混音线程可能还没拉到首帧；此时立即 seek 在个别时序下
            // 会让 Sink::try_seek 永久等待（引擎线程卡死、进度冻结）。
            // 先让出 ~150ms 等输出线程开始消费，再定位（原生解码仅此路径可实现）。
            std::thread::sleep(Duration::from_millis(150));
            self.seek(position_ms)
        } else {
            // FFmpeg 管道：解码进程以 -ss 直接从目标位置起流，
            // 无"先播开头再跳转"的听感跳秒，也无 150ms 等待
            self.play(path, gain, Some((position_ms as f64 / 1000.0, 0.0)))
        }
    }

    fn pause(&mut self) -> AppResult<()> {
        if let Some(playback) = &self.current {
            playback.sink.pause();
        }
        Ok(())
    }

    fn resume(&mut self) -> AppResult<()> {
        if let Some(playback) = &self.current {
            playback.sink.play();
        }
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
        Ok(())
    }

    fn seek(&mut self, position_ms: u64) -> AppResult<()> {
        let Some(playback) = self.current.as_mut() else {
            return Ok(());
        };

        if self.current_is_ffmpeg {
            // ffmpeg 后端：PCM 管道不可 seek，带 -ss 重启解码进程
            let path = self
                .current_path
                .clone()
                .ok_or_else(|| AppError::Player("没有正在播放的曲目".into()))?;
            let (child, source) = spawn_ffmpeg_decoder(&path, position_ms, None)?;
            if let Some(old) = playback.ffmpeg_child.take() {
                let mut old = old;
                let _ = old.kill();
                let _ = old.wait();
            }
            playback.ffmpeg_child = Some(child);
            playback.sink.clear();
            playback.sink.append(source);
            playback.sink.play();
            // ffmpeg 后端 seek 重启后重新应用倍速
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

/// 启动 ffmpeg 将任意格式解码为 PCM s16le（44.1kHz 双声道）并输出到管道，
/// 返回（子进程, rodio 音源）。seek 通过 -ss 在输入端定位实现。
fn spawn_ffmpeg_decoder(
    path: &Path,
    offset_ms: u64,
    duration_ms: Option<u64>,
) -> AppResult<(Child, PcmStreamSource)> {
    let ffmpeg = resolve_ffmpeg().ok_or_else(|| {
        AppError::Player("该格式需要 FFmpeg 解码，未找到 bin/ffmpeg.exe".into())
    })?;

    let mut cmd = Command::new(ffmpeg);
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
        .arg(path)
        .arg("-f")
        .arg("s16le")
        .arg("-acodec")
        .arg("pcm_s16le")
        .arg("-ar")
        .arg("44100")
        .arg("-ac")
        .arg("2")
        .arg("-")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
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
        PcmStreamSource::new(BufReader::new(stdout), 2, 44100),
    ))
}

/// ffmpeg PCM 管道音源（s16le 交错样本 → rodio::Source）
///
/// 内部缓冲大块样本（默认 8192 样本 = 16384 字节），避免逐样本 read 导致的
/// 高频 syscall 与混音线程欠载（表现为爆音/杂音）。
struct PcmStreamSource {
    reader: BufReader<ChildStdout>,
    channels: u16,
    sample_rate: u32,
    /// 解码输出的样本缓冲（i16 交错）
    buffer: Vec<i16>,
    /// 当前缓冲读取位置
    pos: usize,
    /// 是否到达流末尾
    eof: bool,
}

impl PcmStreamSource {
    const CHUNK_SAMPLES: usize = 8192;

    fn new(reader: BufReader<ChildStdout>, channels: u16, sample_rate: u32) -> Self {
        Self {
            reader,
            channels,
            sample_rate,
            buffer: Vec::with_capacity(Self::CHUNK_SAMPLES),
            pos: 0,
            eof: false,
        }
    }

    /// 从管道填充一块样本到 buffer；失败/EOF 时标记 eof
    fn refill(&mut self) {
        if self.eof {
            return;
        }
        let mut bytes = vec![0u8; Self::CHUNK_SAMPLES * 2];
        match self.reader.read(&mut bytes) {
            Ok(0) => {
                self.eof = true;
            }
            Ok(n) => {
                let sample_bytes = n - (n % 2);
                self.buffer.clear();
                self.buffer.extend(
                    bytes[..sample_bytes]
                        .chunks_exact(2)
                        .map(|c| i16::from_le_bytes([c[0], c[1]])),
                );
                self.pos = 0;
            }
            Err(_) => {
                self.eof = true;
            }
        }
    }
}

impl Iterator for PcmStreamSource {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        loop {
            if self.pos < self.buffer.len() {
                let sample = self.buffer[self.pos];
                self.pos += 1;
                return Some(sample);
            }
            if self.eof {
                return None;
            }
            self.refill();
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
