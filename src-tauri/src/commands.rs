#![allow(static_mut_refs)]

use std::fs::{self, DirEntry};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use base64::{Engine as _, engine::general_purpose};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{Accessor, ItemKey};
use rodio::Source;
use tauri::{State, Emitter};

use crate::ffmpeg_transcoder;
use crate::equalizer::{Equalizer, EqualizerPreset};

// 全局播放器(使用unsafe static,但通过PLAYER_MUTEX确保线程安全)
#[allow(static_mut_refs)]
static mut GLOBAL_PLAYER: Option<GlobalPlayer> = None;
static PLAYER_MUTEX: Mutex<()> = Mutex::new(());

// 进度更新线程句柄
static PROGRESS_THREAD_HANDLE: Mutex<Option<thread::JoinHandle<()>>> = Mutex::new(None);

// 音频文件扩展名
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "aac", "ogg", "m4a", "ape", "dsd", "dsf", "dff", "dts", "wma", "opus"
];

// 歌曲结构体
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Song {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub path: String,
    pub duration: String,
    pub cover: String,
    pub year: String,
    pub genre: String,
    pub lyric: String,
    // 音频参数
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub bit_depth: Option<u8>,
    // 转码相关
    pub needs_transcode: bool,
}

// 播放器状态(只存储简单的状态,不包含rodio对象)
pub struct PlayerState {
    pub current_song: Option<Song>,
    pub is_playing: bool,
    pub volume: f32,
    pub position: f64,
    pub position_update_count: u64, // 用于跟踪更新次数
    pub equalizer_bands: [f32; 10],
    pub equalizer: Equalizer,
    // CUE track相关
    pub cue_start_time: Option<f64>, // CUE track开始时间（秒）
    pub cue_end_time: Option<f64>,   // CUE track结束时间（秒）
}

// 全局播放器句柄(使用unsafe存储,不实现Send)
pub struct GlobalPlayer {
    pub sink: Option<rodio::Sink>,
    pub stream_handle: Option<rodio::OutputStreamHandle>,
    pub _stream: Option<rodio::OutputStream>,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            current_song: None,
            is_playing: false,
            volume: 0.8,
            position: 0.0,
            position_update_count: 0,
            equalizer_bands: [0.0; 10],
            equalizer: Equalizer::new(),
            cue_start_time: None,
            cue_end_time: None,
        }
    }
}

impl Default for GlobalPlayer {
    fn default() -> Self {
        Self {
            sink: None,
            stream_handle: None,
            _stream: None,
        }
    }
}

// 扫描目录，查找音频文件
#[tauri::command]
pub async fn scan_directory(directory: String) -> Result<serde_json::Value, String> {
    use crate::cue_parser::{scan_cue_files, parse_cue_file};
    
    let path = Path::new(&directory);
    let mut songs = Vec::new();
    
    // 先扫描所有CUE文件，收集被引用的音频文件路径
    let cue_file_paths = scan_cue_files(path);
    let mut cue_referenced_files = std::collections::HashSet::new();
    
    #[cfg(debug_assertions)]
    println!("[CUE过滤] 扫描到 {} 个CUE文件", cue_file_paths.len());
    for cue_path in &cue_file_paths {
        #[cfg(debug_assertions)]
        println!("[CUE过滤] CUE文件路径: {:?}", cue_path);
    }
    
    for cue_path in cue_file_paths {
        #[cfg(debug_assertions)]
        println!("[CUE过滤] 解析CUE文件: {:?}", cue_path);
        if let Ok(album) = parse_cue_file(&cue_path) {
            let file_path = album.file_path;
            #[cfg(debug_assertions)]
            println!("[CUE过滤] CUE引用的音频文件路径: {:?}", file_path);
            #[cfg(debug_assertions)]
            println!("[CUE过滤] 文件是否存在: {}", file_path.exists());
            // 无论文件是否存在，都添加到CUE引用文件集合中
            // 这样可以确保即使文件不存在，也不会在播放列表中显示
            let canonical_path = file_path.canonicalize().unwrap_or(file_path.clone());
            #[cfg(debug_assertions)]
            println!("[CUE过滤] 规范化后的路径: {:?}", canonical_path);
            cue_referenced_files.insert(canonical_path);
            #[cfg(debug_assertions)]
            println!("[CUE过滤] 添加到CUE引用文件集合");
        } else {
            #[cfg(debug_assertions)]
            println!("[CUE过滤] 解析CUE文件失败");
        }
    }
    
    #[cfg(debug_assertions)]
    println!("[CUE过滤] CUE引用文件集合大小: {}", cue_referenced_files.len());
    for path in &cue_referenced_files {
        #[cfg(debug_assertions)]
        println!("[CUE过滤] CUE引用文件: {:?}", path);
    }
    
    if let Err(err) = scan_directory_recursive(Path::new(&directory), &mut songs, &cue_referenced_files) {
        return Err(format!("扫描目录失败: {}", err));
    }
    
    let result = serde_json::json!({
        "tracks": songs
    });
    
    Ok(result)
}

// 递归扫描目录
fn scan_directory_recursive(
    path: &Path, 
    songs: &mut Vec<Song>,
    cue_referenced_files: &std::collections::HashSet<std::path::PathBuf>
) -> std::io::Result<()> {
    if path.is_dir() {
        println!("[CUE过滤] 扫描目录: {:?}", path);
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                scan_directory_recursive(&path, songs, cue_referenced_files)?;
            } else if is_audio_file(&entry) {
                println!("[CUE过滤] 发现音频文件: {:?}", path);
                // 检查文件是否被CUE引用
                let canonical_path = path.canonicalize().unwrap_or(path.clone());
                println!("[CUE过滤] 规范化后的路径: {:?}", canonical_path);
                if !cue_referenced_files.contains(&canonical_path) {
                    println!("[CUE过滤] 文件未被CUE引用，添加到播放列表");
                    if let Some(song) = parse_audio_file(&path) {
                        songs.push(song);
                    }
                } else {
                    println!("[CUE过滤] 文件被CUE引用，跳过");
                }
            }
        }
    }
    Ok(())
}

// 检查是否为音频文件
fn is_audio_file(entry: &DirEntry) -> bool {
    if let Some(ext) = entry.path().extension() {
        if let Some(ext_str) = ext.to_str() {
            let ext_lower = ext_str.to_lowercase();
            let is_audio = AUDIO_EXTENSIONS.contains(&ext_lower.as_str());

            // 调试: 打印所有音频文件的扩展名
            if is_audio {
                println!("发现音频文件: {:?}, 扩展名: {}", entry.path(), ext_lower);
            }

            return is_audio;
        }
    }
    false
}

