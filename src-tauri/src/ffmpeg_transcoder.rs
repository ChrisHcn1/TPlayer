use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::{SystemTime, Duration};
use std::fs;
use std::env;

// 日志开关：设置为 false 可禁用所有日志输出
const ENABLE_LOGS: bool = false;

// 条件性日志宏
macro_rules! log_info {
    ($($arg:tt)*) => {
        if ENABLE_LOGS {
            println!($($arg)*);
        }
    };
}

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
pub struct TranscodeCacheItem {
    pub original_path: String,
    pub transcoded_path: String,
    pub created_at: SystemTime,
    pub is_transcoding: bool,
    pub is_ready: bool,
}

// 转码缓存管理器
pub struct TranscodeCache {
    cache: Arc<Mutex<HashMap<String, TranscodeCacheItem>>>,
    cache_dir: PathBuf,
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
    
    // 检查文件是否需要转码
    pub fn needs_transcode(path: &str) -> bool {
        let path_lower = path.to_lowercase();
        
        // 首先检查是否在原生支持列表中
        if NATIVE_SUPPORTED_EXTENSIONS.iter().any(|ext| {
            path_lower.ends_with(&format!(".{}", ext))
        }) {
            return false; // 原生支持，无需转码
        }
        
        // 检查是否在需要转码的格式列表中
        if TRANSCODE_EXTENSIONS.iter().any(|ext| {
            path_lower.ends_with(&format!(".{}", ext))
        }) {
            return true; // 需要转码
        }
        
        // 检查文件名或路径中是否包含DTS相关标识（特殊情况处理）
        if path_lower.contains("dts") || path_lower.contains("dts-cd") {
            return true;
        }
        
        false // 其他格式默认不需要转码
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
    
    // 生成缓存键（使用原文件名）
    fn get_cache_key(path: &str) -> String {
        let path = Path::new(path);
        if let Some(file_name) = path.file_name() {
            if let Some(file_stem) = file_name.to_str() {
                // 使用文件名（去除扩展名）作为缓存键
                return file_stem.to_string();
            }
        }
        //  fallback: 使用哈希值
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
    
    // 开始转码
    pub fn start_transcode(&self, original_path: &str) -> Result<String, String> {
        let ffmpeg_path = Self::get_ffmpeg_path()
            .ok_or_else(|| "未找到 FFmpeg，请设置 FFMPEG_PATH 环境变量".to_string())?;
        
        let cache_key = Self::get_cache_key(original_path);
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
                    cache.insert(cache_key, item);
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
        let key = cache_key.clone();
        
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
                cache.insert(cache_key, item);
            }
        }
        
        let original = original_path.to_string();
        let transcoded = transcoded_path.to_string_lossy().to_string();
        let cache = self.cache.clone();
        
        // 在后台线程中执行转码
        std::thread::spawn(move || {
            println!("[FFmpeg] 开始转码: {} -> {}", original, transcoded);
            
            // 先使用ffprobe获取音频信息
            let probe_result = Command::new(&ffmpeg_path)
                .arg("-hide_banner")
                .arg("-show_streams")
                .arg("-select_streams")
                .arg("a")
                .arg("-of")
                .arg("json")
                .arg(&original)
                .output();
            
            let sample_rate = match &probe_result {
                Ok(result) if result.status.success() => {
                    // 解析JSON输出获取采样率
                    if let Ok(json_str) = std::str::from_utf8(&result.stdout) {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                            if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
                                if let Some(first_stream) = streams.first() {
                                    if let Some(rate) = first_stream.get("sample_rate").and_then(|r| r.as_str()) {
                                        if let Ok(rate_val) = rate.parse::<u32>() {
                                            Some(rate_val)
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None
            };
            
            println!("[FFmpeg] 检测到采样率: {:?}", sample_rate);
            
            // 根据文件扩展名确定转码参数
            let mut args: Vec<String> = vec![
                "-i".to_string(),
                original.clone(),
                "-c:a".to_string(),
                "flac".to_string(),
                "-compression_level".to_string(),
                "12".to_string(), // 最高压缩级别，获得最佳音质
                "-y".to_string(), // 覆盖已存在的文件
            ];
            
            // 检测文件扩展名
            let path = Path::new(&original);
            let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
            
            // 对于DSD格式，添加特殊处理
            if extension.eq_ignore_ascii_case("dsf") || extension.eq_ignore_ascii_case("dff") {
                // DSD格式需要特殊的解码和重采样处理
                // 使用检测到的采样率，如果检测失败则使用默认值
                let target_rate = sample_rate.unwrap_or(48000);
                let af_filter = format!("aformat=sample_fmts=s16:channel_layouts=stereo,aresample={}", target_rate);
                args.push("-af".to_string());
                args.push(af_filter);
            } else if extension.eq_ignore_ascii_case("dts") || extension.eq_ignore_ascii_case("dtshd") || extension.eq_ignore_ascii_case("dtshd_ma") {
                // DTS格式需要特殊处理，转换为立体声
                let af_filter = "aformat=sample_fmts=s16:channel_layouts=stereo,aresample=48000".to_string();
                args.push("-af".to_string());
                args.push(af_filter);
            } else {
                // 其他格式使用常规处理
                args.push("-af".to_string());
                args.push("aresample=48000".to_string());
            }
            
            // 添加输出路径
            args.push(transcoded.clone());
            
            println!("[FFmpeg] 执行命令: ffmpeg {:?}", args);
            
            let output = Command::new(&ffmpeg_path)
                .args(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                .stdout(Stdio::null())
                .stderr(Stdio::piped()) // 捕获错误输出
                .output();
            
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
    
    // 等待转码完成（带超时）
    pub fn wait_for_transcode(&self, original_path: &str, timeout_secs: u64) -> Option<String> {
        let cache_key = Self::get_cache_key(original_path);
        let start = SystemTime::now();
        
        loop {
            {
                let cache = self.cache.lock().unwrap();
                if let Some(item) = cache.get(&cache_key) {
                    if item.is_ready && Path::new(&item.transcoded_path).exists() {
                        return Some(item.transcoded_path.clone());
                    }
                }
            }
            
            // 检查超时
            if SystemTime::now().duration_since(start).unwrap_or(Duration::MAX) > Duration::from_secs(timeout_secs) {
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
    TranscodeCache::needs_transcode(&path)
}

// 预转码音频文件（后台静默转码）
#[tauri::command]
pub async fn pretranscode_audio(path: String) -> Result<String, String> {
    let cache = get_transcode_cache()
        .ok_or_else(|| "转码缓存未初始化".to_string())?;
    
    if !TranscodeCache::needs_transcode(&path) {
        println!("[预转码] 文件不需要转码: {}", path);
        return Ok(path);
    }
    
    // 获取或创建缓存项
    let item = cache.get_or_create(&path)
        .ok_or_else(|| "创建缓存项失败".to_string())?;
    
    // 如果已经转码完成，直接返回
    if item.is_ready && std::path::Path::new(&item.transcoded_path).exists() {
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
pub async fn get_transcoded_path(path: String, timeout_secs: Option<u64>) -> Result<String, String> {
    let cache = get_transcode_cache()
        .ok_or_else(|| "转码缓存未初始化".to_string())?;
    
    if !TranscodeCache::needs_transcode(&path) {
        return Ok(path);
    }
    
    // 获取或创建缓存项
    let item = cache.get_or_create(&path)
        .ok_or_else(|| "创建缓存项失败".to_string())?;
    
    // 如果已经转码完成，直接返回
    if item.is_ready && std::path::Path::new(&item.transcoded_path).exists() {
        return Ok(item.transcoded_path);
    }
    
    // 开始转码
    let _transcoded_path = cache.start_transcode(&path)?;
    
    // 等待转码完成
    let timeout = timeout_secs.unwrap_or(30);
    match cache.wait_for_transcode(&path, timeout) {
        Some(path) => Ok(path),
        None => Err("转码超时".to_string()),
    }
}
