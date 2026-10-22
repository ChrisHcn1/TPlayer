//! SACD ISO 解析（纯 Rust 实现，零外部 crate 依赖）。
//!
//! Scarlet Book 规范下的 SACD 镜像特征：
//! - 非标准 ISO9660/UDF 文件系统；直接按 LBA（2048 字节/扇区）访问 Master TOC。
//! - Master TOC 在偏移 0x10000（第一个 TOC 扇区通常位于 LBA 0x20000 附近），
//!   以魔数 "SACDMTA\0" 开头。
//! - 区域 TOC（2ch / 6ch）记录该区域的曲目数、每轨起始 LBA、帧数、文本元数据。
//! - 音频数据按 DSD 帧组织：DSD64 = 每帧 588 字节（双声道），每秒 75 帧。
//!
//! 本模块只做"解析 + 导出临时 DSF"，把解码（DSD→PCM）交给现有 FFmpeg 管道——
//! 这样与项目已有的 dsf/dff 路径完全共用 seek / volume / replaygain 逻辑。
//!
//! 注意：多声道区规范强制 DST 压缩，当前版本只解析 2ch 立体声区（绝大多数音乐碟
//! 立体声区是未压缩 DSD，ffmpeg 可以直接解码）；DST 压缩轨会标记为不可播放，
//! 并返回明确错误以便后续版本集成 DST 解压。
//!
//! v2.0.2 起：DST 压缩区与多声道（5.1）区改由随包分发的 sacd_extract 解码
//! （见 `extract_track_via_external`），两区曲目都会入库（标题带 `[5.1]` 后缀）。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};

use serde::{Deserialize, Serialize};

use super::models::{album_id_of, Track};

/// SACD 扇区大小（字节）
const SACD_SECTOR_SIZE: u64 = 2048;
/// Sony ScarletBooks ISO 每扇区块头长度（v9 实测确认：2048 字节扇区 = 32 字节块头 + 2016 字节 DSD 数据）。
/// 不剥离这些块头会把 32 字节垃圾混入 DSD 位流 → 宽带白噪。
const SACD_SECTOR_HEADER: u64 = 32;
/// 每扇区有效 DSD 数据字节数（2048 − 32）
const SACD_SECTOR_DATA: u64 = SACD_SECTOR_SIZE - SACD_SECTOR_HEADER;
/// DSF 文件头字节数（DSD chunk 28 + fmt chunk 52 + data chunk 头 12）。
pub(crate) const DSF_HEADER_BYTES: u64 = 92;
/// SACD 导出缓存目录版本：导出/校验逻辑变更时递增，整体失效旧缓存，
/// 避免复用更早版本（含去交错/位序/块头错误）产出的损坏 DSF（表现为播放噪声）。
/// v4：SACD 2ch 立体声区为连续 DSD 位流、无每 64KB 块头；每 DSD 帧 = 1176 字节，内为
/// 「588 字节 L + 588 字节 R」的**帧平面**布局（已用真实 ISO 实测：逐字节 L,R 交错假设会把
/// 同一声道奇偶字节拆成两路 → 塌缩为单声道并混叠成噪声）。抽取按帧平面去交错，并以
/// bits_per_sample=1（LSB-first）写出 DSF。
/// v5：DSF 头 `sampling_frequency` 改为写入 disc 实际 `dsd_sample_rate`（DSD64=2822400 /
/// DSD128=5644800 / DSD256=11289600），不再写死 2822400——否则 DSD128/256 的 SACD 头速率
/// 错误会导致 ffmpeg 按错误速率解码（变慢/失真/噪声）。
/// v6：抽取时对每字节做位反转（SACD MSB-first → DSF/LSB-first `dsd_lsbf`）。实测确认位反转方向正确
/// （不反转则频谱平坦度显著升高→更偏噪声）。
/// v7：**修正去交错粒度**：真实 SACD 2ch DSD 是「每帧 588L+588R 平面」而非「逐字节 L,R 交错」。
/// 旧 parity 去交错把同一声道奇偶字节拆为两路→单声道塌缩+混叠（听感为噪声）。改为帧平面去交错后
/// 跨多轨得到正常立体声（声道独立、包络正相关）。升级以整体失效旧版（噪声）缓存。
const SACD_CACHE_VERSION: &str = "v10";
/// ScarletBooks ISO 头部扫描长度：覆盖 Master TOC / 碟级与曲目文本 / 两张 tracklist
/// （本样本中全部位于 < 0x114000，取 0x140000 留足余量且不深入音频区）。
const SB_HEAD_SCAN: u64 = 0x140000;
/// DSD64 帧大小（双声道，每帧 2 × 588 字节/声道 = 1176 字节）
const DSD64_FRAME_BYTES: u64 = 1176;
/// DSD64 帧率（帧/秒）：DSD64 = 64×44100 样本/声道/秒，每帧 4704 样本/声道，故 600 帧/秒
const DSD_FRAMES_PER_SEC: u64 = 600;

/// SACD 区域类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SacdArea {
    Stereo2ch = 0,
    MultiCh = 1,
}

/// 从 ISO 中解析出的单条音轨（内部中间表示）
#[derive(Debug, Clone)]
pub struct SacdTrackMeta {
    pub track_number: u32,
    pub title: String,
    pub artist: Option<String>,
    /// 起始 LBA（扇区索引）
    pub start_lba: u64,
    /// 帧数（DSD 帧）
    pub frames: u64,
    /// 是否 DST 压缩
    pub is_dst: bool,
    /// 声道数（2 = 立体声区，6 = 5.1 多声道区）
    pub channels: u16,
}

/// 从 ISO 中解析出的整张专辑信息
#[derive(Debug)]
pub struct SacdDisc {
    pub album: String,
    pub artist: String,
    pub year: Option<u32>,
    pub area: SacdArea,
    pub dsd_sample_rate: u32, // DSD64/128/256
    pub tracks: Vec<SacdTrackMeta>,
    iso_path: PathBuf,
    file_size: u64,
    iso_mtime: i64,
    /// 立体声区 DSD 是否需抽取时去交错（Sony ScarletBooks ISO 为 true）。SACD 立体声区是连续
    /// DSD 位流、每扇区 2048 字节、每帧 1176 字节 = 588 字节/声道 ×2，帧内为「前 588 字节 = L、
    /// 后 588 字节 = R」的**帧平面**布局（**无每 64KB 块头**），抽取时按帧平面拆 L/R 并重排为 DSF 平面。
    /// 旧「SACDMTA」经验布局为 false（平坦顺序拷贝）。
    block_headers: bool,
    /// 末轨音频结束字节偏移（块结构导出时界定最后一轨；平坦布局用 file_size）。
    audio_end_byte: u64,
    /// 音频区为 DST 压缩或非标准布局（无 Sony 音频扇区签名），无法按裸 DSD 导出。
    /// 典型：SACD 多声道区与部分立体声区使用 DST 压缩，需专用解码器；
    /// 这类镜像请先用 sacd_extract 等工具转换为 DSF 再入库播放。
    pub unsupported: bool,
    /// 多声道（5.1）区曲目数（0 = 无多声道区）。两区曲目按相同顺序对应。
    pub multich_track_count: usize,
    /// 由 `sacd_extract -P` 探测到的真实每轨时长（秒，索引 = 曲目序号）。
    ///
    /// DST 压缩区的存储数据量只有未压缩 DSD 的 ~25%，用「区间字节数 / 每帧字节数」
    /// 推算帧数会把时长压到约 1/4（Energy 每轨 5:52 被显示成 2:44）。
    /// ISO 的 Area TOC 只给区总帧数与各轨起点、不含每轨帧数，因此时长必须由
    /// 解析了区 TOC 的 sacd_extract 提供；探测失败时为 None（回退为数据量推算）。
    probed_durations: Option<Vec<f64>>,
}

