#![allow(dead_code)]

use rodio::Source;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Duration;

/// 读取指定长度的二进制数据
fn read_exact<T: Read>(reader: &mut T, len: usize) -> Result<Vec<u8>, String> {
    let mut buffer = vec![0; len];
    reader.read_exact(&mut buffer).map_err(|e| format!("读取失败: {}", e))?;
    Ok(buffer)
}

/// 安全地从字节数组中解析 u32
fn parse_u32(bytes: &[u8]) -> Result<u32, String> {
    if bytes.len() < 4 {
        return Err("字节长度不足".to_string());
    }
    Ok(u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| "转换失败".to_string())?))
}

/// 安全地从字节数组中解析 u64
fn parse_u64(bytes: &[u8]) -> Result<u64, String> {
    if bytes.len() < 8 {
        return Err("字节长度不足".to_string());
    }
    Ok(u64::from_le_bytes(bytes[0..8].try_into().map_err(|_| "转换失败".to_string())?))
}

/// DSD 音频解码器
pub struct DsdSource {
    reader: BufReader<std::fs::File>,
    sample_rate: u32,
    channels: u16,
    duration: Duration,
    position: u64,
    total_samples: u64,
    bit_offset: u8,
    current_byte: u8,
    is_lsbf: bool,
}

impl DsdSource {
    /// 从 DSF 文件创建 DSD 解码器
    pub fn from_dsf<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("无法打开文件: {}", e))?;
        let mut reader = BufReader::new(file);
        
        // 读取 DSF 头部
        let mut header = [0; 28];
        reader.read_exact(&mut header).map_err(|e| format!("无法读取文件头: {}", e))?;
        
        // 检查文件标识
        if &header[0..4] != b"DSD " {
            return Err("不是有效的 DSF 文件".to_string());
        }
        
        // 读取格式版本（可能是 28 或其他值，跳过严格检查）
        let _version = parse_u32(&header[4..8])?;
        
        // 读取文件大小
        let _file_size = parse_u64(&header[8..16])?;
        
        // 读取格式区块大小
        let format_chunk_size = parse_u64(&header[16..24])?;
        
        // 跳过到格式信息（格式区块从偏移 28 开始）
        reader.seek(SeekFrom::Start(28)).map_err(|e| format!("无法定位到格式区块: {}", e))?;
        
        // 读取格式信息
        let format_data = read_exact(&mut reader, format_chunk_size as usize)?;
        
        // 解析格式信息
        if format_data.len() < 20 {
            return Err("格式数据不足".to_string());
        }
        
        let channels = parse_u32(&format_data[0..4])? as u16;
        let sample_rate = parse_u32(&format_data[4..8])?;
        let _bits_per_sample = parse_u32(&format_data[8..12])?;
        let sample_count = parse_u64(&format_data[12..20])?;
        
        // 检查是否是 LSBF 格式
        let is_lsbf = format_data.len() > 20 && format_data[20] != 0;
        
        // 计算持续时间
        let duration = Duration::from_secs_f64(sample_count as f64 / sample_rate as f64);
        
        // 定位到数据区块
        let data_chunk_pos = 28 + format_chunk_size + 12;
        reader.seek(SeekFrom::Start(data_chunk_pos)).map_err(|e| format!("无法定位到数据区块: {}", e))?;
        
        // 读取数据区块头部
        let mut data_header = [0; 12];
        if reader.read_exact(&mut data_header).is_ok() {
            if &data_header[0..4] != b"data" {
                // 尝试其他位置
                reader.seek(SeekFrom::Start(data_chunk_pos + 28)).ok();
            }
        }
        
        Ok(DsdSource {
            reader,
            sample_rate,
            channels,
            duration,
            position: 0,
            total_samples: sample_count,
            bit_offset: 0,
            current_byte: 0,
            is_lsbf,
        })
    }
}

impl Iterator for DsdSource {
    type Item = f32;
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.total_samples {
            return None;
        }
        
        if self.bit_offset == 0 {
            let mut byte = [0; 1];
            if self.reader.read_exact(&mut byte).is_err() {
                return None;
            }
            self.current_byte = byte[0];
        }
        
        let bit = if self.is_lsbf {
            (self.current_byte >> self.bit_offset) & 1
        } else {
            (self.current_byte >> (7 - self.bit_offset)) & 1
        };
        
        self.bit_offset = (self.bit_offset + 1) % 8;
        
        let sample = if bit == 1 { 1.0 } else { -1.0 };
        
        self.position += 1;
        Some(sample)
    }
}

impl Source for DsdSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    
    fn channels(&self) -> u16 {
        self.channels
    }
    
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    
    fn total_duration(&self) -> Option<Duration> {
        Some(self.duration)
    }
}

/// 尝试打开特殊音频文件
pub fn try_open_special<P: AsRef<Path>>(path: P) -> Result<Box<dyn Source<Item = f32> + Send>, String> {
    let path = path.as_ref();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    
    match ext.to_lowercase().as_str() {
        "dsf" => {
            // 暂时禁用 DSF 解码器，避免栈溢出
            Err("DSF 格式暂不支持，请使用其他格式".to_string())
        }
        "dff" | "dsd" => {
            Err("DFF 和 DSD 原始格式暂不支持".to_string())
        }
        "ape" => {
            Err("APE 格式暂不支持，请使用其他格式".to_string())
        }
        _ => {
            Err("不是特殊音频格式文件".to_string())
        }
    }
}

/// 检查文件是否为特殊音频格式
pub fn is_special_format(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let special_extensions = ["dsf", "dff", "dsd"];
    special_extensions.contains(&ext.to_lowercase().as_str())
}