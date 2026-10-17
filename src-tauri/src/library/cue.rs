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
    // 优先 UTF-8；非法字节（GBK 等本地编码）按 lossy 处理，结构仍可解析（中文可能乱码）
    let content = String::from_utf8_lossy(&bytes).into_owned();

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
            } else if let Some(t) = pending.last_mut() {
                t.title = Some(value);
            }
            continue;
        }
        if upper.starts_with("PERFORMER ") {
            let value = unquote(&line[10..]);
            if current_file.is_none() {
                album_performer = Some(value);
            } else if let Some(t) = pending.last_mut() {
                t.performer = Some(value);
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
            let number = parse_track_number(&line[6..]).unwrap_or(0);
            pending.push(PendingTrack {
                file: current_file.clone().unwrap_or_default(),
                number,
                title: None,
                performer: None,
                start_sec: 0.0,
            });
            continue;
        }
        if upper.starts_with("INDEX ") {
            // INDEX 01 MM:SS:FF；优先使用 INDEX 01，未出现则回退首个（通常为 00 前导）
            if let Some(t) = pending.last_mut() {
                let (idx_no, secs) = parse_index(&line[6..]);
                if t.start_sec == 0.0 || idx_no == 1 {
                    t.start_sec = secs;
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
        let first_artist = artists.first().map(String::as_str).unwrap_or("");
        let album_id = Some(album_id_of(&album, first_artist));

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
        });
    }
    tracks
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
fn resolve_audio_path(path: &str, cue_dir: &Path) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        p
    } else {
        cue_dir.join(p)
    }
}

/// 解析 TRACK 行：`01 AUDIO` -> 1
fn parse_track_number(rest: &str) -> Option<u32> {
    rest.split_whitespace()
        .next()
        .and_then(|n| n.parse::<u32>().ok())
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
        let ff: f64 = parts[2].parse().unwrap_or(0.0);
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
