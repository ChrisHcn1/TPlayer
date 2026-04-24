use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, atomic::{AtomicU32, Ordering}};
use std::collections::HashMap;
use std::time::{SystemTime, Duration};
use std::fs;
use std::env;

// 日志开关：设置为 false 可禁用所有日志输出
#[allow(dead_code)]
const ENABLE_LOGS: bool = false;

// 条件性日志宏
#[allow(unused_macros)]
macro_rules! log_info {
    ($($arg:tt)*) => {
        if ENABLE_LOGS {
            println!($($arg)*);
        }
    };
}
#[allow(unused_macros)]
macro_rules! log_error {
    ($($arg:tt)*) => {
        if ENABLE_LOGS {
            eprintln!($($arg)*);
        }
    };
}

// rodio原生支持的音频格式（无需转码）
const NATIVE_SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp3",    // MPEG Audio Layer III
    "flac",   // Free Lossless Audio Codec
    "wav",    // Waveform Audio File Format
    "aac",    // Advanced Audio Coding
    "ogg",    // Ogg Vorbis
    "m4a",    // MPEG-4 Audio
    "opus",   // Opus Audio Codec
];

// 需要转码的音频格式（rodio不支持的格式）
const TRANSCODE_EXTENSIONS: &[&str] = &["ape", "dsd", "dts", "dff", "dsf", "sacd", "wv"];

// 转码缓存项
#[derive(Clone)]
#[allow(dead_code)]
pub struct TranscodeCacheItem {
    pub original_path: String,
    pub transcoded_path: String,
    pub created_at: SystemTime,
    pub is_transcoding: bool,
    pub is_ready: bool,
}

// ffprobe获取的音频信息
#[derive(Clone)]
#[allow(dead_code)]
pub struct AudioInfo {
    pub sample_rate: Option<u32>,
    pub codec_name: Option<String>,
    pub channels: Option<u32>,
    pub sample_fmt: Option<String>,
    pub bitrate: Option<u64>,
    pub duration: Option<f64>,
}

// 转码缓存管理器
pub struct TranscodeCache {
    cache: Arc<Mutex<HashMap<String, TranscodeCacheItem>>>,
    cache_dir: PathBuf,
}

// 检查文件是否需要转码
pub fn needs_transcode(path: &str) -> bool {
    let path_lower = path.to_lowercase();
    
    // 首先检查是否在原生支持列表中（参考SPlayer实现）
    if NATIVE_SUPPORTED_EXTENSIONS.iter().any(|ext| {
        path_lower.ends_with(&format!(".{}", ext))
    }) {
        // 特别处理WAV文件，确保不转码
        if path_lower.ends_with(".wav") {
            println!("[转码检查] WAV文件，原生支持，无需转码");
            return false;
        }
        println!("[转码检查] 原生支持格式，无需转码");
        return false; // 原生支持，无需转码
    }
    
    // 检查是否在需要转码的格式列表中
    if TRANSCODE_EXTENSIONS.iter().any(|ext| {
        path_lower.ends_with(&format!(".{}", ext))
    }) {
        println!("[转码检查] 需要转码的格式");
        return true; // 需要转码
    }
    
    // 检查文件名或路径中是否包含DTS相关标识（特殊情况处理）
    if path_lower.contains("dts") || path_lower.contains("dts-cd") {
        println!("[转码检查] DTS相关文件，需要转码");
        return true;
    }
    
    println!("[转码检查] 其他格式，默认不需要转码");
    false // 其他格式默认不需要转码
}

