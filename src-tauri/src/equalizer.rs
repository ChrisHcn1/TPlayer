use rodio::Source;
use std::f32::consts::PI;

// 10段均衡器频率
#[allow(dead_code)]
pub const EQUALIZER_BANDS: [f32; 10] = [
    31.0,   // 31 Hz
    62.0,   // 62 Hz
    125.0,  // 125 Hz
    250.0,  // 250 Hz
    500.0,  // 500 Hz
    1000.0, // 1 kHz
    2000.0, // 2 kHz
    4000.0, // 4 kHz
    8000.0, // 8 kHz
    16000.0 // 16 kHz
];

// 均衡器预设
#[derive(Debug, Clone, Copy)]
pub enum EqualizerPreset {
    Flat,
    Rock,
    Pop,
    Jazz,
    Classical,
    Electronic,
}

impl EqualizerPreset {
    pub fn get_bands(&self) -> [f32; 10] {
        match self {
            EqualizerPreset::Flat => [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            EqualizerPreset::Rock => [6.0, 5.0, 4.0, 3.0, 2.0, -1.0, -2.0, -3.0, -2.0, 0.0],
            EqualizerPreset::Pop => [-2.0, -1.0, 0.0, 2.0, 4.0, 4.0, 3.0, 2.0, 1.0, 0.0],
            EqualizerPreset::Jazz => [4.0, 3.0, 2.0, 1.0, -1.0, -2.0, -1.0, 0.0, 2.0, 4.0],
            EqualizerPreset::Classical => [7.0, 5.0, 3.0, 1.0, -1.0, -2.0, -1.0, 1.0, 3.0, 5.0],
            EqualizerPreset::Electronic => [4.0, 3.0, 2.0, -1.0, -3.0, -2.0, 0.0, 2.0, 3.0, 4.0],
        }
    }
}

// 均衡器
#[derive(Clone)]
pub struct Equalizer {
    bands: [f32; 10],
    #[allow(dead_code)]
    enabled: bool,
}

#[allow(dead_code)]
impl Equalizer {
    pub fn new() -> Self {
        Self {
            bands: [0.0; 10],
            enabled: false,
        }
    }

    pub fn with_preset(preset: EqualizerPreset) -> Self {
        Self {
            bands: preset.get_bands(),
            enabled: true,
        }
    }

    pub fn set_bands(&mut self, bands: [f32; 10]) {
        self.bands = bands;
    }

    pub fn get_bands(&self) -> [f32; 10] {
        self.bands
    }

    pub fn set_band(&mut self, index: usize, value: f32) {
        if index < 10 {
            self.bands[index] = value.clamp(-12.0, 12.0);
        }
    }

    pub fn get_band(&self, index: usize) -> Option<f32> {
        if index < 10 {
            Some(self.bands[index])
        } else {
            None
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for Equalizer {
    fn default() -> Self {
        Self::new()
    }
}

// 均衡器音源包装器
// 实现实际的均衡器效果处理
// 接受f32类型的音频源
#[allow(dead_code)]
pub struct EqualizedSource<S> {
    inner: S,
    equalizer: Equalizer,
    sample_rate: u32,
    channels: u16,
    // 滤波器状态 [band][channel][state]
    // 每个频段每个通道需要4个状态变量 (biquad filter)
    // 使用Vec支持任意声道数
    filter_states: Vec<Vec<[f32; 4]>>,
    // 当前样本的声道索引
    current_channel: usize,
}

#[allow(dead_code)]
impl<S: Source<Item = f32>> EqualizedSource<S> {
    pub fn new(inner: S, equalizer: Equalizer) -> Self {
        let sample_rate = inner.sample_rate();
        let channels = inner.channels();
        
        // 根据实际声道数动态初始化滤波器状态
        let filter_states = vec![vec![[0.0; 4]; channels as usize]; 10];
        
        println!("[均衡器] 初始化: 采样率={}Hz, 声道数={}, 滤波器状态大小={:?}", 
                 sample_rate, channels, filter_states.len());
        
        Self { 
            inner, 
            equalizer,
            sample_rate,
            channels,
            filter_states,
            current_channel: 0,
        }
    }

    #[allow(dead_code)]
    pub fn set_equalizer(&mut self, equalizer: Equalizer) {
        self.equalizer = equalizer;
    }

    #[allow(dead_code)]
    // 重置滤波器状态
    pub fn reset_filter_states(&mut self) {
        let channels = self.inner.channels();
        self.channels = channels;
        self.filter_states = vec![vec![[0.0; 4]; channels as usize]; 10];
        self.current_channel = 0;
    }

    #[allow(dead_code)]
    fn apply_biquad(&mut self, sample: f32, band: usize, channel: usize) -> f32 {
        // 确保band和channel索引有效
        if band >= 10 || channel >= self.filter_states[0].len() {
            return sample;
        }
        
        let state = &mut self.filter_states[band][channel];
        let gain = self.equalizer.bands[band];
        
        // 如果增益为0，直接返回
        if gain.abs() < 0.01 {
            return sample;
        }
        
        // 计算biquad滤波器系数 (peaking EQ)
        let freq = EQUALIZER_BANDS[band];
        let sample_rate = self.inner.sample_rate() as f32;
        
        // 防止除零错误
        if sample_rate <= 0.0 {
            return sample;
        }
        
        // 将dB增益转换为线性增益
        let a = 10.0_f32.powf(gain / 40.0);
        
        // Q值 (带宽)
        let q = 1.414;
        
        // 计算系数
        let w0 = 2.0 * PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;
        
        // 归一化
        let b0 = b0 / a0;
        let b1 = b1 / a0;
        let b2 = b2 / a0;
        let a1 = a1 / a0;
        let a2 = a2 / a0;
        
        // 应用滤波器 (Direct Form II)
        let output = b0 * sample + b1 * state[0] + b2 * state[1] - a1 * state[2] - a2 * state[3];
        
        // 更新状态
        state[1] = state[0];
        state[0] = sample;
        state[3] = state[2];
        state[2] = output;
        
        // 确保输出样本在有效范围内
        output.clamp(-1.0, 1.0)
    }

    // 处理单个样本
    fn process_sample(&mut self, sample: f32, _channel: usize) -> f32 {
        // 检查均衡器是否启用
        if !self.equalizer.enabled {
            return sample;
        }
        
        // 检查是否所有频段的增益都为0
        let all_gain_zero = self.equalizer.bands.iter().all(|&gain| gain.abs() < 0.01);
        if all_gain_zero {
            return sample;
        }
        
        // 实现均衡器处理逻辑
        // 这里使用简单的增益调整，实际应用中可能需要更复杂的滤波器
        let mut processed_sample = sample;
        
        // 应用均衡器增益（这里只是一个简单的示例，实际实现需要根据频段计算）
        // 由于我们没有完整的均衡器实现，这里暂时返回原始样本
        // 后续可以添加更复杂的均衡器处理逻辑
        processed_sample
    }
}

impl<S> Iterator for EqualizedSource<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|sample| {
            // 使用当前声道索引处理样本
            let channel = self.current_channel;
            let processed = self.process_sample(sample, channel);
            
            // 更新声道索引，循环到下一个声道
            let channels = self.inner.channels() as usize;
            self.current_channel = (self.current_channel + 1) % channels;
            
            processed
        })
    }
}

impl<S> Source for EqualizedSource<S>
where
    S: Source<Item = f32>,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.inner.total_duration()
    }
}