// 使用ffprobe获取音频文件时长
fn get_duration_with_ffprobe(path: &Path) -> Option<f64> {
    use std::process::{Command, Stdio};
    
    // 尝试获取ffprobe路径
    let ffprobe_path = if let Some(path) = ffmpeg_transcoder::TranscodeCache::get_ffprobe_path() {
        path
    } else {
        println!("[FFprobe] 未找到ffprobe，无法获取音频时长");
        return None;
    };
    
    let path_str = path.to_str()?;
    println!("[FFprobe] 开始获取音频时长: {}", path_str);
    
    let mut cmd = Command::new(&ffprobe_path);
    cmd.arg("-hide_banner")
        .arg(path_str)
        .arg("-show_streams")
        .arg("-select_streams")
        .arg("a")
        .arg("-print_format")
        .arg("json")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    
    // 在Windows系统上设置CREATE_NO_WINDOW标志，隐藏控制台窗口
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    
    let result = cmd.output().ok()?;
    
    if !result.status.success() {
        println!("[FFprobe] 执行失败，退出码: {:?}", result.status.code());
        return None;
    }
    
    let stdout_str = String::from_utf8_lossy(&result.stdout);
    
    if stdout_str.is_empty() {
        println!("[FFprobe] 输出为空");
        return None;
    }
    
    let json: serde_json::Value = serde_json::from_str(&stdout_str).ok()?;
    
    if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
        if let Some(first_stream) = streams.first() {
            if let Some(duration_str) = first_stream.get("duration").and_then(|d| d.as_str()) {
                if let Ok(duration) = duration_str.parse::<f64>() {
                    println!("[FFprobe] 成功获取时长: {} 秒", duration);
                    return Some(duration);
                }
            }
        }
    }
    
    println!("[FFprobe] 无法从ffprobe输出中获取时长");
    None
}

