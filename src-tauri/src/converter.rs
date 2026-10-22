//! 音频转换模块：ffmpeg 批量转码（MP3 320k / FLAC / WAV / AAC 256k）
//!
//! 架构：命令层接收任务列表后交给专用线程顺序执行，进度经
//! `converter://event` 推送前端（与 playback://event 同构的 tag+payload 结构）。
//! 取消通过原子标志：任务间直接跳过，任务中 kill 当前 ffmpeg 子进程。
//! 输出文件与源文件同名换扩展名，已存在时追加 " (n)" 编号，绝不覆盖。

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{AppError, AppResult};
use crate::player::engine::{probe_duration, resolve_ffmpeg};

/// 转换任务输入（前端生成 ID，事件按此回执关联）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertTaskInput {
    pub id: String,
    /// 源文件绝对路径
    pub path: String,
}

/// 输出设置
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertOptions {
    /// 输出格式：mp3-320 / flac / wav / aac-256
    pub format: String,
    /// 采样率（Hz）：44100 / 48000 / 96000
    pub sample_rate: u32,
    /// 位深（16 / 24，仅无损格式生效）
    pub bit_depth: u16,
    /// 输出目录
    pub output_dir: String,
}

/// 转换事件（后端 -> 前端推送）
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "lowercase")]
pub enum ConvertEvent {
    Started {
        #[serde(rename = "taskId")]
        task_id: String,
        #[serde(rename = "outputPath")]
        output_path: String,
    },
    Progress {
        #[serde(rename = "taskId")]
        task_id: String,
        /// 0 ~ 100；时长未知时保持 0，完成时跳到 100
        progress: u8,
    },
    Completed {
        #[serde(rename = "taskId")]
        task_id: String,
        #[serde(rename = "outputPath")]
        output_path: String,
    },
    Failed {
        #[serde(rename = "taskId")]
        task_id: String,
        message: String,
    },
    /// 整批队列结束（含取消）：completed/failed 为已完成/失败数，cancelled 表示被取消
    Finished {
        completed: usize,
        failed: usize,
        cancelled: bool,
    },
}

/// 转换器运行状态（同一时间只允许一批任务）
#[derive(Default)]
pub struct ConverterState {
    running: AtomicBool,
    cancel: Arc<AtomicBool>,
}

impl ConverterState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// 请求取消当前批次（无任务在跑则无副作用）
    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

/// 输出格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Mp3320,
    Flac,
    Wav,
    Aac256,
}

impl OutputFormat {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "mp3-320" => Some(Self::Mp3320),
            "flac" => Some(Self::Flac),
            "wav" => Some(Self::Wav),
            "aac-256" => Some(Self::Aac256),
            _ => None,
        }
    }

    /// 输出文件扩展名
    fn extension(self) -> &'static str {
        match self {
            Self::Mp3320 => "mp3",
            Self::Flac => "flac",
            Self::Wav => "wav",
            Self::Aac256 => "aac",
        }
    }

    /// 音频编码参数（追加在 -ar 之后）
    fn codec_args(self, bit_depth: u16) -> Vec<String> {
        match self {
            Self::Mp3320 => vec![
                "-c:a".into(),
                "libmp3lame".into(),
                "-b:a".into(),
                "320k".into(),
            ],
            Self::Aac256 => vec!["-c:a".into(), "aac".into(), "-b:a".into(), "256k".into()],
            Self::Flac => {
                // flac 无 24bit sample_fmt：24bit 用 s32 容器承载
                let sample_fmt = if bit_depth >= 24 { "s32" } else { "s16" };
                vec!["-c:a".into(), "flac".into(), "-sample_fmt".into(), sample_fmt.into()]
            }
            Self::Wav => {
                let codec = if bit_depth >= 24 { "pcm_s24le" } else { "pcm_s16le" };
                vec!["-c:a".into(), codec.into()]
            }
        }
    }
}

/// 探测文件信息（添加待转文件时展示大小）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileProbe {
    pub path: String,
    pub size_bytes: u64,
    pub exists: bool,
}

pub fn probe_files(paths: Vec<String>) -> Vec<FileProbe> {
    paths
        .into_iter()
        .map(|path| {
            let meta = std::fs::metadata(&path);
            FileProbe {
                size_bytes: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                exists: meta.is_ok(),
                path,
            }
        })
        .collect()
}

