use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// CUE文件中的Track信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CueTrack {
    pub number: u32,
    pub title: String,
    pub performer: String,
    pub start_time: Duration,
    pub end_time: Option<Duration>,
    pub index: Vec<(u32, Duration)>, // (index_number, time)
}

/// CUE专辑信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CueAlbum {
    pub title: String,
    pub performer: String,
    pub file_path: PathBuf,
    pub file_type: String,
    pub tracks: Vec<CueTrack>,
    pub total_duration: Option<Duration>,
}

/// 解析时间字符串 (MM:SS:FF 或 MM:SS)
fn parse_time(time_str: &str) -> Option<Duration> {
    let parts: Vec<&str> = time_str.split(':').collect();
    
    match parts.len() {
        2 => {
            // MM:SS格式
            let minutes: u64 = parts[0].parse().ok()?;
            let seconds: u64 = parts[1].parse().ok()?;
            Some(Duration::from_secs(minutes * 60 + seconds))
        }
        3 => {
            // MM:SS:FF格式 (FF是帧，75帧=1秒)
            let minutes: u64 = parts[0].parse().ok()?;
            let seconds: u64 = parts[1].parse().ok()?;
            let frames: u64 = parts[2].parse().ok()?;
            let total_seconds = minutes * 60 + seconds + frames / 75;
            Some(Duration::from_secs(total_seconds))
        }
        _ => None,
    }
}

/// 格式化时间为字符串 MM:SS
fn format_time(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}", minutes, seconds)
}

/// 解析CUE文件
pub fn parse_cue_file(cue_path: &Path) -> Result<CueAlbum, String> {
    let content = fs::read_to_string(cue_path)
        .map_err(|e| format!("读取CUE文件失败: {}", e))?;
    
    let mut album = CueAlbum {
        title: String::new(),
        performer: String::new(),
        file_path: PathBuf::new(),
        file_type: String::new(),
        tracks: Vec::new(),
        total_duration: None,
    };
    
    let mut current_track: Option<CueTrack> = None;
    let cue_dir = cue_path.parent().unwrap_or(Path::new(""));
    
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("REM") {
            continue;
        }
        
        // 解析专辑标题
        if line.starts_with("TITLE ") {
            let title = extract_quoted_value(line, "TITLE");
            if album.title.is_empty() && current_track.is_none() {
                album.title = title;
            } else if let Some(ref mut track) = current_track {
                track.title = title;
            }
        }
        
        // 解析表演者
        if line.starts_with("PERFORMER ") {
            let performer = extract_quoted_value(line, "PERFORMER");
            if album.performer.is_empty() && current_track.is_none() {
                album.performer = performer;
            } else if let Some(ref mut track) = current_track {
                track.performer = performer;
            }
        }
        
        // 解析音频文件
        if line.starts_with("FILE ") {
            // 保存之前的track
            if let Some(track) = current_track.take() {
                album.tracks.push(track);
            }
            
            let (file_name, file_type) = parse_file_line(line);
            album.file_path = cue_dir.join(&file_name);
            album.file_type = file_type;
        }
        
        // 解析Track
        if line.starts_with("TRACK ") {
            // 保存之前的track
            if let Some(track) = current_track.take() {
                album.tracks.push(track);
            }
            
            let track_num = extract_track_number(line);
            current_track = Some(CueTrack {
                number: track_num,
                title: String::new(),
                performer: String::new(),
                start_time: Duration::ZERO,
                end_time: None,
                index: Vec::new(),
            });
        }
        
        // 解析INDEX
        if line.starts_with("INDEX ") && current_track.is_some() {
            if let Some((index_num, time)) = parse_index_line(line) {
                if let Some(ref mut track) = current_track {
                    track.index.push((index_num, time));
                }
            }
        }
    }
    
    // 保存最后一个track
    if let Some(track) = current_track {
        album.tracks.push(track);
    }
    
    // 计算每个track的结束时间
    calculate_track_times(&mut album);
    
    Ok(album)
}

/// 从引号中提取值
fn extract_quoted_value(line: &str, key: &str) -> String {
    let start = line.find('"').unwrap_or(0) + 1;
    let end = line.rfind('"').unwrap_or(line.len());
    if start < end {
        line[start..end].to_string()
    } else {
        line[key.len()..].trim().to_string()
    }
}

/// 解析FILE行
fn parse_file_line(line: &str) -> (String, String) {
    // FILE "filename" WAVE
    let start = line.find('"').unwrap_or(0) + 1;
    let end = line.rfind('"').unwrap_or(line.len());
    let file_name = if start < end {
        line[start..end].to_string()
    } else {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            parts[1].to_string()
        } else {
            String::new()
        }
    };
    
    let file_type = if line.contains("WAVE") {
        "WAVE".to_string()
    } else if line.contains("MP3") {
        "MP3".to_string()
    } else if line.contains("FLAC") {
        "FLAC".to_string()
    } else {
        "WAVE".to_string()
    };
    
    (file_name, file_type)
}

/// 提取track编号
fn extract_track_number(line: &str) -> u32 {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1].parse().unwrap_or(0)
    } else {
        0
    }
}

/// 解析INDEX行
fn parse_index_line(line: &str) -> Option<(u32, Duration)> {
    // INDEX 01 00:00:00
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 3 {
        let index_num: u32 = parts[1].parse().ok()?;
        let time = parse_time(parts[2])?;
        Some((index_num, time))
    } else {
        None
    }
}