impl SacdDisc {
    /// 打开 SACD ISO 并解析 Master TOC；不是 SACD 镜像或解析失败返回 None。
    pub fn open(iso_path: &Path) -> Option<Self> {
        let mut file = File::open(iso_path).ok()?;
        let file_size = file.metadata().ok()?.len();
        let iso_mtime = file
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // 尝试若干已知 Master TOC 偏移（不同压碟/抓轨工具可能不同）
        let mtoc_offsets = [
            0x20000u64 * SACD_SECTOR_SIZE, // 规范：LBA 0x20000（SACD 最常见）
            0x00000u64 * SACD_SECTOR_SIZE, // 部分镜像 TOC 前移
            0x10000u64 * SACD_SECTOR_SIZE, // 部分抓轨偏移
            0x30000u64 * SACD_SECTOR_SIZE, // 部分压碟/抓轨工具偏移
            0x40000u64 * SACD_SECTOR_SIZE,
            0x80000u64 * SACD_SECTOR_SIZE,
        ];

        let mut toc_buf = vec![0u8; 32 * 1024]; // 32KB 足以覆盖 Master TOC 头
        let mut disc: Option<SacdDisc> = None;

        for off in mtoc_offsets {
            if off >= file_size {
                continue;
            }
            if file.seek(SeekFrom::Start(off)).is_err() {
                continue;
            }
            let n = file.read(&mut toc_buf).unwrap_or(0);
            if n < 16 {
                continue;
            }
            // Master TOC 魔数
            if &toc_buf[0..8] == b"SACDMTA\x00" || &toc_buf[4..12] == b"SACDMTA\x00" {
                if let Some(d) = parse_toc(&toc_buf[..n], iso_path, file_size, iso_mtime) {
                    disc = Some(d);
                    break;
                }
            }
        }

        // 降级方案：扫描文件头 4MB 找 Master TOC 签名（部分镜像 TOC 位置偏后）
        if disc.is_none() {
            let mut head = vec![0u8; 4 * 1024 * 1024];
            let _ = file.seek(SeekFrom::Start(0));
            let n = file.read(&mut head).unwrap_or(0);
            if n >= 8 {
                for i in 0..n.saturating_sub(8) {
                    if &head[i..i + 8] == b"SACDMTA\x00" {
                        if let Some(d) = parse_toc(
                            &head[i..n.min(i + 32 * 1024)],
                            iso_path,
                            file_size,
                            iso_mtime,
                        ) {
                            disc = Some(d);
                            break;
                        }
                    }
                }
            }
        }

        // Sony ScarletBooks 变体（SACDMTOC / SACDText / SACDTRL1 / SACDTTxt）：
        // 真实抓轨 ISO 使用该结构，而非上面假设的 "SACDMTA" 经验布局。
        // 读取头部区域（TOC + 文本，不含音频）解析曲目表与标题。
        if disc.is_none() {
            let head_len = file_size.min(SB_HEAD_SCAN) as usize;
            let mut head = vec![0u8; head_len];
            let _ = file.seek(SeekFrom::Start(0));
            // 循环读满：单次 read 可能返回不足一整块（大文件/信号中断）
            let mut filled = 0usize;
            while filled < head_len {
                match file.read(&mut head[filled..]) {
                    Ok(0) => break,
                    Ok(n) => filled += n,
                    Err(_) => break,
                }
            }
            if filled > 0 {
                head.truncate(filled);
                disc = parse_scarletbooks(&head, iso_path, file_size, iso_mtime);
            }
        }

        disc
    }

