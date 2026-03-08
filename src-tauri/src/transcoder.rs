use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use serde::{Serialize, Deserialize};
use log;

/// 转码任务状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TranscodeStatus {
    Pending,      // 等待转码
    InProgress,   // 正在转码
    Completed,    // 转码完成
    Failed(String), // 转码失败
}

/// 转码任务信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscodeTask {
    pub source_path: String,
    pub output_path: String,
    pub status: TranscodeStatus,
    pub progress: f32,        // 转码进度 0.0 - 100.0
    pub created_at: SystemTime,
    pub completed_at: Option<SystemTime>,
}

/// 转码管理器
pub struct Transcoder {
    /// 转码缓存目录
    cache_dir: PathBuf,
    /// 转码任务映射表
    tasks: Arc<Mutex<HashMap<String, TranscodeTask>>>,
    /// 当前正在转码的任务
    current_task: Arc<Mutex<Option<String>>>,
    /// 是否启用转码
    enabled: bool,
    /// 预转码提前时间（秒）
    preload_seconds: u64,
    /// 输出格式
    output_format: String,
    /// 输出音质
    output_quality: String,
}

impl Transcoder {
    /// 创建新的转码管理器
    pub fn new(cache_dir: PathBuf) -> Self {
        // 确保缓存目录存在
        if !cache_dir.exists() {
            std::fs::create_dir_all(&cache_dir).unwrap_or_else(|e| {
                log::error!("无法创建转码缓存目录: {}", e);
            });
        }
        
        Self {
            cache_dir,
            tasks: Arc::new(Mutex::new(HashMap::new())),
            current_task: Arc::new(Mutex::new(None)),
            enabled: false,
            preload_seconds: 15, // 默认提前15秒
            output_format: "flac".to_string(),
            output_quality: "0".to_string(),
        }
    }
    
    /// 检查FFmpeg是否可用
    pub fn check_ffmpeg() -> bool {
        log::info!("[Transcoder] 检查FFmpeg可用性...");
        
        // 1. 首先尝试直接调用 ffmpeg 命令
        match Command::new("ffmpeg")
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status() 
        {
            Ok(status) => {
                if status.success() {
                    log::info!("[Transcoder] ✅ FFmpeg 在系统PATH中找到");
                    return true;
                }
            }
            Err(_) => {
                log::info!("[Transcoder] 直接调用 ffmpeg 失败，检查环境变量...");
            }
        }
        
        // 2. 检查环境变量中的 ffmpeg.exe
        if let Ok(path) = std::env::var("PATH") {
            for dir in path.split(';') {
                let ffmpeg_path = Path::new(dir).join("ffmpeg.exe");
                if ffmpeg_path.exists() {
                    log::info!("[Transcoder] ✅ FFmpeg 在环境变量中找到: {}", ffmpeg_path.display());
                    return true;
                }
            }
        }
        
        // 3. 检查常见的安装位置
        let common_paths = [
            "C:\\Program Files\\ffmpeg\\bin\\ffmpeg.exe",
            "C:\\Program Files (x86)\\ffmpeg\\bin\\ffmpeg.exe",
            "C:\\Users\\Public\\ffmpeg\\bin\\ffmpeg.exe",
        ];
        
        for path in &common_paths {
            let ffmpeg_path = Path::new(path);
            if ffmpeg_path.exists() {
                log::info!("[Transcoder] ✅ FFmpeg 在常见位置找到: {}", ffmpeg_path.display());
                return true;
            }
        }
        
        log::error!("[Transcoder] ❌ 未找到FFmpeg，请确保FFmpeg已安装并在环境变量中");
        log::error!("[Transcoder] 💡 下载地址: https://ffmpeg.org/download.html");
        false
    }
    
