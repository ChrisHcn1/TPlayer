//! 实验室·元数据增强：识别结果的持久化
//!
//! 目标：已识别 / 校正的标题、艺术家、专辑落到磁盘，重新导入或重扫时直接读取，
//! 不再重复联网识别。
//!
//! 存储策略（按要求优先级）：
//!  1. **优先写入音频文件自身的标签**（lofty）：
//!     - MP3 / AAC / AIFF / WAV → ID3v2
//!     - FLAC / OGG / Opus → Vorbis Comments
//!     - M4A / MP4 → iTunes ilst
//!     - APE / WavPack → APE Tag
//!  2. 格式不受支持（DSF / DFF / TTA / TAK / DTS / WMA 等 lofty 无法写入）、
//!     文件只读 / 被占用 / 写入失败时，降级写入**与音频同名的附属文件**
//!     `<文件名含扩展名>.tplayer-meta.json`。文件名带音频扩展名，确保曲目与
//!     记录一一对应（`song.flac` 与 `song.mp3` 各存一份，不串味），也不占用 /
//!     污染 `.lrc` 歌词文件；
//!  3. 扫描期 [`super::scan::parse_track`] 只在标签缺失 / 未知时读取该附属文件
//!     补位，因此重新导入、重新扫描都能还原，且绝不覆盖已有真实标签。
//!
//! 写入标签会改变文件 mtime 并触发目录监听重扫。写入前调用 [`mark_self_write`]
//! 登记路径，`watch::flush_batch` 通过 [`is_self_write`] 跳过自己刚写入的文件，
//! 避免「写入 → 监听重扫 → 再次解析」的无谓循环。

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{Accessor, Tag, TagType};
use serde::{Deserialize, Serialize};

use super::models::Track;

/// 附属文件后缀（紧跟在音频文件名之后，保留音频扩展名）
const SIDECAR_SUFFIX: &str = ".tplayer-meta.json";
/// 自写抑制窗口：覆盖目录监听的 600ms 去抖窗口并留足余量
const SELF_WRITE_TTL: Duration = Duration::from_secs(10);

/// 附属文件中保存的识别结果
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedMeta {
    /// 结构版本（后续扩展时兼容读取）
    pub version: u32,
    /// 音频文件名（含扩展名）：用于校验一一对应，防止张冠李戴
    pub file: String,
    /// 标题（空串表示未知，不保存 / 不写入）
    pub title: String,
    /// 艺术家列表（空数组表示未知）
    pub artists: Vec<String>,
    /// 专辑（空串表示未知）
    pub album: String,
    /// 保存时间（本地 "YYYY-MM-DD HH:MM:SS"）
    pub saved_at: String,
}

/// 保存结果：供命令返回与识别日志展示
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistOutcome {
    pub track_id: String,
    /// tag = 已写入音频标签；sidecar = 已写入附属文件；none = 未保存
    pub target: String,
    /// 中文说明（成功位置 / 降级原因 / 失败原因）
    pub detail: String,
    pub at: String,
}

/// 自写路径登记表（小写路径 + 时间），供目录监听跳过自己触发的变更
static SELF_WRITES: Mutex<Vec<(String, Instant)>> = Mutex::new(Vec::new());

fn prune_self_writes(list: &mut Vec<(String, Instant)>) {
    let now = Instant::now();
    list.retain(|(_, at)| now.duration_since(*at) < SELF_WRITE_TTL);
}

/// 写入前登记：该路径由本应用写入，监听重扫时跳过
pub fn mark_self_write(path: &Path) {
    let key = path.to_string_lossy().to_lowercase();
    let mut list = SELF_WRITES.lock().unwrap_or_else(|e| e.into_inner());
    prune_self_writes(&mut list);
    list.push((key, Instant::now()));
}

/// 是否为短时间内由本应用写入的路径（目录监听使用）
pub fn is_self_write(path: &Path) -> bool {
    let key = path.to_string_lossy().to_lowercase();
    let mut list = SELF_WRITES.lock().unwrap_or_else(|e| e.into_inner());
    prune_self_writes(&mut list);
    list.iter().any(|(p, _)| *p == key)
}

/// 附属文件路径：`song.flac` → `song.flac.tplayer-meta.json`
fn sidecar_path(audio: &Path) -> Option<PathBuf> {
    let name = audio.file_name()?.to_str()?;
    if name.is_empty() {
        return None;
    }
    Some(audio.with_file_name(format!("{name}{SIDECAR_SUFFIX}")))
}