    /// 把某一音轨的 DSD 数据导出为临时 DSF 文件，返回临时文件路径。
    ///
    /// DSF 是标准 DSD 文件格式，ffmpeg 原生支持（现有 dsf 路径即可直接解码）。
    /// 仅用于未压缩 DSD 轨；DST 压缩轨返回 Err。
    pub fn export_track_to_dsf(&self, track_index: usize, out_path: &Path) -> std::io::Result<()> {
        let t = &self.tracks[track_index];
        if t.is_dst {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "SACD 多声道区使用 DST 压缩，当前版本暂不支持 DST 解压；请选择立体声 2ch 区音轨",
            ));
        }
        if self.unsupported {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "该 SACD 音频区为 DST 压缩（或非标准布局），当前版本无法解码；\
                 请先用 sacd_extract 等工具转换为 DSF 后入库播放",
            ));
        }
        // 音轨在 ISO 中的字节范围：[本轨起始扇区, 下一轨起始扇区)；末轨到 audio_end_byte。
        // v9：扇区前 32 字节为块头，DSD 数据从扇区边界 +32 开始（网格以数据起点为锚）。
        let sector_start = t.start_lba * SACD_SECTOR_SIZE;
        let start_byte = sector_start + SACD_SECTOR_HEADER;
        let end_byte = if track_index + 1 < self.tracks.len() {
            self.tracks[track_index + 1].start_lba * SACD_SECTOR_SIZE
        } else {
            self.audio_end_byte.max(start_byte + 1)
        };

        // v9：块结构需剥离每扇区 32 字节块头；旧「SACDMTA」平坦布局直接取 frames 对应字节。
        let payload_len = if self.block_headers {
            sacd_block_payload_len(start_byte, end_byte)
        } else {
            t.frames * DSD64_FRAME_BYTES
        };
        let frames = payload_len / DSD64_FRAME_BYTES;

        // 诊断日志（终端可见）：记录本轨导出的字节范围与 DSD 参数，便于排查"白噪/无法播放"问题。
        eprintln!(
            "SACD_EXPORT track={} start_byte={} end_byte={} payload_len={} frames={} dsd_sr={} deint=byteL+byteR,sector=2048/32hdr",
            track_index, start_byte, end_byte, payload_len, frames, self.dsd_sample_rate
        );

        let mut out = File::create(out_path)?;
        write_dsf_header(
            &mut out,
            frames,
            2,
            self.dsd_sample_rate,
            payload_len,
        )?;

        if self.block_headers {
            export_dsd_blocks(&self.iso_path, &mut out, start_byte, end_byte)?;
        } else {
            // 旧「SACDMTA」布局：平坦 DSD 顺序拷贝，限制长度防止跨轨读入下一曲。
            let mut file = File::open(&self.iso_path)?;
            file.seek(SeekFrom::Start(start_byte))?;
            use std::io::Write;
            let mut remaining = payload_len as usize;
            let mut buf = vec![0u8; 64 * 1024];
            while remaining > 0 {
                let to_read = remaining.min(buf.len());
                let n = file.read(&mut buf[..to_read])?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n])?;
                remaining -= n;
            }
        }
        Ok(())
    }

    /// 用外部 sacd_extract 把某轨解码为 DSF 并写入 `out_path`。
    ///
    /// 适用场景（内置裸 DSD 导出覆盖不了的情况）：
    ///  - DST 压缩的立体声区（如 Fourplay Energy 的 2ch 区）；
    ///  - 多声道（5.1）区（SACDTRL2，多为 DST，且扇区表语义与立体声区不同）。
    ///
    /// sacd_extract 自行解析 ISO 的区结构，我们只需给出区（`-2` / `-m`）与曲目号
    /// （`-t N`，1-based），输出 DSF（`-s`）到临时目录后取回。该工具随安装包分发
    /// （tauri.conf.json bundle.resources → bin/sacd_extract.exe，GPL-3.0）。
    pub fn extract_track_via_external(
        &self,
        track_number: u32,
        multich: bool,
        out_path: &Path,
    ) -> std::io::Result<()> {
        let exe = resolve_sacd_extract().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "未找到 sacd_extract（应随安装包分发），无法解码该音轨",
            )
        })?;

        // 临时输出目录：sacd_extract 会在其下建 <专辑>/<Stereo|6ch>/ 子目录
        let work = out_path.with_extension("extract-work");
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work)?;

        let mut cmd = crate::util::hidden_command(&exe);
        cmd.arg(if multich { "-m" } else { "-2" })
            .arg("-s") // 输出 DSF
            .arg("-t")
            .arg(track_number.to_string())
            .arg("-i")
            .arg(&self.iso_path)
            .arg("-y")
            .arg(&work)
            // 隔离工作目录，避免读取/生成 sacd_extract.cfg 到用户目录
            .current_dir(&work)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        eprintln!(
            "SACD_EXTERNAL exe={:?} track={} multich={} iso={:?}",
            exe, track_number, multich, self.iso_path
        );
        let out = cmd.output()?;
        if !out.status.success() {
            let detail = String::from_utf8_lossy(&out.stderr);
            let last = detail.lines().last().unwrap_or("").trim();
            let _ = std::fs::remove_dir_all(&work);
            return Err(std::io::Error::other(format!(
                "sacd_extract 解码失败（退出码 {:?}）：{}",
                out.status.code(),
                last
            )));
        }

        let dsf = find_first_dsf(&work).ok_or_else(|| {
            let _ = std::fs::remove_dir_all(&work);
            std::io::Error::other("sacd_extract 未产出 DSF 文件")
        })?;
        let copied = std::fs::copy(&dsf, out_path);
        let _ = std::fs::remove_dir_all(&work);
        copied.map(|_| ())
    }

    /// 某轨导出为 DSF 后的预期文件总字节数（DSF 头 + DSD 载荷）。
    /// 播放链路据此校验磁盘缓存是否与当前导出一致，避免复用旧版/损坏的 DSF（噪声）。
    /// 仅适用于内置（未压缩 DSD）导出路径。
    pub fn exported_dsf_len(&self, track_index: usize) -> u64 {
        let t = &self.tracks[track_index];
        // v9：与 export_track_to_dsf 保持一致——数据起点 = 扇区边界 + 32 字节块头
        let start = t.start_lba * SACD_SECTOR_SIZE + SACD_SECTOR_HEADER;
        let end = if track_index + 1 < self.tracks.len() {
            self.tracks[track_index + 1].start_lba * SACD_SECTOR_SIZE
        } else {
            self.audio_end_byte.max(start + 1)
        };
        let payload = if self.block_headers {
            sacd_block_payload_len(start, end)
        } else {
            t.frames * DSD64_FRAME_BYTES
        };
        DSF_HEADER_BYTES + payload
    }

    /// 导出为 Track 模型（供曲库扫描直接 upsert）
    ///
    /// 两区曲目都会入库：立体声区每条一轨（`sacd_area = Stereo2ch`），多声道区
    /// （若有）同样每条一轨（`sacd_area = MultiCh`，标题带 `[5.1]` 后缀），
    /// 用户可在曲库中任选版本播放。`sacd_track_index` 为合并列表中的索引
    /// （前 n 条 = 立体声区，其后 = 多声道区）。
    /// 探测每轨真实时长（扫描期每碟调用一次）。
    ///
    /// DST 压缩区的存储数据量远小于未压缩 DSD，按字节推算的时长会被压缩率压低
    /// （Energy 实测约 1/2.4）。ISO 的 Area TOC 不含每轨帧数，只有 `-P` 输出的
    /// `Duration: MM:SS:FF` 是权威值（sacd_extract 自行解析区 TOC）。
    /// 工具缺失 / 解析失败 / 曲目数不匹配时保持 None，回退为数据量推算。
    pub fn probe_durations(&mut self) {
        let Some(d) = probe_durations_via_external(&self.iso_path) else {
            return;
        };
        if d.len() == self.tracks.len() {
            self.probed_durations = Some(d);
        }
    }

    /// 是否已拿到探测时长（调用方据此决定是否写回库，避免把推算值当权威值）
    pub fn has_probed_durations(&self) -> bool {
        self.probed_durations.is_some()
    }

    /// 该立体声区曲目的数据是否经过 DST 压缩。
    ///
    /// Area TOC 不携带「该区是否 DST」的可靠标志（Energy 2ch 区即被误判为未压缩），
    /// 但压缩与否可由**真实时长 vs 镜像数据量**的倍率判定：DST 无损压缩率约 2~2.5 倍，
    /// 按字节推算的帧数会显著小于真实帧数。真实时长来自 `sacd_extract -P`（探测后才有）。
    pub fn track_is_compressed(&self, index: usize) -> bool {
        let Some(real) = self
            .probed_durations
            .as_ref()
            .and_then(|v| v.get(index))
            .copied()
        else {
            return false;
        };
        let Some(t) = self.tracks.get(index) else {
            return false;
        };
        // 真实帧数明显大于按数据量推算的帧数 → 镜像里存的是压缩数据
        real * (DSD_FRAMES_PER_SEC as f64) > (t.frames as f64) * 1.2
    }

    /// 单轨时长：优先探测值，否则按数据量推算（对未压缩区准确，DST 区偏短）
    fn track_duration(&self, i: usize, t: &SacdTrackMeta) -> f64 {
        self.probed_durations
            .as_ref()
            .and_then(|v| v.get(i).copied())
            .unwrap_or_else(|| (t.frames as f64) / (DSD_FRAMES_PER_SEC as f64))
    }

    pub fn to_tracks(&self) -> Vec<Track> {
        let album = self.album.clone();
        let artist = self.artist.clone();
        let first_artist = artist.as_str();
        let album_id = Some(album_id_of(&album, first_artist));

        let mut tracks: Vec<Track> = Vec::with_capacity(self.tracks.len() * 2);
        for (i, t) in self.tracks.iter().enumerate() {
            let duration = self.track_duration(i, t);
            let artists = t
                .artist
                .as_ref()
                .map(|a| split_artists(a))
                .unwrap_or_else(|| split_artists(&artist));
            let base_title = if t.title.trim().is_empty() {
                format!("Track {:02}", t.track_number)
            } else {
                t.title.clone()
            };
            // DST / 非标准布局：由外部 sacd_extract 解码，格式标记为 iso-dst。
            // 压缩与否也可能是探测后才确定的（TOC 无可靠标志），见 track_is_compressed。
            let fmt = if t.is_dst || self.unsupported || self.track_is_compressed(i) {
                "iso-dst"
            } else {
                "iso-dsd"
            };
            tracks.push(Track {
                id: sacd_track_id(&self.iso_path, t.track_number, false),
                title: base_title.clone(),
                artists: artists.clone(),
                album: album.clone(),
                album_id: album_id.clone(),
                duration,
                track_number: if t.track_number == 0 {
                    None
                } else {
                    Some(t.track_number)
                },
                year: self.year,
                // 所有分轨共享 ISO 路径，与 CUE 分轨共享镜像路径同理
                file_path: self.iso_path.to_string_lossy().to_string(),
                file_size: self.file_size,
                format: fmt.to_string(),
                bitrate: None,
                sample_rate: Some(self.dsd_sample_rate),
                has_cover: false, // ISO 不内嵌图片，封面走同目录查找/在线匹配
                replaygain_db: None,
                replaygain_peak: None,
                added_at: chrono::Utc::now().timestamp(),
                modified_at: self.iso_mtime,
                is_cue: false,
                cue_offset: None,
                is_sacd: true,
                sacd_track_index: i as u32,
                sacd_area: Some(SacdArea::Stereo2ch),
                sacd_dsd_rate: Some(self.dsd_sample_rate.to_string()),
            });
        }

        // 多声道区：曲目数与立体声区相同（顺序对应），时长取立体声区对应轨
        // （两区时长基本一致，误差 <2%，仅影响进度条显示）。
        let n = self.tracks.len();
        for i in 0..self.multich_track_count.min(n) {
            let t = &self.tracks[i];
            let duration = self.track_duration(i, t);
            let artists = t
                .artist
                .as_ref()
                .map(|a| split_artists(a))
                .unwrap_or_else(|| split_artists(&artist));
            tracks.push(Track {
                id: sacd_track_id(&self.iso_path, t.track_number, true),
                title: format!("{} [5.1]", if t.title.trim().is_empty() {
                    format!("Track {:02}", t.track_number)
                } else {
                    t.title.clone()
                }),
                artists,
                album: album.clone(),
                album_id: album_id.clone(),
                duration,
                track_number: if t.track_number == 0 {
                    None
                } else {
                    Some(t.track_number)
                },
                year: self.year,
                file_path: self.iso_path.to_string_lossy().to_string(),
                file_size: self.file_size,
                // 多声道区多为 DST 压缩，统一按 iso-dst 标记（由外部工具解码）
                format: "iso-dst".to_string(),
                bitrate: None,
                sample_rate: Some(self.dsd_sample_rate),
                has_cover: false,
                replaygain_db: None,
                replaygain_peak: None,
                added_at: chrono::Utc::now().timestamp(),
                modified_at: self.iso_mtime,
                is_cue: false,
                cue_offset: None,
                is_sacd: true,
                sacd_track_index: (n + i) as u32,
                sacd_area: Some(SacdArea::MultiCh),
                sacd_dsd_rate: Some(self.dsd_sample_rate.to_string()),
            });
        }
        tracks
    }
}

/// 从 Master TOC 头部解析专辑与曲目信息。
///
/// 这个实现采用 Scarlet Book 规范的字段偏移（保守版），覆盖市面上常见抓轨。
/// 对于编码异常或私有扩展的镜像，失败时返回 None，不阻断整个扫描。
fn parse_toc(
    toc: &[u8],
    iso_path: &Path,
    file_size: u64,
    iso_mtime: i64,
) -> Option<SacdDisc> {
    if toc.len() < 64 {
        return None;
    }

    // Master TOC 头结构（简化）:
    // offset 0: 8字节 "SACDMTA\0"
    // offset 8: 1字节区域数, 1字节保留
    // offset 0x10: 区域TOC偏移表 (每个区域 8 字节)
    // 我们优先解析 2ch 区（区域 0），因为多声道区多为 DST 压缩
    // 区域 TOC:
    // offset 0x80 within area TOC: 曲目数 u8
    // 曲目表每条 16 字节: start_lba(u32) | frames(u32) | flags(u8) | reserved(7)
    // 文本区位于区域 TOC 末尾，以 0x20 字节为单位存储专辑名/艺人/曲目名

    // 尝试定位文本区（简化实现：从 TOC 0x100 起的偏移中按已知结构扫）
    let album = read_sacd_text(toc, 0x100).unwrap_or_else(|| {
        iso_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown SACD")
            .to_string()
    });
    let artist = read_sacd_text(toc, 0x200).unwrap_or_else(|| "Unknown Artist".to_string());
    let year = None; // SACD TOC 中 year 无标准字段，留给同目录/在线元数据补齐

    // 尝试解析 2ch 区曲目表
    // 起始 LBA / frames 在 TOC 中按大端 u32 排列；默认 DSD64
    let area_toc_base = 0x800usize; // 经验偏移（2ch 区 TOC 起点）
    let track_count = read_u8(toc, area_toc_base).unwrap_or(0) as usize;

    let mut tracks = Vec::new();
    if track_count > 0 && track_count <= 99 {
        // 曲目表通常紧跟区域头，每轨 16 字节
        let table_base = area_toc_base + 0x80;
        for i in 0..track_count {
            let off = table_base + i * 16;
            if off + 16 > toc.len() {
                break;
            }
            let start_lba = read_u32_be(toc, off).unwrap_or(0) as u64;
            let frames = read_u32_be(toc, off + 4).unwrap_or(0) as u64;
            let flags = toc.get(off + 8).copied().unwrap_or(0);
            let is_dst = (flags & 0x02) != 0; // DST 标志位

            // 粗略校验：LBA 必须落在镜像范围内，帧数 > 0
            if start_lba == 0 || frames == 0 || start_lba * SACD_SECTOR_SIZE >= file_size {
                continue;
            }

            // 曲目名从 TOC 文本区查找（偏移 0x1000 起，每 0x60 字节一轨）
            let title_off = 0x1000 + i * 0x60;
            let title = read_sacd_text(toc, title_off)
                .unwrap_or_else(|| format!("Track {:02}", i + 1));

            tracks.push(SacdTrackMeta {
                track_number: (i + 1) as u32,
                title,
                artist: None, // 曲目级艺术家与专辑相同；如需 per-track 可扩展
                start_lba,
                frames,
                is_dst,
                channels: 2, // 旧「SACDMTA」经验布局仅覆盖立体声区
            });
        }
    }

    // 如果按经验偏移没读到曲目，使用启发式：在 TOC 中扫描连续的（LBA, frames）对
    if tracks.is_empty() {
        tracks = heuristic_track_table(toc, file_size);
    }

    if tracks.is_empty() {
        return None;
    }

    Some(SacdDisc {
        album,
        artist,
        year,
        area: SacdArea::Stereo2ch,
        dsd_sample_rate: 2_822_400, // DSD64 默认
        tracks,
        iso_path: iso_path.to_path_buf(),
        file_size,
        iso_mtime,
        block_headers: false,
        audio_end_byte: file_size,
        unsupported: false,
        multich_track_count: 0,
        probed_durations: None,
    })
}

