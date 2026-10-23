//! CUE 分轨索引解析：把 .cue 描述的整盘音频拆成多条虚拟曲目。
//!
//! 典型结构（简化）：
//! ```text
//! REM DATE 1995
//! TITLE "Album"
//! PERFORMER "Artist"
//! FILE "image.flac" WAVE
//!   TRACK 01 AUDIO
//!     TITLE "Song1"
//!     PERFORMER "Artist"
//!     INDEX 01 00:00:00
//!   TRACK 02 AUDIO
//!     TITLE "Song2"
//!     INDEX 01 03:45:20
//! ```
//! - 时长 = 相邻 INDEX 01 之差；最后一个音轨用音频文件总时长 - 起始偏移。
//! - 同一 cue 可含多个 FILE（跨碟），每个 FILE 的最后一轨时长取其文件总时长。
//! - 解析产物为一组 `Track`，共享同一专辑聚合（album_id），`is_cue = true`，
//!   `cue_offset` 记录该轨在镜像文件中的起始秒数，供播放引擎 seek。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::ItemKey;
use sha2::{Digest, Sha256};

use super::models::{album_id_of, Track};

/// CUE 分轨时长计算基准：1 帧 = 1/75 秒（CDDA）
const CUE_FRAME_RATE: f64 = 75.0;