impl TranscodeCache {
    pub fn new() -> Result<Self, String> {
        // 使用系统临时目录作为缓存目录
        let cache_dir = std::env::temp_dir().join("tplayer_transcode_cache");
        
        // 确保缓存目录存在
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir)
                .map_err(|e| format!("创建缓存目录失败: {}", e))?;
        }
        
        Ok(Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            cache_dir,
        })
    }
    
    // 获取 FFmpeg 路径（公共方法，用于检查FFmpeg是否可用）
    pub fn get_ffmpeg_path() -> Option<String> {
        // 0. 首先检查应用内置的ffmpeg
        // 开发环境：直接检查src-tauri/bin/ffmpeg.exe
        let current_dir = std::env::current_dir().ok()?;
        let dev_ffmpeg = current_dir.join("src-tauri").join("bin").join("ffmpeg.exe");
        if dev_ffmpeg.exists() {
            println!("[FFmpeg] 使用开发环境的内置ffmpeg: {:?}", dev_ffmpeg);
            return Some(dev_ffmpeg.to_string_lossy().to_string());
        }
        
        // 生产环境：检查应用目录下的bin/ffmpeg.exe
        if let Ok(resource_dir) = std::env::current_exe() {
            let app_dir = resource_dir.parent()?;
            let builtin_ffmpeg = app_dir.join("bin").join("ffmpeg.exe");
            if builtin_ffmpeg.exists() {
                println!("[FFmpeg] 使用生产环境的内置ffmpeg: {:?}", builtin_ffmpeg);
                return Some(builtin_ffmpeg.to_string_lossy().to_string());
            }
        }
        
        // 1. 检查环境变量
        if let Ok(ffmpeg_path) = env::var("FFMPEG_PATH") {
            if Path::new(&ffmpeg_path).exists() {
                return Some(ffmpeg_path);
            }
        }
        
        // 2. 检查 PATH 中的 ffmpeg
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    
    // 在Windows系统上设置CREATE_NO_WINDOW标志，隐藏控制台窗口
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    
    if cmd.output().is_ok() {
        return Some("ffmpeg".to_string());
    }
        
        // 3. 检查常见安装路径
        let common_paths = [
            r"C:\ffmpeg\bin\ffmpeg.exe",
            r"C:\Program Files\ffmpeg\bin\ffmpeg.exe",
            r"C:\Program Files (x86)\ffmpeg\bin\ffmpeg.exe",
        ];
        
        for path in &common_paths {
            if Path::new(path).exists() {
                return Some(path.to_string());
            }
        }
        
        None
    }
    
    pub fn get_ffprobe_path() -> Option<String> {
        // 0. 首先检查应用内置的ffprobe
        // 开发环境：直接检查src-tauri/bin/ffprobe.exe
        let current_dir = std::env::current_dir().ok()?;
        let dev_ffprobe = current_dir.join("src-tauri").join("bin").join("ffprobe.exe");
        if dev_ffprobe.exists() {
            println!("[FFmpeg] 使用开发环境的内置ffprobe: {:?}", dev_ffprobe);
            return Some(dev_ffprobe.to_string_lossy().to_string());
        }
        
        // 生产环境：检查应用目录下的bin/ffprobe.exe
        if let Ok(resource_dir) = std::env::current_exe() {
            let app_dir = resource_dir.parent()?;
            let builtin_ffprobe = app_dir.join("bin").join("ffprobe.exe");
            if builtin_ffprobe.exists() {
                println!("[FFmpeg] 使用生产环境的内置ffprobe: {:?}", builtin_ffprobe);
                return Some(builtin_ffprobe.to_string_lossy().to_string());
            }
        }
        
        // 1. 检查环境变量
        if let Ok(ffprobe_path) = env::var("FFPROBE_PATH") {
            if Path::new(&ffprobe_path).exists() {
                return Some(ffprobe_path);
            }
        }
        
        // 2. 检查 PATH 中的 ffprobe
        let mut cmd = Command::new("ffprobe");
        cmd.arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        
        // 在Windows系统上设置CREATE_NO_WINDOW标志，隐藏控制台窗口
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        
        if cmd.output().is_ok() {
            return Some("ffprobe".to_string());
        }
        
        // 3. 检查常见安装路径
        let common_paths = [
            r"C:\ffmpeg\bin\ffprobe.exe",
            r"C:\Program Files\ffmpeg\bin\ffprobe.exe",
            r"C:\Program Files (x86)\ffmpeg\bin\ffprobe.exe",
        ];
        
        for path in &common_paths {
            if Path::new(path).exists() {
                return Some(path.to_string());
            }
        }
        
        None
    }
    
    // 获取 FFplay 路径（公共方法，用于检查FFplay是否可用）
    pub fn get_ffplay_path() -> Option<String> {
        // 0. 首先检查应用内置的ffplay
        // 开发环境：使用CARGO_MANIFEST_DIR检查src-tauri/bin/ffplay.exe
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let dev_ffplay = manifest_dir.join("bin").join("ffplay.exe");
        if dev_ffplay.exists() {
            println!("[FFmpeg] 使用开发环境的内置ffplay: {:?}", dev_ffplay);
            return Some(dev_ffplay.to_string_lossy().to_string());
        } else {
            println!("[FFmpeg] 开发环境内置ffplay不存在: {:?}", dev_ffplay);
        }
        
        // 生产环境：检查应用目录下的bin/ffplay.exe
        if let Ok(resource_dir) = std::env::current_exe() {
            let app_dir = resource_dir.parent()?;
            let builtin_ffplay = app_dir.join("bin").join("ffplay.exe");
            if builtin_ffplay.exists() {
                println!("[FFmpeg] 使用生产环境的内置ffplay: {:?}", builtin_ffplay);
                return Some(builtin_ffplay.to_string_lossy().to_string());
            } else {
                println!("[FFmpeg] 生产环境内置ffplay不存在: {:?}", builtin_ffplay);
            }
        }
        
        // 1. 检查环境变量
        if let Ok(ffplay_path) = env::var("FFPLAY_PATH") {
            if Path::new(&ffplay_path).exists() {
                return Some(ffplay_path);
            }
        }
        
        // 2. 检查 PATH 中的 ffplay
        let mut cmd = Command::new("ffplay");
        cmd.arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        
        // 在Windows系统上设置CREATE_NO_WINDOW标志，隐藏控制台窗口
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        
        if cmd.output().is_ok() {
            return Some("ffplay".to_string());
        }
        
        // 3. 检查常见安装路径
        let common_paths = [
            r"C:\ffmpeg\bin\ffplay.exe",
            r"C:\Program Files\ffmpeg\bin\ffplay.exe",
            r"C:\Program Files (x86)\ffmpeg\bin\ffplay.exe",
        ];
        
        for path in &common_paths {
            if Path::new(path).exists() {
                return Some(path.to_string());
            }
        }
        
        None
    }
    
    // 获取或创建转码缓存项
    pub fn get_or_create(&self, original_path: &str) -> Option<TranscodeCacheItem> {
        let cache_key = Self::get_cache_key(original_path);
        // 使用源文件名作为转码文件名，去掉后缀改为.flac
        let transcoded_filename = Self::get_transcoded_filename(original_path);
        let transcoded_path = self.cache_dir.join(transcoded_filename);
        
        // 检查转码文件是否已存在
        let is_ready = transcoded_path.exists();
        
        {
            let cache = self.cache.lock().unwrap();
            if let Some(item) = cache.get(&cache_key) {
                // 检查转码文件是否仍然存在
                if Path::new(&item.transcoded_path).exists() {
                    return Some(item.clone());
                }
            }
        }
        
        // 创建新的缓存项
        let item = TranscodeCacheItem {
            original_path: original_path.to_string(),
            transcoded_path: transcoded_path.to_string_lossy().to_string(),
            created_at: SystemTime::now(),
            is_transcoding: false,
            is_ready: is_ready,
        };
        
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(cache_key, item.clone());
        }
        
        Some(item)
    }
    
    // 生成缓存键（使用完整路径的哈希值）
    fn get_cache_key(path: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let path = Path::new(path);
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
    
    // 生成转码文件名（使用源文件名，去掉后缀改为.flac）
    fn get_transcoded_filename(path: &str) -> String {
        let path = Path::new(path);
        let filename_without_ext = path.file_stem().unwrap_or_default().to_string_lossy();
        format!("{}.flac", filename_without_ext)
    }
    
    // 开始转码（使用前端传递的音频信息）
    pub fn start_transcode_with_info(&self, original_path: &str, audio_info_json: Option<serde_json::Value>) -> Result<String, String> {
        let ffmpeg_path = Self::get_ffmpeg_path()
            .ok_or_else(|| "未找到 FFmpeg，请设置 FFMPEG_PATH 环境变量".to_string())?;
        
        let cache_key = Self::get_cache_key(original_path);
        // 使用源文件名作为转码文件名，去掉后缀改为.flac
        let transcoded_filename = Self::get_transcoded_filename(original_path);
        let transcoded_path = self.cache_dir.join(transcoded_filename);
        
        // 检查转码文件是否已存在
        if transcoded_path.exists() {
            println!("[FFmpeg] 转码文件已存在，直接使用: {}", transcoded_path.to_string_lossy());
            // 更新缓存状态
            {
                let mut cache = self.cache.lock().unwrap();
                if let Some(item) = cache.get_mut(&cache_key) {
                    item.is_transcoding = false;
                    item.is_ready = true;
                } else {
                    // 创建新的缓存项
                    let item = TranscodeCacheItem {
                        original_path: original_path.to_string(),
                        transcoded_path: transcoded_path.to_string_lossy().to_string(),
                        created_at: SystemTime::now(),
                        is_transcoding: false,
                        is_ready: true,
                    };
                    cache.insert(cache_key.clone(), item);
                }
            }
            return Ok(transcoded_path.to_string_lossy().to_string());
        }
        
        // 检查是否已经在转码
        {
            let cache = self.cache.lock().unwrap();
            if let Some(item) = cache.get(&cache_key) {
                if item.is_transcoding {
                    return Ok(item.transcoded_path.clone());
                }
            }
        }
        
        // 克隆 cache_key 用于后续使用
        let _key = cache_key.clone();
        
        // 更新状态为正在转码
        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(item) = cache.get_mut(&cache_key) {
                item.is_transcoding = true;
            } else {
                // 创建新的缓存项
                let item = TranscodeCacheItem {
                    original_path: original_path.to_string(),
                    transcoded_path: transcoded_path.to_string_lossy().to_string(),
                    created_at: SystemTime::now(),
                    is_transcoding: true,
                    is_ready: false,
                };
                cache.insert(cache_key.clone(), item);
            }
        }
        
        let original = original_path.to_string();
        
        // 解析音频信息
        let audio_info = if let Some(json) = audio_info_json {
            println!("[FFmpeg] 使用前端传递的音频信息");
            // 从JSON解析音频信息
            let sample_rate = json.get("sample_rate")
                .and_then(|r| r.as_u64())
                .map(|r| r as u32);
            
            let codec_name = json.get("codec_name")
                .and_then(|c| c.as_str())
                .map(|c| c.to_string());
            
            let channels = json.get("channels")
                .and_then(|c| c.as_u64())
                .map(|c| c as u32);
            
            let sample_fmt = json.get("sample_fmt")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            
            let bitrate = json.get("bitrate")
                .and_then(|b| b.as_u64());
            
            let duration = json.get("duration")
                .and_then(|d| d.as_f64());
            
            if let Some(rate) = sample_rate {
                println!("[FFmpeg] 使用前端传递的采样率: {} Hz", rate);
            }
            if let Some(codec) = &codec_name {
                println!("[FFmpeg] 使用前端传递的编码器: {}", codec);
            }
            if let Some(chans) = channels {
                println!("[FFmpeg] 使用前端传递的声道数: {}", chans);
            }
            if let Some(fmt) = &sample_fmt {
                println!("[FFmpeg] 使用前端传递的采样格式: {}", fmt);
            }
            if let Some(bitrate_val) = bitrate {
                println!("[FFmpeg] 使用前端传递的比特率: {} bps", bitrate_val);
            }
            if let Some(dur) = duration {
                println!("[FFmpeg] 使用前端传递的时长: {} 秒", dur);
            }
            
            Some(AudioInfo {
                sample_rate,
                codec_name,
                channels,
                sample_fmt,
                bitrate,
                duration,
            })
        } else {
            // 前端未传递音频信息，使用ffprobe获取
            println!("[FFmpeg] 前端未传递音频信息，使用ffprobe获取");
            let ffprobe_path = Self::get_ffprobe_path()
                .ok_or_else(|| "未找到 FFprobe，请设置 FFPROBE_PATH 环境变量".to_string())?;
            println!("[FFmpeg] 开始获取音频信息: {}", original);
            let probe_cmd = format!("{} -hide_banner {} -show_streams -select_streams a -print_format json", ffprobe_path, original);
            println!("[FFmpeg] 执行ffprobe命令: {}", probe_cmd);
            let mut cmd = Command::new(&ffprobe_path);
            cmd.arg("-hide_banner")
                .arg(&original)
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
            
            let probe_result = cmd.output();
            
            match &probe_result {
                Ok(result) => {
                    let stdout_str = String::from_utf8_lossy(&result.stdout);
                    let stderr_str = String::from_utf8_lossy(&result.stderr);
                    
                    println!("[FFmpeg] ffprobe执行完成，退出码: {:?}, stdout长度: {}, stderr长度: {}", 
                             result.status.code(), stdout_str.len(), stderr_str.len());
                    if !stderr_str.is_empty() {
                        println!("[FFmpeg] ffprobe stderr: {}", stderr_str);
                    }
                    
                    if !stdout_str.is_empty() && !stdout_str.contains("error") && !stdout_str.contains("Error") {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout_str) {
                            if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
                                if let Some(first_stream) = streams.first() {
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
                                    
                                    // 获取时长
                                    let duration = first_stream.get("duration")
                                        .and_then(|d| d.as_str())
                                        .and_then(|d| d.parse::<f64>().ok());
                                    
                                    if let Some(rate) = sample_rate {
                                        println!("[FFmpeg] 检测到采样率: {} Hz", rate);
                                    }
                                    if let Some(codec) = &codec_name {
                                        println!("[FFmpeg] 检测到编码器: {}", codec);
                                    }
                                    if let Some(chans) = channels {
                                        println!("[FFmpeg] 检测到声道数: {}", chans);
                                    }
                                    if let Some(fmt) = &sample_fmt {
                                        println!("[FFmpeg] 检测到采样格式: {}", fmt);
                                    }
                                    if let Some(bitrate_val) = bitrate {
                                        println!("[FFmpeg] 检测到比特率: {} bps", bitrate_val);
                                    }
                                    if let Some(dur) = duration {
                                        println!("[FFmpeg] 检测到时长: {} 秒", dur);
                                    }
                                    
                                    Some(AudioInfo {
                                        sample_rate,
                                        codec_name,
                                        channels,
                                        sample_fmt,
                                        bitrate,
                                        duration,
                                    })
                                } else {
                                    println!("[FFmpeg] 未找到音频流");
                                    None
                                }
                            } else {
                                println!("[FFmpeg] 未找到streams数组，stdout: {}", stdout_str);
                                None
                            }
                        } else {
                            println!("[FFmpeg] JSON解析失败，stdout: {}", stdout_str);
                            None
                        }
                    } else {
                        println!("[FFmpeg] ffprobe输出为空或包含错误，stdout: {}, stderr: {}", stdout_str, stderr_str);
                        None
                    }
                }
                Err(e) => {
                    println!("[FFmpeg] ffprobe执行失败: {}", e);
                    None
                }
            }
        };
        
        let transcoded = transcoded_path.to_string_lossy().to_string();
        let key = cache_key.clone();
        let cache = self.cache.clone();
        let ffmpeg_path = ffmpeg_path.clone();
        let audio_info = audio_info;
        
        println!("[FFmpeg] 音频信息获取完成，开始转码: {} -> {}", original, transcoded);
        
        // 在后台线程中执行转码
        std::thread::spawn(move || {
            println!("[FFmpeg] 开始转码: {} -> {}", original, transcoded);
            
            // 根据文件扩展名确定转码参数
            let mut args: Vec<String> = vec![
                "-i".to_string(),
                original.clone(),
                "-c:a".to_string(),
                "flac".to_string(),
                "-compression_level".to_string(),
                "8".to_string(), // 中等压缩级别，平衡音质和转码速度
                "-y".to_string(), // 覆盖已存在的文件
                "-loglevel".to_string(),
                "quiet".to_string(), // 减少输出，避免命令行窗口显示
            ];
            
            // 检测文件扩展名
            let path = Path::new(&original);
            let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
            
            // 使用获取的音频信息
            let sample_rate = audio_info.as_ref().and_then(|info| info.sample_rate);
            let _channels = audio_info.as_ref().and_then(|info| info.channels);
            let sample_fmt = audio_info.as_ref().and_then(|info| info.sample_fmt.as_deref());
            let bitrate = audio_info.as_ref().and_then(|info| info.bitrate);
            
            // 对于多声道格式，添加特殊处理以确保良好的立体声效果
            // 采样率限制：高于192kHz使用192kHz，低于192kHz使用原采样率
            let max_sample_rate = 192000; // 192kHz
            let target_rate = sample_rate.map(|rate| if rate > max_sample_rate {
                println!("[FFmpeg] 检测到的采样率 {} Hz 高于192kHz，限制为192kHz", rate);
                max_sample_rate
            } else {
                println!("[FFmpeg] 使用检测到的采样率: {} Hz", rate);
                rate
            }).unwrap_or(48000);
            
            let target_sample_fmt = sample_fmt.unwrap_or("pcm_s16le");
            
            let af_filter = if extension.eq_ignore_ascii_case("dsf") || extension.eq_ignore_ascii_case("dff") {
                // DSD格式需要特殊的解码和重采样处理
                println!("[FFmpeg] DSD格式，使用采样率: {} Hz, 采样格式: {}", target_rate, target_sample_fmt);
                format!("aformat=sample_fmts={}:channel_layouts=stereo,aresample={}", target_sample_fmt, target_rate)
            } else if extension.eq_ignore_ascii_case("dts") || extension.eq_ignore_ascii_case("dtshd") || extension.eq_ignore_ascii_case("dtshd_ma") {
                // DTS格式需要特殊处理，转换为立体声
                println!("[FFmpeg] DTS格式，使用采样率: {} Hz, 采样格式: {}", target_rate, target_sample_fmt);
                format!("aformat=sample_fmts={}:channel_layouts=stereo,aresample={}", target_sample_fmt, target_rate)
            } else if extension.eq_ignore_ascii_case("wav") || extension.eq_ignore_ascii_case("flac") || extension.eq_ignore_ascii_case("aiff") {
                // 对于WAV、FLAC、AIFF等无损格式，使用高质量的立体声转换
                // 使用pan滤镜确保良好的立体声平衡
                println!("[FFmpeg] 无损格式，使用采样率: {} Hz, 采样格式: {}", target_rate, target_sample_fmt);
                format!("pan=stereo|c0=0.5*c0+0.5*c2+0.3*c4|c1=0.5*c1+0.5*c3+0.3*c5,aresample={}", target_rate)
            } else {
                // 其他格式使用常规处理
                println!("[FFmpeg] 其他格式，使用采样率: {} Hz, 采样格式: {}", target_rate, target_sample_fmt);
                format!("aresample={}", target_rate)
            };
            
            args.push("-af".to_string());
            args.push(af_filter);
            
            // 添加比特率参数（如果检测到）
            if let Some(bitrate_val) = bitrate {
                println!("[FFmpeg] 使用检测到的比特率: {} bps", bitrate_val);
                args.push("-b:a".to_string());
                args.push(bitrate_val.to_string());
            }
            
            // 添加输出路径
            args.push(transcoded.clone());
            
            println!("[FFmpeg] 执行命令: ffmpeg {:?}", args);
            
            let mut cmd = Command::new(&ffmpeg_path);
            cmd.args(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                .stdout(Stdio::null())
                .stderr(Stdio::piped()); // 捕获错误输出
            
            // 在Windows系统上设置CREATE_NO_WINDOW标志，隐藏控制台窗口
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }
            
            let output = cmd.output();
            
            let mut cache = cache.lock().unwrap();
            if let Some(item) = cache.get_mut(&key) {
                match output {
                    Ok(result) if result.status.success() => {
                        println!("[FFmpeg] 转码完成: {}", transcoded);
                        item.is_transcoding = false;
                        item.is_ready = true;
                    }
                    Ok(result) => {
                        // 转码失败，打印错误信息
                        if let Ok(error_message) = std::str::from_utf8(&result.stderr) {
                            eprintln!("[FFmpeg] 转码失败: {}", original);
                            eprintln!("[FFmpeg] 错误信息: {}", error_message);
                        }
                        item.is_transcoding = false;
                        item.is_ready = false;
                    }
                    Err(e) => {
                        // 执行命令失败
                        eprintln!("[FFmpeg] 执行命令失败: {}", e);
                        item.is_transcoding = false;
                        item.is_ready = false;
                    }
                }
            }
        });
        
        Ok(transcoded_path.to_string_lossy().to_string())
    }
    
    // 开始转码（默认调用带音频信息的版本）
    pub fn start_transcode(&self, original_path: &str) -> Result<String, String> {
        self.start_transcode_with_info(original_path, None)
    }
    
    // 等待转码完成（带超时）
    pub fn wait_for_transcode(&self, original_path: &str, timeout_secs: u64) -> Option<String> {
        let cache_key = Self::get_cache_key(original_path);
        let start = SystemTime::now();
        let mut last_ready_state = false;
        
        loop {
            {
                let cache = self.cache.lock().unwrap();
                if let Some(item) = cache.get(&cache_key) {
                    // 检查转码是否完成
                    let is_ready = item.is_ready && Path::new(&item.transcoded_path).exists();
                    
                    // 如果状态发生变化，打印日志
                    if is_ready != last_ready_state {
                        if is_ready {
                            println!("[转码] 转码完成，文件已就绪: {}", item.transcoded_path);
                        }
                        last_ready_state = is_ready;
                    }
                    
                    if is_ready {
                        return Some(item.transcoded_path.clone());
                    }
                    // 检查转码是否失败（不再转码且未准备就绪）
                    if !item.is_transcoding && !item.is_ready {
                        println!("[转码] 转码失败，停止等待");
                        return None;
                    }
                }
            }
            
            // 检查超时
            if SystemTime::now().duration_since(start).unwrap_or(Duration::MAX) > Duration::from_secs(timeout_secs) {
                println!("[转码] 转码超时，等待时间超过 {} 秒", timeout_secs);
                return None;
            }
            
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    
    // 清理过期缓存
    pub fn cleanup_cache(&self, max_age_hours: u64) -> Result<usize, String> {
        let mut cache = self.cache.lock().unwrap();
        let now = SystemTime::now();
        let mut removed_count = 0;
        
        let keys_to_remove: Vec<String> = cache
            .iter()
            .filter_map(|(key, item)| {
                if let Ok(age) = now.duration_since(item.created_at) {
                    if age > Duration::from_secs(max_age_hours * 3600) {
                        // 删除文件
                        let _ = fs::remove_file(&item.transcoded_path);
                        return Some(key.clone());
                    }
                }
                None
            })
            .collect();
        
        for key in keys_to_remove {
            cache.remove(&key);
            removed_count += 1;
        }
        
        Ok(removed_count)
    }
}

// 全局转码缓存实例
use once_cell::sync::Lazy;

static TRANSCODE_CACHE: Lazy<Arc<Mutex<Option<TranscodeCache>>>> = 
    Lazy::new(|| Arc::new(Mutex::new(None)));

// 初始化转码缓存
pub fn init_transcode_cache() -> Result<(), String> {
    let cache = TranscodeCache::new()?;
    let mut global = TRANSCODE_CACHE.lock().unwrap();
    *global = Some(cache);
    Ok(())
}

// 获取转码缓存
pub fn get_transcode_cache() -> Option<Arc<TranscodeCache>> {
    let global = TRANSCODE_CACHE.lock().unwrap();
    global.as_ref().map(|cache| Arc::new(TranscodeCache {
        cache: cache.cache.clone(),
        cache_dir: cache.cache_dir.clone(),
    }))
}

// 检查文件是否需要转码
#[tauri::command]
pub fn check_needs_transcode(path: String) -> bool {
    needs_transcode(&path)
}

// 预转码音频文件（后台静默转码）
#[tauri::command]
pub async fn pretranscode_audio(path: String, force_transcode: Option<bool>) -> Result<String, String> {
    let cache = get_transcode_cache()
        .ok_or_else(|| "转码缓存未初始化".to_string())?;
    
    let force_transcode = force_transcode.unwrap_or(false);
    
    if !needs_transcode(&path) && !force_transcode {
        println!("[预转码] 文件不需要转码: {}", path);
        return Ok(path);
    }
    
    // 获取或创建缓存项
    let item = cache.get_or_create(&path)
        .ok_or_else(|| "创建缓存项失败".to_string())?;
    
    // 如果已经转码完成，直接返回
    if item.is_ready && std::path::Path::new(&item.transcoded_path).exists() && !force_transcode {
        println!("[预转码] 文件已转码完成: {}", item.transcoded_path);
        return Ok(item.transcoded_path.clone());
    }
    
    // 如果正在转码中，直接返回
    if item.is_transcoding {
        println!("[预转码] 文件正在转码中: {}", path);
        return Ok(item.transcoded_path.clone());
    }
    
    // 开始转码
    println!("[预转码] 开始预转码: {}", path);
    let transcoded_path = cache.start_transcode(&path)?;
    println!("[预转码] 预转码已启动: {}", transcoded_path);
    
    Ok(transcoded_path)
}

// 获取转码后的路径（等待转码完成）
#[tauri::command]
pub async fn get_transcoded_path(path: String, timeout_secs: Option<u64>, audio_info: Option<serde_json::Value>) -> Result<String, String> {
    let cache = get_transcode_cache()
        .ok_or_else(|| "转码缓存未初始化".to_string())?;
    
    if !needs_transcode(&path) {
        // 对于不需要转码的文件，也返回HTTP URL
        if let Some(http_url) = crate::http_server::get_file_url(&path) {
            return Ok(http_url);
        } else {
            return Ok(path);
        }
    }
    
    // 获取或创建缓存项
    let item = cache.get_or_create(&path)
        .ok_or_else(|| "创建缓存项失败".to_string())?;
    
    // 如果已经转码完成，直接返回HTTP URL
    if item.is_ready && std::path::Path::new(&item.transcoded_path).exists() {
        if let Some(http_url) = crate::http_server::get_file_url(&item.transcoded_path) {
            return Ok(http_url);
        } else {
            return Ok(item.transcoded_path);
        }
    }
    
    // 开始转码
    let _transcoded_path = cache.start_transcode_with_info(&path, audio_info)?;
    
    // 等待转码完成
    // 使用前端传递的超时时间，如果没有传递则使用默认值300秒（5分钟）
    let timeout = timeout_secs.unwrap_or(300);
    println!("[转码] 等待转码完成，超时时间: {}秒", timeout);
    match cache.wait_for_transcode(&path, timeout) {
        Some(transcoded_path) => {
            // 返回HTTP URL
            if let Some(http_url) = crate::http_server::get_file_url(&transcoded_path) {
                Ok(http_url)
            } else {
                Ok(transcoded_path)
            }
        }
        None => Err("转码超时".to_string()),
    }
}

// FFplay播放器模块
use std::process::Child;

// FFplay播放器状态
#[derive(Clone, serde::Serialize)]
pub struct FFplayStatus {
    pub is_playing: bool,
    pub duration: f64,
    pub position: f64,
    pub volume: f32,
}

// 全局FFplay进程句柄
static FFPLAY_PROCESS: Mutex<Option<Child>> = Mutex::new(None);
static FFPLAY_STATUS: Mutex<FFplayStatus> = Mutex::new(FFplayStatus {
    is_playing: false,
    duration: 0.0,
    position: 0.0,
    volume: 1.0,
});
// FFplay 进程计数器
static FFPLAY_PROCESS_COUNT: AtomicU32 = AtomicU32::new(0);
// FFplay 进程数量限制
const MAX_FFPLAY_PROCESSES: u32 = 3;

// 保存暂停时的播放位置
static PAUSED_POSITION: Mutex<Option<f64>> = Mutex::new(None);

// 监控线程句柄
static MONITOR_THREAD: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

// 清理FFplay资源（在程序退出时调用）
pub fn cleanup_ffplay() {
    println!("[FFplay] 清理FFplay资源");
    
    // 停止FFplay播放
    let _ = stop_ffplay();
    
    // 确保进程已完全停止
    let mut process = FFPLAY_PROCESS.lock().unwrap();
    if let Some(mut child) = process.take() {
        match child.try_wait() {
            Ok(Some(_)) => {
                println!("[FFplay] 进程已结束");
            }
            Ok(None) => {
                // 进程仍在运行，强制终止
                println!("[FFplay] 强制终止FFplay进程");
                let _ = child.kill();
                // 减少进程计数
            FFPLAY_PROCESS_COUNT.fetch_sub(1, Ordering::SeqCst);
            println!("[FFplay] 进程已结束，当前进程数量：{}/{}", FFPLAY_PROCESS_COUNT.load(Ordering::SeqCst), MAX_FFPLAY_PROCESSES);
            }
            Err(e) => {
                println!("[FFplay] 检查进程状态失败: {:?}", e);
            }
        }
    }
    
    println!("[FFplay] FFplay资源清理完成");
}

// 使用FFplay播放音频文件
#[tauri::command]
pub async fn play_with_ffplay(path: String, start_time: Option<f64>, duration: Option<f64>) -> Result<serde_json::Value, String> {
    // 先停止当前播放
    stop_ffplay()?;
    
    // 检查FFplay是否可用
    let ffplay_path = TranscodeCache::get_ffplay_path()
        .ok_or_else(|| "FFplay未找到，请确保已安装FFplay或使用内置版本".to_string())?;
    
    println!("[FFplay] 使用ffplay播放: {}", path);
    
    // 构建FFplay命令
    let mut cmd = Command::new(&ffplay_path);
    
    // 设置FFplay参数
    cmd.arg("-autoexit"); // 播放完成后自动退出
    cmd.arg("-nodisp");  // 不显示视频窗口
    cmd.arg("-loglevel"); cmd.arg("error"); // 错误模式，只显示错误
    
    // 设置起始时间
    if let Some(start) = start_time {
        cmd.arg("-ss");
        cmd.arg(start.to_string());
    }
    
    // 设置音量
    cmd.arg("-volume");
    cmd.arg("100");
    
    // 添加音频文件路径
    cmd.arg(&path);
    
    // 在Windows系统上设置CREATE_NO_WINDOW标志，隐藏控制台窗口
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    
    // 重定向输出到文件，便于调试
    let temp_dir = std::env::temp_dir();
    let log_path = temp_dir.join("ffplay_output.log");
    let log_file = std::fs::File::create(&log_path).map_err(|e| format!("创建日志文件失败: {}", e))?;
    cmd.stdout(log_file.try_clone().map_err(|e| format!("克隆文件句柄失败: {}", e))?);

    // 不重定向错误输出，以便捕获FFplay的错误信息
    // cmd.stderr(Stdio::null());

    // 重定向stdin，以便发送控制命令
    cmd.stdin(Stdio::piped());
    
    // 打印完整的FFplay命令
    println!("[FFplay] 启动命令: {:?}", cmd);
    println!("[FFplay] FFplay日志文件: {:?}", log_path);

    // 启动FFplay进程
    let child = cmd.spawn()
        .map_err(|e| format!("启动FFplay失败: {}", e))?;

    println!("[FFplay] 进程已启动，PID: {}", child.id());

    // 保存进程句柄
    {
        let mut process = FFPLAY_PROCESS.lock().unwrap();
        *process = Some(child);
    }

    // 获取音频文件信息
    let audio_info = get_audio_info(&path);
    let mut final_duration = 300.0; // 先设置默认值
    
    // 更新播放状态
    {
        let mut status = FFPLAY_STATUS.lock().unwrap();
        status.is_playing = true;
        status.position = start_time.unwrap_or(0.0);

    // 尝试获取音频时长，优先使用ffprobe获取的时长
        if let Some(info) = &audio_info {
            status.duration = info.duration;
            final_duration = info.duration;
            println!("[FFplay] 使用ffprobe获取的音频时长: {}秒", info.duration);
        } else if let Some(d) = duration {
            status.duration = d;
            final_duration = d;
            println!("[FFplay] 使用前端传递的音频时长: {}秒", d);
        } else {
            status.duration = 300.0;
            println!("[FFplay] 无法获取音频时长，使用默认值: 300秒");
        }

        // 打印初始状态
        println!("[FFplay] 初始状态: is_playing={}, position={}, duration={}",
            status.is_playing, status.position, status.duration);
    }
    
    // 启动状态监控线程
    start_ffplay_monitor();
    
    // 构建返回对象，包含音频文件信息
    let mut response = serde_json::json!({
        "success": true,
        "message": format!("FFplay已开始播放: {}", path),
        "path": path,
        "duration": final_duration,
        "position": start_time.unwrap_or(0.0),
        "is_ffplay": true
    });
    
    // 添加音频文件详细信息
    if let Some(info) = audio_info {
        response["format"] = serde_json::Value::String(info.format);
        if let Some(sample_rate) = info.sample_rate {
            response["sample_rate"] = serde_json::Value::Number(serde_json::Number::from(sample_rate));
        }
        if let Some(channels) = info.channels {
            response["channels"] = serde_json::Value::Number(serde_json::Number::from(channels));
        }
        if let Some(bit_rate) = info.bit_rate {
            response["bit_rate"] = serde_json::Value::Number(serde_json::Number::from(bit_rate));
        }
        if let Some(bit_depth) = info.bit_depth {
            response["bit_depth"] = serde_json::Value::Number(serde_json::Number::from(bit_depth));
        }
    }
    
    Ok(response)
}

// 停止FFplay播放
#[tauri::command]
pub fn stop_ffplay() -> Result<String, String> {
    let mut process = FFPLAY_PROCESS.lock().unwrap();
    
    if let Some(mut child) = process.take() {
        // 尝试优雅地终止进程
        match child.kill() {
            Ok(_) => {
                println!("[FFplay] 已停止播放");
                // 不等待进程结束，避免阻塞
                // // 减少进程计数
            FFPLAY_PROCESS_COUNT.fetch_sub(1, Ordering::SeqCst);
            println!("[FFplay] 进程已结束，当前进程数量：{}/{}", FFPLAY_PROCESS_COUNT.load(Ordering::SeqCst), MAX_FFPLAY_PROCESSES);
            }
            Err(e) => {
                println!("[FFplay] 停止播放失败: {}", e);
                return Err(format!("停止FFplay失败: {}", e));
            }
        }
    }
    
    // 更新播放状态
    {
        let mut status = FFPLAY_STATUS.lock().unwrap();
        status.is_playing = false;
        // 不重置position，避免播放进度回到0
        // status.position = 0.0;
    }
    
    Ok("FFplay已停止".to_string())
}

// 暂停FFplay播放（通过发送空格键）
#[tauri::command]
pub fn pause_ffplay() -> Result<String, String> {
    let mut process = FFPLAY_PROCESS.lock().unwrap();
    
    if let Some(ref mut child) = *process {
        // 向FFplay发送暂停命令（p键）
        if let Some(ref mut stdin) = child.stdin {
            use std::io::Write;
            if let Err(e) = stdin.write_all(b"p") {
                println!("[FFplay] 发送暂停命令失败: {}", e);
                return Err(format!("发送暂停命令失败: {}", e));
            }
            if let Err(e) = stdin.flush() {
                println!("[FFplay] 刷新stdin失败: {}", e);
                return Err(format!("刷新stdin失败: {}", e));
            }
            println!("[FFplay] 已发送暂停命令");
            
            // 更新播放状态
            let mut status = FFPLAY_STATUS.lock().unwrap();
            status.is_playing = false;
            
            // 保存当前播放位置
            let paused_position = status.position;
            drop(status);
            
            // 保存到全局变量
            let mut paused_pos = PAUSED_POSITION.lock().unwrap();
            *paused_pos = Some(paused_position);
            drop(paused_pos);
            
            println!("[FFplay] 暂停播放，保存位置: {:.2}秒", paused_position);
            
            return Ok("FFplay已暂停".to_string());
        }
        return Err("无法访问FFplay进程的stdin".to_string());
    }
    
    Err("FFplay未在播放".to_string())
}

// 恢复FFplay播放
#[tauri::command]
pub fn resume_ffplay() -> Result<String, String> {
    let mut process = FFPLAY_PROCESS.lock().unwrap();
    
    if let Some(ref mut child) = *process {
        // 向FFplay发送恢复命令（p键）
        if let Some(ref mut stdin) = child.stdin {
            use std::io::Write;
            if let Err(e) = stdin.write_all(b"p") {
                println!("[FFplay] 发送恢复命令失败: {}", e);
                return Err(format!("发送恢复命令失败: {}", e));
            }
            if let Err(e) = stdin.flush() {
                println!("[FFplay] 刷新stdin失败: {}", e);
                return Err(format!("刷新stdin失败: {}", e));
            }
            println!("[FFplay] 已发送恢复命令");
            
            // 保存当前播放位置
            let paused_position = {
                let paused_pos = PAUSED_POSITION.lock().unwrap();
                *paused_pos
            };
            
            // 更新播放状态
            let mut status = FFPLAY_STATUS.lock().unwrap();
            status.is_playing = true;
            
            // 从保存的位置恢复
            if let Some(pos) = paused_position {
                status.position = pos;
            }
            
            println!("[FFplay] 恢复播放");
            
            return Ok("FFplay已恢复播放".to_string());
        }
        return Err("无法访问FFplay进程的stdin".to_string());
    }
    
    Err("FFplay未在播放".to_string())
}

// 跳转到指定位置
#[tauri::command]
pub async fn seek_ffplay(path: String, position: f64) -> Result<serde_json::Value, String> {
    println!("[FFplay] 开始seek到位置: {:.2}秒, 路径: {}", position, path);
    
    // 获取当前播放状态
    let was_playing = {
        let status = FFPLAY_STATUS.lock().unwrap();
        status.is_playing
    };
    
    // 停止当前播放
    stop_ffplay()?;
    
    // 从指定位置重新开始播放
    let result = play_with_ffplay(path, Some(position), None).await;
    
    // 如果之前在播放，确保恢复播放状态
    if was_playing {
        let mut status = FFPLAY_STATUS.lock().unwrap();
        status.is_playing = true;
    }
    
    println!("[FFplay] seek完成: {:?}", result);
    result
}

// 设置音量
#[tauri::command]
pub fn set_ffplay_volume(volume: f32) -> Result<String, String> {
    let mut status = FFPLAY_STATUS.lock().unwrap();
    status.volume = volume.clamp(0.0, 1.0);
    
    // FFplay不支持动态调整音量，需要重新启动
    Ok(format!("音量已设置为: {}", status.volume))
}

// 启动FFplay状态监控线程
fn start_ffplay_monitor() {
    // 停止之前的监控线程
    let mut old_thread = MONITOR_THREAD.lock().unwrap();
    if let Some(handle) = old_thread.take() {
        drop(old_thread);
        // 等待旧线程结束
        let _ = handle.join();
        println!("[FFplay] 已停止旧的监控线程");
    }
    
    let thread = std::thread::spawn(|| {
        println!("[FFplay] 状态监控线程已启动");
        
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500)); // 每500毫秒更新一次，提高响应速度
            
            // 检查是否有FFplay进程在运行
            let mut process = FFPLAY_PROCESS.lock().unwrap();
            if let Some(child) = process.as_mut() {
                // 检查进程是否还在运行
                match child.try_wait() {
                    Ok(Some(exit_status)) => {
                        // 进程已结束，更新状态
                        println!("[FFplay] 进程已结束，退出状态: {:?}", exit_status);
                        drop(process);
                        let _ = stop_ffplay();
                        break;
                    }
                    Ok(None) => {
                        // 进程仍在运行，更新播放位置
                        drop(process);
                        
                        let mut status = FFPLAY_STATUS.lock().unwrap();
                        // 检查是否正在播放且进程在运行
                        if status.is_playing && status.position < status.duration {
                            // 只有当位置小于时长时才增加
                            if status.position < status.duration - 0.5 {
                                status.position += 0.5; // 每500毫秒增加0.5秒
                            } else {
                                // 已经接近结尾，将位置设置为时长，标记为播放完成
                                status.position = status.duration;
                                status.is_playing = false;
                                println!("[FFplay] 播放状态: 播放完成, 位置: {:.2}秒, 总时长: {:.2}秒", status.position, status.duration);
                            }
                            println!("[FFplay] 播放状态: 正在播放, 位置: {:.2}秒, 总时长: {:.2}秒", status.position, status.duration);
                        } else if !status.is_playing {
                            // 暂停状态，不更新位置
                            println!("[FFplay] 播放状态: 已暂停, 位置: {:.2}秒, 总时长: {:.2}秒", status.position, status.duration);
                        }
                    }
                    Err(e) => {
                        // 检查失败，继续监控
                        println!("[FFplay] 检查进程状态失败: {:?}", e);
                    }
                }
            } else {
                // 没有FFplay进程，退出监控
                println!("[FFplay] 没有FFplay进程，退出监控");
                break;
            }
        }
        
        println!("[FFplay] 状态监控线程已退出");
    });
    
    // 保存线程句柄
    let mut thread_handle = MONITOR_THREAD.lock().unwrap();
    *thread_handle = Some(thread);
}