/// 启动一批转换任务：校验后交给专用线程顺序执行。
/// 返回接受的任务数；ffmpeg 缺失/输出目录不可用/已有批次在跑时返回错误。
pub fn start_batch(
    state: &ConverterState,
    app: &AppHandle,
    tasks: Vec<ConvertTaskInput>,
    options: ConvertOptions,
) -> AppResult<usize> {
    let count = tasks.len();
    if tasks.is_empty() {
        return Err(AppError::Converter("没有待转换的文件".into()));
    }
    if state.running.swap(true, Ordering::SeqCst) {
        return Err(AppError::Converter("已有转换任务在进行中".into()));
    }

    let result: AppResult<()> = (|| {
        let format = OutputFormat::parse(&options.format)
            .ok_or_else(|| AppError::Converter(format!("不支持的输出格式: {}", options.format)))?;
        resolve_ffmpeg().ok_or_else(|| {
            // 平台化提示：Linux 下 ffmpeg 可来自系统 PATH 或随包内置
            // （/usr/lib/tplayer/bin）；Windows 下随安装包分发于 bin/。
            #[cfg(target_os = "linux")]
            let hint = "未找到 ffmpeg（请安装系统 ffmpeg：apt install ffmpeg，或使用随包内置版本）";
            #[cfg(not(target_os = "linux"))]
            let hint = "未找到 bin/ffmpeg.exe，无法执行转换";
            AppError::Converter(hint.into())
        })?;

        let output_dir = PathBuf::from(&options.output_dir);
        std::fs::create_dir_all(&output_dir)
            .map_err(|e| AppError::Converter(format!("输出目录不可用: {e}")))?;

        let cancel = state.cancel.clone();
        cancel.store(false, Ordering::SeqCst);

        let app = app.clone();
        thread::Builder::new()
            .name("converter-worker".into())
            .spawn(move || run_batch(&app, tasks, options, format, output_dir, cancel))
            .map_err(|e| AppError::Converter(format!("启动转换线程失败: {e}")))?;
        Ok(())
    })();

    if result.is_err() {
        // 启动失败：释放运行标志
        state.running.store(false, Ordering::SeqCst);
    }
    Ok(count)
}

/// 转换工作线程主体：顺序执行任务，逐个推送事件，结束后推送 Finished
fn run_batch(
    app: &AppHandle,
    tasks: Vec<ConvertTaskInput>,
    options: ConvertOptions,
    format: OutputFormat,
    output_dir: PathBuf,
    cancel: Arc<AtomicBool>,
) {
    let mut completed = 0usize;
    let mut failed = 0usize;
    let mut cancelled = false;

    for task in tasks {
        if cancel.load(Ordering::SeqCst) {
            cancelled = true;
            break;
        }
        match convert_one(app, &task, &options, format, &output_dir, &cancel) {
            Ok(()) => completed += 1,
            Err(TaskError::Cancelled) => {
                cancelled = true;
                emit(
                    app,
                    ConvertEvent::Failed {
                        task_id: task.id,
                        message: "已取消".into(),
                    },
                );
                break;
            }
            Err(TaskError::Failed(message)) => {
                failed += 1;
                emit(
                    app,
                    ConvertEvent::Failed {
                        task_id: task.id,
                        message,
                    },
                );
            }
        }
    }

    emit(
        app,
        ConvertEvent::Finished {
            completed,
            failed,
            cancelled,
        },
    );

    // 批次结束：复位运行标志，允许下一批任务
    if let Some(state) = app.try_state::<crate::AppState>() {
        state.converter.running.store(false, Ordering::SeqCst);
    }
}

/// 单任务失败语义：Cancelled（用户取消，停止整批）与 Failed（跳过该任务继续）
enum TaskError {
    Cancelled,
    Failed(String),
}

