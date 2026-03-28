use std::path::Path;
use serde_json;

/// 从title中解析时间信息
/// 格式: 标题::开始时间::结束时间
#[allow(dead_code)]
fn parse_time_from_title(title: &str) -> Option<(String, Option<String>, Option<String>)> {
    let parts: Vec<&str> = title.split("::").collect();
    if parts.len() >= 3 {
        Some((
            parts[0].to_string(),
            Some(parts[1].to_string()),
            if !parts[2].is_empty() { Some(parts[2].to_string()) } else { None }
        ))
    } else {
        None
    }
}

// 扫描CUE文件命令
#[tauri::command]
pub async fn scan_cue_files(directory: String) -> Result<serde_json::Value, String> {
    use crate::cue_parser::{scan_cue_files, parse_cue_file, CueTrackSong};

    let path = Path::new(&directory);
    let cue_file_paths = scan_cue_files(path);

    let mut all_tracks = Vec::new();
    let mut albums = Vec::new();

    for cue_path in cue_file_paths {
        match parse_cue_file(&cue_path) {
            Ok(album) => {
                let album_info = serde_json::json!({
                    "title": album.title.clone(),
                    "performer": album.performer.clone(),
                    "file_path": album.file_path.to_string_lossy().to_string(),
                    "file_type": album.file_type.clone(),
                    "track_count": album.tracks.len(),
                });
                albums.push(album_info);

                for track in &album.tracks {
                    let track_song = CueTrackSong::from_cue_track(
                        track,
                        &album,
                        &album.file_path.to_string_lossy()
                    );
                    all_tracks.push(serde_json::to_value(track_song).unwrap());
                }
            }
            Err(e) => {
                eprintln!("解析CUE文件失败 {}: {}", cue_path.display(), e);
            }
        }
    }

    Ok(serde_json::json!({
        "albums": albums,
        "tracks": all_tracks,
        "count": all_tracks.len(),
    }))
}

// 解析单个CUE文件
#[tauri::command]
pub async fn parse_cue_file_command(cue_path: String) -> Result<serde_json::Value, String> {
    use crate::cue_parser::parse_cue_file;

    let path = Path::new(&cue_path);
    match parse_cue_file(path) {
        Ok(album) => {
            Ok(serde_json::json!({
                "title": album.title,
                "performer": album.performer,
                "file_path": album.file_path.to_string_lossy().to_string(),
                "file_type": album.file_type,
                "tracks": album.tracks.iter().map(|t| {
                    let title_with_times = format!(
                        "{}::{}::{}",
                        if t.title.is_empty() { format!("Track {}", t.number) } else { t.title.clone() },
                        t.start_time.as_secs(),
                        t.end_time.map(|d| d.as_secs().to_string()).unwrap_or_default()
                    );
                    serde_json::json!({
                        "number": t.number.to_string(),
                        "title": title_with_times,
                        "performer": t.performer,
                        "start_time": t.start_time.as_secs().to_string(),
                        "end_time": t.end_time.map(|d| d.as_secs().to_string()),
                    })
                }).collect::<Vec<_>>(),
            }))
        }
        Err(e) => Err(e),
    }
}