    /// 设置是否启用转码
    #[allow(dead_code)]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        log::info!("转码功能已{}", if enabled { "启用" } else { "禁用" });
    }
    
    /// 获取是否启用转码
    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    /// 设置预转码提前时间
    #[allow(dead_code)]
    pub fn set_preload_seconds(&mut self, seconds: u64) {
        self.preload_seconds = seconds;
    }
    
    /// 获取预转码提前时间
    #[allow(dead_code)]
    pub fn get_preload_seconds(&self) -> u64 {
        self.preload_seconds
    }
    
    /// 设置输出格式
    #[allow(dead_code)]
    pub fn set_output_format(&mut self, format: String) {
        self.output_format = format;
    }
    
    /// 设置输出音质
    #[allow(dead_code)]
    pub fn set_output_quality(&mut self, quality: String) {
        self.output_quality = quality;
    }
    
    /// 检查文件是否需要转码
    pub fn needs_transcode(&self, path: &str) -> bool {
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        // 这些格式需要转码
        let need_transcode_formats = [
            "ape", "dsd", "dsf", "dff", "dts", "tta", "wv", "mpc", "tak"
        ];
        
        need_transcode_formats.contains(&ext.as_str())
    }
    
    /// 获取转码后的文件路径
    pub fn get_transcoded_path(&self, source_path: &str) -> Option<String> {
        let tasks = self.tasks.lock().ok()?;
        if let Some(task) = tasks.get(source_path) {
            if matches!(task.status, TranscodeStatus::Completed) {
                return Some(task.output_path.clone());
            }
        }
        None
    }
    
    /// 检查文件是否已转码（基于文件修改时间）
    pub fn is_transcoded(&self, source_path: &str) -> bool {
        if let Some(transcoded_path) = self.get_transcoded_path(source_path) {
            // 检查源文件和转码文件的修改时间
            let source_path = Path::new(source_path);
            let transcoded_path = Path::new(&transcoded_path);
            
            if let (Ok(source_meta), Ok(transcoded_meta)) = (
                source_path.metadata(),
                transcoded_path.metadata()
            ) {
                if let (Ok(source_mtime), Ok(transcoded_mtime)) = (
                    source_meta.modified(),
                    transcoded_meta.modified()
                ) {
                    // 如果转码文件的修改时间晚于源文件，则认为已转码
                    if transcoded_mtime >= source_mtime {
                        log::info!("[Transcoder] 使用缓存的转码文件: {}", transcoded_path.display());
                        return true;
                    } else {
                        log::info!("[Transcoder] 源文件已更新，需要重新转码: {}", source_path.display());
                        return false;
                    }
                }
            }
        }
        false
    }
    
    /// 生成转码后的文件路径
    fn generate_output_path(&self, source_path: &str) -> PathBuf {
        let file_name = Path::new(source_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        
        // 使用源文件路径的哈希作为子目录，避免文件名冲突
        let hash = Self::calculate_hash(source_path);
        let sub_dir = self.cache_dir.join(format!("{:04x}", hash % 65536));
        
        if !sub_dir.exists() {
            let _ = std::fs::create_dir_all(&sub_dir);
        }
        
        sub_dir.join(format!("{}.{}.{}", file_name, hash, self.output_format))
    }
    
    /// 计算字符串哈希
    fn calculate_hash(s: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }
    
    /// 开始转码任务
    pub fn start_transcode(&self, source_path: String) -> Result<(), String> {
        if !self.enabled {
            return Err("转码功能未启用".to_string());
        }
        
        if !Self::check_ffmpeg() {
            return Err("未找到FFmpeg，请确保FFmpeg已安装并在环境变量中".to_string());
        }
        
        // 检查是否已经在转码中
        {
            let tasks = self.tasks.lock().map_err(|e| e.to_string())?;
            if let Some(task) = tasks.get(&source_path) {
                if matches!(task.status, TranscodeStatus::InProgress | TranscodeStatus::Completed) {
                    return Ok(()); // 已经在转码或已完成
                }
            }
        }
        
        let output_path = self.generate_output_path(&source_path);
        let output_path_str = output_path.to_string_lossy().to_string();
        
        // 创建任务
        let task = TranscodeTask {
            source_path: source_path.clone(),
            output_path: output_path_str.clone(),
            status: TranscodeStatus::InProgress,
            progress: 0.0,
            created_at: SystemTime::now(),
            completed_at: None,
        };
        
        {
            let mut tasks = self.tasks.lock().map_err(|e| e.to_string())?;
            tasks.insert(source_path.clone(), task);
        }
        
        // 设置当前任务
        {
            let mut current = self.current_task.lock().map_err(|e| e.to_string())?;
            *current = Some(source_path.clone());
        }
        
        // 在后台线程中执行转码
        let tasks_clone = Arc::clone(&self.tasks);
        let current_task_clone = Arc::clone(&self.current_task);
        let output_format = self.output_format.clone();
        let output_quality = self.output_quality.clone();
        
        std::thread::spawn(move || {
            let result = Self::run_ffmpeg_transcode(
                &source_path,
                &output_path_str,
                &output_format,
                &output_quality,
            );
            
            // 更新任务状态
            if let Ok(mut tasks) = tasks_clone.lock() {
                if let Some(task) = tasks.get_mut(&source_path) {
                    match result {
                        Ok(_) => {
                            task.status = TranscodeStatus::Completed;
                            task.progress = 100.0;
                            task.completed_at = Some(SystemTime::now());
                            log::info!("转码完成: {}", source_path);
                        }
                        Err(e) => {
                            task.status = TranscodeStatus::Failed(e.clone());
                            task.progress = 0.0;
                            log::error!("转码失败: {} - {}", source_path, e);
                        }
                    }
                }
            }
            
            // 清除当前任务
            if let Ok(mut current) = current_task_clone.lock() {
                if *current == Some(source_path) {
                    *current = None;
                }
            }
        });
        
        Ok(())
    }
    
    /// 执行FFmpeg转码
    fn run_ffmpeg_transcode(
        source_path: &str,
        output_path: &str,
        format: &str,
        quality: &str,
    ) -> Result<(), String> {
        log::info!("开始转码: {} -> {}", source_path, output_path);
        log::info!("转码参数: format={}, quality={}", format, quality);
        
        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-i")
            .arg(source_path)
            .arg("-y") // 覆盖输出文件
            .arg("-vn") // 禁用视频
            .arg("-progress") // 启用进度输出
            .arg("pipe:1") // 输出到标准输出
            .arg("-hide_banner"); // 隐藏横幅
        
        // 根据格式设置编码参数，优化音质和音量
        match format {
            "flac" => {
                // FLAC 优化参数
                cmd.arg("-c:a").arg("flac")
                    .arg("-compression_level").arg("5") // 压缩级别 5（平衡速度和文件大小）
                    .arg("-ar").arg("48000") // 采样率 48kHz（保持音质）
                    .arg("-sample_fmt").arg("s16") // 16位采样
                    .arg("-ac").arg("2") // 双声道
                    .arg("-af").arg("volume=1.2,dynaudnorm=p=0.9:s=5"); // 音量增益 1.2 倍 + 动态归一化（解决声音小和噪音问题）
                log::info!("使用FLAC编码，压缩级别5，48kHz采样率，双声道，音量增益1.2，动态归一化");
            }
            "wav" => {
                // WAV 优化参数
                cmd.arg("-c:a").arg("pcm_s16le")
                    .arg("-ar").arg("48000") // 采样率 48kHz
                    .arg("-ac").arg("2") // 双声道
                    .arg("-af").arg("volume=1.2,dynaudnorm=p=0.9:s=5"); // 音量增益 1.2 倍 + 动态归一化
                log::info!("使用WAV编码，PCM 16位，48kHz采样率，双声道，音量增益1.2，动态归一化");
            }
            "mp3" => {
                // MP3 优化参数
                cmd.arg("-c:a").arg("libmp3lame")
                    .arg("-b:a").arg(quality)
                    .arg("-ar").arg("48000") // 采样率 48kHz
                    .arg("-q:a").arg("0") // 最高质量
                    .arg("-ac").arg("2") // 双声道
                    .arg("-af").arg("volume=1.2,dynaudnorm=p=0.9:s=5"); // 音量增益 1.2 倍 + 动态归一化
                log::info!("使用MP3编码，比特率: {}，48kHz采样率，双声道，音量增益1.2，动态归一化", quality);
            }
            _ => {
                // 默认使用 FLAC
                cmd.arg("-c:a").arg("flac")
                    .arg("-compression_level").arg("5")
                    .arg("-ar").arg("48000")
                    .arg("-sample_fmt").arg("s16")
                    .arg("-ac").arg("2")
                    .arg("-af").arg("volume=1.2,dynaudnorm=p=0.9:s=5");
                log::info!("使用默认FLAC编码，压缩级别5，48kHz采样率，双声道，音量增益1.2，动态归一化");
            }
        }
        
        cmd.arg(output_path);
        
        // 输出完整的命令行
        log::info!("执行命令: ffmpeg -i {} -y -vn -c:a flac -compression_level 5 -ar 48000 -sample_fmt s16 -ac 2 -af volume=1.2,dynaudnorm=p=0.9:s=5 {}", 
            source_path, 
            output_path
        );
        
        // 执行命令并捕获输出
        let output = cmd.output()
            .map_err(|e| format!("无法执行FFmpeg: {}", e))?;
        
        if output.status.success() {
            log::info!("转码成功完成: {} -> {}", source_path, output_path);
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::error!("FFmpeg转码失败: {}", stderr);
            Err(format!("FFmpeg转码失败: {}", stderr))
        }
    }

    /// 预转码下一首曲目
    pub fn preload_next_track(&self, next_path: &str) {
        if !self.enabled {
            return;
        }
        
        if !self.needs_transcode(next_path) {
            return; // 不需要转码
        }
        
        if self.is_transcoded(next_path) {
            return; // 已经转码完成
        }
        
        // 检查是否正在转码
        {
            let tasks = match self.tasks.lock() {
                Ok(tasks) => tasks,
                Err(_) => return,
            };
            if let Some(task) = tasks.get(next_path) {
                if matches!(task.status, TranscodeStatus::InProgress) {
                    return; // 已经在转码中
                }
            }
        }
        
        // 开始转码
        if let Err(e) = self.start_transcode(next_path.to_string()) {
            log::error!("预转码失败: {} - {}", next_path, e);
        } else {
            log::info!("开始预转码: {}", next_path);
        }
    }
    
    /// 获取转码状态
    #[allow(dead_code)]
    pub fn get_task_status(&self, source_path: &str) -> Option<TranscodeStatus> {
        let tasks = self.tasks.lock().ok()?;
        tasks.get(source_path).map(|t| t.status.clone())
    }
    
    /// 获取转码进度
    #[allow(dead_code)]
    pub fn get_task_progress(&self, source_path: &str) -> Option<f32> {
        let tasks = self.tasks.lock().ok()?;
        tasks.get(source_path).map(|t| t.progress)
    }
    
    /// 清理过期的转码缓存
    #[allow(dead_code)]
    pub fn cleanup_cache(&self, max_age_days: u64) -> Result<usize, String> {
        let mut cleaned_count = 0;
        let now = SystemTime::now();
        
        let tasks = self.tasks.lock().map_err(|e| e.to_string())?;
        
        for (source_path, task) in tasks.iter() {
            if let Some(completed_at) = task.completed_at {
                if let Ok(age) = now.duration_since(completed_at) {
                    if age > Duration::from_secs(max_age_days * 24 * 60 * 60) {
                        // 删除转码后的文件
                        if std::fs::remove_file(&task.output_path).is_ok() {
                            cleaned_count += 1;
                            log::info!("清理过期缓存: {}", source_path);
                        }
                    }
                }
            }
        }
        
        Ok(cleaned_count)
    }
    
    /// 获取所有转码任务
    #[allow(dead_code)]
    pub fn get_all_tasks(&self) -> Vec<TranscodeTask> {
        match self.tasks.lock() {
            Ok(tasks) => tasks.values().cloned().collect(),
            Err(_) => vec![],
        }
    }
    
    /// 清除所有转码任务和缓存
    #[allow(dead_code)]
    pub fn clear_all(&self) -> Result<(), String> {
        let tasks = self.tasks.lock().map_err(|e| e.to_string())?;
        
        for task in tasks.values() {
            if Path::new(&task.output_path).exists() {
                let _ = std::fs::remove_file(&task.output_path);
            }
        }
        
        drop(tasks);
        
        let mut tasks = self.tasks.lock().map_err(|e| e.to_string())?;
        tasks.clear();
        
        log::info!("已清除所有转码缓存");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_needs_transcode() {
        let cache_dir = std::env::temp_dir().join("test_transcoder");
        let transcoder = Transcoder::new(cache_dir);
        
        assert!(transcoder.needs_transcode("test.ape"));
        assert!(transcoder.needs_transcode("test.dsd"));
        assert!(transcoder.needs_transcode("test.dts"));
        assert!(!transcoder.needs_transcode("test.mp3"));
        assert!(!transcoder.needs_transcode("test.flac"));
    }
    
    #[test]
    fn test_calculate_hash() {
        let hash1 = Transcoder::calculate_hash("test1");
        let hash2 = Transcoder::calculate_hash("test1");
        let hash3 = Transcoder::calculate_hash("test2");
        
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
}