fn convert_one(
    app: &AppHandle,
    task: &ConvertTaskInput,
    options: &ConvertOptions,
    format: OutputFormat,
    output_dir: &Path,
    cancel: &AtomicBool,
) -> Result<(), TaskError> {
    // Linux：曲库中的路径可能是非 UTF-8 文件名的容错编码（NTFS 场景），
    // 先还原为文件系统真实路径，否则 is_file 探测失败、转换无法进行。
    // 合法路径零成本（一次 exists 探测）；Windows 行为不变。
    #[cfg(target_os = "linux")]
    let input = crate::library::scan_linux::resolve_path_str(&task.path);
    #[cfg(not(target_os = "linux"))]
    let input = PathBuf::from(&task.path);
    if !input.is_file() {
        return Err(TaskError::Failed(format!("文件不存在: {}", task.path)));
    }

    // 输出路径：同名换扩展名，冲突时追加 " (n)"
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output")
        .to_string();
    let output = unique_output(output_dir, &stem, format.extension());

    let duration_ms = (probe_duration(&input).unwrap_or(0.0) * 1000.0) as u64;

    emit(
        app,
        ConvertEvent::Started {
            task_id: task.id.clone(),
            output_path: output.display().to_string(),
        },
    );

    let ffmpeg = resolve_ffmpeg().expect("start_batch 已校验 ffmpeg 存在");
    let mut cmd = crate::util::hidden_command(ffmpeg);
    cmd.arg("-y")
        .arg("-v")
        .arg("error")
        .arg("-i")
        .arg(&input)
        .arg("-vn")
        .arg("-map_metadata")
        .arg("0")
        .arg("-ar")
        .arg(options.sample_rate.to_string())
        .args(format.codec_args(options.bit_depth))
        .arg("-progress")
        .arg("pipe:1")
        .arg("-nostats")
        .arg(&output)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| TaskError::Failed(format!("启动 ffmpeg 失败: {e}")))?;

    // 进度读取线程：解析 -progress 输出的 out_time 时间串换算为毫秒
    let out_time_ms = Arc::new(AtomicU64::new(0));
    let stderr_text = Arc::new(Mutex::new(String::new()));
    let readers = spawn_readers(&mut child, out_time_ms.clone(), stderr_text.clone());

    // 轮询：等待退出 / 推送进度 / 响应取消
    let mut last_progress = 0u8;
    let status = loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            join_readers(readers);
            let _ = std::fs::remove_file(&output); // 回收半成品
            return Err(TaskError::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                join_readers(readers);
                return Err(TaskError::Failed(format!("等待 ffmpeg 失败: {e}")));
            }
        }
        if duration_ms > 0 {
            let played = out_time_ms.load(Ordering::Relaxed);
            let progress = ((played.min(duration_ms) * 100) / duration_ms).min(99) as u8;
            if progress > last_progress {
                last_progress = progress;
                emit(
                    app,
                    ConvertEvent::Progress {
                        task_id: task.id.clone(),
                        progress,
                    },
                );
            }
        }
        thread::sleep(Duration::from_millis(200));
    };

    join_readers(readers);

    if status.success() {
        emit(
            app,
            ConvertEvent::Progress {
                task_id: task.id.clone(),
                progress: 100,
            },
        );
        emit(
            app,
            ConvertEvent::Completed {
                task_id: task.id.clone(),
                output_path: output.display().to_string(),
            },
        );
        Ok(())
    } else {
        let stderr = stderr_text.lock().unwrap().clone();
        let detail = stderr.trim();
        let message = if detail.is_empty() {
            format!("ffmpeg 退出码: {}", status.code().unwrap_or(-1))
        } else {
            // 只保留最后一行错误（stderr 可能包含多行进度噪音）
            detail.lines().last().unwrap_or("未知错误").to_string()
        };
        let _ = std::fs::remove_file(&output); // 回收半成品
        Err(TaskError::Failed(message))
    }
}

/// 转换线程的进度/错误读取线程（防止 pipe 缓冲塞满导致 ffmpeg 卡死）
fn spawn_readers(
    child: &mut Child,
    out_time_ms: Arc<AtomicU64>,
    stderr_text: Arc<Mutex<String>>,
) -> Vec<thread::JoinHandle<()>> {
    let mut handles = Vec::with_capacity(2);
    if let Some(stdout) = child.stdout.take() {
        handles.push(thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(value) = line.strip_prefix("out_time=") {
                    if let Some(ms) = parse_out_time_ms(value.trim()) {
                        out_time_ms.store(ms, Ordering::Relaxed);
                    }
                }
            }
        }));
    }
    if let Some(stderr) = child.stderr.take() {
        handles.push(thread::spawn(move || {
            let mut text = String::new();
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if !line.is_empty() {
                    text.push_str(&line);
                    text.push('\n');
                }
                // 上限 8KB，防止异常输出无限累积
                if text.len() > 8 * 1024 {
                    break;
                }
            }
            *stderr_text.lock().unwrap() = text;
        }));
    }
    handles
}

fn join_readers(handles: Vec<thread::JoinHandle<()>>) {
    for handle in handles {
        let _ = handle.join();
    }
}

/// 解析 -progress 输出的 out_time（HH:MM:SS.ffffff）为毫秒。
/// 不用 out_time_ms：部分 ffmpeg 版本该字段输出的是微秒值（2s 音频 → 2000000），
/// 直接按毫秒读会快 1000 倍；out_time 时间串无歧义。N/A 等非法值返回 None。
fn parse_out_time_ms(value: &str) -> Option<u64> {
    let mut parts = value.split(':');
    let hours: u64 = parts.next()?.parse().ok()?;
    let minutes: u64 = parts.next()?.parse().ok()?;
    let seconds: f64 = parts.next()?.parse().ok()?;
    let total_secs = (hours * 3600 + minutes * 60) as f64 + seconds;
    if total_secs < 0.0 {
        return None;
    }
    Some((total_secs * 1000.0) as u64)
}

/// 同名冲突时追加 " (n)" 编号，绝不覆盖既有文件
fn unique_output(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let base = dir.join(format!("{stem}.{ext}"));
    if !base.exists() {
        return base;
    }
    for n in 1.. {
        let candidate = dir.join(format!("{stem} ({n}).{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn emit(app: &AppHandle, event: ConvertEvent) {
    let _ = app.emit("converter://event", &event);
}