/// 读取附属文件中的识别结果；文件不存在、损坏或与当前音频文件名不匹配时返回 None
pub(crate) fn read_sidecar(audio: &Path) -> Option<SavedMeta> {
    let path = sidecar_path(audio)?;
    let bytes = std::fs::read(path).ok()?;
    let meta: SavedMeta = serde_json::from_slice(&bytes).ok()?;
    let expect = audio.file_name()?.to_str()?;
    if !meta.file.eq_ignore_ascii_case(expect) {
        // 关联校验失败：拒绝使用，避免把别的曲目的识别结果套过来
        return None;
    }
    Some(meta)
}

/// 是否支持把标签写回音频文件本身（按扩展名静态判定；实际写入失败仍会降级）
pub(crate) fn tag_write_supported(audio: &Path) -> bool {
    let ext = audio
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    matches!(
        ext.as_str(),
        "mp3"
            | "mp2"
            | "mp1"
            | "flac"
            | "wav"
            | "wave"
            | "aac"
            | "m4a"
            | "m4b"
            | "m4r"
            | "mp4"
            | "ogg"
            | "oga"
            | "opus"
            | "ape"
            | "wv"
            | "aif"
            | "aiff"
    )
}

/// 保存单曲识别结果：优先文件标签，失败 / 不支持时降级附属文件。
///
/// 只写入有实际值的字段：占位文本（未知标题 / 未知艺术家 / 未知专辑）一律不写，
/// 避免把「未知」固化进文件。
pub fn save_track(track: &Track) -> PersistOutcome {
    let at = now_str();
    let outcome = |target: &str, detail: String| PersistOutcome {
        track_id: track.id.clone(),
        target: target.to_string(),
        detail,
        at: at.clone(),
    };

    // CUE / SACD 虚拟分轨没有独立音频文件，无法写标签，也无法做唯一映射
    if track.is_cue || track.is_sacd || track.iso_placeholder {
        return outcome(
            "none",
            "CUE / SACD 分轨不对应单一音频文件，无法保存识别结果".to_string(),
        );
    }

    let path = Path::new(&track.file_path);
    if !path.is_file() {
        return outcome("none", "音频文件不存在（可能已被移动或删除）".to_string());
    }

    let meta = build_meta(
        path.file_name().and_then(|s| s.to_str()).unwrap_or(""),
        &track.title,
        &track.artists,
        &track.album,
        &at,
    );
    if meta.title.is_empty() && meta.artists.is_empty() && meta.album.is_empty() {
        return outcome(
            "none",
            "标题 / 艺术家 / 专辑均为未知，没有可保存的识别结果".to_string(),
        );
    }

    if tag_write_supported(path) {
        mark_self_write(path);
        match write_tags(path, &meta) {
            Ok(label) => {
                // 标签已写入：清理此前可能存在的降级附属文件
                remove_sidecar(path);
                return outcome("tag", format!("已写入音频标签（{label}）"));
            }
            Err(tag_err) => {
                // 写入失败（只读 / 被占用 / 特殊编码）→ 降级附属文件
                return match write_sidecar(path, &meta) {
                    Ok(name) => outcome(
                        "sidecar",
                        format!("文件标签写入失败（{tag_err}），已保存到附属文件 {name}"),
                    ),
                    Err(side_err) => outcome(
                        "none",
                        format!("标签写入失败（{tag_err}）；附属文件写入失败（{side_err}）"),
                    ),
                };
            }
        }
    }

    match write_sidecar(path, &meta) {
        Ok(name) => outcome(
            "sidecar",
            format!("该格式不支持写入文件标签，已保存到附属文件 {name}"),
        ),
        Err(err) => outcome(
            "none",
            format!("该格式不支持写入文件标签，附属文件写入失败（{err}）"),
        ),
    }
}

/// 由曲目字段构造待保存记录：占位值转为空，保持「未知不落盘」
fn build_meta(file: &str, title: &str, artists: &[String], album: &str, at: &str) -> SavedMeta {
    let title = title.trim();
    let title = if title.is_empty() || title == "未知标题" {
        String::new()
    } else {
        title.to_string()
    };
    let artists: Vec<String> = artists
        .iter()
        .map(|a| a.trim())
        .filter(|a| !a.is_empty() && *a != "未知艺术家")
        .map(|a| a.to_string())
        .collect();
    let album = album.trim();
    let album = if album.is_empty() || album == "未知专辑" {
        String::new()
    } else {
        album.to_string()
    };
    SavedMeta {
        version: 1,
        file: file.to_string(),
        title,
        artists,
        album,
        saved_at: at.to_string(),
    }
}