/// 解析单个 .cue 文件，返回该整盘对应的全部虚拟曲目。
///
/// `cue_path` 为 .cue 文件绝对路径。返回空表示解析失败或无有效音轨。
pub fn parse_cue(cue_path: &Path) -> Vec<Track> {
    let bytes = match std::fs::read(cue_path) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    // 自动识别编码：UTF-8(BOM) → UTF-8 → GB18030(GBK/GB2312 超集) → lossy 兜底。
    // 中文 CUE 多为 GBK，若按 UTF-8 lossy 处理，TITLE/PERFORMER 会乱码，
    // 且 FILE "…wav" 中的中文文件名会变成乱码路径 → 找不到音频文件 → 播放失败。
    let content = decode_text(&bytes);

    let cue_dir = cue_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let cue_mtime = std::fs::metadata(cue_path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // 专辑级上下文
    let mut album_title: Option<String> = None;
    let mut album_performer: Option<String> = None;
    let mut album_year: Option<u32> = None;

    // 当前 FILE 上下文
    let mut current_file: Option<PathBuf> = None;

    // 每个 FILE 的元数据缓存（时长 + 标签），避免同一镜像被多轨重复探测
    let mut file_meta: HashMap<PathBuf, FileMeta> = HashMap::new();

    // 收集到的音轨（含其 INDEX 偏移，最后统一计算时长）
    struct PendingTrack {
        file: PathBuf,
        number: u32,
        title: Option<String>,
        performer: Option<String>,
        start_sec: f64,
    }
    let mut pending: Vec<PendingTrack> = Vec::new();
    // 当前 TRACK 是否为可播放的音频轨；非 AUDIO（如 DATA 数据轨）整轨跳过，
    // 其后的 TITLE/PERFORMER/INDEX 也一并忽略，避免污染上一条音频轨。
    let mut track_is_audio = true;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let upper = line.to_uppercase();
        if upper.starts_with("REM ") {
            if let Some(date) = parse_rem_date(&line[4..]) {
                album_year = Some(date);
            }
            continue;
        }
        if upper.starts_with("TITLE ") {
            let value = unquote(&line[6..]);
            if current_file.is_none() {
                album_title = Some(value);
            } else if track_is_audio {
                if let Some(t) = pending.last_mut() {
                    t.title = Some(value);
                }
            }
            continue;
        }
        if upper.starts_with("PERFORMER ") {
            let value = unquote(&line[10..]);
            if current_file.is_none() {
                album_performer = Some(value);
            } else if track_is_audio {
                if let Some(t) = pending.last_mut() {
                    t.performer = Some(value);
                }
            }
            continue;
        }
        if upper.starts_with("FILE ") {
            let (path, _fmt) = parse_file_line(&line[5..]);
            let abs = resolve_audio_path(&path, &cue_dir);
            current_file = Some(abs.clone());
            file_meta.entry(abs).or_insert_with(|| FileMeta::probe(cue_dir.join(&path)));
            continue;
        }
        if upper.starts_with("TRACK ") {
            let (number, is_audio) = parse_track(&line[6..]);
            track_is_audio = is_audio;
            if is_audio {
                pending.push(PendingTrack {
                    file: current_file.clone().unwrap_or_default(),
                    number,
                    title: None,
                    performer: None,
                    start_sec: 0.0,
                });
            }
            continue;
        }
        if upper.starts_with("INDEX ") {
            // INDEX 01 MM:SS:FF；优先使用 INDEX 01，未出现则回退首个（通常为 00 前导）。
            // 仅当当前为音频轨时才记录（DATA 等跳过轨的索引不污染上一条音频轨）。
            if track_is_audio {
                if let Some(t) = pending.last_mut() {
                    let (idx_no, secs) = parse_index(&line[6..]);
                    if t.start_sec == 0.0 || idx_no == 1 {
                        t.start_sec = secs;
                    }
                }
            }
            continue;
        }
        // 其它（CATALOG / ISRC / FLAGS / PREGAP / POSTGAP 等）忽略
    }

    // 计算时长并产出 Track
    let fallback_album = cue_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("未知专辑")
        .to_string();
    let album = album_title.clone().unwrap_or_else(|| fallback_album.clone());
    let album_artist = album_performer
        .clone()
        .unwrap_or_else(|| "未知艺术家".to_string());

    let mut tracks: Vec<Track> = Vec::new();
    for i in 0..pending.len() {
        let t = &pending[i];
        // 缺少 FILE 行的音轨无法定位音频，直接跳过（避免入库无效条目）
        if t.file.as_os_str().is_empty() {
            continue;
        }
        // 下一同 FILE 音轨的起始，作为本轨结束；否则取镜像文件总时长
        let end_sec = if i + 1 < pending.len() && pending[i + 1].file == t.file {
            pending[i + 1].start_sec
        } else {
            file_meta
                .get(&t.file)
                .and_then(|m| m.duration)
                .unwrap_or(0.0)
        };
        let duration = if end_sec > t.start_sec {
            end_sec - t.start_sec
        } else {
            0.0
        };

        let artists = match &t.performer {
            Some(p) if !p.trim().is_empty() => split_artists(p),
            _ => split_artists(&album_artist),
        };
        // 分轨专辑仅按「专辑名」聚合（album_id 不带演唱者），避免合辑（如 6CD 群星）
        // 每轨不同歌手被拆成"每轨一专辑"；曲目自身仍保留各自演唱者供"艺术家"分类使用。
        let album_id = Some(album_id_of(&album, ""));

        let title = t
            .title
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| {
                t.file
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "未知标题".to_string())
            });

        let meta = file_meta.get(&t.file);
        let id = cue_track_id(cue_path, &t.file, t.number);
        tracks.push(Track {
            id,
            title,
            artists,
            album: album.clone(),
            album_id,
            duration,
            track_number: if t.number == 0 { None } else { Some(t.number) },
            year: album_year,
            file_path: t.file.to_string_lossy().to_string(),
            file_size: meta.map(|m| m.file_size).unwrap_or(0),
            format: meta.map(|m| m.format.clone()).unwrap_or_default(),
            bitrate: meta.and_then(|m| m.bitrate),
            sample_rate: meta.and_then(|m| m.sample_rate),
            has_cover: meta.map(|m| m.has_cover).unwrap_or(false),
            replaygain_db: meta.and_then(|m| m.replaygain_db),
            replaygain_peak: meta.and_then(|m| m.replaygain_peak),
            added_at: chrono::Utc::now().timestamp(),
            modified_at: cue_mtime,
            is_cue: true,
            cue_offset: if t.start_sec > 0.0 {
                Some(t.start_sec)
            } else {
                None
            },
            is_sacd: false,
            sacd_track_index: 0,
            sacd_area: None,
            sacd_dsd_rate: None,
            metadata_pending: false,
            iso_placeholder: false,
        });
    }
    tracks
}