// 解析音频文件元数据
fn parse_audio_file(path: &Path) -> Option<Song> {
    let path_str = path.to_str()?.to_string();

    println!("正在解析音频文件: {}", path_str);

    // 默认值
    let mut title = "".to_string();
    let mut artist = "未知艺术家".to_string();
    let mut album = "未知专辑".to_string();
    let mut year = "".to_string();
    let mut genre = "".to_string();
    let mut duration = 0.0;

    // 先从文件名解析信息（作为后备）
    if let Some(file_name) = path.file_name() {
        if let Some(name_str) = file_name.to_str() {
            // 移除扩展名
            let filename_without_ext = if let Some(pos) = name_str.rfind('.') {
                name_str[..pos].to_string()
            } else {
                name_str.to_string()
            };

            // 尝试从文件名解析更多信息
            // 格式可能是: "艺术家 - 专辑 - 曲目.mp3" 或 "艺术家 - 曲目.mp3"
            let parts: Vec<&str> = filename_without_ext
                .split(" - ")
                .map(|s| s.trim())
                .collect();

            if parts.len() >= 2 {
                // 格式: "艺术家 - 曲目"
                if parts.len() == 2 {
                    artist = parts[0].to_string();
                    title = parts[1].to_string();
                }
                // 格式: "艺术家 - 专辑 - 曲目"
                else if parts.len() == 3 {
                    artist = parts[0].to_string();
                    album = parts[1].to_string();
                    title = parts[2].to_string();
                }
            } else {
                // 没有分隔符，使用完整文件名作为标题
                title = filename_without_ext;
            }
        }
    }

    // 尝试使用lofty获取更详细的信息
    if let Ok(tagged_file) = lofty::read_from_path(path) {
        // 获取属性
        let properties = tagged_file.properties();
        duration = properties.duration().as_secs_f64();

        // 尝试获取标签信息 - 优先使用primary_tag
        let mut got_metadata = false;

        if let Some(tags) = tagged_file.primary_tag() {
            got_metadata = true;
            if let Some(t) = tags.title() {
                if !t.is_empty() {
                    title = t.to_string();
                }
            }
            if let Some(a) = tags.artist() {
                if !a.is_empty() {
                    artist = a.to_string();
                }
            }
            if let Some(a) = tags.album() {
                if !a.is_empty() {
                    album = a.to_string();
                }
            }
            if let Some(y) = tags.year() {
                year = y.to_string();
            }
            if let Some(g) = tags.genre() {
                if !g.is_empty() {
                    genre = g.to_string();
                }
            }
        }

        // 如果primary_tag没有提供足够信息,尝试其他标签
        if !got_metadata || title.is_empty() || artist == "未知艺术家" || album == "未知专辑" {
            for tag in tagged_file.tags() {
                // 标题:只有当当前标题是文件名时才覆盖
                if tag.title().is_some() {
                    let t = tag.title().unwrap();
                    if !t.is_empty() && (title.is_empty() || title == path.file_name()?.to_str()?) {
                        title = t.to_string();
                    }
                }
                // 艺术家
                if tag.artist().is_some() && artist == "未知艺术家" {
                    let a = tag.artist().unwrap();
                    if !a.is_empty() {
                        artist = a.to_string();
                    }
                }
                // 专辑
                if tag.album().is_some() && album == "未知专辑" {
                    let a = tag.album().unwrap();
                    if !a.is_empty() {
                        album = a.to_string();
                    }
                }
                // 年份
                if tag.year().is_some() && year.is_empty() {
                    year = tag.year().unwrap().to_string();
                }
                // 流派
                if tag.genre().is_some() && genre.is_empty() {
                    let g = tag.genre().unwrap();
                    if !g.is_empty() {
                        genre = g.to_string();
                    }
                }
            }
        }
    } else {
        eprintln!("警告: {} 无法读取元数据", path_str);
    }
    
    // 尝试使用rodio获取音频参数
    let mut sample_rate: Option<u32> = None;
    let mut channels: Option<u16> = None;
    let mut bit_depth: Option<u8> = None;
    
    if let Ok(file) = std::fs::File::open(path) {
        if let Ok(decoder) = rodio::Decoder::new(file) {
            sample_rate = Some(decoder.sample_rate());
            channels = Some(decoder.channels());
            // rodio不直接提供位深度信息，这里设置为默认值
            bit_depth = Some(16);
        }
    }

    // 检查是否为特殊格式（DSD、DTS等无法获取时长的格式）
    let is_special_format = if let Some(ext) = path.extension() {
        if let Some(ext_str) = ext.to_str() {
            let ext_lower = ext_str.to_lowercase();
            matches!(ext_lower.as_str(), "dsf" | "dff" | "dsd" | "dts" | "ape")
        } else {
            false
        }
    } else {
        false
    };

    // 如果duration为0或为特殊格式，尝试使用ffprobe获取时长
    if duration <= 0.0 || is_special_format {
        if let Some(ffprobe_duration) = get_duration_with_ffprobe(path) {
            println!("[FFprobe] 成功获取时长: {} 秒", ffprobe_duration);
            duration = ffprobe_duration;
        } else {
            if is_special_format {
                duration = -1.0; // 特殊标记：时长未知
            } else {
                duration = 180.0; // 默认3分钟
            }
        }
    }

    // 清理艺术家和专辑,如果为空或为"Unknown"则使用默认值
    let empty_values = ["", "Unknown", "unknown", "NONE", "none"];
    if empty_values.contains(&artist.as_str()) {
        artist = "未知艺术家".to_string();
    }
    if empty_values.contains(&album.as_str()) {
        album = "未知专辑".to_string();
    }

    // 确保标题不为空
    if title.is_empty() {
        if let Some(file_name) = path.file_name() {
            if let Some(name_str) = file_name.to_str() {
                if let Some(pos) = name_str.rfind('.') {
                    title = name_str[..pos].to_string();
                } else {
                    title = name_str.to_string();
                }
            }
        }
    }

    // 尝试查找并解析歌词文件
    let mut lyric = String::new();
    println!("【后端歌词扫描】开始扫描歌词文件，歌曲路径: {:?}", path);
    if let Some(parent) = path.parent() {
        // 尝试同名的 .lrc 文件
        if let Some(file_stem) = path.file_stem() {
            if let Some(stem_str) = file_stem.to_str() {
                let lrc_path = parent.join(format!("{}.lrc", stem_str));
                println!("【后端歌词扫描】尝试读取歌词文件: {:?}", lrc_path);
                if lrc_path.exists() {
                    println!("【后端歌词扫描】歌词文件存在: {:?}", lrc_path);
                    match std::fs::read_to_string(&lrc_path) {
                        Ok(content) => {
                            lyric = content;
                            println!("【后端歌词扫描】成功读取歌词文件，长度: {}", lyric.len());
                        }
                        Err(e) => {
                            println!("【后端歌词扫描】读取歌词文件失败: {}", e);
                        }
                    }
                } else {
                    println!("【后端歌词扫描】歌词文件不存在: {:?}", lrc_path);
                }
            }
        }
    } else {
        println!("【后端歌词扫描】无法获取父目录");
    }

    // 如果没有找到外部歌词文件，尝试从音频文件读取内嵌歌词
    if lyric.is_empty() {
        println!("【后端歌词扫描】未找到外部歌词文件，尝试读取内嵌歌词");
        match lofty::read_from_path(&path) {
            Ok(tagged_file) => {
                println!("【后端歌词扫描】成功读取音频文件元数据");
                // 尝试从 primary_tag 读取歌词
                if let Some(tag) = tagged_file.primary_tag() {
                    println!("【后端歌词扫描】primary_tag 存在");
                    // 使用 ItemKey::Lyrics 获取歌词
                    let lyrics = tag.get_strings(&ItemKey::Lyrics);
                    println!("【后端歌词扫描】get_strings 返回的歌词数量：{}", lyrics.len());
                    for lyric_text in lyrics {
                        if !lyric_text.is_empty() {
                            lyric = lyric_text.to_string();
                            println!("【后端歌词扫描】成功读取内嵌歌词 (Lyrics)，长度：{}", lyric.len());
                            println!("【后端歌词扫描】歌词预览：{}", lyric.chars().take(100).collect::<String>());
                            break;
                        }
                    }
                }
                
                // 如果 primary_tag 没有歌词，尝试所有标签
                if lyric.is_empty() {
                    println!("【后端歌词扫描】primary_tag 没有歌词，尝试所有标签");
                    for tag in tagged_file.tags() {
                        let lyrics = tag.get_strings(&ItemKey::Lyrics);
                        println!("【后端歌词扫描】标签中 get_strings 返回的歌词数量：{}", lyrics.len());
                        for lyric_text in lyrics {
                            if !lyric_text.is_empty() {
                                lyric = lyric_text.to_string();
                                println!("【后端歌词扫描】从标签读取内嵌歌词，长度：{}", lyric.len());
                                break;
                            }
                        }
                        if !lyric.is_empty() {
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                println!("【后端歌词扫描】读取音频文件元数据失败：{}", e);
            }
        }
    }
    
    // 最终确认歌词数据
    if !lyric.is_empty() {
        println!("【后端歌词扫描】最终歌词数据：长度={}, 预览={}", lyric.len(), lyric.chars().take(50).collect::<String>());
    } else {
        println!("【后端歌词扫描】最终结果：未找到歌词");
    }

    // 尝试查找本地封面图片
    let mut cover = String::new();
    println!("【后端封面扫描】开始扫描封面文件，歌曲路径: {:?}", path);
    if let Some(parent) = path.parent() {
        if let Some(file_stem) = path.file_stem() {
            if let Some(stem_str) = file_stem.to_str() {
                // 尝试多种常见的封面图片格式
                let cover_extensions = ["jpg", "jpeg", "png", "bmp", "webp"];
                for ext in &cover_extensions {
                    let cover_path = parent.join(format!("{}.{}", stem_str, ext));
                    println!("【后端封面扫描】尝试读取封面文件: {:?}", cover_path);
                    if cover_path.exists() {
                        println!("【后端封面扫描】封面文件存在: {:?}", cover_path);
                        // 读取图片文件并转换为 base64
                        match std::fs::read(&cover_path) {
                            Ok(image_data) => {
                                let base64_image = general_purpose::STANDARD.encode(&image_data);
                                // 根据文件扩展名确定 MIME 类型
                                let mime_type = match *ext {
                                    "png" => "image/png",
                                    "jpg" | "jpeg" => "image/jpeg",
                                    "bmp" => "image/bmp",
                                    "webp" => "image/webp",
                                    _ => "image/jpeg",
                                };
                                cover = format!("data:{};base64,{}", mime_type, base64_image);
                                println!("【后端封面扫描】成功读取封面文件，大小: {} 字节", image_data.len());
                                break; // 找到封面后停止搜索
                            }
                            Err(e) => {
                                println!("【后端封面扫描】读取封面文件失败: {}", e);
                            }
                        }
                    }
                }
                
                // 如果没有找到同名封面，尝试查找常见的封面文件名
                if cover.is_empty() {
                    let common_cover_names = ["cover", "folder", "album", "front"];
                    for name in &common_cover_names {
                        for ext in &cover_extensions {
                            let cover_path = parent.join(format!("{}.{}", name, ext));
                            if cover_path.exists() {
                                println!("【后端封面扫描】找到常见封面文件: {:?}", cover_path);
                                match std::fs::read(&cover_path) {
                                    Ok(image_data) => {
                                        let base64_image = general_purpose::STANDARD.encode(&image_data);
                                        let mime_type = match *ext {
                                            "png" => "image/png",
                                            "jpg" | "jpeg" => "image/jpeg",
                                            "bmp" => "image/bmp",
                                            "webp" => "image/webp",
                                            _ => "image/jpeg",
                                        };
                                        cover = format!("data:{};base64,{}", mime_type, base64_image);
                                        println!("【后端封面扫描】成功读取常见封面文件");
                                        break;
                                    }
                                    Err(e) => {
                                        println!("【后端封面扫描】读取常见封面文件失败: {}", e);
                                    }
                                }
                            }
                        }
                        if !cover.is_empty() {
                            break;
                        }
                    }
                }
            }
        }
    }
    
    if cover.is_empty() {
        println!("【后端封面扫描】未找到本地封面文件");
    }

    // 生成唯一ID
    let id = format!("{:x}", md5::compute(&path_str));

    // 判断文件是否需要转码
    let needs_transcode = ffmpeg_transcoder::needs_transcode(&path_str);

    Some(Song {
        id,
        title,
        artist,
        album,
        path: path_str,
        duration: format_duration(duration),
        cover,
        year,
        genre,
        lyric,
        sample_rate,
        channels,
        bit_depth,
        needs_transcode,
    })
}

// 格式化时长
fn format_duration(seconds: f64) -> String {
    if seconds < 0.0 {
        return "未知".to_string();
    }
    let mins = (seconds / 60.0) as u32;
    let secs = (seconds % 60.0) as u32;
    format!("{}:{:02}", mins, secs)
}

// 初始化音频流（不获取锁，由调用者确保线程安全）
fn initialize_audio_stream() -> Result<rodio::OutputStreamHandle, String> {
    unsafe {
        if GLOBAL_PLAYER.is_none() {
            let (stream, handle) = rodio::OutputStream::try_default()
                .map_err(|e| format!("无法获取音频设备: {}", e))?;
            
            GLOBAL_PLAYER = Some(GlobalPlayer {
                sink: None,
                stream_handle: Some(handle),
                _stream: Some(stream),
            });
        }
        
        if let Some(player) = &GLOBAL_PLAYER {
            player.stream_handle.clone()
                .ok_or_else(|| "音频流未初始化".to_string())
        } else {
            Err("全局播放器未初始化".to_string())
        }
    }
}

// 播放歌曲
#[tauri::command]
pub async fn play_song(
    path: String,
    volume: Option<f32>,
    force_transcode: Option<bool>,
    position: Option<f64>,
    start_time: Option<String>,  // CUE track开始时间（秒），字符串类型
    end_time: Option<String>,    // CUE track结束时间（秒），字符串类型
    track_number: Option<String>,  // CUE track编号
    title: Option<String>,     // CUE track标题
    state: State<'_, Arc<Mutex<PlayerState>>>,
    window: tauri::Window,
) -> Result<serde_json::Value, String> {
    // 如果start_time或end_time为空,尝试从title中解析
    let (start_time_val, end_time_val) = if start_time.is_none() || end_time.is_none() || start_time.as_deref() == Some(&"".to_string()) || end_time.as_deref() == Some(&"".to_string()) {
        // 从title中解析时间信息(格式: 标题::开始时间::结束时间)
        match title.as_deref() {
            Some(title_str) if !title_str.is_empty() => {
                let parts: Vec<&str> = title_str.split("::").collect();
                if parts.len() >= 3 {
                    let parsed_start = parts[1].parse::<f64>().ok();
                    let parsed_end = if !parts[2].is_empty() {
                        parts[2].parse::<f64>().ok()
                    } else {
                        None
                    };
                    println!("[播放] 从title解析时间: start={:?}, end={:?}", parsed_start, parsed_end);
                    (parsed_start, parsed_end)
                } else {
                    (start_time.clone().and_then(|s| s.parse().ok()), end_time.clone().and_then(|s| s.parse().ok()))
                }
            }
            _ => (start_time.clone().and_then(|s| s.parse().ok()), end_time.clone().and_then(|s| s.parse().ok()))
        }
    } else {
        (start_time.clone().and_then(|s| s.parse().ok()), end_time.clone().and_then(|s| s.parse().ok()))
    };

    // 记录所有接收到的参数
    println!("[播放] === 前端传递的参数 ===");
    println!("[播放] path: {}", path);
    println!("[播放] volume: {:?}", volume);
    println!("[播放] force_transcode: {:?}", force_transcode);
    println!("[播放] position: {:?}", position);
    println!("[播放] start_time (String): {:?}", start_time);
    println!("[播放] end_time (String): {:?}", end_time);
    println!("[播放] track_number: {:?}", track_number);
    println!("[播放] title: {:?}", title);
    println!("[播放] 解析后: start_time={:?}, end_time={:?}", start_time_val, end_time_val);
    println!("[播放] ======================");
    
    // 检查是否需要使用FFplay播放（原引擎不支持的无损音频）
    if ffmpeg_transcoder::needs_ffplay_playback(&path) {
        println!("[播放] 检测到需要FFplay播放的格式: {}", path);
        
        // 停止当前的rodio播放
        unsafe {
            if let Some(player) = &mut GLOBAL_PLAYER {
                if let Some(old_sink) = player.sink.take() {
                    println!("[播放] 停止旧的rodio播放");
                    old_sink.stop();
                }
            }
        }
        
        // 使用 FFplay 播放
        let start_position = position.unwrap_or(start_time_val.unwrap_or(0.0));
        // 传递 None 作为 duration 参数，让后端自己获取
        match ffmpeg_transcoder::play_with_ffplay(path.clone(), Some(start_position), None).await {
            Ok(result) => {
                println!("[播放] FFplay 播放已启动");
                
                // 从结果中获取音频时长
                let duration = result.get("duration").and_then(|d: &serde_json::Value| d.as_f64()).unwrap_or(180.0);
                println!("[播放] 音频时长: {}秒", duration);
                
                // 更新播放状态
                let mut state_guard = state.lock().unwrap();
                state_guard.is_playing = true;
                state_guard.volume = volume.unwrap_or(state_guard.volume);
                state_guard.position = start_position;
                state_guard.cue_start_time = start_time_val;
                state_guard.cue_end_time = end_time_val;
                
                // 发送事件通知前端
                let _ = window.emit("song-changed", serde_json::json!({
                    "path": path,
                    "title": title.unwrap_or_else(|| "".to_string()),
                    "artist": "".to_string(),
                    "album": "".to_string(),
                    "duration": duration,
                    "position": start_position,
                    "is_ffplay": true
                }));
                
                return Ok(serde_json::json!({
                    "success": true,
                    "message": "FFplay播放已启动",
                    "path": path,
                    "duration": duration,
                    "position": start_position,
                    "is_ffplay": true
                }));
            }
            Err(e) => {
                eprintln!("[播放] FFplay播放失败: {}", e);
                return Err(format!("FFplay播放失败: {}", e));
            }
        }
    }
    
    let _lock = PLAYER_MUTEX.lock().unwrap();
    
    // 先停止当前的播放，避免多首同时播放
    unsafe {
        if let Some(player) = &mut GLOBAL_PLAYER {
            if let Some(old_sink) = player.sink.take() {
                println!("[播放] 停止旧的播放");
                old_sink.stop();
            }
        }
    }
    
    // 检查是否需要转码（参考SPlayer实现，只对特殊格式转码）
    let play_path = if force_transcode.unwrap_or(false) || ffmpeg_transcoder::needs_transcode(&path) {
        // 检查文件扩展名
        let file_ext = std::path::Path::new(&path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        // 参考SPlayer，WAV文件不使用转码
        if file_ext == "wav" {
            println!("[播放] WAV文件直接播放，不使用转码");
            path.clone()
        } else {
            println!("[播放] 检测到需要转码的格式: {}", path);
            
            // 尝试获取转码缓存
            if let Some(cache) = ffmpeg_transcoder::get_transcode_cache() {
                // 获取或创建缓存项
                if let Some(item) = cache.get_or_create(&path) {
                    // 如果已经转码完成，直接使用
                    if item.is_ready && std::path::Path::new(&item.transcoded_path).exists() {
                        println!("[播放] 使用已转码的文件: {}", item.transcoded_path);
                        item.transcoded_path.clone()
                    } else {
                        // 检查是否正在转码
                        if item.is_transcoding {
                            println!("[播放] 正在转码中，等待完成...");
                        } else {
                            // 开始转码
                            println!("[播放] 开始转码...");
                            match cache.start_transcode(&path) {
                                Ok(_transcoded_path) => {
                                    println!("[播放] 转码已启动");
                                }
                                Err(e) => {
                                    eprintln!("[播放] 启动转码失败: {}", e);
                                    return Err(format!("转码失败: {}", e));
                                }
                            }
                        }
                        
                        // 等待转码完成（最多等待120秒，给DSD等大文件足够时间）
                        println!("[播放] 等待转码完成...");
                        match cache.wait_for_transcode(&path, 120) {
                            Some(ready_path) => {
                                println!("[播放] 转码完成，开始播放: {}", ready_path);
                                ready_path
                            }
                            None => {
                                // 转码超时，返回错误
                                eprintln!("[播放] 转码超时");
                                return Err("转码超时，文件可能过大或格式不支持，请稍后重试".to_string());
                            }
                        }
                    }
                } else {
                    return Err("转码缓存初始化失败".to_string());
                }
            } else {
                return Err("FFmpeg转码服务不可用".to_string());
            }
        }
    } else {
        path.clone()
    };
    
    println!("[播放] 准备播放文件: {}", play_path);
    
    // 获取音频时长
    let duration = match lofty::read_from_path(&play_path) {
        Ok(file) => {
            let dur = file.properties().duration().as_secs_f64();
            println!("[播放] 音频时长: {:.2}秒", dur);
            dur
        },
        Err(e) => {
            println!("[播放] 获取音频时长失败: {}, 使用默认值180.0秒", e);
            180.0
        }
    };
    
    // 初始化音频流
    println!("[播放] 初始化音频流...");
    let handle = initialize_audio_stream()?;
    println!("[播放] 音频流初始化成功");
    
    // 打开文件并解码
    println!("[播放] 打开文件: {}", play_path);
    let file = std::fs::File::open(&play_path)
        .map_err(|e| {
            println!("[播放] 打开文件失败: {}", e);
            format!("打开文件失败: {}", e)
        })?;
    
    println!("[播放] 文件打开成功，开始解码...");
    let mut source = rodio::Decoder::new(file)
        .map_err(|e| {
            println!("[播放] 解码失败: {}", e);
            format!("解码失败: {}", e)
        })?;
    
    println!("[播放] 解码成功");
    
    // 打印音频参数
    let sample_rate = source.sample_rate();
    let channels = source.channels();
    let current_frame_len = source.current_frame_len();
    let total_duration = source.total_duration();
    println!("[播放] 音频参数: 采样率={}Hz, 声道数={}, 当前帧长度={:?}, 总时长={:?}, 格式=PCM (pcm_s16le)", 
             sample_rate, channels, current_frame_len, total_duration);
    
    // 检测文件扩展名
    let file_path = std::path::Path::new(&play_path);
    let extension = file_path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    println!("[播放] 文件扩展名: {}", extension);
    
    // 计算实际开始位置（考虑CUE track的start_time和position参数）
    // 如果start_time和end_time都是None或0，说明不是CUE track，从0开始播放
    let cue_start_val = start_time_val.unwrap_or(0.0);
    let end_time_val_for_cue = end_time_val.unwrap_or(0.0);
    
    let (cue_start, track_end_time) = if cue_start_val == 0.0 && end_time_val_for_cue == 0.0 {
        // 如果不是CUE track，从0开始播放
        println!("[播放] 不是CUE track，从0秒开始播放");
        (0.0, None)
    } else {
        // 是CUE track，使用传入的时间参数
        println!("[播放] 是CUE track，使用传入的时间参数");
        (cue_start_val, Some(end_time_val_for_cue))
    };
    
    let position_offset = position.unwrap_or(0.0);
    let start_position = cue_start + position_offset;
    
    // 计算结束位置
    let _end_position = track_end_time;
    
    println!("[播放] CUE参数详细信息:");
    println!("[播放] - start_time参数: {:?}", start_time_val);
    println!("[播放] - end_time参数: {:?}", end_time_val);
    println!("[播放] - position参数: {:?}", position);
    println!("[播放] - 计算结果: cue_start={:.2}s, position_offset={:.2}s, start_position={:.2}s", 
             cue_start, position_offset, start_position);
    
    // 检查是否有CUE参数
    if start_time_val.unwrap_or(0.0) == 0.0 && end_time_val.unwrap_or(0.0) == 0.0 {
        println!("[播放] 不是CUE track，将从0秒开始播放");
    } else {
        println!("[播放] 是CUE track，将从 {:?}s 播放到 {:?}s", start_time_val, end_time_val);
    }
    
    // 处理样本跳过
    let source = if start_position > 0.0 {
        // 计算需要跳过的样本数
        let sample_rate = source.sample_rate();
        let channels = source.channels();
        let samples_to_skip = (start_position * sample_rate as f64 * channels as f64) as u64;
        println!("[播放] 跳过样本计算:");
        println!("[播放] - 采样率: {}Hz", sample_rate);
        println!("[播放] - 声道数: {}", channels);
        println!("[播放] - 需要跳过的样本数: {}", samples_to_skip);
        
        // 手动跳过样本
        let mut skipped_samples = 0;
        for _ in 0..samples_to_skip {
            if source.next().is_none() {
                println!("[播放] 跳过样本时遇到文件结束，已跳过 {} 个样本", skipped_samples);
                break;
            }
            skipped_samples += 1;
        }
        println!("[播放] 实际跳过的样本数: {}", skipped_samples);
        source
    } else {
        source
    };
    
    // 使用跳过样本后的source
    let boxed_source: Box<dyn Source<Item = f32> + Send> = Box::new(source.convert_samples::<f32>());
    
    unsafe {
        if let Some(player) = &mut GLOBAL_PLAYER {
            // 停止旧的sink
            if let Some(old_sink) = player.sink.take() {
                println!("[播放] 停止旧的播放");
                old_sink.stop();
            }
            
            // 创建新的sink
            let new_sink = rodio::Sink::try_new(&handle)
                .map_err(|e| format!("创建播放器失败: {}", e))?;
            
            println!("[播放] 播放器创建成功");
            
            // 设置音量 - 使用传入的音量或状态中的音量
            let normalized_volume = if let Some(vol) = volume {
                vol / 100.0
            } else {
                let player_state = state.lock().unwrap();
                player_state.volume
            };
            new_sink.set_volume(normalized_volume);
            println!("[播放] 音量设置为: {:.2}", normalized_volume);
            
            // 更新状态中的音量
            {
                let mut player_state = state.lock().unwrap();
                player_state.volume = normalized_volume;
            }
            
            // 直接添加原始音频源，不经过均衡器处理
            println!("[播放] 添加音频源到播放器");
            new_sink.append(boxed_source);
            new_sink.play();
            println!("[播放] 开始播放，sink状态: 空={}, 已结束={}", new_sink.empty(), new_sink.is_paused());
            
            player.sink = Some(new_sink);
        }
    }
    
    // 4. 更新状态（在启动进度更新线程之前）
    {
        let mut player_state = state.lock().unwrap();
        player_state.current_song = Some(parse_audio_file(Path::new(&path)).unwrap_or(Song {
            id: format!("{:x}", md5::compute(&path)),
            title: "".to_string(),
            artist: "".to_string(),
            album: "".to_string(),
            path: path.to_string(),
            duration: format_duration(duration),
            cover: "".to_string(),
            year: "".to_string(),
            genre: "".to_string(),
            lyric: "".to_string(),
            sample_rate: None,
            channels: None,
            bit_depth: None,
            needs_transcode: false,
        }));
        player_state.is_playing = true;
        player_state.position = start_position;
        player_state.position_update_count = 0;
        // 保存CUE track时间信息
        player_state.cue_start_time = if cue_start_val > 0.0 { Some(cue_start_val) } else { None };
        player_state.cue_end_time = if end_time_val_for_cue > 0.0 { Some(end_time_val_for_cue) } else { None };
        println!("[播放] CUE时间信息: start_time={:?}, end_time={:?}", player_state.cue_start_time, player_state.cue_end_time);
    }
    
    // 5. 先停止旧的进度更新线程
    stop_progress_updater();
    
    // 6. 短暂延迟，确保旧线程有时间退出
    std::thread::sleep(Duration::from_millis(100));

    // 7. 启动新的进度更新
    println!("[播放] 启动进度更新线程");
    start_progress_updater(state.inner().clone(), window);
    
    let result = serde_json::json!({
        "duration": duration
    });
    
    Ok(result)
}

// 停止进度更新线程
fn stop_progress_updater() {
    let mut handle = PROGRESS_THREAD_HANDLE.lock().unwrap();
    if let Some(thread_handle) = handle.take() {
        // 注意: 这里不调用 handle.join(),因为线程会在循环中自然退出
        // 我们只是移除旧的句柄引用,让旧线程自然结束
        drop(thread_handle);
    }
}

// 启动进度更新线程
fn start_progress_updater(
    state: Arc<Mutex<PlayerState>>,
    window: tauri::Window,
) {
    println!("启动进度更新线程");

    // 获取初始播放位置
    let initial_position = {
        let player_state = state.lock().unwrap();
        player_state.position
    };

    println!("初始播放位置: {:.1}秒", initial_position);

    let handle = std::thread::spawn(move || {
        let start_time = std::time::Instant::now();
        let mut iteration = 0u64;

        loop {
            iteration += 1;

            // 先检查是否在播放
            let is_playing = {
                let player_state = state.lock().unwrap();
                player_state.is_playing
            };

            if !is_playing {
                println!("进度更新线程: 播放已停止,退出循环");
                break;
            }

            // 检查播放器是否播放完成
            let should_stop = unsafe {
                if let Some(player) = &GLOBAL_PLAYER {
                    if let Some(sink) = &player.sink {
                        sink.empty()
                    } else {
                        false
                    }
                } else {
                    false
                }
            };

            if should_stop {
                // 播放完成,先更新状态再发送事件
                {
                    let mut player_state = state.lock().unwrap();
                    player_state.is_playing = false;
                    // 确保位置更新到歌曲总长度
                    if let Some(ref song) = player_state.current_song {
                        if let Ok(duration) = song.duration.parse::<f64>() {
                            player_state.position = duration;
                        }
                    }
                }
                println!("播放完成,发送playback_finished事件");
                let _ = window.emit("playback_finished", ());
                break;
            }

            // 基于实际经过的时间更新播放进度
            let elapsed = start_time.elapsed().as_secs_f64();
            let current_position = initial_position + elapsed;
            
            // 检查是否超过CUE track结束时间
            let cue_end_reached = {
                let player_state = state.lock().unwrap();
                if let Some(end_time) = player_state.cue_end_time {
                    current_position >= end_time
                } else {
                    false
                }
            };
            
            if cue_end_reached {
                // CUE track播放完成
                {
                    let mut player_state = state.lock().unwrap();
                    player_state.is_playing = false;
                    // 停止播放器
                    unsafe {
                        if let Some(player) = &GLOBAL_PLAYER {
                            if let Some(sink) = &player.sink {
                                sink.stop();
                            }
                        }
                    }
                }
                println!("CUE track播放完成,发送playback_finished事件");
                let _ = window.emit("playback_finished", ());
                break;
            }
            
            {
                let mut player_state = state.lock().unwrap();
                player_state.position = current_position;
                player_state.position_update_count = iteration;
            }

            // 每10次迭代(1秒)输出一次日志
            if iteration % 10 == 0 {
                let current_pos = {
                    let player_state = state.lock().unwrap();
                    player_state.position
                };
                println!("进度更新: 第{}次, 当前位置: {:.1}秒, 实际经过时间: {:.1}秒, 初始位置: {:.1}秒", iteration, current_pos, elapsed, initial_position);
            }

            std::thread::sleep(Duration::from_millis(100));
        }

        println!("进度更新线程已结束");
    });

    let mut thread_handle = PROGRESS_THREAD_HANDLE.lock().unwrap();
    *thread_handle = Some(handle);
}

// 暂停播放
#[tauri::command]
pub async fn pause_song(
    state: State<'_, Arc<Mutex<PlayerState>>>
) -> Result<(), String> {
    println!("暂停播放");
    let _lock = PLAYER_MUTEX.lock().unwrap();

    unsafe {
        if let Some(player) = &GLOBAL_PLAYER {
            if let Some(sink) = &player.sink {
                sink.pause();
            }
        }
    }

    let mut player_state = state.lock().unwrap();
    player_state.is_playing = false;
    println!("暂停完成,当前进度: {:.1}秒", player_state.position);
    Ok(())
}

// 恢复播放
#[tauri::command]
pub async fn resume_song(
    state: State<'_, Arc<Mutex<PlayerState>>>
) -> Result<(), String> {
    println!("恢复播放");
    let _lock = PLAYER_MUTEX.lock().unwrap();

    unsafe {
        if let Some(player) = &GLOBAL_PLAYER {
            if let Some(sink) = &player.sink {
                sink.play();
            }
        }
    }

    let mut player_state = state.lock().unwrap();
    player_state.is_playing = true;
    println!("恢复完成,当前进度: {:.1}秒", player_state.position);
    Ok(())
}

// 停止播放
#[tauri::command]
pub async fn stop_song(
    state: State<'_, Arc<Mutex<PlayerState>>>
) -> Result<(), String> {
    let _lock = PLAYER_MUTEX.lock().unwrap();

    unsafe {
        if let Some(player) = &GLOBAL_PLAYER {
            if let Some(sink) = &player.sink {
                sink.stop();
            }
        }
    }

    // 停止FFplay播放
    let _ = ffmpeg_transcoder::stop_ffplay();

    // 停止进度更新线程
    stop_progress_updater();

    let mut player_state = state.lock().unwrap();
    player_state.is_playing = false;
    player_state.position = 0.0;
    Ok(())
}

// 设置音量
#[tauri::command]
pub async fn set_volume(
    volume: f32,
    state: State<'_, Arc<Mutex<PlayerState>>>
) -> Result<(), String> {
    let _lock = PLAYER_MUTEX.lock().unwrap();
    
    let normalized_volume = volume / 100.0;
    
    unsafe {
        if let Some(player) = &GLOBAL_PLAYER {
            if let Some(sink) = &player.sink {
                sink.set_volume(normalized_volume);
            }
        }
    }
    
    let mut player_state = state.lock().unwrap();
    player_state.volume = normalized_volume;
    Ok(())
}

// 跳转播放位置
#[tauri::command]
pub async fn seek_song(
    position: f64,
    state: State<'_, Arc<Mutex<PlayerState>>>
) -> Result<(), String> {
    // Rodio不支持精确seek,只能重新加载文件并跳过前面的部分
    let mut player_state = state.lock().unwrap();
    
    // 这里简化处理,只更新位置记录
    // 实际的seek需要更复杂的实现
    player_state.position = position;
    
    Err("Rodio暂不支持精确seek功能".to_string())
}

// 设置均衡器
#[tauri::command]
pub async fn set_equalizer(
    bands: Vec<f32>,
    state: State<'_, Arc<Mutex<PlayerState>>>
) -> Result<(), String> {
    let mut player_state = state.lock().unwrap();
    
    let mut band_array = [0.0f32; 10];
    for (i, &band) in bands.iter().enumerate() {
        if i < 10 {
            band_array[i] = band.clamp(-12.0, 12.0);
            player_state.equalizer_bands[i] = band_array[i];
        }
    }
    
    player_state.equalizer.set_bands(band_array);
    
    // 注意: rodio本身不支持实时均衡器应用
    // 实际的均衡器需要在使用DSP库如biquad的情况下实现
    // 这里只是存储均衡器设置
    
    Ok(())
}

// 应用均衡器预设
#[tauri::command]
pub async fn apply_equalizer_preset(
    preset_name: String,
    state: State<'_, Arc<Mutex<PlayerState>>>
) -> Result<(), String> {
    let preset = match preset_name.as_str() {
        "flat" => EqualizerPreset::Flat,
        "rock" => EqualizerPreset::Rock,
        "pop" => EqualizerPreset::Pop,
        "jazz" => EqualizerPreset::Jazz,
        "classical" => EqualizerPreset::Classical,
        "electronic" => EqualizerPreset::Electronic,
        _ => return Err("未知的均衡器预设".to_string()),
    };
    
    let bands = preset.get_bands();
    
    let mut player_state = state.lock().unwrap();
    player_state.equalizer_bands = bands;
    player_state.equalizer.set_bands(bands);
    
    Ok(())
}

// 获取当前播放进度
#[tauri::command]
pub async fn get_position(
    state: State<'_, Arc<Mutex<PlayerState>>>
) -> Result<f64, String> {
    let player_state = state.lock().unwrap();
    let absolute_position = player_state.position;
    
    // 如果是CUE track，返回相对于开始时间的位置
    let relative_position = if let Some(start_time) = player_state.cue_start_time {
        let rel_pos = absolute_position - start_time;
        // 确保不会返回负数
        if rel_pos < 0.0 { 0.0 } else { rel_pos }
    } else {
        absolute_position
    };
    
    println!("get_position 被调用, 绝对位置: {:.1}秒, 相对位置: {:.1}秒, 更新次数: {}", 
             absolute_position, relative_position, player_state.position_update_count);
    Ok(relative_position)
}

// 停止播放(清理资源)
#[tauri::command]
pub async fn cleanup_player() -> Result<(), String> {
    let _lock = PLAYER_MUTEX.lock().unwrap();

    // 停止进度更新线程
    stop_progress_updater();

    unsafe {
        if let Some(mut player) = GLOBAL_PLAYER.take() {
            if let Some(sink) = player.sink.take() {
                sink.stop();
            }
        }
    }

    Ok(())
}

// 获取音频信息（包含时长、采样率、编码器等）
#[tauri::command]
pub async fn get_audio_duration(path: String) -> Result<serde_json::Value, String> {
    println!("[音频信息] 开始获取音频信息: {}", path);
    
    // 优先使用lofty库读取音乐元数据
    if let Ok(file) = lofty::read_from_path(&path) {
        let dur = file.properties().duration().as_secs_f64();
        if dur > 0.0 {
            println!("[音频信息] lofty获取音频时长: {:.2}秒", dur);
            return Ok(serde_json::json!({
                    "duration": dur,
                    "sample_rate": Option::<u32>::None,
                    "codec_name": Option::<String>::None,
                    "channels": Option::<u32>::None,
                    "sample_fmt": Option::<String>::None,
                    "bitrate": Option::<u64>::None,
                    "source": "lofty"
                }));
        }
        println!("[音频信息] lofty获取时长为0，尝试使用ffprobe");
    } else {
        println!("[音频信息] lofty读取失败，尝试使用ffprobe");
    }
    
    // lofty失败时，使用ffprobe作为备用，获取完整音频信息
    if let Some(ffprobe_path) = ffmpeg_transcoder::TranscodeCache::get_ffprobe_path() {
        let mut cmd = Command::new(&ffprobe_path);
        cmd.arg("-hide_banner")
            .arg(&path)
            .arg("-show_streams")
            .arg("-select_streams")
            .arg("a")
            .arg("-print_format")
            .arg("json")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        
        // 在Windows系统上设置CREATE_NO_WINDOW标志，隐藏控制台窗口
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        
        let probe_result = cmd.output();
        
        match &probe_result {
            Ok(result) => {
                let stdout_str = String::from_utf8_lossy(&result.stdout);
                
                if !stdout_str.is_empty() && !stdout_str.contains("error") && !stdout_str.contains("Error") {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout_str) {
                        if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
                            if let Some(first_stream) = streams.first() {
                                // 获取时长
                                let duration = first_stream.get("duration")
                                    .and_then(|d| d.as_str())
                                    .and_then(|d| d.parse::<f64>().ok());
                                
                                // 获取采样率
                                let sample_rate = first_stream.get("sample_rate")
                                    .and_then(|r| r.as_str())
                                    .and_then(|r| r.parse::<u32>().ok());
                                
                                // 获取编码器名称
                                let codec_name = first_stream.get("codec_name")
                                    .and_then(|c| c.as_str())
                                    .map(|c| c.to_string());
                                
                                // 获取声道数
                                let channels = first_stream.get("channels")
                                    .and_then(|c| c.as_u64())
                                    .map(|c| c as u32);
                                
                                // 获取采样格式
                                let sample_fmt = first_stream.get("sample_fmt")
                                    .and_then(|s| s.as_str())
                                    .map(|s| s.to_string());
                                
                                // 获取比特率
                                let bitrate = first_stream.get("bit_rate")
                                    .and_then(|b| b.as_str())
                                    .and_then(|b| b.parse::<u64>().ok());
                                
                                if let Some(dur_val) = duration {
                                    println!("[音频信息] ffprobe获取音频时长: {:.2}秒", dur_val);
                                    if let Some(rate) = sample_rate {
                                        println!("[音频信息] ffprobe获取采样率: {} Hz", rate);
                                    }
                                    if let Some(codec) = &codec_name {
                                        println!("[音频信息] ffprobe获取编码器: {}", codec);
                                    }
                                    if let Some(chans) = channels {
                                        println!("[音频信息] ffprobe获取声道数: {}", chans);
                                    }
                                    if let Some(fmt) = &sample_fmt {
                                        println!("[音频信息] ffprobe获取采样格式: {}", fmt);
                                    }
                                    if let Some(bitrate_val) = bitrate {
                                        println!("[音频信息] ffprobe获取比特率: {} bps", bitrate_val);
                                    }
                                    
                                    return Ok(serde_json::json!({
                                        "duration": dur_val,
                                        "sample_rate": sample_rate,
                                        "codec_name": codec_name,
                                        "channels": channels,
                                        "sample_fmt": sample_fmt,
                                        "bitrate": bitrate,
                                        "source": "ffprobe"
                                    }));
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                println!("[音频信息] ffprobe执行失败: {}", e);
            }
        }
    } else {
        println!("[音频信息] 未找到FFprobe，使用默认值");
    }
    
    // 所有方法都失败，使用默认值
    println!("[音频信息] 获取音频信息失败，使用默认值");
    Ok(serde_json::json!({
                "duration": 180.0,
                "sample_rate": Option::<u32>::None,
                "codec_name": Option::<String>::None,
                "channels": Option::<u32>::None,
                "sample_fmt": Option::<String>::None,
                "bitrate": Option::<u64>::None,
                "source": "default"
            }))
}

// 最小化窗口
#[tauri::command]
pub async fn minimize_window(window: tauri::Window) -> Result<(), String> {
    window.minimize().map_err(|e| e.to_string())
}

// 切换最大化状态
#[tauri::command]
pub async fn toggle_maximize_window(window: tauri::Window) -> Result<(), String> {
    if window.is_maximized().map_err(|e| e.to_string())? {
        window.unmaximize().map_err(|e| e.to_string())
    } else {
        window.maximize().map_err(|e| e.to_string())
    }
}

// 关闭窗口
#[tauri::command]
pub async fn close_window(window: tauri::Window) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}

// 显示/隐藏窗口
#[tauri::command]
pub async fn toggle_window_visibility(window: tauri::Window) -> Result<bool, String> {
    let is_visible = window.is_visible().map_err(|e| e.to_string())?;
    if is_visible {
        window.hide().map_err(|e| e.to_string())?;
        Ok(false)
    } else {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        Ok(true)
    }
}


// CUE分段音频源 - 用于在指定时间范围内播放音频
#[allow(dead_code)]
pub struct CueSegmentSource {
    inner: Box<dyn Source<Item = f32> + Send>,
    current_time: f64,
    end_time: f64,
    sample_rate: u32,
    channels: u16,
}

impl CueSegmentSource {
    #[allow(dead_code)]
    pub fn new(inner: Box<dyn Source<Item = f32> + Send>, start_time: f64, end_time: f64) -> Self {
        let sample_rate = inner.sample_rate();
        let channels = inner.channels();
        
        Self {
            inner,
            current_time: start_time,
            end_time,
            sample_rate,
            channels,
        }
    }
}

impl Iterator for CueSegmentSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_time >= self.end_time {
            return None;
        }
        let sample = self.inner.next()?;
        let samples_per_second = self.sample_rate as f64 * self.channels as f64;
        self.current_time += 1.0 / samples_per_second;
        Some(sample)
    }
}

impl Source for CueSegmentSource {
    fn current_frame_len(&self) -> Option<usize> {
        let remaining_time = self.end_time - self.current_time;
        let samples_per_second = self.sample_rate as f64 * self.channels as f64;
        let remaining_samples = (remaining_time * samples_per_second) as usize;
        Some(remaining_samples)
    }

    fn channels(&self) -> u16 { self.channels }
    fn sample_rate(&self) -> u32 { self.sample_rate }
    fn total_duration(&self) -> Option<std::time::Duration> {
        let duration = self.end_time - self.current_time;
        Some(std::time::Duration::from_secs_f64(duration))
    }
}

// 打开README.md文件
#[tauri::command]
pub async fn open_readme() -> Result<(), String> {
    use std::path::Path;
    use std::process::Command;
    
    // 获取应用可执行文件所在目录
    let exe_path = std::env::current_exe().map_err(|e| format!("获取可执行文件路径失败: {}", e))?;
    let exe_dir = exe_path.parent().unwrap_or_else(|| Path::new("."));
    
    // 构建README.md的路径（向上两级，因为可执行文件在target目录下）
    let readme_path = exe_dir.join("..").join("..").join("README.md");
    
    // 检查README.md文件是否存在
    if !readme_path.exists() {
        // 如果在应用目录中找不到，尝试在当前工作目录查找
        let cwd = std::env::current_dir().map_err(|e| format!("获取当前工作目录失败: {}", e))?;
        let cwd_readme_path = cwd.join("README.md");
        if !cwd_readme_path.exists() {
            return Err(format!("README.md文件不存在: {:?} 或 {:?}", readme_path, cwd_readme_path));
        }
        
        // 打开README.md文件
        #[cfg(windows)]
        {
            let mut cmd = Command::new("cmd");
            cmd.args(["/c", "start", "", cwd_readme_path.to_str().unwrap()]);
            
            // 设置CREATE_NO_WINDOW标志，隐藏控制台窗口
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            
            cmd.spawn()
                .map_err(|e| format!("打开README.md文件失败: {}", e))?;
        }
        
        #[cfg(not(windows))]
        {
            Command::new("open")
                .arg(cwd_readme_path.to_str().unwrap())
                .spawn()
                .map_err(|e| format!("打开README.md文件失败: {}", e))?;
        }
    } else {
        // 打开README.md文件
        #[cfg(windows)]
        {
            let mut cmd = Command::new("cmd");
            cmd.args(["/c", "start", "", readme_path.to_str().unwrap()]);
            
            // 设置CREATE_NO_WINDOW标志，隐藏控制台窗口
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            
            cmd.spawn()
                .map_err(|e| format!("打开README.md文件失败: {}", e))?;
        }
        
        #[cfg(not(windows))]
        {
            Command::new("open")
                .arg(readme_path.to_str().unwrap())
                .spawn()
                .map_err(|e| format!("打开README.md文件失败: {}", e))?;
        }
    }
    
    Ok(())
}