/// 启发式：在 TOC 数据中扫描符合"start_lba 递增、frames 合理"的连续条目
fn heuristic_track_table(toc: &[u8], file_size: u64) -> Vec<SacdTrackMeta> {
    let mut tracks: Vec<SacdTrackMeta> = Vec::new();
    let mut last_lba: u64 = 0;
    // 扫描窗口：每 16 字节为一个候选槽位
    for base in (0x800..toc.len().saturating_sub(16)).step_by(16) {
        let start_lba = match read_u32_be(toc, base) {
            Some(v) if v > 0 => v as u64,
            _ => continue,
        };
        let frames = match read_u32_be(toc, base + 4) {
            Some(v) if v > 0 => v as u64,
            _ => continue,
        };
        // 合理性校验：
        // - LBA 在镜像内
        // - 帧数对应合理时长（3秒 ~ 60分钟）
        // - LBA 单调递增
        let min_frames = 3 * DSD_FRAMES_PER_SEC;
        let max_frames = 60 * 60 * DSD_FRAMES_PER_SEC;
        if start_lba * SACD_SECTOR_SIZE >= file_size
            || frames < min_frames
            || frames > max_frames
            || start_lba <= last_lba
        {
            continue;
        }
        // 相邻条目的间隔与帧数对应（粗略验证）
        if last_lba > 0 {
            let expected_bytes = tracks.last().unwrap().frames * DSD64_FRAME_BYTES;
            let actual_bytes = (start_lba - last_lba) * SACD_SECTOR_SIZE;
            // 允许 2 个扇区容差（TOC 与音频数据对齐）
            if actual_bytes.saturating_sub(expected_bytes) > 2 * SACD_SECTOR_SIZE
                && expected_bytes.saturating_sub(actual_bytes) > 2 * SACD_SECTOR_SIZE
            {
                // 不满足连续性则认为上一组已结束；若当前集合 < 2 条则清空重来
                if tracks.len() < 2 {
                    tracks.clear();
                } else {
                    break;
                }
            }
        }
        tracks.push(SacdTrackMeta {
            track_number: (tracks.len() + 1) as u32,
            title: format!("Track {:02}", tracks.len() + 1),
            artist: None,
            start_lba,
            frames,
            is_dst: false, // 启发式无法判定 DST，按 DSD 处理；后续失败再回退
            channels: 2,
        });
        last_lba = start_lba;
    }
    tracks
}

/// 写 DSF 文件头（DSD64 双声道）。
///
/// DSF 规范参考：DSF File Format Specification v1.01（Sony）
/// 布局：
///   [0..27]     "DSD " chunk: id=DSD , size=28, file_size(头+data), offset_to_data=80
///   [28..79]    fmt chunk: 52 字节格式块（版本=1, format_id=0(DSD),
///               channel_type=2(双声道), channel_num=2, sample_rate=2822400,
///               bits_per_sample=1, sample_count=每声道样本数, block_size=4096, reserved）
///   [80..]      data chunk: "data" + data_size + DSD 数据
fn write_dsf_header<W: std::io::Write>(
    w: &mut W,
    _frames: u64,
    channels: u32,
    sample_rate: u32, // DSD 采样率（DSD64=2822400 / DSD128=5644800 / DSD256=11289600，动态传入）
    data_bytes: u64,
) -> std::io::Result<()> {
    let data_offset: u64 = 80;
    let file_size = data_offset + 12 + data_bytes; // 92 + "data"+size + payload

    // DSD chunk (28 bytes)
    w.write_all(b"DSD ")?;
    w.write_all(&28u64.to_le_bytes())?;
    w.write_all(&file_size.to_le_bytes())?;
    w.write_all(&data_offset.to_le_bytes())?;

    // fmt chunk (52 bytes, starts at offset 28)
    w.write_all(b"fmt ")?;
    w.write_all(&52u64.to_le_bytes())?; // fmt chunk size = 52 (version..reserved)
    w.write_all(&1u32.to_le_bytes())?; // format version = 1
    w.write_all(&0u32.to_le_bytes())?; // format id = 0 (DSD raw)
    w.write_all(&2u32.to_le_bytes())?; // channel type = 2 (stereo)
    w.write_all(&channels.to_le_bytes())?; // channel num = 2
    w.write_all(&sample_rate.to_le_bytes())?; // sampling rate = 传入的 DSD 速率（不再写死 2822400）
    w.write_all(&1u32.to_le_bytes())?; // bits per sample = 1
    // 每声道样本数 = 总 bit 数 / 声道数（1 bit per sample）；DSF fmt 的 SampleCount 为每声道值
    let sample_count = data_bytes * 8 / channels as u64;
    w.write_all(&sample_count.to_le_bytes())?;
    w.write_all(&4096u32.to_le_bytes())?; // block size per channel = 4096 bytes
    w.write_all(&0u32.to_le_bytes())?; // reserved

    // data chunk header (12 bytes)
    w.write_all(b"data")?;
    w.write_all(&data_bytes.to_le_bytes())?;
    Ok(())
}

// ====== 工具函数 ======

fn read_u8(buf: &[u8], off: usize) -> Option<u8> {
    buf.get(off).copied()
}
fn read_u32_be(buf: &[u8], off: usize) -> Option<u32> {
    if off + 4 > buf.len() {
        return None;
    }
    Some(u32::from_be_bytes([
        buf[off],
        buf[off + 1],
        buf[off + 2],
        buf[off + 3],
    ]))
}

/// 从 buf 的指定偏移读取 SACD 文本（UTF-8/ASCII，以 \0 或全 0x00 结尾）。
/// SACD TOC 文本常见是 Latin-1/Shift-JIS，这里做 lossy 处理，非 ASCII 字符按 UTF-8 尝试。
fn read_sacd_text(buf: &[u8], offset: usize) -> Option<String> {
    if offset >= buf.len() {
        return None;
    }
    let slice = &buf[offset..];
    // 找结束位置：\0 或连续 4 个 0x00 或 0x20 填充
    let mut end = 0usize;
    let mut zero_run = 0usize;
    for (i, &b) in slice.iter().enumerate().take(256.min(slice.len())) {
        if b == 0 {
            zero_run += 1;
            if zero_run >= 2 {
                end = i - zero_run + 1;
                break;
            }
        } else {
            zero_run = 0;
        }
        // 遇到非打印字符（控制字符，除空格/制表）则截断
        if b < 0x20 && b != b' ' && b != b'\t' {
            end = i;
            break;
        }
        end = i + 1;
    }
    if end == 0 {
        return None;
    }
    let raw = &slice[..end];
    // 去除尾部空格
    let trimmed = raw
        .split(|&b| b == 0)
        .next()
        .unwrap_or(raw)
        .trim_ascii_end();
    if trimmed.is_empty() {
        return None;
    }
    // 先按 UTF-8 解，失败则按 Latin-1 lossy
    match std::str::from_utf8(trimmed) {
        Ok(s) => {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        }
        Err(_) => Some(trimmed.iter().map(|&b| b as char).collect::<String>().trim().to_string()),
    }
}