/// 轻量提取 .cue 引用的音频文件路径：**只读文本，不探测音频**。
///
/// `parse_cue` 会顺带探测镜像时长（lofty / ffprobe），网络共享上代价过高；
/// 本函数只解析 `FILE` 行，用于两处轻量判定：
///  1. 判断某个整碟镜像是否被 CUE 引用（扫描期避免把镜像当普通单曲入库）；
///  2. 判断某个 CUE 是否已被解析过（比对 cue 文件 mtime 与分轨的 modified_at）。
pub fn referenced_audio_files(cue_path: &Path) -> Vec<PathBuf> {
    let bytes = match std::fs::read(cue_path) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let content = decode_text(&bytes);
    let cue_dir = cue_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut out = Vec::new();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if !line.to_uppercase().starts_with("FILE ") {
            continue;
        }
        let (path, _fmt) = parse_file_line(&line[5..]);
        if path.trim().is_empty() {
            continue;
        }
        out.push(resolve_audio_path(&path, &cue_dir));
    }
    out
}

/// 镜像文件的元数据（时长 + 标签信息）
struct FileMeta {
    duration: Option<f64>,
    file_size: u64,
    format: String,
    has_cover: bool,
    bitrate: Option<u32>,
    sample_rate: Option<u32>,
    replaygain_db: Option<f32>,
    replaygain_peak: Option<f32>,
}

impl FileMeta {
    /// 探测单个镜像文件：优先 lofty（flac/ape/wav/wv 等），时长缺失时回退 ffprobe
    fn probe(path: PathBuf) -> FileMeta {
        let mut m = FileMeta {
            duration: None,
            file_size: 0,
            format: String::new(),
            has_cover: false,
            bitrate: None,
            sample_rate: None,
            replaygain_db: None,
            replaygain_peak: None,
        };
        if let Ok(md) = std::fs::metadata(&path) {
            m.file_size = md.len();
        }
        m.format = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        if let Ok(f) = lofty::read_from_path(&path) {
            let props = f.properties();
            let d = props.duration().as_secs_f64();
            if d > 0.0 {
                m.duration = Some(d);
            }
            m.bitrate = props.audio_bitrate();
            m.sample_rate = props.sample_rate();
            if let Some(tag) = f.primary_tag().cloned() {
                m.has_cover = !tag.pictures().is_empty();
                m.replaygain_db = tag
                    .get_strings(&ItemKey::ReplayGainTrackGain)
                    .find_map(parse_replaygain_gain_db);
                m.replaygain_peak = tag
                    .get_strings(&ItemKey::ReplayGainTrackPeak)
                    .find_map(parse_replaygain_peak);
            }
        }
        // lofty 无法给出时长（dsf/dff/dts/tta/tak 等）时用 ffprobe 兜底
        if m.duration.is_none() {
            m.duration = crate::player::engine::probe_duration(&path);
        }
        m
    }
}