/// 计算每个track的开始和结束时间
/// 规则：
/// - 开始时间：使用 INDEX 01（音轨实际开始时间）
/// - 结束时间：使用下一个track的 INDEX 01（音轨实际结束时间）
/// - INDEX 00 是预间隙开始时间，不用于计算音轨时长
fn calculate_track_times(album: &mut CueAlbum) {
    let track_count = album.tracks.len();
    if track_count == 0 {
        return;
    }
    
    // 第一步：收集所有track的Index 01时间（音轨实际开始时间）
    let mut index_01_times: Vec<Option<Duration>> = Vec::new();
    
    for track in &album.tracks {
        let mut index_01 = None;
        
        for (index_num, time) in &track.index {
            if *index_num == 1 {
                index_01 = Some(*time);
                break; // 找到 INDEX 01 就退出
            }
        }
        
        index_01_times.push(index_01);
    }
    
    // 第二步：计算每个track的开始和结束时间
    for i in 0..track_count {
        let track = &mut album.tracks[i];
        
        // 开始时间：使用 INDEX 01，如果没有则使用 0
        let start_time = index_01_times[i].unwrap_or(Duration::ZERO);
        
        // 结束时间：使用下一个track的 INDEX 01
        let end_time = if i < track_count - 1 {
            // 使用下一个track的 INDEX 01 作为当前track的结束时间
            index_01_times[i + 1]
        } else {
            // 最后一个track，使用专辑总时长
            album.total_duration
        };
        
        track.start_time = start_time;
        track.end_time = end_time;
        
        // 调试输出
        let duration_secs = end_time.map(|e| e.as_secs()).unwrap_or(start_time.as_secs()) - start_time.as_secs();
        println!(
            "[CUE解析] Track {}: INDEX 01={}s, end_time={}s, duration={}s",
            track.number,
            start_time.as_secs(),
            end_time.map(|d| d.as_secs()).unwrap_or(0),
            duration_secs
        );
    }
}

/// 获取音频文件时长
#[allow(dead_code)]
pub fn get_audio_duration(_path: &Path) -> Option<Duration> {
    // 这里需要使用 lofty 或其他库来获取音频文件时长
    // 暂时返回 None，实际实现时需要集成到项目中
    None
}

/// 扫描目录查找CUE文件
pub fn scan_cue_files(directory: &Path) -> Vec<PathBuf> {
    let mut cue_files = Vec::new();
    
    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            
            if path.is_dir() {
                // 递归扫描子目录
                cue_files.extend(scan_cue_files(&path));
            } else if let Some(ext) = path.extension() {
                if ext.to_string_lossy().to_lowercase() == "cue" {
                    cue_files.push(path);
                }
            }
        }
    }
    
    cue_files
}

/// 将CueTrack转换为Song结构（用于前端显示）
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct CueTrackSong {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub path: String,
    pub duration: String,
    #[serde(rename = "startTime")]
    pub start_time: String, // 开始时间（秒，文本格式）
    #[serde(rename = "endTime")]
    pub end_time: Option<String>, // 结束时间（秒，文本格式）
    #[serde(rename = "trackNumber")]
    pub track_number: String,
    #[serde(rename = "isCueTrack")]
    pub is_cue_track: bool,
    #[serde(rename = "parentFile")]
    pub parent_file: String,
    // 额外的CUE信息字段，用于标签编辑
    #[serde(rename = "cueInfo")]
    pub cue_info: String,
    // 转码相关
    pub needs_transcode: bool,
}

impl CueTrackSong {
    pub fn from_cue_track(track: &CueTrack, album: &CueAlbum, parent_file: &str) -> Self {
        let duration_str = match track.end_time {
            Some(end) => {
                let duration = end - track.start_time;
                format_time(duration)
            }
            None => "未知".to_string(),
        };

        // 生成CUE信息字符串，包含音轨号、开始时间和结束时间
        let cue_info = format!(
            "Track No.: {}\nStart Time: {}s\nEnd Time: {}s",
            track.number,
            track.start_time.as_secs(),
            track.end_time.map(|d| d.as_secs().to_string()).unwrap_or("未知".to_string())
        );

        // 构建标题，不包含时间信息
        let title = if track.title.is_empty() {
            format!("Track {}", track.number)
        } else {
            track.title.clone()
        };

        // 判断父文件是否需要转码
        let needs_transcode = crate::ffmpeg_transcoder::needs_transcode(parent_file);

        Self {
            id: format!("{}_{}", parent_file, track.number),
            title: title,
            artist: if track.performer.is_empty() {
                album.performer.clone()
            } else {
                track.performer.clone()
            },
            album: album.title.clone(),
            path: parent_file.to_string(),
            duration: duration_str,
            start_time: track.start_time.as_secs().to_string(),
            end_time: track.end_time.map(|d| d.as_secs().to_string()),
            track_number: track.number.to_string(),
            is_cue_track: true,
            parent_file: parent_file.to_string(),
            cue_info: cue_info,
            needs_transcode: needs_transcode,
        }
    }
}

/// 从title中解析时间信息
/// 格式: 标题::开始时间::结束时间
#[allow(dead_code)]
pub fn parse_time_from_title(title: &str) -> Option<(String, Option<String>, Option<String>)> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time() {
        assert_eq!(parse_time("01:30"), Some(Duration::from_secs(90)));
        assert_eq!(parse_time("02:45:50"), Some(Duration::from_secs(165)));
        assert_eq!(parse_time("00:00"), Some(Duration::from_secs(0)));
    }

    #[test]
    fn test_extract_quoted_value() {
        let line = r#"TITLE "Test Album""#;
        assert_eq!(extract_quoted_value(line, "TITLE"), "Test Album");
    }

    #[test]
    fn test_parse_file_line() {
        let line = r#"FILE "test.wav" WAVE"#;
        let (name, ftype) = parse_file_line(line);
        assert_eq!(name, "test.wav");
        assert_eq!(ftype, "WAVE");
    }
}
