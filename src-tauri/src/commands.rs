use std::fs::{self, DirEntry};
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use base64::{Engine as _, engine::general_purpose};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;
use rodio::Source;
use tauri::{State, Emitter};

use crate::equalizer::{Equalizer, EqualizerPreset};
use crate::ffmpeg_transcoder::{self, TranscodeCache};

// 全局播放器(不实现Send,使用unsafe)
// 需要使用unsafe static来绕过rodio的Send限制
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
    let mut songs = Vec::new();
    
    if let Err(err) = scan_directory_recursive(Path::new(&directory), &mut songs) {
        return Err(format!("扫描目录失败: {}", err));
    }
    
    let result = serde_json::json!({
        "tracks": songs
    });
    
    Ok(result)
}

// 递归扫描目录
fn scan_directory_recursive(path: &Path, songs: &mut Vec<Song>) -> std::io::Result<()> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                scan_directory_recursive(&path, songs)?;
            } else if is_audio_file(&entry) {
                if let Some(song) = parse_audio_file(&path) {
                    songs.push(song);
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

    // 检查是否为特殊格式（DSD、DTS等无法获取时长的格式）
    let is_special_format = if let Some(ext) = path.extension() {
        if let Some(ext_str) = ext.to_str() {
            let ext_lower = ext_str.to_lowercase();
            matches!(ext_lower.as_str(), "dsf" | "dff" | "dsd" | "dts")
        } else {
            false
        }
    } else {
        false
    };

    // 如果duration为0或为特殊格式，给一个特殊标记
    if duration <= 0.0 || is_special_format {
        if is_special_format {
            duration = -1.0; // 特殊标记：时长未知
        } else {
            duration = 180.0; // 默认3分钟
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

// 初始化音频流
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
    state: State<'_, Arc<Mutex<PlayerState>>>,
    window: tauri::Window,
) -> Result<serde_json::Value, String> {
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
    
    // 检查是否需要转码
    let play_path = if force_transcode.unwrap_or(false) || TranscodeCache::needs_transcode(&path) {
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
    } else {
        path.clone()
    };
    
    // 获取音频时长
    let duration = match lofty::read_from_path(&play_path) {
        Ok(file) => file.properties().duration().as_secs_f64(),
        Err(_) => 180.0,
    };
    
    // 初始化音频流
    let handle = initialize_audio_stream()?;
    
    // 打开文件并解码
    let file = std::fs::File::open(&play_path)
        .map_err(|e| format!("打开文件失败: {}", e))?;
    
    let mut source = rodio::Decoder::new(file)
        .map_err(|e| format!("解码失败: {}", e))?;
    
    // 跳过指定位置的音频
    let start_position = position.unwrap_or(0.0);
    if start_position > 0.0 {
        // 计算需要跳过的样本数
        let sample_rate = source.sample_rate();
        let channels = source.channels();
        let samples_to_skip = (start_position * sample_rate as f64 * channels as f64) as u64;
        println!("跳过 {} 秒，{} 样本", start_position, samples_to_skip);
        
        // 手动跳过样本
        for _ in 0..samples_to_skip {
            if source.next().is_none() {
                break;
            }
        }
    }
    
    unsafe {
        if let Some(player) = &mut GLOBAL_PLAYER {
            // 停止旧的sink
            if let Some(old_sink) = player.sink.take() {
                old_sink.stop();
            }
            
            // 创建新的sink
            let new_sink = rodio::Sink::try_new(&handle)
                .map_err(|e| format!("创建播放器失败: {}", e))?;
            
            // 设置音量 - 使用传入的音量或状态中的音量
            let normalized_volume = if let Some(vol) = volume {
                vol / 100.0
            } else {
                let player_state = state.lock().unwrap();
                player_state.volume
            };
            new_sink.set_volume(normalized_volume);
            
            // 更新状态中的音量
            {
                let mut player_state = state.lock().unwrap();
                player_state.volume = normalized_volume;
            }
            
            // 应用均衡器效果
            let equalizer = {
                let player_state = state.lock().unwrap();
                player_state.equalizer.clone()
            };
            
            // 先将音频源转换为f32格式，然后应用均衡器效果
            let converted_source = source.convert_samples();
            let equalized_source = crate::equalizer::EqualizedSource::new(converted_source, equalizer);
            
            // 添加音频源
            new_sink.append(equalized_source);
            new_sink.play();
            
            player.sink = Some(new_sink);
        }
    }
    
    // 先停止旧的进度更新线程
    // 1. 暂时将播放状态设置为false，让旧线程退出
    {
        let mut player_state = state.lock().unwrap();
        player_state.is_playing = false;
    }
    
    // 2. 停止旧的进度更新线程
    stop_progress_updater();
    
    // 3. 短暂延迟，确保旧线程有时间退出
    std::thread::sleep(Duration::from_millis(100));

    // 4. 更新状态
    {
        let mut player_state = state.lock().unwrap();
        player_state.current_song = Some(parse_audio_file(Path::new(&path)).unwrap_or(Song {
            id: format!("{:x}", md5::compute(&path)),
            title: "".to_string(),
            artist: "".to_string(),
            album: "".to_string(),
            path: path.clone(),
            duration: format_duration(duration),
            cover: "".to_string(),
            year: "".to_string(),
            genre: "".to_string(),
            lyric: "".to_string(),
        }));
        player_state.is_playing = true;
        player_state.position = start_position;
        player_state.position_update_count = 0;
    }

    // 5. 启动新的进度更新
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
    let position = player_state.position;
    println!("get_position 被调用,返回: {:.1}秒, 更新次数: {}", position, player_state.position_update_count);
    Ok(position)
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
