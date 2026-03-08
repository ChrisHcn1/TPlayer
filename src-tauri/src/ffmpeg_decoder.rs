use rodio::Source;
use std::ffi::CString;
use std::path::Path;
use std::time::Duration;

// 定义 FFmpeg 相关的 C 结构体和函数
extern "C" {
    // 初始化函数
    fn avformat_open_input(ps: *mut *mut AVFormatContext, url: *const i8, fmt: *mut AVInputFormat, options: *mut AVDictionary) -> i32;
    fn avformat_find_stream_info(ps: *mut AVFormatContext, options: *mut AVDictionary) -> i32;
    fn avformat_close_input(ps: *mut *mut AVFormatContext);
    
    // 解码器相关
    fn avcodec_find_decoder(id: AVCodecID) -> *mut AVCodec;
    fn avcodec_alloc_context3(codec: *mut AVCodec) -> *mut AVCodecContext;
    fn avcodec_parameters_to_context(ctx: *mut AVCodecContext, par: *const AVCodecParameters) -> i32;
    fn avcodec_open2(ctx: *mut AVCodecContext, codec: *mut AVCodec, options: *mut AVDictionary) -> i32;
    fn avcodec_free_context(ps: *mut *mut AVCodecContext);
    
    // 数据包和帧
    fn av_packet_alloc() -> *mut AVPacket;
    fn av_packet_free(pkt: *mut *mut AVPacket);
    fn av_packet_unref(pkt: *mut AVPacket);
    fn av_frame_alloc() -> *mut AVFrame;
    fn av_frame_free(frame: *mut *mut AVFrame);
    
    // 解码
    fn av_read_frame(s: *mut AVFormatContext, pkt: *mut AVPacket) -> i32;
    fn avcodec_send_packet(ctx: *mut AVCodecContext, pkt: *const AVPacket) -> i32;
    fn avcodec_receive_frame(ctx: *mut AVCodecContext, frame: *mut AVFrame) -> i32;
}

// 定义 FFmpeg 结构体
#[repr(C)]
pub struct AVFormatContext {
    pub nb_streams: u32,
    pub streams: *mut *mut AVStream,
    pub duration: i64,
    // 其他字段
    _unused: [u8; 1024],
}

#[repr(C)]
pub struct AVStream {
    pub codecpar: *mut AVCodecParameters,
    // 其他字段
    _unused: [u8; 1024],
}

#[repr(C)]
pub struct AVCodecParameters {
    pub codec_id: AVCodecID,
    pub format: AVMediaType,
    // 其他字段
    _unused: [u8; 1024],
}

#[repr(C)]
pub struct AVCodecContext {
    pub sample_rate: i32,
    pub channels: i32,
    // 其他字段
    _unused: [u8; 1024],
}

#[repr(C)]
pub struct AVPacket {
    pub stream_index: i32,
    // 其他字段
    _unused: [u8; 1024],
}

#[repr(C)]
pub struct AVFrame {
    pub nb_samples: i32,
    pub channels: i32,
    pub format: AVSampleFormat,
    pub data: [*mut u8; 8],
    // 其他字段
    _unused: [u8; 1024],
}

#[repr(C)]
pub struct AVCodec {
    // 字段
    _unused: [u8; 1024],
}

#[repr(C)]
pub struct AVInputFormat {
    // 字段
    _unused: [u8; 1024],
}

#[repr(C)]
pub struct AVDictionary {
    // 字段
    _unused: [u8; 1024],
}

// 枚举类型
#[repr(C)]
#[derive(PartialEq)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
pub enum AVMediaType {
    AVMEDIA_TYPE_UNKNOWN = -1,
    AVMEDIA_TYPE_VIDEO,
    AVMEDIA_TYPE_AUDIO,
    AVMEDIA_TYPE_DATA,
    AVMEDIA_TYPE_SUBTITLE,
    AVMEDIA_TYPE_ATTACHMENT,
    AVMEDIA_TYPE_NB,
}

#[repr(C)]
#[derive(PartialEq)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
pub enum AVSampleFormat {
    AV_SAMPLE_FMT_NONE = -1,
    AV_SAMPLE_FMT_U8,
    AV_SAMPLE_FMT_S16,
    AV_SAMPLE_FMT_S32,
    AV_SAMPLE_FMT_FLT,
    AV_SAMPLE_FMT_DBL,
    AV_SAMPLE_FMT_U8P,
    AV_SAMPLE_FMT_S16P,
    AV_SAMPLE_FMT_S32P,
    AV_SAMPLE_FMT_FLTP,
    AV_SAMPLE_FMT_DBLP,
    AV_SAMPLE_FMT_S64,
    AV_SAMPLE_FMT_S64P,
    AV_SAMPLE_FMT_NB,
}

// 解码器 ID 类型
type AVCodecID = i32;