/// 写入音频标签：保留原标签（封面 / 歌词 / ReplayGain 等），只覆盖标题 / 艺术家 / 专辑
fn write_tags(path: &Path, meta: &SavedMeta) -> Result<&'static str, String> {
    let mut tagged =
        lofty::read_from_path(path).map_err(|e| format!("读取音频标签失败：{e}"))?;
    let tag_type = tagged.primary_tag_type();
    if tagged.tag(tag_type).is_none() {
        tagged.insert_tag(Tag::new(tag_type));
    }
    let tag = tagged
        .tag_mut(tag_type)
        .ok_or_else(|| "无法创建或打开音频标签".to_string())?;
    if !meta.title.is_empty() {
        tag.set_title(meta.title.clone());
    }
    if !meta.artists.is_empty() {
        // 标签 Artist 只有单值，多位艺术家以 " / " 连接，扫描时按分隔符还原
        tag.set_artist(meta.artists.join(" / "));
    }
    if !meta.album.is_empty() {
        tag.set_album(meta.album.clone());
    }
    tagged
        .save_to_path(path, WriteOptions::default())
        .map_err(|e| format!("保存音频标签失败：{e}"))?;
    Ok(tag_type_label(tag_type))
}

fn tag_type_label(tag_type: TagType) -> &'static str {
    match tag_type {
        TagType::Id3v1 => "ID3v1",
        TagType::Id3v2 => "ID3v2",
        TagType::Ape => "APE 标签",
        TagType::VorbisComments => "Vorbis Comments",
        TagType::Mp4Ilst => "MP4 ilst",
        TagType::RiffInfo => "RIFF INFO",
        TagType::AiffText => "AIFF 文本块",
        // TagType 标记为 non_exhaustive：未来新增类型时按通用文案兜底
        _ => "音频标签",
    }
}

/// 写入附属文件，返回文件名（供提示）
fn write_sidecar(audio: &Path, meta: &SavedMeta) -> Result<String, String> {
    let path = sidecar_path(audio).ok_or_else(|| "无法确定附属文件路径".to_string())?;
    let text = serde_json::to_string_pretty(meta).map_err(|e| format!("序列化失败：{e}"))?;
    std::fs::write(&path, text).map_err(|e| format!("写入失败：{e}"))?;
    Ok(path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(SIDECAR_SUFFIX)
        .to_string())
}

