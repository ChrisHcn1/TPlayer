//! 播放引擎：rodio 实现 + 线程通道封装
//!
//! rodio 的 OutputStream/Sink 在 Windows 上非 Send（COM STA 绑定），
//! 因此引擎运行在专用线程，PlayerHandle 通过 mpsc 通道发送命令。

use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::error::{AppError, AppResult};

/// 切歌淡入 / 淡出时长（毫秒）：旧曲目淡出与新曲目淡入重叠形成交叉淡化
const FADE_MS: u64 = 400;
/// 渐变驱动步进：引擎线程命令循环的轮询间隔
const FADE_TICK_MS: u64 = 20;

/// symphonia 原生可解码的扩展名；其余格式走 ffmpeg 子进程解码为 PCM
const NATIVE_EXT: &[&str] = &["mp3", "flac", "wav", "ogg", "oga", "opus", "aac", "m4a"];

/// ffmpeg 可解码的补充格式（ape/dsd/dts/wma/wv/tta/tak/aiff 等）
const FFMPEG_EXT: &[&str] = &["wma", "aif", "aiff", "ape", "wv", "tta", "tak", "dsf", "dff", "dts"];

fn is_native_format(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| NATIVE_EXT.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// 定位 bin 目录下的工具（ffmpeg/ffprobe）：沿可执行文件祖先目录查找
/// （覆盖开发期项目根与安装布局）
fn resolve_bin_tool(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    for ancestor in exe.ancestors().skip(1) {
        let candidate = ancestor.join("bin").join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn resolve_ffmpeg() -> Option<PathBuf> {
    resolve_bin_tool("ffmpeg.exe")
}

pub fn resolve_ffprobe() -> Option<PathBuf> {
    resolve_bin_tool("ffprobe.exe")
}

pub fn ffmpeg_available() -> bool {
    resolve_ffmpeg().is_some()
}

/// 用 ffprobe 探测音频时长（秒）。
/// 用于 lofty 无法解析的格式（dsf/dff/dts/tta/tak 等）在扫描与播放时的兜底；
/// 失败（工具缺失/输出异常/时长非正）返回 None。
pub fn probe_duration(path: &Path) -> Option<f64> {
    let ffprobe = resolve_ffprobe()?;
    let mut cmd = Command::new(ffprobe);
    cmd.arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("csv=p=0")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW：避免探测时弹出控制台窗口
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let secs = text.trim().parse::<f64>().ok()?;
    (secs > 0.0).then_some(secs)
}

/// ffmpeg 解码输出的后端支持格式清单（供 capabilities 使用）
pub fn ffmpeg_formats() -> Vec<String> {
    FFMPEG_EXT.iter().map(|s| s.to_string()).collect()
}

/// 播放器能力声明
#[derive(Debug, Clone, Serialize)]
pub struct EngineCapabilities {
    pub seek: bool,
    #[serde(rename = "volumeControl")]
    pub volume_control: bool,
    #[serde(rename = "rateControl")]
    pub rate_control: bool,
    #[serde(rename = "supportedFormats")]
    pub supported_formats: Vec<String>,
}

/// 发往引擎线程的命令
enum EngineCommand {
    /// 播放（附回执通道：打开/解码是否成功同步回传，失败可转为 error 事件）
    Play(PathBuf, mpsc::Sender<AppResult<()>>),
    /// 播放并定位到指定毫秒（断点续播专用：等混音线程起流后再 seek，规避启动竞态）
    PlayAt(PathBuf, u64, mpsc::Sender<AppResult<()>>),
    Pause,
    Resume,
    Stop,
    Seek(u64),
    SetVolume(f32),
    SetMuted(bool),
    /// 当前 sink 是否已播完（无音源在播）：供状态机兜底时长未知（0）时的结束检测
    IsEnded(mpsc::Sender<bool>),
}

/// 引擎句柄（Send）：PlayerHandle 通过它控制播放
pub struct EngineHandle {
    tx: mpsc::Sender<EngineCommand>,
}

impl EngineHandle {
    pub fn play(&self, path: PathBuf) -> AppResult<()> {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.tx
            .send(EngineCommand::Play(path, ack_tx))
            .map_err(|_| AppError::Player("引擎线程已退出".into()))?;
        ack_rx
            .recv()
            .map_err(|_| AppError::Player("引擎线程已退出".into()))?
    }
    pub fn play_at(&self, path: PathBuf, position_ms: u64) -> AppResult<()> {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.tx
            .send(EngineCommand::PlayAt(path, position_ms, ack_tx))
            .map_err(|_| AppError::Player("引擎线程已退出".into()))?;
        ack_rx
            .recv()
            .map_err(|_| AppError::Player("引擎线程已退出".into()))?
    }
    pub fn pause(&self) -> AppResult<()> {
        self.tx
            .send(EngineCommand::Pause)
            .map_err(|_| AppError::Player("引擎线程已退出".into()))
    }
    pub fn resume(&self) -> AppResult<()> {
        self.tx
            .send(EngineCommand::Resume)
            .map_err(|_| AppError::Player("引擎线程已退出".into()))
    }
    pub fn stop(&self) -> AppResult<()> {
        self.tx
            .send(EngineCommand::Stop)
            .map_err(|_| AppError::Player("引擎线程已退出".into()))
    }
    pub fn seek(&self, ms: u64) -> AppResult<()> {
        self.tx
            .send(EngineCommand::Seek(ms))
            .map_err(|_| AppError::Player("引擎线程已退出".into()))
    }
    pub fn set_volume(&self, v: f32) -> AppResult<()> {
        self.tx
            .send(EngineCommand::SetVolume(v))
            .map_err(|_| AppError::Player("引擎线程已退出".into()))
    }
    pub fn set_muted(&self, m: bool) -> AppResult<()> {
        self.tx
            .send(EngineCommand::SetMuted(m))
            .map_err(|_| AppError::Player("引擎线程已退出".into()))
    }
    /// 当前曲目是否已在引擎侧播完（sink 无音源）
    pub fn is_current_ended(&self) -> bool {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.tx
            .send(EngineCommand::IsEnded(ack_tx))
            .map(|_| ack_rx.recv().unwrap_or(false))
            .unwrap_or(false)
    }
}

/// 启动引擎线程，返回可发送命令的句柄
pub fn spawn_engine() -> EngineHandle {
    let (tx, rx) = mpsc::channel::<EngineCommand>();
    std::thread::Builder::new()
        .name("audio-engine".into())
        .spawn(move || {
            let mut engine = RodioEngine::new();
            // recv 改为带超时轮询：无命令期间也能以 ~20ms 步进驱动淡入淡出
            loop {
                match rx.recv_timeout(Duration::from_millis(FADE_TICK_MS)) {
                    Ok(cmd) => match cmd {
                        EngineCommand::Play(path, ack) => {
                            let _ = ack.send(engine.play(&path));
                        }
                        EngineCommand::PlayAt(path, pos_ms, ack) => {
                            let _ = ack.send(engine.play_at(&path, pos_ms));
                        }
                        EngineCommand::Pause => {
                            let _ = engine.pause();
                        }
                        EngineCommand::Resume => {
                            let _ = engine.resume();
                        }
                        EngineCommand::Stop => {
                            let _ = engine.stop();
                        }
                        EngineCommand::Seek(ms) => {
                            let _ = engine.seek(ms);
                        }
                        EngineCommand::SetVolume(v) => {
                            let _ = engine.set_volume(v);
                        }
                        EngineCommand::SetMuted(m) => {
                            let _ = engine.set_muted(m);
                        }
                        EngineCommand::IsEnded(ack) => {
                            let _ = ack.send(engine.current_ended());
                        }
                    },
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
                engine.tick_fades();
            }
        })
        .expect("failed to spawn audio engine thread");
    EngineHandle { tx }
}

/// 一条播放链路：sink + 其专属 ffmpeg 子进程（非原生格式）。
/// 交叉淡化期间新旧两条链路同时存活。
struct Playback {
    sink: rodio::Sink,
    ffmpeg_child: Option<Child>,
}

impl Playback {
    /// 立即终止播放（sink 停止拉流，ffmpeg 子进程随后退出；Drop 再 kill 兜底）
    fn halt(mut self) {
        self.sink.stop();
        if let Some(child) = self.ffmpeg_child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.ffmpeg_child = None;
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        if let Some(mut child) = self.ffmpeg_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// 正在淡出的旧曲目
struct FadingOut {
    playback: Playback,
    started: Instant,
    from: f32,
}

/// rodio 引擎本体（非 Send，仅在引擎线程内使用）
struct RodioEngine {
    /// 输出设备流：整个引擎生命周期复用一条（多个 sink 可挂在同一 handle 上），
    /// 避免每次切歌重建输出设备造成的爆音
    stream: Option<rodio::OutputStream>,
    handle: Option<rodio::OutputStreamHandle>,
    /// 当前曲目
    current: Option<Playback>,
    /// 切歌时正在淡出的上一曲目
    fading_out: Option<FadingOut>,
    /// 当前曲目淡入起点（播放 / seek 后短暂存在）
    fade_in_started: Option<Instant>,
    volume: f32,
    muted: bool,
    /// 当前曲目路径（ffmpeg 后端 seek 需带 -ss 重启进程）
    current_path: Option<PathBuf>,
    /// 当前曲目是否走 ffmpeg 管道（决定 seek 方式）
    current_is_ffmpeg: bool,
}

impl RodioEngine {
    fn new() -> Self {
        // 尽早打开输出设备；失败（如无音频设备）则延迟到首次 play 重试
        let (stream, handle) = match rodio::OutputStream::try_default() {
            Ok((stream, handle)) => (Some(stream), Some(handle)),
            Err(_) => (None, None),
        };
        Self {
            stream,
            handle,
            current: None,
            fading_out: None,
            fade_in_started: None,
            volume: 1.0,
            muted: false,
            current_path: None,
            current_is_ffmpeg: false,
        }
    }

    /// 用户期望的目标音量（静音为 0）
    fn target_volume(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.volume
        }
    }

    /// 立即把当前 sink 设到目标音量（音量 / 静音变更时调用）
    fn apply_volume(&self) {
        if let Some(playback) = &self.current {
            playback.sink.set_volume(self.target_volume());
        }
    }

    /// smoothstep 缓动，避免线性渐变在端点处可闻的台阶感
    fn ease(t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// 渐变驱动（每 ~20ms）：新曲目淡入到目标音量，旧曲目淡出到静默后回收
    fn tick_fades(&mut self) {
        // 新曲目淡入
        if let Some(started) = self.fade_in_started {
            let t = started.elapsed().as_millis() as f32 / FADE_MS as f32;
            if let Some(playback) = &self.current {
                playback.sink.set_volume(self.target_volume() * Self::ease(t));
            }
            if t >= 1.0 {
                self.fade_in_started = None;
                self.apply_volume();
            }
        }

        // 旧曲目淡出：结束即停止并释放（含 ffmpeg 子进程）
        if let Some(fading) = &self.fading_out {
            let t = fading.started.elapsed().as_millis() as f32 / FADE_MS as f32;
            let gain = fading.from * (1.0 - Self::ease(t));
            fading.playback.sink.set_volume(gain);
            if t >= 1.0 {
                if let Some(done) = self.fading_out.take() {
                    done.playback.halt();
                }
            }
        }
    }

    /// 确保输出设备可用（首次播放或启动时无设备的延迟重试）
    fn ensure_stream(&mut self) -> AppResult<&rodio::OutputStreamHandle> {
        if self.handle.is_none() {
            let (stream, handle) = rodio::OutputStream::try_default()
                .map_err(|e| AppError::Player(format!("无法创建音频输出设备: {}", e)))?;
            self.stream = Some(stream);
            self.handle = Some(handle);
        }
        self.handle
            .as_ref()
            .ok_or_else(|| AppError::Player("音频输出设备不可用".into()))
    }

    fn play(&mut self, path: &std::path::Path) -> AppResult<()> {
        let handle = self.ensure_stream()?.clone();

        // 旧曲目转入淡出链路（交叉淡化）；淡出起点取其当前实际增益，
        // 淡入中途切歌也不会出现音量跳变
        if let Some(old) = self.current.take() {
            let from = match &self.fade_in_started {
                Some(started) => {
                    let t = started.elapsed().as_millis() as f32 / FADE_MS as f32;
                    self.target_volume() * Self::ease(t)
                }
                None => self.target_volume(),
            };
            // 同一时刻只保留一条淡出链路：更早的旧链路立即回收
            if let Some(older) = self.fading_out.take() {
                older.playback.halt();
            }
            self.fading_out = Some(FadingOut {
                playback: old,
                started: Instant::now(),
                from,
            });
        }
        self.fade_in_started = None;

        let sink = rodio::Sink::try_new(&handle)
            .map_err(|e| AppError::Player(format!("无法创建音频 sink: {}", e)))?;

        let is_ffmpeg = !is_native_format(path);
        let ffmpeg_child = if is_ffmpeg {
            let (child, source) = spawn_ffmpeg_decoder(path, 0)?;
            sink.append(source);
            Some(child)
        } else {
            let file = std::fs::File::open(path).map_err(|e| {
                AppError::Player(format!("无法打开音频文件 {}: {}", path.display(), e))
            })?;
            let decoder = rodio::Decoder::new(BufReader::new(file))
                .map_err(|e| AppError::Player(format!("不支持的音频格式或文件损坏: {}", e)))?;
            sink.append(decoder);
            None
        };

        // 新曲目从静默开始淡入
        sink.set_volume(0.0);
        self.current = Some(Playback {
            sink,
            ffmpeg_child,
        });
        self.fade_in_started = Some(Instant::now());
        self.current_path = Some(path.to_path_buf());
        self.current_is_ffmpeg = is_ffmpeg;
        Ok(())
    }

    /// 播放并定位到断点（断点续播专用）
    fn play_at(&mut self, path: &std::path::Path, position_ms: u64) -> AppResult<()> {
        self.play(path)?;
        if position_ms > 0 {
            // sink 新建后混音线程可能还没拉到首帧；此时立即 seek 在个别时序下
            // 会让 Sink::try_seek 永久等待（引擎线程卡死、进度冻结）。
            // 先让出 ~150ms 等输出线程开始消费，再定位。
            std::thread::sleep(Duration::from_millis(150));
            self.seek(position_ms)?;
        }
        Ok(())
    }

    fn pause(&mut self) -> AppResult<()> {
        if let Some(playback) = &self.current {
            playback.sink.pause();
        }
        Ok(())
    }

    fn resume(&mut self) -> AppResult<()> {
        if let Some(playback) = &self.current {
            playback.sink.play();
        }
        Ok(())
    }

    fn stop(&mut self) -> AppResult<()> {
        if let Some(playback) = self.current.take() {
            playback.halt();
        }
        if let Some(fading) = self.fading_out.take() {
            fading.playback.halt();
        }
        self.fade_in_started = None;
        self.current_path = None;
        self.current_is_ffmpeg = false;
        Ok(())
    }

    fn seek(&mut self, position_ms: u64) -> AppResult<()> {
        let Some(playback) = self.current.as_mut() else {
            return Ok(());
        };

        if self.current_is_ffmpeg {
            // ffmpeg 后端：PCM 管道不可 seek，带 -ss 重启解码进程
            let path = self
                .current_path
                .clone()
                .ok_or_else(|| AppError::Player("没有正在播放的曲目".into()))?;
            let (child, source) = spawn_ffmpeg_decoder(&path, position_ms)?;
            if let Some(old) = playback.ffmpeg_child.take() {
                let mut old = old;
                let _ = old.kill();
                let _ = old.wait();
            }
            playback.ffmpeg_child = Some(child);
            playback.sink.clear();
            playback.sink.append(source);
            playback.sink.play();
        } else {
            playback
                .sink
                .try_seek(std::time::Duration::from_millis(position_ms))
                .map_err(|e| AppError::Player(format!("seek 失败: {}", e)))?;
        }

        // seek 后做一次快速淡入，消除解码重启 / 定位点的爆音
        self.fade_in_started = Some(Instant::now());
        Ok(())
    }

    fn set_volume(&mut self, volume: f32) -> AppResult<()> {
        self.volume = volume.clamp(0.0, 1.0);
        // 淡入进行中由 tick 渐变到新目标；否则立即生效
        if self.fade_in_started.is_none() {
            self.apply_volume();
        }
        Ok(())
    }

    fn set_muted(&mut self, muted: bool) -> AppResult<()> {
        self.muted = muted;
        if self.fade_in_started.is_none() {
            self.apply_volume();
        }
        Ok(())
    }

    /// 当前曲目是否已在引擎侧播完（sink 中无音源）。
    /// 用途：ffmpeg 缺失/秒退、时长探测为 0 等无法靠时间判定结束的场景。
    fn current_ended(&self) -> bool {
        self.current
            .as_ref()
            .map(|playback| playback.sink.empty())
            .unwrap_or(false)
    }
}

impl Drop for RodioEngine {
    fn drop(&mut self) {
        if let Some(playback) = self.current.take() {
            playback.halt();
        }
        if let Some(fading) = self.fading_out.take() {
            fading.playback.halt();
        }
    }
}

/// 启动 ffmpeg 将任意格式解码为 PCM s16le（44.1kHz 双声道）并输出到管道，
/// 返回（子进程, rodio 音源）。seek 通过 -ss 在输入端定位实现。
fn spawn_ffmpeg_decoder(path: &Path, offset_ms: u64) -> AppResult<(Child, PcmStreamSource)> {
    let ffmpeg = resolve_ffmpeg().ok_or_else(|| {
        AppError::Player("该格式需要 FFmpeg 解码，未找到 bin/ffmpeg.exe".into())
    })?;

    let mut cmd = Command::new(ffmpeg);
    if offset_ms > 0 {
        cmd.arg("-ss")
            .arg(format!("{:.3}", offset_ms as f64 / 1000.0));
    }
    cmd.arg("-v")
        .arg("error")
        .arg("-i")
        .arg(path)
        .arg("-f")
        .arg("s16le")
        .arg("-acodec")
        .arg("pcm_s16le")
        .arg("-ar")
        .arg("44100")
        .arg("-ac")
        .arg("2")
        .arg("-")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW：避免解码期间弹出控制台窗口
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::Player(format!("无法启动 ffmpeg: {}", e)))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Player("无法获取 ffmpeg 输出管道".into()))?;

    Ok((
        child,
        PcmStreamSource::new(BufReader::new(stdout), 2, 44100),
    ))
}

/// ffmpeg PCM 管道音源（s16le 交错样本 → rodio::Source）
///
/// 内部缓冲大块样本（默认 8192 样本 = 16384 字节），避免逐样本 read 导致的
/// 高频 syscall 与混音线程欠载（表现为爆音/杂音）。
struct PcmStreamSource {
    reader: BufReader<ChildStdout>,
    channels: u16,
    sample_rate: u32,
    /// 解码输出的样本缓冲（i16 交错）
    buffer: Vec<i16>,
    /// 当前缓冲读取位置
    pos: usize,
    /// 是否到达流末尾
    eof: bool,
}

impl PcmStreamSource {
    const CHUNK_SAMPLES: usize = 8192;

    fn new(reader: BufReader<ChildStdout>, channels: u16, sample_rate: u32) -> Self {
        Self {
            reader,
            channels,
            sample_rate,
            buffer: Vec::with_capacity(Self::CHUNK_SAMPLES),
            pos: 0,
            eof: false,
        }
    }

    /// 从管道填充一块样本到 buffer；失败/EOF 时标记 eof
    fn refill(&mut self) {
        if self.eof {
            return;
        }
        let mut bytes = vec![0u8; Self::CHUNK_SAMPLES * 2];
        match self.reader.read(&mut bytes) {
            Ok(0) => {
                self.eof = true;
            }
            Ok(n) => {
                let sample_bytes = n - (n % 2);
                self.buffer.clear();
                self.buffer.extend(
                    bytes[..sample_bytes]
                        .chunks_exact(2)
                        .map(|c| i16::from_le_bytes([c[0], c[1]])),
                );
                self.pos = 0;
            }
            Err(_) => {
                self.eof = true;
            }
        }
    }
}

impl Iterator for PcmStreamSource {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        loop {
            if self.pos < self.buffer.len() {
                let sample = self.buffer[self.pos];
                self.pos += 1;
                return Some(sample);
            }
            if self.eof {
                return None;
            }
            self.refill();
        }
    }
}

impl rodio::Source for PcmStreamSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}