/// FFmpeg 音频解码器
pub struct FFmpegSource {
    format_context: *mut AVFormatContext,
    codec_context: *mut AVCodecContext,
    stream_index: i32,
    packet: *mut AVPacket,
    frame: *mut AVFrame,
    buffer: Vec<f32>,
    buffer_pos: usize,
    sample_rate: u32,
    channels: u16,
    duration: Option<Duration>,
}

// 实现 Send trait 以允许在线程间传递
unsafe impl Send for FFmpegSource {}

/// 从文件创建 FFmpeg 解码器
pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Box<dyn Source<Item = f32> + Send>, String> {
    let path = path.as_ref();
    let path_str = path.to_str().ok_or("无法将路径转换为字符串")?;
    let path_cstr = CString::new(path_str).map_err(|e| format!("无法创建 CString: {}", e))?;
    
    // 初始化 FFmpeg - av_register_all is deprecated in newer versions
    // In FFmpeg 4.0+, codecs are registered automatically
    
    // 打开输入文件
    let mut format_context: *mut AVFormatContext = std::ptr::null_mut();
    unsafe {
        if avformat_open_input(&mut format_context, path_cstr.as_ptr(), std::ptr::null_mut(), std::ptr::null_mut()) < 0 {
            return Err("无法打开文件".to_string());
        }
    }
    
    // 查找流信息
    unsafe {
        if avformat_find_stream_info(format_context, std::ptr::null_mut()) < 0 {
            avformat_close_input(&mut format_context);
            return Err("无法查找流信息".to_string());
        }
    }
    
    // 找到音频流
    let mut stream_index = -1;
    unsafe {
        for i in 0..(*format_context).nb_streams {
            let stream_ptr = *(*format_context).streams.offset(i as isize);
            if stream_ptr != std::ptr::null_mut() {
                let stream = &*stream_ptr;
                if stream.codecpar != std::ptr::null_mut() {
                    let codec_par = &*stream.codecpar;
                    if codec_par.format == AVMediaType::AVMEDIA_TYPE_AUDIO {
                        stream_index = i as i32;
                        break;
                    }
                }
            }
        }
    }
    
    if stream_index == -1 {
        unsafe {
            avformat_close_input(&mut format_context);
        }
        return Err("未找到音频流".to_string());
    }
    
    // 获取解码器参数
    let codec_par = unsafe {
        let stream_ptr = *(*format_context).streams.offset(stream_index as isize);
        &*(*stream_ptr).codecpar
    };
    
    // 查找解码器
    let codec = unsafe {
        avcodec_find_decoder(codec_par.codec_id)
    };
    
    if codec.is_null() {
        unsafe {
            avformat_close_input(&mut format_context);
        }
        return Err("未找到解码器".to_string());
    }
    
    // 创建解码器上下文
    let mut codec_context = unsafe {
        avcodec_alloc_context3(codec)
    };
    
    if codec_context.is_null() {
        unsafe {
            avformat_close_input(&mut format_context);
        }
        return Err("无法创建解码器上下文".to_string());
    }
    
    // 复制解码器参数
    unsafe {
        if avcodec_parameters_to_context(codec_context, codec_par) < 0 {
            avcodec_free_context(&mut codec_context);
            avformat_close_input(&mut format_context);
            return Err("无法复制解码器参数".to_string());
        }
    }
    
    // 打开解码器
    unsafe {
        if avcodec_open2(codec_context, codec, std::ptr::null_mut()) < 0 {
            avcodec_free_context(&mut codec_context);
            avformat_close_input(&mut format_context);
            return Err("无法打开解码器".to_string());
        }
    }
    
    // 计算持续时间
    let duration = unsafe {
        if (*format_context).duration != std::i64::MAX {
            Some(Duration::from_secs_f64((*format_context).duration as f64 / 1000000.0))
        } else {
            None
        }
    };
    
    let sample_rate = unsafe { (*codec_context).sample_rate as u32 };
    let channels = unsafe { (*codec_context).channels as u16 };
    
    // 初始化数据包和帧
    let mut packet = unsafe {
        av_packet_alloc()
    };
    
    let frame = unsafe {
        av_frame_alloc()
    };
    
    if frame.is_null() {
        unsafe {
            av_packet_free(&mut packet);
            avcodec_free_context(&mut codec_context);
            avformat_close_input(&mut format_context);
        }
        return Err("无法分配帧".to_string());
    }
    
    Ok(Box::new(FFmpegSource {
        format_context,
        codec_context,
        stream_index,
        packet,
        frame,
        buffer: Vec::new(),
        buffer_pos: 0,
        sample_rate,
        channels,
        duration,
    }))
}