/// 标签写入成功后清理历史降级文件（不存在 / 删除失败均忽略）
fn remove_sidecar(audio: &Path) {
    if let Some(path) = sidecar_path(audio) {
        if path.is_file() {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn now_str() -> String {
    chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SEQ: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir(tag: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "tplayer-persist-{}-{}-{}",
            std::process::id(),
            tag,
            SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn ability_mapping_covers_mainstream_and_fallback_formats() {
        for writable in [
            "song.mp3", "song.flac", "song.wav", "song.m4a", "song.aac", "song.ogg", "song.ape",
            "song.wv", "song.aiff",
        ] {
            assert!(
                tag_write_supported(Path::new(writable)),
                "{writable} 应支持写入标签"
            );
        }
        for fallback in ["song.dsf", "song.dff", "song.tta", "song.tak", "song.wma"] {
            assert!(
                !tag_write_supported(Path::new(fallback)),
                "{fallback} 应降级附属文件"
            );
        }
    }

    #[test]
    fn sidecar_name_keeps_audio_extension() {
        let path = sidecar_path(Path::new("C:/Music/song.flac")).unwrap();
        assert_eq!(
            path.file_name().and_then(|s| s.to_str()),
            Some("song.flac.tplayer-meta.json")
        );
    }

    #[test]
    fn sidecar_roundtrip_and_one_to_one_guard() {
        let dir = temp_dir("roundtrip");
        let audio = dir.join("song.flac");
        std::fs::write(&audio, b"").unwrap();

        let meta = build_meta(
            "song.flac",
            "夜曲",
            &["周杰伦".to_string(), "费玉清".to_string()],
            "十一月的萧邦",
            "2026-10-26 12:00:00",
        );
        write_sidecar(&audio, &meta).unwrap();
        let read = read_sidecar(&audio).unwrap();
        assert_eq!(read, meta);

        // 文件名不匹配的附属文件必须拒绝，避免张冠李戴
        let other = dir.join("other.flac");
        std::fs::write(&other, b"").unwrap();
        std::fs::write(
            dir.join("other.flac.tplayer-meta.json"),
            serde_json::to_string(&meta).unwrap(),
        )
        .unwrap();
        assert!(read_sidecar(&other).is_none());
    }

    #[test]
    fn sidecar_rejects_corrupted_json() {
        let dir = temp_dir("corrupt");
        let audio = dir.join("song.dsf");
        std::fs::write(&audio, b"").unwrap();
        std::fs::write(dir.join("song.dsf.tplayer-meta.json"), b"{not json").unwrap();
        assert!(read_sidecar(&audio).is_none());
    }

    #[test]
    fn placeholders_are_not_persisted() {
        let meta = build_meta(
            "song.flac",
            "未知标题",
            &["未知艺术家".to_string()],
            "未知专辑",
            "2026-10-26 12:00:00",
        );
        assert!(meta.title.is_empty());
        assert!(meta.artists.is_empty());
        assert!(meta.album.is_empty());
    }

    #[test]
    fn self_write_registry_expires_entries() {
        let path = Path::new("C:/Music/self-write-test.flac");
        assert!(!is_self_write(path));
        mark_self_write(path);
        assert!(is_self_write(path));
    }

    /// 构造最小可解析 WAV（0.1 秒静音 PCM），供 lofty 读写标签。
    fn write_min_wav(path: &Path) {
        use std::io::Write;
        let channels: u16 = 2;
        let bits: u16 = 16;
        let sample_rate: u32 = 44_100;
        let byte_rate = sample_rate * channels as u32 * bits as u32 / 8;
        let num_samples = sample_rate / 10;
        let data_len = num_samples * channels as u32 * bits as u32 / 8;
        let mut bytes: Vec<u8> = Vec::with_capacity(44 + data_len as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        bytes.extend_from_slice(&(channels * bits / 8).to_le_bytes());
        bytes.extend_from_slice(&bits.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.extend(std::iter::repeat(0u8).take(data_len as usize));
        let mut f = std::fs::File::create(path).expect("create wav");
        f.write_all(&bytes).expect("write wav");
    }

    #[test]
    fn tag_write_roundtrip_for_wav() {
        let dir = temp_dir("wav-tag");
        let audio = dir.join("song.wav");
        write_min_wav(&audio);
        let meta = build_meta(
            "song.wav",
            "夜曲",
            &["周杰伦".to_string()],
            "十一月的萧邦",
            "2026-10-26 12:00:00",
        );
        let label = write_tags(&audio, &meta).expect("WAV 应支持写入 ID3v2 标签");
        assert_eq!(label, "ID3v2");

        let tagged = lofty::read_from_path(&audio).expect("写后应可重新读取");
        let tag = tagged.primary_tag().expect("应存在主标签");
        assert_eq!(tag.title().as_deref(), Some("夜曲"));
        assert_eq!(tag.artist().as_deref(), Some("周杰伦"));
        assert_eq!(tag.album().as_deref(), Some("十一月的萧邦"));

        // 写入后再由扫描器读取，应直接还原（模拟重新导入 / 重扫）
        let track = crate::library::scan::parse_track(&audio).expect("曲目应可入库");
        assert_eq!(track.title, "夜曲");
        assert_eq!(track.artists, vec!["周杰伦".to_string()]);
        assert_eq!(track.album, "十一月的萧邦");
    }

    #[test]
    fn scan_restores_sidecar_metadata_for_unreadable_format() {
        let dir = temp_dir("scan-sidecar");
        let audio = dir.join("song.dsf");
        std::fs::write(&audio, b"").unwrap();
        let meta = build_meta(
            "song.dsf",
            "夜曲",
            &["周杰伦".to_string()],
            "十一月的萧邦",
            "2026-10-26 12:00:00",
        );
        write_sidecar(&audio, &meta).unwrap();

        // dsf 无法被 lofty 解析 → 走附属文件还原分支
        let track = crate::library::scan::parse_track(&audio).expect("曲目应可入库");
        assert_eq!(track.title, "夜曲");
        assert_eq!(track.artists, vec!["周杰伦".to_string()]);
        assert_eq!(track.album, "十一月的萧邦");
    }
}