fn split_artists(raw: &str) -> Vec<String> {
    raw.split(['/', ';', ','])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// 在 buf 中查找 8 字节 ASCII 标记，返回其起始下标。
fn find_marker(buf: &[u8], marker: &[u8]) -> Option<usize> {
    if marker.is_empty() || buf.len() < marker.len() {
        return None;
    }
    buf.windows(marker.len()).position(|w| w == marker)
}

/// 读取 2 字节大端整数；越界返回 0。
fn read_u16_be(buf: &[u8], off: usize) -> u16 {
    if off + 2 > buf.len() {
        return 0;
    }
    u16::from_be_bytes([buf[off], buf[off + 1]])
}

/// 逐字节位反转（MSB-first ↔ LSB-first）。
///
/// SACD 立体声区 DSD 位流以 MSB-first 存储，而 DSF（ffmpeg 的 `dsd_lsbf` 解码器）
/// 要求 LSB-first。导出 DSF 时必须对每字节做位反转，否则 ffmpeg 把样本位序全反，
/// 解码出的 PCM 表现为噪声（与 sacd-ripper 的 dsf.c `dsf_write_frame` 位反转一致）。
#[inline]
fn reverse_bits(mut b: u8) -> u8 {
    b = ((b & 0xF0) >> 4) | ((b & 0x0F) << 4);
    b = ((b & 0xCC) >> 2) | ((b & 0x33) << 2);
    b = ((b & 0xAA) >> 1) | ((b & 0x55) << 1);
    b
}

/// 读取 SACD 文本字符串：跳过前导控制/零字节（字符集/语言头），读取到 \0 或控制字符为止。
fn read_sb_string(buf: &[u8], start: usize) -> String {
    if start >= buf.len() {
        return String::new();
    }
    let mut i = start;
    while i < buf.len() && (buf[i] == 0 || buf[i] < 0x20) {
        i += 1;
    }
    let mut end = i;
    while end < buf.len() && buf[end] != 0 && buf[end] >= 0x20 {
        end += 1;
    }
    let raw = &buf[i..end];
    if raw.is_empty() {
        return String::new();
    }
    match std::str::from_utf8(raw) {
        Ok(s) => s.trim().to_string(),
        Err(_) => {
            // 非 UTF-8：先尝试 UTF-16BE（CJK 标题常见；ASCII 的高位字节为 0x00，故偶数字节大量为 0）
            let zero_even = raw.iter().step_by(2).filter(|&&b| b == 0).count();
            let half = (raw.len() + 1) / 2;
            if raw.len() >= 2 && half > 0 && zero_even * 2 >= half {
                let units: Vec<u16> = raw
                    .chunks(2)
                    .map(|c| {
                        let hi = c[0] as u16;
                        let lo = if c.len() > 1 { c[1] as u16 } else { 0 };
                        (hi << 8) | lo
                    })
                    .collect();
                if let Ok(s) = String::from_utf16(&units) {
                    let s = s.trim().to_string();
                    if !s.is_empty() {
                        return s;
                    }
                }
            }
            // 最后回退 Latin-1（逐字节转字符）
            raw.iter()
                .map(|&b| b as char)
                .collect::<String>()
                .trim()
                .to_string()
        }
    }
}

/// 解析 SACD 文本块（SACDTTxt / SACDText）：8 字节魔数 +（可选 8 字节保留）+ 2 字节大端偏移表 +
/// 各偏移处以 \0 结尾的字符串。不同块头长度不同：`SACDTTxt` 偏移表紧接魔数（base+8）；
/// `SACDText` 魔数后有 8 字节保留（全 0），偏移表在 base+16。这里两种都尝试，返回非空者。
fn parse_sb_text_table(buf: &[u8], marker: &[u8]) -> Vec<String> {
    let Some(base) = find_marker(buf, marker) else {
        return Vec::new();
    };
    let parse_at = |hdr: usize| -> Vec<String> {
        let mut out = Vec::new();
        for i in 0..99 {
            let off = read_u16_be(buf, base + hdr + i * 2) as usize;
            if off == 0 {
                break;
            }
            let s = read_sb_string(buf, base + off);
            if !s.is_empty() {
                out.push(s);
            }
        }
        out
    };
    // 先按紧接魔数（SACDTTxt）解析；若为空再按魔数后 8 字节保留（SACDText）解析
    let v = parse_at(8);
    if v.is_empty() {
        parse_at(16)
    } else {
        v
    }
}

/// 计算字节范围 [start, end) 内的 DSD 载荷长度（v9：Sony ScarletBooks ISO 实测结构）。
///
/// 实测确认（与同碟参考立体声音乐逐字节比对，0 误差）：立体声区音频按 **2048 字节扇区**组织，
/// 每扇区 = **32 字节块头 + 2016 字节 DSD 数据**。故载荷长度 = 跨度 − 跨度内需跳过的 32 字节块头数。
/// `start` 应为扇区数据起点（扇区边界 + 32），`end` 为下一轨扇区边界。
fn sacd_block_payload_len(start: u64, end: u64) -> u64 {
    let span = end.saturating_sub(start);
    // 严格位于 (start, end) 内的扇区边界（≡0 mod 2048）个数 = 需跳过的块头数
    let first_boundary = (start / SACD_SECTOR_SIZE + 1) * SACD_SECTOR_SIZE;
    let headers = if first_boundary < end {
        (end - first_boundary).div_ceil(SACD_SECTOR_SIZE)
    } else {
        0
    };
    span.saturating_sub(headers * SACD_SECTOR_HEADER)
}

/// SACD 2ch 立体声 DSD 抽取：把位流重排为 DSF 平面布局后写入 `w`。
///
/// 关键（v9，用真实 ISO + 同碟参考立体声音乐**逐字节比对**确认，0 误差）：
/// 1. **扇区块头**：Sony ScarletBooks ISO 立体声区音频按 **2048 字节扇区**组织，每扇区 =
///    **32 字节块头 + 2016 字节 DSD 数据**。不剥离块头会把 32 字节垃圾混进 DSD 位流 → 白噪。
/// 2. **去交错粒度**：DSD 数据为**逐字节 L,R 交错**（1 字节 L + 1 字节 R，连续跨扇区），
///    而非「每帧 588L+588R 帧平面」也非「12L+12R」。判据：以 12L+12R 导出后与参考音乐
///    逐字节比对有 97% 不一致；改为逐字节交错后 0 误差。
/// 3. **位序**：ISO 侧 DSD 为 MSB-first，DSF（`dsd_lsbf`）需 LSB-first → 逐字节位反转。
///
/// `start` 必须是该轨**真实的 DSD 数据起点**（扇区边界 + 32，或轨内偏移），因为扇区网格的相位
/// 以数据起点为锚；`end` 为该轨数据结束的扇区边界。输出按 DSF 平面布局（4096 字节/声道/块）。
fn export_dsd_blocks<W: std::io::Write>(
    iso: &Path,
    w: &mut W,
    start: u64,
    end: u64,
) -> std::io::Result<()> {
    const PLANAR_BLK: usize = 4096; // 与 write_dsf_header 的 block_size_per_channel 一致
    let total = end.saturating_sub(start);
    let mut file = File::open(iso)?;
    file.seek(SeekFrom::Start(start))?;
    let mut l_buf: Vec<u8> = Vec::with_capacity(PLANAR_BLK);
    let mut r_buf: Vec<u8> = Vec::with_capacity(PLANAR_BLK);
    let mut read_buf = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    while done < total {
        let pos = start + done;
        // 下一个扇区边界（绝对位置）：到边界前的数据连续，边界处跳过 32 字节块头
        let next_boundary = (pos / SACD_SECTOR_SIZE + 1) * SACD_SECTOR_SIZE;
        let want = read_buf
            .len()
            .min(next_boundary.saturating_sub(pos).min(total - done) as usize);
        let n = file.read(&mut read_buf[..want])?;
        if n == 0 {
            break;
        }
        // 逐字节 L,R 交错去交错 + 位反转（MSB-first → LSB-first）
        let mut i = 0;
        while i + 1 < n {
            l_buf.push(reverse_bits(read_buf[i]));
            r_buf.push(reverse_bits(read_buf[i + 1]));
            i += 2;
        }
        done += n as u64;
        // 到达扇区边界且还有数据要读：跳过 32 字节块头（网格以数据起点为锚）
        if start + done == next_boundary && done < total {
            file.seek(SeekFrom::Start(next_boundary + SACD_SECTOR_HEADER))?;
            done += SACD_SECTOR_HEADER;
        }
        // 凑满一整块（4096 L + 4096 R）即按平面布局写出
        while l_buf.len() >= PLANAR_BLK && r_buf.len() >= PLANAR_BLK {
            w.write_all(&l_buf[..PLANAR_BLK])?;
            w.write_all(&r_buf[..PLANAR_BLK])?;
            l_buf.drain(..PLANAR_BLK);
            r_buf.drain(..PLANAR_BLK);
        }
    }
    // 收尾：写出残余（L/R 长度相等：每对交错字节各贡献 1 字节）
    w.write_all(&l_buf)?;
    w.write_all(&r_buf)?;
    Ok(())
}

/// Sony ScarletBooks 变体解析（`SACDMTOC` / `SACDText` / `SACDTRL1` / `SACDTTxt`）。
///
/// 真实抓轨 SACD ISO 使用该结构：立体声区音轨起始扇区表 `SACDTRL1`（4 字节大端、
/// 单调递增、0 结束）、曲目标题 `SACDTTxt`、碟级文本 `SACDText`（[0]=专辑/[1]=艺术家）、
/// 主 TOC `SACDMTOC`（+0x40=立体声区起始扇区，+0x44=结束扇区）。2ch 立体声 DSD 为
/// 连续位流、每帧 1176 字节、帧内「前 588 字节=L、后 588 字节=R」的**帧平面**布局
/// （无每 64KB 块头），由 `export_dsd_blocks` 按帧平面去交错为 DSF 平面布局。
///
/// `buf` 为 ISO 头部区域（含全部 TOC 与文本）。无法解析返回 None。
fn parse_scarletbooks(
    buf: &[u8],
    iso_path: &Path,
    file_size: u64,
    iso_mtime: i64,
) -> Option<SacdDisc> {
    // 立体声区音轨位置表 SACDTRL1
    let trl1 = find_marker(buf, b"SACDTRL1")?;
    let mut sectors: Vec<u64> = Vec::new();
    let mut p = trl1 + 8;
    while sectors.len() < 99 {
        match read_u32_be(buf, p) {
            Some(v) if v > 0 => {
                let v = v as u64;
                if sectors.last().is_some_and(|&last| v <= last) {
                    break; // 非递增即表结束
                }
                sectors.push(v);
                p += 4;
            }
            _ => break,
        }
    }
    if sectors.len() < 2 {
        return None;
    }

    // 多声道（5.1）区曲目数：SACDTRL2 存在即含多声道区，项数 = 曲目数
    // （该表值为时间码而非扇区号，故仅取数量；曲目顺序与立体声区一一对应）。
    let multich_track_count = find_marker(buf, b"SACDTRL2")
        .map(|trl2| {
            let mut count = 0usize;
            let mut q = trl2 + 8;
            while count < 99 {
                match read_u32_be(buf, q) {
                    Some(v) if v > 0 => {
                        count += 1;
                        q += 4;
                    }
                    _ => break,
                }
            }
            count
        })
        .unwrap_or(0);

    // 曲目标题与碟级文本
    let titles = parse_sb_text_table(buf, b"SACDTTxt");
    let disc = parse_sb_text_table(buf, b"SACDText");
    let album = disc
        .first()
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| {
            iso_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown SACD")
                .to_string()
        });
    let artist = disc
        .get(1)
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| "Unknown Artist".to_string());

    // 立体声区结束扇区（主 TOC +0x44），界定末轨；无效则回退文件尾
    let last_start = *sectors.last().unwrap();
    let area_end_byte = find_marker(buf, b"SACDMTOC")
        .and_then(|m| read_u32_be(buf, m + 0x44))
        .map(|s| s as u64 * SACD_SECTOR_SIZE)
        .filter(|&b| b > last_start * SACD_SECTOR_SIZE && b <= file_size)
        .unwrap_or(file_size);

    // 逐轨计算 payload 帧数（剥离块头），供时长显示与导出一致使用
    let mut tracks = Vec::with_capacity(sectors.len());
    for (i, &start_sector) in sectors.iter().enumerate() {
        let start = start_sector * SACD_SECTOR_SIZE;
        let end = if i + 1 < sectors.len() {
            sectors[i + 1] * SACD_SECTOR_SIZE
        } else {
            area_end_byte
        };
        if end <= start || start >= file_size {
            continue;
        }
        let payload = sacd_block_payload_len(start, end);
        let frames = payload / DSD64_FRAME_BYTES;
        if frames == 0 {
            continue;
        }
        let title = titles
            .get(i)
            .filter(|s| !s.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| format!("Track {:02}", i + 1));
        tracks.push(SacdTrackMeta {
            track_number: (i + 1) as u32,
            title,
            artist: Some(artist.clone()),
            start_lba: start_sector,
            frames,
            is_dst: false,
            channels: 2, // SACDTRL1 为立体声区轨表
        });
    }
    if tracks.is_empty() {
        return None;
    }

    // DST / 非标准布局检测：Sony ScarletBooks 未压缩音频扇区以固定签名开头
    // （第二字节恒为 0x18，如 0x4418_1897 / 0x4018_1b17）。DST 压缩区（多声道区常见，
    // 部分立体声区也是）与其它非标准抓轨的扇区没有该签名——把这类数据按裸 DSD 去交错
    // 只会得到白噪。此处标记为 unsupported，播放链路据此给出明确错误而非输出噪声。
    let unsupported = !sacd_audio_has_sector_signature(iso_path, tracks[0].start_lba);

    Some(SacdDisc {
        album,
        artist,
        year: None,
        area: SacdArea::Stereo2ch,
        dsd_sample_rate: 2_822_400, // DSD64（立体声区常规）
        tracks,
        iso_path: iso_path.to_path_buf(),
        file_size,
        iso_mtime,
        block_headers: true,
        audio_end_byte: area_end_byte,
        unsupported,
        multich_track_count,
        probed_durations: None,
    })
}

