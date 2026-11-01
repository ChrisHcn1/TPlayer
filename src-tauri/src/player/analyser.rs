//! 实验室·音频可视化：后端频谱分析器（pass-through Source）。
//!
//! 以 rodio `Source` 包装器形式插入播放链（与 `EqSource` 并列），逐样本**直通**，
//! 仅「读取」样本喂给 `AnalyserProcessor` 算 FFT；**不修改任何样本**，故关闭/开启
//! 对输出 PCM 逐字节一致（零影响）。开关由全局 `AUDIO_VISUALIZER` 原子标志控制：
//! 关闭时 `next()` 仅做一次原子读 + 直通，近乎零开销；开启时按帧算频谱并经全局
//! `AppHandle` 发射 `audio-analyser` 事件推前端。切换即时生效，无需重建播放链。

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use rodio::Source;
use tauri::Emitter;

use crate::player::engine::{AUDIO_VISUALIZER, ENGINE_APP};

/// FFT 长度（必须为 2 的幂）。1024 @ 44100Hz ≈ 23ms/帧 → ~43fps，流畅且开销低。
const FFT_SIZE: usize = 1024;

/// 预计算 Hann 窗，降低频谱泄漏。
fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (n as f32 - 1.0)).cos()))
        .collect()
}

/// 就地迭代式基-2 Cooley-Tukey FFT。`re`/`im` 同长且 `n` 为 2 的幂。
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    // 位反转置换
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    // 蝶形运算
    let mut len = 2usize;
    while len <= n {
        let ang = -2.0 * std::f32::consts::PI / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        let mut i = 0usize;
        while i < n {
            let mut cur_r = 1.0f32;
            let mut cur_i = 0.0f32;
            for k in 0..len / 2 {
                let a_r = re[i + k];
                let a_i = im[i + k];
                let b_r = re[i + k + len / 2];
                let b_i = im[i + k + len / 2];
                let t_r = b_r * cur_r - b_i * cur_i;
                let t_i = b_r * cur_i + b_i * cur_r;
                re[i + k] = a_r - t_r;
                im[i + k] = a_i - t_i;
                re[i + k + len / 2] = a_r + t_r;
                im[i + k + len / 2] = a_i + t_i;
                let nwr = cur_r * wr - cur_i * wi;
                let nwi = cur_r * wi + cur_i * wr;
                cur_r = nwr;
                cur_i = nwi;
            }
            i += len;
        }
        len <<= 1;
    }
}

/// 频谱处理器：累积单声道样本，满 `FFT_SIZE` 即算一次频谱并以 u8 数组返回。
pub struct AnalyserProcessor {
    buf: Vec<f32>,
    filled: usize,
    window: Vec<f32>,
}

impl AnalyserProcessor {
    pub fn new() -> Self {
        let window = hann(FFT_SIZE);
        Self {
            buf: vec![0.0; FFT_SIZE],
            filled: 0,
            window,
        }
    }

    /// 喂入一个单声道样本（[-1,1]）。填满一帧返回频谱 bins（长度 `FFT_SIZE/2`，0..=255），否则 None。
    pub fn feed(&mut self, sample: f32) -> Option<Vec<u8>> {
        self.buf[self.filled] = sample * self.window[self.filled];
        self.filled += 1;
        if self.filled < FFT_SIZE {
            return None;
        }
        self.filled = 0;
        let mut re = self.buf.clone();
        let mut im = vec![0.0f32; FFT_SIZE];
        fft(&mut re, &mut im);
        let bins = FFT_SIZE / 2;
        let mut out = Vec::with_capacity(bins);
        for i in 0..bins {
            let mag = (re[i] * re[i] + im[i] * im[i]).sqrt();
            // 对数感知缩放：-50dB..0dB 映射到 0..255
            let db = 20.0 * (mag + 1e-3).log10();
            let v = ((db + 50.0) / 50.0).clamp(0.0, 1.0) * 255.0;
            out.push(v as u8);
        }
        Some(out)
    }
}

/// 包装任意 `Source<Item = i16>`，直通样本并（开启时）抽取频谱。
pub struct AnalyserSource<S> {
    inner: S,
    proc: Arc<Mutex<AnalyserProcessor>>,
    channels: u16,
    ch_pos: usize,
    /// 立体声下混时的前一声道样本
    partial: Option<f32>,
}

impl<S> AnalyserSource<S>
where
    S: Source<Item = i16>,
{
    pub fn new(inner: S, proc: &Arc<Mutex<AnalyserProcessor>>) -> Self {
        let channels = inner.channels();
        Self {
            inner,
            proc: Arc::clone(proc),
            channels,
            ch_pos: 0,
            partial: None,
        }
    }

    /// 把逐样本 i16 转为单声道 f32 并喂给处理器（仅在可视化开启时）。
    fn tap(&mut self, s: i16, ch: usize) {
        if !AUDIO_VISUALIZER.load(Ordering::Relaxed) {
            return;
        }
        let v = s as f32 / 32768.0;
        let mono = if self.channels <= 1 {
            v
        } else if ch == 0 {
            self.partial = Some(v);
            return;
        } else {
            let p = self.partial.take().unwrap_or(v);
            (p + v) / 2.0
        };
        if let Some(bins) = self.proc.lock().unwrap().feed(mono) {
            if let Some(app) = ENGINE_APP.get() {
                let _ = app.emit("audio-analyser", bins);
            }
        }
    }
}

impl<S> Iterator for AnalyserSource<S>
where
    S: Source<Item = i16>,
{
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        let s = self.inner.next()?;
        let ch = self.ch_pos;
        self.ch_pos = (self.ch_pos + 1) % self.channels.max(1) as usize;
        self.tap(s, ch);
        Some(s)
    }
}

impl<S> Source for AnalyserSource<S>
where
    S: Source<Item = i16>,
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
