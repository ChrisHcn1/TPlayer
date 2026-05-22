//! 音频格式转换模块
//! 
//! 该模块提供了音频格式转换的功能，支持多种音频格式和质量设置。
//! 使用 FFmpeg 作为底层转换工具。

use crate::ffmpeg_transcoder;
use std::path::Path;
use std::process::{Command, Stdio};

/// 音频转换配置
#[derive(Debug, Clone)]
pub struct AudioConversionConfig {
    /// 输入文件路径
    pub input_path: String,
    /// 输出文件夹路径
    pub output_folder: String,
    /// 输出格式（文件扩展名）
    pub output_format: String,
    /// 音频编码器
    pub codec: String,
    /// 比特率（可选）
    pub bitrate: Option<String>,
    /// 压缩级别（可选）
    pub compression_level: Option<String>,
}

impl AudioConversionConfig {
    /// 创建新的音频转换配置
    pub fn new(
        input_path: String,
        output_folder: String,
        output_format: String,
        codec: String,
    ) -> Self {
        Self {
            input_path,
            output_folder,
            output_format,
            codec,
            bitrate: None,
            compression_level: None,
        }
    }

    /// 设置比特率
    pub fn with_bitrate(mut self, bitrate: String) -> Self {
        self.bitrate = Some(bitrate);
        self
    }

    /// 设置压缩级别
    pub fn with_compression_level(mut self, level: String) -> Self {
        self.compression_level = Some(level);
        self
    }

    /// 验证配置
    pub fn validate(&self) -> Result<(), String> {
        // 检查输入文件是否存在
        let input_path = Path::new(&self.input_path);
        if !input_path.exists() {
            return Err(format!("输入文件不存在: {}", self.input_path));
        }

        // 检查输入是否为文件
        if !input_path.is_file() {
            return Err(format!("输入不是文件: {}", self.input_path));
        }

        // 检查输出文件夹是否存在
        let output_path = Path::new(&self.output_folder);
        if !output_path.exists() {
            return Err(format!("输出文件夹不存在: {}", self.output_folder));
        }

        if !output_path.is_dir() {
            return Err(format!("输出不是文件夹: {}", self.output_folder));
        }

        Ok(())
    }

    /// 生成输出文件路径
    pub fn generate_output_path(&self) -> Result<String, String> {
        let input_path_obj = Path::new(&self.input_path);
        let file_stem = input_path_obj
            .file_stem()
            .ok_or_else(|| "无法获取文件名".to_string())?
            .to_string_lossy();

        let output_path = Path::new(&self.output_folder)
            .join(format!("{}.{}", file_stem, self.output_format))
            .to_string_lossy()
            .to_string();

        Ok(output_path)
    }

    /// 生成 FFmpeg 命令参数
    fn generate_ffmpeg_args(&self, output_path: &str) -> Vec<String> {
        let mut args: Vec<String> = vec![
            "-i".to_string(),
            self.input_path.clone(),
            "-c:a".to_string(),
            self.codec.clone(),
            "-y".to_string(),
            "-loglevel".to_string(),
            "quiet".to_string(),
        ];

        if let Some(br) = &self.bitrate {
            args.push("-b:a".to_string());
            args.push(br.clone());
        }

        if let Some(comp) = &self.compression_level {
            args.push("-compression_level".to_string());
            args.push(comp.clone());
        }

        args.push(output_path.to_string());

        args
    }
}

/// 执行音频转换（内部函数）
fn perform_conversion(config: AudioConversionConfig) -> Result<String, String> {
    println!("[音频转换] 开始转换: {}", config.input_path);
    println!("[音频转换] 输出格式: {}, 编码器: {}", config.output_format, config.codec);

    // 验证配置
    config.validate()?;

    let ffmpeg_path = ffmpeg_transcoder::TranscodeCache::get_ffmpeg_path()
        .ok_or_else(|| "未找到FFmpeg".to_string())?;

    let output_path = config.generate_output_path()?;
    println!("[音频转换] 输出文件: {}", output_path);

    let args = config.generate_ffmpeg_args(&output_path);
    println!("[音频转换] FFmpeg 命令: {}", args.join(" "));

    let mut cmd = Command::new(&ffmpeg_path);
    cmd.args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped()); // 捕获stderr用于错误报告

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    let result = cmd
        .output()
        .map_err(|e| format!("执行FFmpeg命令失败: {}", e))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(format!("转换失败: {}", stderr));
    }

    // 验证输出文件是否创建成功
    let output_file = Path::new(&output_path);
    if !output_file.exists() {
        return Err("转换失败：输出文件未创建".to_string());
    }

    println!("[音频转换] 转换成功");
    Ok(output_path)
}

/// Tauri 命令：转换音频文件
#[tauri::command]
pub async fn convert_audio(
    input_path: String,
    output_folder: String,
    output_format: String,
    codec: String,
    bitrate: Option<String>,
    compression: Option<String>,
) -> Result<String, String> {
    let mut config = AudioConversionConfig::new(
        input_path,
        output_folder,
        output_format,
        codec,
    );

    if let Some(br) = bitrate {
        config = config.with_bitrate(br);
    }

    if let Some(comp) = compression {
        config = config.with_compression_level(comp);
    }

    perform_conversion(config)
}