/// 检测音频区是否为标准 Sony ScarletBooks 布局（未压缩裸 DSD）。
///
/// 采样该轨起始扇区及其后若干扇区，任一扇区命中签名（`buf[1] == 0x18` 且
/// `buf[0]` 为 0x40/0x44 类帧头）即视为标准布局；全部不命中则判定为 DST 压缩
/// 或非标准抓轨（无法按裸 DSD 导出）。
///
/// 实测判据（真实 ISO）：
///  - 未压缩 DSD（Sandy Lam SACD）：扇区头 `44 18 18 97 e0 00 02 …` / `40 18 1b 17 …`，命中；
///  - DST 压缩（Fourplay Energy 2ch/5.1 区）：`fd 90 0b 90 …` / `21 17 fd 22 …`，不命中。
fn sacd_audio_has_sector_signature(iso_path: &Path, first_lba: u64) -> bool {
    let Ok(mut file) = File::open(iso_path) else {
        return true; // 读不到时不阻断，沿用旧行为
    };
    // 检查起始扇区 + 其后 3 个轨内扇区（曲目间隙/边界处可能是数据而非头）
    for step in 0..4u64 {
        let sector = first_lba + step * 64;
        if sector * SACD_SECTOR_SIZE >= file.metadata().map(|m| m.len()).unwrap_or(0) {
            break;
        }
        if file.seek(SeekFrom::Start(sector * SACD_SECTOR_SIZE)).is_err() {
            continue;
        }
        let mut head = [0u8; 8];
        if file.read_exact(&mut head).is_err() {
            continue;
        }
        if head[1] == 0x18 && (0x40..=0x7f).contains(&head[0]) {
            return true;
        }
    }
    false
}

/// 生成 SACD 轨 ID。`multich` 区分立体声区与多声道区（同曲目号在两区各有一轨）。
fn sacd_track_id(iso: &Path, num: u32, multich: bool) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"sacd|");
    if multich {
        hasher.update(b"mch|");
    }
    hasher.update(iso.to_string_lossy().as_bytes());
    hasher.update(b"|");
    hasher.update(num.to_string().as_bytes());
    let r = hasher.finalize();
    r.iter().take(8).map(|b| format!("{:02x}", b)).collect()
}

/// 定位随包分发的 sacd_extract（DST/多声道区外部解码器）。
///
/// 查找顺序与 ffmpeg 一致（开发期源码树 `bin/`、安装包资源 `bin/`、系统 PATH）：
/// 1. 沿可执行文件祖先目录的 `bin/`（安装版：安装目录/bin/sacd_extract.exe）；
/// 2. 系统 PATH。
fn resolve_sacd_extract() -> Option<PathBuf> {
    crate::player::engine::resolve_bin_tool("sacd_extract")
}

/// 调 `sacd_extract -P` 取每轨真实时长。
///
/// `-P` 只解析 ISO 的区 TOC（不读音频数据），单碟耗时约几十毫秒，仅扫描期调用。
fn probe_durations_via_external(iso: &Path) -> Option<Vec<f64>> {
    let exe = resolve_sacd_extract()?;
    let mut cmd = crate::util::hidden_command(&exe);
    cmd.arg("-P")
        .arg("-i")
        .arg(iso)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_probe_output(&String::from_utf8_lossy(&out.stdout))
}

