//! 曲库扫描：遍历目录 + lofty 元数据解析

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;

use super::models::{album_id_of, ScanProgress, ScanStage, Track};

/// 扫描支持的扩展名（索引范围 ≥ 播放范围：dsd/dts/ape 等可入库展示，
/// 播放时若引擎无法解码会以明确错误事件反馈）
const SUPPORTED_EXT: &[&str] = &[
    // 可直接解码播放（rodio/symphonia）
    "mp3", "flac", "wav", "ogg", "oga", "opus", "aac", "m4a",
    // 可索引（lofty 读取标签或回退文件名），播放需后续解码后端
    "wma", "aif", "aiff", "ape", "wv", "tta", "tak", "dsf", "dff", "dts",
];

/// 增量扫描结果
pub struct ScanResult {
    /// 新增 / 文件已变更（mtime 变化）的曲目
    pub upserts: Vec<Track>,
    /// 磁盘上已不存在、需从库中移除的曲目 id
    pub removed_ids: Vec<String>,
    /// 本次实际重新解析的文件数（未变文件直接跳过）
    pub changed_count: usize,
}

/// 递归发现目录下所有音频文件
fn discover_audio_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if SUPPORTED_EXT.contains(&ext.to_lowercase().as_str()) {
                    files.push(path);
                }
            }
        }
    }
    files
}

/// 生成稳定 ID（基于文件路径的 SHA-256 前 16 位 hex）
fn track_id_from_path(path: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    let result = hasher.finalize();
    result.iter().take(8).map(|b| format!("{:02x}", b)).collect()
}

/// 解析单个音频文件的元数据
fn parse_track(path: &Path) -> Option<Track> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let file_size = metadata.len();
    let format = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    let id = track_id_from_path(path);

    // 用 lofty 读取标签
    let (title, artists, album, track_number, year, mut duration, bitrate, sample_rate, has_cover) =
        match lofty::read_from_path(path) {
            Ok(file) => {
                let tag = file.primary_tag().cloned();
                let props = file.properties();
                let title = tag
                    .as_ref()
                    .and_then(|t| t.title().map(|s| s.to_string()))
                    .unwrap_or_else(|| {
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "未知标题".to_string())
                    });
                let artists = tag
                    .as_ref()
                    .and_then(|t| t.artist().map(|a| split_artists(a.as_ref())))
                    .unwrap_or_else(|| vec!["未知艺术家".to_string()]);
                let album = tag
                    .as_ref()
                    .and_then(|t| t.album().map(|s| s.to_string()))
                    .unwrap_or_else(|| "未知专辑".to_string());
                let track_number = tag.as_ref().and_then(|t| t.track());
                let year = tag.as_ref().and_then(|t| t.year());
                let duration = props.duration().as_secs_f64();
                let bitrate = props.audio_bitrate();
                let sample_rate = props.sample_rate();
                let has_cover = tag.as_ref().map(|t| !t.pictures().is_empty()).unwrap_or(false);
                (
                    title,
                    artists,
                    album,
                    track_number,
                    year,
                    duration,
                    bitrate,
                    sample_rate,
                    has_cover,
                )
            }
            Err(_) => (
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "未知标题".to_string()),
                vec!["未知艺术家".to_string()],
                "未知专辑".to_string(),
                None,
                None,
                0.0,
                None,
                None,
                false,
            ),
        };

    // 时长兜底：lofty 无法解析的格式（dsf/dff/dts/tta 等）用 ffprobe 探测，
    // 否则 duration=0 会导致播放进度不推进、自动连播失效
    if duration <= 0.0 {
        if let Some(probed) = crate::player::engine::probe_duration(path) {
            duration = probed;
        }
    }

    // 专辑稳定 ID 与 Album 聚合同规则（主艺术家 = artists[0]）
    let first_artist = artists.first().map(String::as_str).unwrap_or("");
    let album_id = Some(album_id_of(&album, first_artist));

    Some(Track {
        id,
        title,
        artists,
        album,
        album_id,
        duration,
        track_number,
        year,
        file_path: path.to_string_lossy().to_string(),
        file_size,
        format,
        bitrate,
        sample_rate,
        has_cover,
        added_at: chrono::Utc::now().timestamp(),
        modified_at,
    })
}

fn split_artists(raw: &str) -> Vec<String> {
    raw.split(['/', ';', ','])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// 读取文件 mtime（秒）；无法读取时返回 0（退化为总是重新解析）
fn file_mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

///
/// 执行增量扫描并推送进度事件。
///
/// `existing` 为库内 (id, file_path, modified_at) 基线：
///  - mtime 未变的文件跳过元数据解析（大曲库二次扫描几乎零成本）；
///  - 新增 / 变更文件重新解析进 upserts；
///  - 基线中磁盘已消失的文件进 removed_ids。
pub fn scan_directory(
    roots: &[PathBuf],
    existing: &[(String, String, i64)],
    app: &AppHandle,
) -> ScanResult {
    let mut progress = ScanProgress {
        stage: ScanStage::Discovering,
        processed: 0,
        total: 0,
        current_path: None,
        error: None,
    };
    emit(app, &progress);

    // 基线索引：path → (id, mtime)
    let existing_by_path: std::collections::HashMap<String, (String, i64)> = existing
        .iter()
        .map(|(id, path, mtime)| (path.clone(), (id.clone(), *mtime)))
        .collect();

    // 发现阶段
    let mut all_files = Vec::new();
    for root in roots {
        all_files.extend(discover_audio_files(root));
    }
    progress.total = all_files.len() as u32;
    progress.stage = ScanStage::Parsing;
    emit(app, &progress);

    // 解析阶段（仅新增 / 变更文件）
    let mut upserts = Vec::new();
    let mut seen_paths = std::collections::HashSet::with_capacity(all_files.len());
    for (i, path) in all_files.iter().enumerate() {
        let path_str = path.to_string_lossy().to_string();
        seen_paths.insert(path_str.clone());
        progress.current_path = Some(path_str.clone());

        let mtime = file_mtime(path);
        let unchanged = existing_by_path
            .get(&path_str)
            .is_some_and(|(_, old_mtime)| *old_mtime == mtime && mtime != 0);
        if !unchanged {
            if let Some(track) = parse_track(path) {
                upserts.push(track);
            }
        }

        progress.processed = (i + 1) as u32;
        // 每 20 个推送一次进度，避免事件洪泛
        if i % 20 == 0 {
            emit(app, &progress);
        }
    }

    // 删除检测：基线中存在、本轮未在磁盘发现的路径
    let mut removed_ids: Vec<String> = existing
        .iter()
        .filter(|(_, path, _)| !seen_paths.contains(path))
        .map(|(id, _, _)| id.clone())
        .collect();
    removed_ids.sort();

    let changed_count = upserts.len();
    progress.stage = ScanStage::Writing;
    emit(app, &progress);

    progress.stage = ScanStage::Done;
    progress.current_path = None;
    emit(app, &progress);

    ScanResult {
        upserts,
        removed_ids,
        changed_count,
    }
}

fn emit(app: &AppHandle, progress: &ScanProgress) {
    let _ = app.emit("library://scan-progress", progress);
}