impl FFmpegSource {
    /// 填充音频缓冲区
    fn fill_buffer(&mut self) -> Result<(), String> {
        // 当缓冲区为空时，填充数据
        if self.buffer_pos >= self.buffer.len() {
            self.buffer.clear();
            self.buffer_pos = 0;
            
            unsafe {
                // 读取数据包
                while av_read_frame(self.format_context, self.packet) >= 0 {
                    if (*self.packet).stream_index == self.stream_index {
                        // 发送数据包到解码器
                        if avcodec_send_packet(self.codec_context, self.packet) < 0 {
                            av_packet_unref(self.packet);
                            continue;
                        }
                        
                        // 接收解码后的帧
                        while avcodec_receive_frame(self.codec_context, self.frame) >= 0 {
                            let samples = (*self.frame).nb_samples as usize;
                            let channels = (*self.frame).channels as usize;
                            
                            // 根据样本格式转换数据
                            match (*self.frame).format {
                                AVSampleFormat::AV_SAMPLE_FMT_FLT => {
                                    // 直接使用 f32 数据
                                    let data = (*self.frame).data[0];
                                    let f32_data = std::slice::from_raw_parts(
                                        data as *const f32,
                                        samples * channels
                                    );
                                    self.buffer.extend_from_slice(f32_data);
                                },
                                AVSampleFormat::AV_SAMPLE_FMT_S16 => {
                                    // 从 i16 转换为 f32
                                    let data = (*self.frame).data[0];
                                    let i16_data = std::slice::from_raw_parts(
                                        data as *const i16,
                                        samples * channels
                                    );
                                    for &sample in i16_data {
                                        self.buffer.push(sample as f32 / 32768.0);
                                    }
                                },
                                AVSampleFormat::AV_SAMPLE_FMT_S32 => {
                                    // 从 i32 转换为 f32
                                    let data = (*self.frame).data[0];
                                    let i32_data = std::slice::from_raw_parts(
                                        data as *const i32,
                                        samples * channels
                                    );
                                    for &sample in i32_data {
                                        self.buffer.push(sample as f32 / 2147483648.0);
                                    }
                                },
                                _ => {
                                    // 其他格式暂时不支持
                                    return Err("不支持的音频样本格式".to_string());
                                },
                            }
                        }
                    }
                    av_packet_unref(self.packet);
                }
                
                // 发送 NULL 数据包以刷新解码器
                avcodec_send_packet(self.codec_context, std::ptr::null_mut());
                while avcodec_receive_frame(self.codec_context, self.frame) >= 0 {
                    let samples = (*self.frame).nb_samples as usize;
                    let channels = (*self.frame).channels as usize;
                    
                    // 根据样本格式转换数据
                    match (*self.frame).format {
                        AVSampleFormat::AV_SAMPLE_FMT_FLT => {
                            // 直接使用 f32 数据
                            let data = (*self.frame).data[0];
                            let f32_data = std::slice::from_raw_parts(
                                data as *const f32,
                                samples * channels
                            );
                            self.buffer.extend_from_slice(f32_data);
                        },
                        AVSampleFormat::AV_SAMPLE_FMT_S16 => {
                            // 从 i16 转换为 f32
                            let data = (*self.frame).data[0];
                            let i16_data = std::slice::from_raw_parts(
                                data as *const i16,
                                samples * channels
                            );
                            for &sample in i16_data {
                                self.buffer.push(sample as f32 / 32768.0);
                            }
                        },
                        AVSampleFormat::AV_SAMPLE_FMT_S32 => {
                            // 从 i32 转换为 f32
                            let data = (*self.frame).data[0];
                            let i32_data = std::slice::from_raw_parts(
                                data as *const i32,
                                samples * channels
                            );
                            for &sample in i32_data {
                                self.buffer.push(sample as f32 / 2147483648.0);
                            }
                        },
                        _ => {
                            // 其他格式暂时不支持
                            return Err("不支持的音频样本格式".to_string());
                        },
                    }
                }
            }
        }
        
        Ok(())
    }
}

impl Iterator for FFmpegSource {
    type Item = f32;
    
    fn next(&mut self) -> Option<Self::Item> {
        // 确保缓冲区有数据
        if self.buffer_pos >= self.buffer.len() {
            if let Err(e) = self.fill_buffer() {
                eprintln!("填充缓冲区失败: {}", e);
                return None;
            }
        }
        
        // 从缓冲区返回样本
        if self.buffer_pos < self.buffer.len() {
            let sample = self.buffer[self.buffer_pos];
            self.buffer_pos += 1;
            Some(sample)
        } else {
            None
        }
    }
}

impl Source for FFmpegSource {
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
        self.duration
    }
}

impl Drop for FFmpegSource {
    fn drop(&mut self) {
        unsafe {
            if !self.frame.is_null() {
                av_frame_free(&mut self.frame);
            }
            if !self.packet.is_null() {
                av_packet_free(&mut self.packet);
            }
            if !self.codec_context.is_null() {
                avcodec_free_context(&mut self.codec_context);
            }
            if !self.format_context.is_null() {
                avformat_close_input(&mut self.format_context);
            }
        }
    }
}