// 获取FFplay播放状态
#[tauri::command]
pub fn get_ffplay_status() -> Result<FFplayStatus, String> {
    let status = FFPLAY_STATUS.lock().unwrap();
    println!("[get_ffplay_status] 返回状态: is_playing={}, position={:.2}, duration={:.2}",
        status.is_playing, status.position, status.duration);
    Ok(status.clone())
}

// 音频文件信息结构体
#[derive(Debug, Clone, serde::Serialize)]
pub struct AudioFileInfo {
    pub duration: f64,         // 时长（秒）
    pub format: String,        // 音频格式
    pub sample_rate: Option<u32>, // 采样率（Hz）
    pub channels: Option<u32>,   // 声道数
    pub bit_rate: Option<u32>,   // 比特率（bps）
    pub bit_depth: Option<u32>,  // 比特深度
}

// 获取音频文件信息（使用ffprobe）
fn get_audio_info(path: &str) -> Option<AudioFileInfo> {
    let ffprobe_path = TranscodeCache::get_ffprobe_path()?;
    println!("[FFplay] 正在使用ffprobe获取音频文件信息: {}, 路径: {}", ffprobe_path, path);
    
    // 使用ffprobe获取详细的音频文件信息
    let output = Command::new(&ffprobe_path)
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration,format_name,bit_rate:stream=codec_name,sample_rate,channels,bits_per_raw_sample")
        .arg("-of")
        .arg("json")
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;
    
    if output.status.success() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        println!("[FFplay] ffprobe输出: {}", output_str);
        
        // 解析JSON输出
        #[derive(Debug, serde::Deserialize)]
        struct Stream {
            codec_name: Option<String>,
            sample_rate: Option<String>,
            channels: Option<u32>,
            bits_per_raw_sample: Option<String>,
        }
        
        #[derive(Debug, serde::Deserialize)]
        struct Format {
            duration: Option<String>,
            format_name: Option<String>,
            bit_rate: Option<String>,
        }
        
        #[derive(Debug, serde::Deserialize)]
        struct FFprobeOutput {
            streams: Vec<Stream>,
            format: Format,
        }
        
        match serde_json::from_str::<FFprobeOutput>(&output_str) {
            Ok(parsed) => {
                // 提取音频流信息（通常是第一个流）
                let audio_stream = parsed.streams.first();
                
                // 解析时长
                let duration = parsed.format.duration
                    .and_then(|d| d.parse::<f64>().ok())
                    .unwrap_or(300.0); // 默认300秒
                
                // 解析格式名称
                let format = audio_stream
                    .and_then(|s| s.codec_name.clone())
                    .or_else(|| parsed.format.format_name.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                
                // 解析采样率
                let sample_rate = audio_stream
                    .and_then(|s| s.sample_rate.as_ref().and_then(|sr| sr.parse::<u32>().ok()));
                
                // 解析声道数
                let channels = audio_stream.and_then(|s| s.channels);
                
                // 解析比特率
                let bit_rate = parsed.format.bit_rate
                    .and_then(|br| br.parse::<u32>().ok());
                
                // 解析比特深度
                let bit_depth = audio_stream
                    .and_then(|s| s.bits_per_raw_sample.as_ref().and_then(|bd| bd.parse::<u32>().ok()));
                
                let info = AudioFileInfo {
                    duration,
                    format,
                    sample_rate,
                    channels,
                    bit_rate,
                    bit_depth,
                };
                
                println!("[FFplay] 解析音频文件信息成功: {:?}", info);
                Some(info)
            }
            Err(e) => {
                println!("[FFplay] 解析ffprobe输出失败: {}", e);
                None
            }
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("[FFplay] ffprobe执行失败: {}", stderr);
        None
    }
}

// 获取音频时长（使用ffprobe）
fn get_audio_duration(path: &str) -> Option<f64> {
    get_audio_info(path).map(|info| info.duration)
}

// 检查文件是否需要使用FFplay播放（原引擎不支持的无损音频）
pub fn needs_ffplay_playback(path: &str) -> bool {
    // 检查文件扩展名
    let path_lower = path.to_lowercase();
    let unsupported_formats = [
        ".dsf",  // DSD格式
        ".dff",  // DSD格式
        ".dsd",  // DSD格式
        ".mqa",  // MQA格式
        ".wv",   // WavPack格式
        ".tta",  // TTA格式
        ".ape",  // Monkey's Audio格式
        ".wma",  // Windows Media Audio
        ".m4a",  // AAC格式（某些情况下）
        ".aac",  // AAC格式
    ];
    
    for ext in &unsupported_formats {
        if path_lower.ends_with(ext) {
            return true;
        }
    }
    
    false
}
