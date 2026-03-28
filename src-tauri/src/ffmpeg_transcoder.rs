use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
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
        // 1. 首先检查环境变量
        if let Ok(ffmpeg_path) = env::var("FFMPEG_PATH") {
            if Path::new(&ffmpeg_path).exists() {
                return Some(ffmpeg_path);
            }
        }
        
        // 2. 检查 PATH 中的 ffmpeg
        if Command::new("ffmpeg").arg("-version").output().is_ok() {
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
        // 1. 首先检查环境变量
        if let Ok(ffprobe_path) = env::var("FFPROBE_PATH") {
            if Path::new(&ffprobe_path).exists() {
                return Some(ffprobe_path);
            }
        }
        
        // 2. 检查 PATH 中的 ffprobe
        if Command::new("ffprobe").arg("-version").output().is_ok() {
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
    
    // 获取或创建转码缓存项
    pub fn get_or_create(&self, original_path: &str) -> Option<TranscodeCacheItem> {
        let cache_key = Self::get_cache_key(original_path);
        let transcoded_path = self.cache_dir.join(format!("{}.flac", cache_key));
        
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
    
    // 开始转码（使用前端传递的音频信息）
    pub fn start_transcode_with_info(&self, original_path: &str, audio_info_json: Option<serde_json::Value>) -> Result<String, String> {
        let ffmpeg_path = Self::get_ffmpeg_path()
            .ok_or_else(|| "未找到 FFmpeg，请设置 FFMPEG_PATH 环境变量".to_string())?;
        
        let cache_key = Self::get_cache_key(original_path);
        // 使用缓存键作为转码文件名，与get_or_create保持一致
        let transcoded_path = self.cache_dir.join(format!("{}.flac", cache_key));
        
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
            let probe_result = Command::new(&ffprobe_path)
                .arg("-hide_banner")
                .arg(&original)
                .arg("-show_streams")
                .arg("-select_streams")
                .arg("a")
                .arg("-print_format")
                .arg("json")
                .output();
            
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