/// 解析 `-P` 输出：按 `Area Information [N]` 分段收集 `Duration: MM:SS:FF`。
///
/// 优先取 `Speaker config: 2 Channel` 的区（曲目顺序与 SACDTRL1 一致，
/// 多声道区与之逐轨对应）；无 2ch 区时取第一个非空区。
fn parse_probe_output(text: &str) -> Option<Vec<f64>> {
    let mut areas: Vec<(bool, Vec<f64>)> = Vec::new();
    let mut cur: Option<(bool, Vec<f64>)> = None;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with("Area Information [") {
            if let Some(c) = cur.take() {
                areas.push(c);
            }
            cur = Some((false, Vec::new()));
        } else if l.starts_with("Speaker config:") {
            if let Some(c) = cur.as_mut() {
                c.0 = l.contains("2 Channel");
            }
        } else if let Some(rest) = l.strip_prefix("Duration:") {
            if let Some(c) = cur.as_mut() {
                if let Some(secs) = parse_msf(rest.trim()) {
                    c.1.push(secs);
                }
            }
        }
    }
    if let Some(c) = cur.take() {
        areas.push(c);
    }
    areas
        .iter()
        .find(|(is_2ch, v)| *is_2ch && !v.is_empty())
        .or_else(|| areas.iter().find(|(_, v)| !v.is_empty()))
        .map(|(_, v)| v.clone())
}

/// `05:52:06 [mins:secs:frames]` → 秒（SACD 帧率 75 fps）
fn parse_msf(s: &str) -> Option<f64> {
    let token = s.split_whitespace().next()?;
    let mut it = token.split(':');
    let mins: f64 = it.next()?.parse().ok()?;
    let secs: f64 = it.next()?.parse().ok()?;
    let frames: f64 = it.next().unwrap_or("0").parse().unwrap_or(0.0);
    Some(mins * 60.0 + secs + frames / 75.0)
}

/// 在目录树中查找第一个 .dsf 文件（sacd_extract 输出 <专辑>/<Stereo|6ch>/NN - Title.dsf）。
fn find_first_dsf(dir: &Path) -> Option<PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("dsf"))
            {
                return Some(path);
            }
        }
    }
    None
}

/// 获取 SACD ISO 临时 DSF 缓存目录（每张 ISO 一个目录，按 mtime 校验缓存有效性）。
/// 缓存位于系统临时目录下 tplayer-sacd-<版本>/ 子目录（版本号见 SACD_CACHE_VERSION）。
pub fn sacd_cache_dir(iso_path: &Path) -> Option<PathBuf> {
    let mut h = Sha256::new();
    h.update(iso_path.to_string_lossy().as_bytes());
    let digest: String = h
        .finalize()
        .iter()
        .take(8)
        .map(|b| format!("{:02x}", b))
        .collect();
    let tmp = std::env::temp_dir()
        .join(format!("tplayer-sacd-{SACD_CACHE_VERSION}"))
        .join(digest);
    std::fs::create_dir_all(&tmp).ok()?;
    Some(tmp)
}

/// 获取某一轨的 DSF 缓存路径；不存在时返回 None，调用方应调用 export 生成。
pub fn cached_dsf_path(iso_path: &Path, track_index: usize) -> Option<PathBuf> {
    let dir = sacd_cache_dir(iso_path)?;
    Some(dir.join(format!("track_{:03}.dsf", track_index)))
}

/// SACD 缓存版本戳：导出逻辑版本变化时整体清空缓存目录（丢弃可能含早期错误解包
/// 数据的历史文件），保证后续解包仅产生/读取当前正确参数导出的 DSF。
const SACD_CACHE_STAMP: &str = ".export-stamp";

/// 确保 SACD 缓存目录为「当前导出逻辑版本」：若版本戳不符（首次或逻辑变更），
/// 整体删除旧目录并重建，从而清除历史解包文件，避免读取到旧的错误 DSD 数据。
/// 应在每次解包（导出某轨）前调用。
pub fn ensure_sacd_cache_fresh(dir: &Path) {
    let stamp = dir.join(SACD_CACHE_STAMP);
    let fresh = std::fs::read_to_string(&stamp)
        .map(|s| s.trim() == SACD_CACHE_VERSION)
        .unwrap_or(false);
    if !fresh {
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(&stamp, SACD_CACHE_VERSION);
    }
}

#[cfg(test)]
mod sacd_export_tests {
    use super::*;
    use std::io::Write;
    use std::process::Command;

    /// `-P` 输出解析：优先 2 Channel 区，MSF 帧按 75fps 换算
    #[test]
    fn probe_output_prefers_stereo_area_and_parses_msf() {
        let text = "\r\nAlbum Information:\r\n\tTitle: Energy\r\n\r\nArea count: 2\r\n\
            \tArea Information [0]:\r\n\r\n\tTrack Count: 2\r\n\
            \tTotal play time: 09:59:81 [mins:secs:frames]\r\n\
            \tSpeaker config: 2 Channel\r\n\tTrack list [0]:\r\n\
            \t\tTitle[0]: Fortune Teller\r\n\t\tDuration: 05:52:06 [mins:secs:frames]\r\n\r\n\
            \t\tTitle[1]: Argentina\r\n\t\tDuration: 04:07:75 [mins:secs:frames]\r\n\r\n\
            \tArea Information [1]:\r\n\r\n\tTrack Count: 2\r\n\
            \tSpeaker config: 6 Channel\r\n\tTrack list [1]:\r\n\
            \t\tTitle[0]: Fortune Teller\r\n\t\tDuration: 05:52:00 [mins:secs:frames]\r\n";
        let d = parse_probe_output(text).expect("应解析出 2ch 区时长");
        assert_eq!(d.len(), 2);
        assert!((d[0] - (5.0 * 60.0 + 52.0 + 6.0 / 75.0)).abs() < 1e-6);
        // 75 帧 = 1 秒 → 4:08
        assert!((d[1] - (4.0 * 60.0 + 8.0)).abs() < 1e-6);
    }

    /// 真实 ISO 端到端：DST 压缩区时长必须来自 -P（而非数据量推算）。
    /// 设置 `SACD_TEST_ISO` 环境变量后运行；工具缺失时跳过。
    #[test]
    fn probe_real_iso_durations_when_available() {
        let Ok(iso) = std::env::var("SACD_TEST_ISO") else {
            return;
        };
        if resolve_sacd_extract().is_none() {
            return;
        }
        let Some(mut disc) = SacdDisc::open(Path::new(&iso)) else {
            return;
        };
        disc.probe_durations();
        let tracks = disc.to_tracks();
        let durations: Vec<f64> = tracks.iter().map(|t| t.duration).collect();
        eprintln!("tracks={} durations={:?}", tracks.len(), durations);
        assert!(
            tracks.iter().all(|t| t.duration > 60.0),
            "每轨时长应大于 1 分钟（DST 区若按数据量推算会缩到约 40%）：{durations:?}"
        );
        // 总时长应与 -P 的 "Total play time" 量级一致（Energy ≈ 50 分钟）
        let total: f64 = durations.iter().take(disc.tracks.len()).sum();
        assert!(total > 1800.0, "立体声区总时长应在 30 分钟以上：{total}");

        // DST 识别：Energy 的 2ch 区 TOC 无压缩标志，靠「探测时长 vs 数据量」倍率判定，
        // 否则播放会走内置裸 DSD 导出 → 白噪（只能靠 guard 失败后兜底重解）。
        let derived = disc.tracks[0].frames as f64 / (DSD_FRAMES_PER_SEC as f64);
        eprintln!(
            "track0: derived={derived:.2}s probed={:.2}s format={} compressed={}",
            tracks[0].duration,
            tracks[0].format,
            disc.track_is_compressed(0)
        );
        assert!(
            tracks[0].duration > derived * 1.2,
            "该碟 2ch 区为 DST 压缩，探测时长应显著大于按数据量推算的值"
        );
        assert_eq!(tracks[0].format, "iso-dst", "DST 区应标记为 iso-dst");
    }

    /// 只有 6 Channel 区的碟：回退为第一个非空区
    #[test]
    fn probe_output_falls_back_to_first_area() {
        let text = "Area Information [0]:\n\tSpeaker config: 6 Channel\n\
            \t\tDuration: 03:00:00 [mins:secs:frames]\n";
        let d = parse_probe_output(text).expect("应解析出多声道区时长");
        assert_eq!(d, vec![180.0]);
    }