/// 自动识别并解码本地文本文件（CUE/LRC 等）：
/// 1. UTF-8 BOM → 2. UTF-16 BOM(LE/BE) → 3. 合法 UTF-8 →
/// 4. GB18030（GBK/GB2312 超集，中文 CUE/LRC 最常见）→ 5. UTF-8 lossy 兜底。
///
/// 中文 CUE 多为 GBK；若按 UTF-8 lossy 解码，TITLE/PERFORMER 乱码，且 `FILE "…wav"` 的
/// 中文文件名会变成乱码路径，导致找不到音频文件、整轨无法播放。
pub(crate) fn decode_text(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        // UTF-16：encode() 会做 BOM 嗅探
        let enc = if bytes[0] == 0xFF {
            encoding_rs::UTF_16LE
        } else {
            encoding_rs::UTF_16BE
        };
        let (cow, _, _) = enc.decode(bytes);
        return cow.into_owned();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    let (cow, _, had_errors) = encoding_rs::GB18030.decode(bytes);
    if !had_errors {
        return cow.into_owned();
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// 去掉字符串首尾的双引号（CUE 用双引号包裹字段）
fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// 解析 FILE 行剩余部分：`"image.flac" WAVE` 或 `image.flac WAVE`
fn parse_file_line(rest: &str) -> (String, String) {
    let rest = rest.trim();
    if let Some(stripped) = rest.strip_prefix('"') {
        if let Some(end) = stripped.find('"') {
            let path = stripped[..end].to_string();
            let fmt = stripped[end + 1..].trim().to_string();
            return (path, fmt);
        }
        return (stripped.to_string(), String::new());
    }
    let mut parts = rest.split_whitespace();
    let path = parts.next().unwrap_or("").to_string();
    let fmt = parts.next().unwrap_or("").to_string();
    (path, fmt)
}

/// 把 cue 内的相对 FILE 解析为绝对路径（以 cue 所在目录为基准）
///
/// Windows 上制作的 CUE，其 `FILE "..."` 行常用反斜杠作目录分隔符；Linux 下反斜杠
/// **不是**路径分隔符，`PathBuf::from("a\\b.flac")` 会被当成单个文件名，解析出的路径
/// 与磁盘实际文件不一致——后果是分轨定位不到音频、且"整碟大文件被分轨引用故排除"的
/// 去重也失效（大文件会连同分轨一起出现在播放列表）。故先把反斜杠统一为正斜杠。
fn resolve_audio_path(path: &str, cue_dir: &Path) -> PathBuf {
    let normalized = if path.contains('\\') {
        std::borrow::Cow::Owned(path.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(path)
    };
    let p = PathBuf::from(normalized.as_ref());
    if p.is_absolute() {
        p
    } else {
        cue_dir.join(p)
    }
}

/// 解析 TRACK 行：`01 AUDIO` -> (轨号, 是否可播放音频轨)。
///
/// CUE 的 TRACK 类型（第二个 token）常见为 `AUDIO`；混合模式碟末轨常为 `DATA`（数据轨），
/// 此类不计入音乐曲目，返回 `is_audio = false` 由上层整轨跳过。类型缺失按 `AUDIO` 处理。
fn parse_track(rest: &str) -> (u32, bool) {
    let mut it = rest.split_whitespace();
    let num = it.next().and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
    let ty = it.next().unwrap_or("AUDIO").to_uppercase();
    let is_audio = ty.is_empty() || ty == "AUDIO";
    (num, is_audio)
}

/// 解析 INDEX 行：`01 MM:SS:FF` -> (1, 秒数)
fn parse_index(rest: &str) -> (u32, f64) {
    let mut it = rest.split_whitespace();
    let idx = it.next().and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
    let secs = parse_cue_time(it.next().unwrap_or(""));
    (idx, secs)
}

/// 解析 CUE 时间：`MM:SS:FF` / `MM:SS` / `SS`
fn parse_cue_time(time: &str) -> f64 {
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() == 3 {
        let mm: f64 = parts[0].parse().unwrap_or(0.0);
        let ss: f64 = parts[1].parse().unwrap_or(0.0);
        let ff: f64 = parts[2].parse::<f64>().unwrap_or(0.0).clamp(0.0, 74.0);
        mm * 60.0 + ss + ff / CUE_FRAME_RATE
    } else if parts.len() == 2 {
        let mm: f64 = parts[0].parse().unwrap_or(0.0);
        let ss: f64 = parts[1].parse().unwrap_or(0.0);
        mm * 60.0 + ss
    } else {
        parts.first().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0)
    }
}

/// 解析 `REM DATE 1995` / `REM DATE 1995.12.31` -> 1995
fn parse_rem_date(rest: &str) -> Option<u32> {
    let v = rest.trim();
    if let Some(year) = v.strip_prefix("DATE ") {
        year
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .and_then(|y| y.parse::<u32>().ok())
    } else {
        None
    }
}

fn split_artists(raw: &str) -> Vec<String> {
    raw.split(['/', ';', ','])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_replaygain_gain_db(raw: &str) -> Option<f32> {
    let trimmed = raw.trim();
    let numeric = trimmed
        .trim_end_matches("dB")
        .trim_end_matches("db")
        .trim_end_matches("DB")
        .trim();
    numeric.parse::<f32>().ok().map(|v| v.clamp(-24.0, 24.0))
}

fn parse_replaygain_peak(raw: &str) -> Option<f32> {
    let trimmed = raw.trim();
    let numeric = trimmed
        .trim_end_matches("dBFS")
        .trim_end_matches("dbfs")
        .trim();
    numeric.parse::<f32>().ok().map(|v| v.clamp(0.0, 10.0))
}

/// CUE 分轨稳定 ID：基于 cue 路径 + 镜像路径 + 音轨号，保证跨扫描稳定
fn cue_track_id(cue: &Path, file: &Path, num: u32) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"cue|");
    hasher.update(cue.to_string_lossy().as_bytes());
    hasher.update(b"|");
    hasher.update(file.to_string_lossy().as_bytes());
    hasher.update(b"|");
    hasher.update(num.to_string().as_bytes());
    let r = hasher.finalize();
    r.iter().take(8).map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// 把一段 CUE 文本写入临时文件，返回路径（调用方负责 cleanup）。
    fn write_temp_cue(name: &str, content: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(name);
        let mut f = std::fs::File::create(&p).expect("create temp cue");
        f.write_all(content.as_bytes()).expect("write temp cue");
        p
    }

    fn cleanup(p: &std::path::Path) {
        let _ = std::fs::remove_file(p);
    }

    // 取自真实 EAC 导出的多轨交响乐 CUE（含 INDEX 00 前间隙 + 中文/括号文件名）
    const MOZART: &str = r#"REM GENRE Symphony
REM DATE 2002
REM DISCID C9124D0F
REM COMMENT "ExactAudioCopy v0.95b3"
PERFORMER "Mozart (complete works)"
TITLE "Volume 1(CD6) Symphonies KV 111a-130-132-183"
FILE "Volume 1(CD6) Symphonies KV 111a-130-132-183.flac" WAVE
  TRACK 01 AUDIO
    TITLE "Symphony in D major, KV 111a - 1. Allegro assai"
    PERFORMER "Mozart (complete works)"
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    TITLE "Symphony in D major, KV 111a - 2. Andante grazioso"
    PERFORMER "Mozart (complete works)"
    INDEX 01 03:31:05
  TRACK 03 AUDIO
    TITLE "Symphony in D major, KV 111a - 3. Presto"
    PERFORMER "Mozart (complete works)"
    INDEX 01 04:49:48
  TRACK 04 AUDIO
    TITLE "Symphony No. 18 in F major, KV 130 - 1. Allegro"
    PERFORMER "Mozart (complete works)"
    INDEX 00 06:07:58
    INDEX 01 06:13:47
  TRACK 05 AUDIO
    TITLE "Symphony No. 18 in F major, KV 130 - 2. Andantino grazioso"
    PERFORMER "Mozart (complete works)"
    INDEX 01 13:50:47
  TRACK 06 AUDIO
    TITLE "Symphony No. 18 in F major, KV 130 - 3. Menuetto"
    PERFORMER "Mozart (complete works)"
    INDEX 01 20:44:50
  TRACK 07 AUDIO
    TITLE "Symphony No. 18 in F major, KV 130 - 4. Molto allegro"
    PERFORMER "Mozart (complete works)"
    INDEX 01 23:11:74
  TRACK 08 AUDIO
    TITLE "Symphony No. 19 in E flat major, KV 132 - 1. Allegro"
    PERFORMER "Mozart (complete works)"
    INDEX 00 30:46:64
    INDEX 01 30:51:45
  TRACK 09 AUDIO
    TITLE "Symphony No. 19 in E flat major, KV 132 - 2. Andante"
    PERFORMER "Mozart (complete works)"
    INDEX 01 35:07:42
  TRACK 10 AUDIO
    TITLE "Symphony No. 19 in E flat major, KV 132 - 3. Menuetto"
    PERFORMER "Mozart (complete works)"
    INDEX 01 42:00:03
  TRACK 11 AUDIO
    TITLE "Symphony No. 19 in E flat major, KV 132 - 4. Allegro"
    PERFORMER "Mozart (complete works)"
    INDEX 01 46:05:28
  TRACK 12 AUDIO
    TITLE "Symphony No. 25 in G minor, KV 183 - 1. Allegro con brio"
    PERFORMER "Mozart (complete works)"
    INDEX 00 50:05:18
    INDEX 01 50:12:07
  TRACK 13 AUDIO
    TITLE "Symphony No. 25 in G minor, KV 183 - 2. Andante"
    PERFORMER "Mozart (complete works)"
    INDEX 01 61:15:70
  TRACK 14 AUDIO
    TITLE "Symphony No. 25 in G minor, KV 183 - 3. Menuetto"
    PERFORMER "Mozart (complete works)"
    INDEX 01 67:23:43
  TRACK 15 AUDIO
    TITLE "Symphony No. 25 in G minor, KV 183 - 4. Allegro"
    PERFORMER "Mozart (complete works)"
    INDEX 01 70:46:22
"#;

    #[test]
    fn golden_mozart_offsets_and_index_priority() {
        let p = write_temp_cue("tplayer_cue_golden.cue", MOZART);
        let tracks = parse_cue(&p);
        assert_eq!(tracks.len(), 15, "应为 15 条音轨");

        // TRACK 01 起点 0 -> cue_offset 为 None，播放从头开始
        assert_eq!(tracks[0].cue_offset, None);
        // TRACK 02 03:31:05 -> 211.0667
        assert!((tracks[1].cue_offset.unwrap() - 211.0667).abs() < 0.01);
        // TRACK 04 必须取 INDEX 01 (06:13:47 = 373.6267)，而非 INDEX 00 (06:07:58)
        assert!((tracks[3].cue_offset.unwrap() - 373.6267).abs() < 0.01);
        // TRACK 15 70:46:22 -> 4246.2933
        assert!((tracks[14].cue_offset.unwrap() - 4246.2933).abs() < 0.01);

        // cue_offset 必须严格单调递增（不变量）
        let mut prev = -1.0_f64;
        for t in &tracks {
            if let Some(o) = t.cue_offset {
                assert!(o > prev, "cue_offset 必须单调递增: {:?}", t.title);
                prev = o;
            }
        }

        // 同专辑聚合：15 轨 album_id 一致
        let aid = tracks[0].album_id.clone();
        assert!(tracks.iter().all(|t| t.album_id == aid));

        // 字段解析
        assert!(tracks[1].title.contains("KV 111a - 2."));
        assert_eq!(tracks[0].year, Some(2002));
        assert!(tracks[0].artists.iter().any(|a| a.contains("Mozart")));
        cleanup(&p);
    }

    #[test]
    fn skips_non_audio_data_tracks() {
        let cue = "FILE \"img.bin\" BINARY\nTRACK 01 AUDIO\nINDEX 01 00:00:00\nTRACK 02 DATA\nINDEX 01 01:00:00\nTRACK 03 AUDIO\nINDEX 01 02:00:00\n";
        let p = write_temp_cue("tplayer_cue_nonaudio.cue", cue);
        let tracks = parse_cue(&p);
        assert_eq!(tracks.len(), 2, "DATA 数据轨应被整轨跳过");
        assert_eq!(tracks[0].track_number, Some(1));
        assert_eq!(tracks[1].track_number, Some(3));
        cleanup(&p);
    }

    #[test]
    fn clamps_out_of_range_frames() {
        // FF=75 超出 CD 帧上限(0..=74)，应被 clamp 到 74 -> 74/75 秒
        let cue = "FILE \"x.flac\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:75\nTRACK 02 AUDIO\nINDEX 01 00:01:00\n";
        let p = write_temp_cue("tplayer_cue_clamp.cue", cue);
        let tracks = parse_cue(&p);
        assert_eq!(tracks.len(), 2);
        assert!((tracks[0].cue_offset.unwrap() - 74.0 / 75.0).abs() < 1e-6);
        cleanup(&p);
    }

    #[test]
    fn skips_tracks_without_file() {
        // 缺少 FILE 行的音轨无法定位音频，应被跳过
        let cue = "TRACK 01 AUDIO\nINDEX 01 00:00:00\nFILE \"x.flac\" WAVE\nTRACK 02 AUDIO\nINDEX 01 01:00:00\n";
        let p = write_temp_cue("tplayer_cue_nofile.cue", cue);
        let tracks = parse_cue(&p);
        assert_eq!(tracks.len(), 1, "缺少 FILE 的轨应被跳过");
        assert_eq!(tracks[0].track_number, Some(2));
        cleanup(&p);
    }
}
