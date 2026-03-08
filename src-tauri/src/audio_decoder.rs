use rodio::Source;
use std::io::BufReader;
use std::path::Path;

/// 尝试打开音频文件，支持多种格式
/// 
/// # 参数
/// * `path` - 音频文件路径
/// * `allow_problematic_formats` - 是否允许尝试解码有问题的格式（如APE、DSD等）
///   当转码功能启用且文件已转码后，应设置为true
pub fn try_open_audio<P: AsRef<Path>>(path: P) -> Result<Box<dyn Source<Item = f32> + Send>, String> {
    let path = path.as_ref();
    
    // 检查文件扩展名
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    // 这些格式已知会导致 FFmpeg 崩溃，需要转码后才能播放
    let problematic_formats = ["ape", "dsd", "dsf", "dff", "dts"];
    if problematic_formats.contains(&ext.as_str()) {
        return Err(format!("UNSUPPORTED_FORMAT:{}", ext));
    }
    
    // 尝试使用 rodio 的 Decoder 打开
    let file = std::fs::File::open(path).map_err(|e| format!("无法打开文件: {}", e))?;
    let reader = BufReader::new(file);
    
    match rodio::Decoder::new(reader) {
        Ok(source) => {
            // 转换为 f32 格式
            Ok(Box::new(source.convert_samples()))
        }
        Err(e) => {
            eprintln!("Rodio 解码失败: {}", e);
            // 只使用 Rodio 解码，不尝试 FFmpeg
            Err(format!("无法解码音频: {}", e))
        }
    }
}

/// 检查文件是否为音频文件
pub fn is_audio_file(ext: &str) -> bool {
    let audio_extensions = [
        // 常见格式
        "mp3", "wav", "flac", "aac", "ogg", "m4a", "alac",
        // 特殊格式
        "wma", "ape", "tta", "opus", "webm", "3gp",
        // DSD 格式
        "dsd", "dsf", "dff",
        // 其他格式
        "aiff", "au", "snd", "voc", "w64"
    ];
    audio_extensions.contains(&ext.to_lowercase().as_str())
}