    /// ffmpeg 是否可用（缺少则回归测试跳过解码校验，仅做结构校验）。
    fn ffmpeg_ok() -> bool {
        Command::new("ffmpeg")
            .args(["-hide_banner", "-version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// 用 ffmpeg 把 DSF 解码为指定频段的 f32le 原始 PCM（取前 5 秒）。
    fn decode_band(dsf: &Path, af: &str) -> Option<Vec<f32>> {
        let raw = std::env::temp_dir().join("tplayer_sacd_band.raw");
        let status = Command::new("ffmpeg")
            .args(["-y", "-hide_banner", "-loglevel", "error", "-i"])
            .arg(dsf)
            .args(["-t", "5", "-af", &format!("aresample=44100,{af}"), "-f", "f32le"])
            .arg(&raw)
            .status()
            .ok()?;
        if !status.success() {
            return None;
        }
        let bytes = std::fs::read(&raw).ok()?;
        let n = bytes.len() / 4;
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            v.push(f32::from_le_bytes([
                bytes[4 * i],
                bytes[4 * i + 1],
                bytes[4 * i + 2],
                bytes[4 * i + 3],
            ]));
        }
        Some(v)
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt()
    }

    /// 针对真实 ScarletBooks 样本的抽取回归（需设置环境变量 SACD_TEST_ISO 指向样本 .iso）。
    /// 端到端校验：导出第 1 轨为 DSF 后，用 ffmpeg 解码并比较低/高频能量比——
    /// 正确立体声（音乐）低/高频比 > 0.4；噪声（位序错 / 声道错乱 / 块头未正确剥离）比 ≈ 0.1。
    /// ISO 或 ffmpeg 缺失时优雅跳过（不报错），便于 CI 与离线环境。
    #[test]
    fn scarletbooks_export_regression() {
        let iso = match std::env::var("SACD_TEST_ISO") {
            Ok(p) if Path::new(&p).exists() => p,
            _ => {
                eprintln!("SACD_TEST_ISO 未设置或文件不存在 -> 跳过");
                return;
            }
        };
        if !ffmpeg_ok() {
            eprintln!("ffmpeg 不可用 -> 跳过解码校验");
            return;
        }

        let disc = match SacdDisc::open(Path::new(&iso)) {
            Some(d) => d,
            None => {
                eprintln!("SACD ISO 解析失败 -> 跳过");
                return;
            }
        };
        assert!(disc.block_headers, "应识别为 ScarletBooks 立体声区（需去交错）");
        assert_eq!(disc.tracks.len(), 16, "应解析出 16 条音轨");

        let out = std::env::temp_dir().join("tplayer_sacd_regress.dsf");
        disc.export_track_to_dsf(0, &out).expect("导出第 1 轨");
        let len = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
        assert_eq!(len, disc.exported_dsf_len(0), "导出文件大小与预期不一致");

        let low = decode_band(&out, "lowpass=f=10000").expect("解码低频段");
        let high = decode_band(&out, "highpass=f=10000").expect("解码高频段");
        let ratio = rms(&low) / rms(&high).max(1e-9);
        println!("导出 DSF 低/高频能量比 = {ratio:.3}（音乐应 > 0.4，噪声 ≈ 0.1）");
        let _ = std::fs::remove_file(&out);
        assert!(
            ratio > 0.4,
            "导出 DSF 解码为噪声（低/高频比={ratio:.3} < 0.4）：位序(bits_per_sample)或去交错/64KB 块头逻辑有误"
        );
    }

    /// 诊断：用真实 ISO 抽出第 1 轨原始 DSD，对比多种「去交错方式 × 位反转」变换，
    /// 各自解码算低/高频能量比（音乐 > 0.4，噪声 ≈ 0.1），找出正确组合。
    /// 需 SACD_ISO 指向样本 .iso 且 ffmpeg 可用，否则跳过。
    #[test]
    fn sacd_diag_iso() {
        let iso = match std::env::var("SACD_ISO") {
            Ok(p) if Path::new(&p).exists() => p,
            _ => {
                eprintln!("SACD_ISO 未设置或不存在 -> 跳过");
                return;
            }
        };
        if !ffmpeg_ok() {
            eprintln!("ffmpeg 不可用 -> 跳过");
            return;
        }
        let disc = match SacdDisc::open(Path::new(&iso)) {
            Some(d) => d,
            None => {
                eprintln!("SACD ISO 解析失败 -> 跳过");
                return;
            }
        };
        eprintln!(
            "DIAG ALBUM={} SR={} block_headers={} ntracks={} audio_end_byte={}",
            disc.album, disc.dsd_sample_rate, disc.block_headers, disc.tracks.len(), disc.audio_end_byte
        );
        let tmp = Path::new(r"E:\TPlayerNext\sacd_diag");
        let _ = std::fs::create_dir_all(tmp);
        let sr = disc.dsd_sample_rate;

        // 跨多轨对比 parity_rev 与 frame_rev：正确的去交错应在各轨都得到「正常立体声」
        // （正 envCorr、低平坦度），而非单声道(corr~1)或反相关(envCorr<0)/噪声(高平坦度)。
        let mut tracks: Vec<usize> = vec![0, 1, 2, 7, 15];
        tracks.retain(|&i| i < disc.tracks.len());
        if tracks.is_empty() {
            tracks.push(0);
        }
        for &idx in &tracks {
            let t = &disc.tracks[idx];
            let start = t.start_lba * SACD_SECTOR_SIZE;
            let end = if idx + 1 < disc.tracks.len() {
                disc.tracks[idx + 1].start_lba * SACD_SECTOR_SIZE
            } else {
                disc.audio_end_byte.max(start + 1)
            };
            // 对齐到 DSD 帧边界（1176 字节），避免半截帧导致 L/R 平面长度不等、解码错位
            let cap = (((end - start).min(6_000_000)) / DSD64_FRAME_BYTES) * DSD64_FRAME_BYTES;
            let mut file = File::open(&disc.iso_path).unwrap();
            file.seek(SeekFrom::Start(start)).unwrap();
            let mut raw = vec![0u8; cap as usize];
            file.read_exact(&mut raw).unwrap();
            for deint in ["parity", "frame"] {
                let out = tmp.join(format!("v_t{}_{}_rev.dsf", idx, deint));
                build_dsf_variant(&out, &raw, deint, true, sr);
            }
            if idx == 0 {
                let out = tmp.join("v_current.dsf");
                let _ = disc.export_track_to_dsf(idx, &out);
            }
            eprintln!("  track {} (frames={}) -> parity_rev/frame_rev written", idx, t.frames);
        }
        eprintln!("DIAG DONE -> {}", tmp.display());
    }
/// 由原始 DSD 字节按指定「去交错方式 × 位反转」构造一个 DSF（双声道平面布局）。
/// - deint="parity": L,R 逐字节奇偶去交错（原实现假设）
/// - deint="block8": 每 16 字节为一组，前 8 字节属 L、后 8 字节属 R
/// - deint="half":  把原始字节直接对半拆为 L/R 平面（假设本就平面）
fn build_dsf_variant(out: &Path, raw: &[u8], deint: &str, rev: bool, sample_rate: u32) {
    let mut l_plane: Vec<u8> = Vec::new();
    let mut r_plane: Vec<u8> = Vec::new();
    match deint {
        "parity" => {
            for (i, &b) in raw.iter().enumerate() {
                let rb = if rev { reverse_bits(b) } else { b };
                if i % 2 == 0 {
                    l_plane.push(rb);
                } else {
                    r_plane.push(rb);
                }
            }
        }
        "block8" => {
            for chunk in raw.chunks(16) {
                if chunk.len() >= 8 {
                    for &b in &chunk[..8] {
                        let rb = if rev { reverse_bits(b) } else { b };
                        l_plane.push(rb);
                    }
                }
                if chunk.len() >= 16 {
                    for &b in &chunk[8..16] {
                        let rb = if rev { reverse_bits(b) } else { b };
                        r_plane.push(rb);
                    }
                }
            }
        }
        "frame" => {
            // 每 DSD 帧 = 588 字节 L + 588 字节 R（帧平面布局）
            for chunk in raw.chunks(1176) {
                let l = chunk.len().min(588);
                for &b in &chunk[..l] {
                    let rb = if rev { reverse_bits(b) } else { b };
                    l_plane.push(rb);
                }
                if chunk.len() > 588 {
                    for &b in &chunk[588..] {
                        let rb = if rev { reverse_bits(b) } else { b };
                        r_plane.push(rb);
                    }
                }
            }
        }
        "half" => {
            let mid = raw.len() / 2;
            for &b in &raw[..mid] {
                l_plane.push(if rev { reverse_bits(b) } else { b });
            }
            for &b in &raw[mid..] {
                r_plane.push(if rev { reverse_bits(b) } else { b });
            }
        }
        _ => {}
    }
    let payload = (l_plane.len() + r_plane.len()) as u64;
    let mut f = match File::create(out) {
        Ok(f) => f,
        Err(_) => return,
    };
    let _ = write_dsf_header(&mut f, payload / DSD64_FRAME_BYTES, 2, sample_rate, payload);
    let blk = 4096usize;
    let mut li = 0;
    let mut ri = 0;
    while li + blk <= l_plane.len() && ri + blk <= r_plane.len() {
        let _ = f.write_all(&l_plane[li..li + blk]);
        let _ = f.write_all(&r_plane[ri..ri + blk]);
        li += blk;
        ri += blk;
    }
    let _ = f.write_all(&l_plane[li..]);
    let _ = f.write_all(&r_plane[ri..]);
}

} // mod sacd_export_tests
