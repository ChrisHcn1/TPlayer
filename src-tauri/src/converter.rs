//! 音频转换模块：ffmpeg 批量转码（MP3 320k / FLAC / WAV / AAC 256k）
//!
//! 架构：命令层接收任务列表后交给专用线程执行，进度经
//! `converter://event` 推送前端（与 playback://event 同构的 tag+payload 结构）。
//! 执行阶段为**受控并发**（worker 数 = 核数一半、上限 4），多个 ffmpeg 同时转码；
//! 输出名在分配时串行预留，保证并行下不重名、不覆盖。
//! 取消通过原子标志：任务间直接跳过，任务中 kill 当前 ffmpeg 子进程。
//! 输出文件与源文件同名换扩展名，已存在时追加 " (n)" 编号，绝不覆盖。

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{AppError, AppResult};
use crate::library::cue;
use crate::library::sacd::SacdDisc;
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
            // 缺依赖走独立错误码：前端据此弹出「安装解码组件」引导，
            // 而非只显示一条会被忽略的 toast。
            AppError::DependencyMissing(
                "音频转换需要 FFmpeg。请在「设置 → 解码组件」中安装 FFmpeg 后重试".into(),
            )
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

/// 转换工作线程主体：把每个输入按类型展开为工作单元（普通音频 / CUE / SACD ISO），
/// 顺序执行，逐个推送事件，结束后推送 Finished。
///
/// 统一流程：不再区分「单文件转换」与「分轨转换」，由 `expand_input` 自动识别输入类型，
/// 同一批次可混合普通文件与 .cue / .iso，用户只需一次「开始转换」。
fn run_batch(
    app: &AppHandle,
    tasks: Vec<ConvertTaskInput>,
    options: ConvertOptions,
    format: OutputFormat,
    output_dir: PathBuf,
    cancel: Arc<AtomicBool>,
) {
    // 展开阶段：每个输入 → 一组工作单元（分轨可展开为多轨）
    let mut work: Vec<WorkUnit> = Vec::new();
    let mut failed = 0usize;
    for task in &tasks {
        match expand_input(task, &options, &output_dir) {
            Ok(items) => work.extend(items),
            Err(e) => {
                failed += 1;
                emit(
                    app,
                    ConvertEvent::Failed {
                        task_id: task.id.clone(),
                        message: e.to_string(),
                    },
                );
            }
        }
    }

    // ---- 执行阶段：受控并发（多个 ffmpeg 同时转码）----
    //
    // 性能：原为顺序执行，整批耗时 ≈ 各文件耗时之和；现按核数派生 worker 数
    // （上限 4，兼顾整批吞吐与单任务速度）并行转码，整批耗时大幅下降。
    //
    // 正确性保障：
    //  - 输出名在分配时串行预留（unique_output），并行下绝不重名、绝不覆盖既有文件；
    //  - 进度 / 完成 / 失败事件仍按 task_id 独立上报，前端按任务维护状态，无需改动；
    //  - 取消沿用原子标志：待执行任务直接跳过，进行中的 ffmpeg 子进程被 kill；
    //  - 计数用原子变量，批次结束统一汇总。
    reset_output_reservations();
    let completed = AtomicUsize::new(0);
    let failed_running = AtomicUsize::new(0);
    let cancelled = AtomicBool::new(false);
    let next_index = AtomicUsize::new(0);
    let worker_count = convert_worker_count();

    thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| {
                loop {
                    // 取消：不再领取新任务
                    if cancel.load(Ordering::SeqCst) {
                        cancelled.store(true, Ordering::SeqCst);
                        break;
                    }
                    let idx = next_index.fetch_add(1, Ordering::SeqCst);
                    let Some(seg) = work.get(idx) else {
                        break;
                    };
                    // SACD 解码失败的占位任务：明确错误，避免误导为「文件不存在」
                    if seg.input.to_string_lossy().starts_with("__sacd_missing_") {
                        failed_running.fetch_add(1, Ordering::SeqCst);
                        emit(
                            app,
                            ConvertEvent::Failed {
                                task_id: seg.task_id.clone(),
                                message: format!(
                                    "SACD 分轨失败（{}）：该轨解码失败，可能是 DST 压缩或多声道，暂不支持",
                                    seg.stem
                                ),
                            },
                        );
                        continue;
                    }
                    match convert_segment(app, seg, &options, format, &output_dir, &cancel) {
                        Ok(()) => {
                            completed.fetch_add(1, Ordering::SeqCst);
                            // 分轨导出的临时 DSF 转码完成后清理
                            if seg.temp_input {
                                let _ = std::fs::remove_file(&seg.input);
                            }
                        }
                        Err(TaskError::Cancelled) => {
                            cancelled.store(true, Ordering::SeqCst);
                            emit(
                                app,
                                ConvertEvent::Failed {
                                    task_id: seg.task_id.clone(),
                                    message: "已取消".into(),
                                },
                            );
                        }
                        Err(TaskError::Failed(message)) => {
                            failed_running.fetch_add(1, Ordering::SeqCst);
                            emit(
                                app,
                                ConvertEvent::Failed {
                                    task_id: seg.task_id.clone(),
                                    message,
                                },
                            );
                        }
                    }
                }
            });
        }
    });

    let completed = completed.load(Ordering::SeqCst);
    let failed = failed + failed_running.load(Ordering::SeqCst);
    let cancelled = cancelled.load(Ordering::SeqCst);

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

/// 把单个转换输入按类型展开为工作单元（统一流程的核心分流点）：
/// - `.cue` → CUE 分轨（每条音轨一段）
/// - `.iso` → SACD ISO 分轨（每轨一段，DSD 导出失败回退 sacd_extract）
/// - 其它音频 → 整文件转码（单段）
/// 无法识别 / 解析失败则整体返回错误，由 `run_batch` 登记为失败任务。
fn expand_input(
    task: &ConvertTaskInput,
    _options: &ConvertOptions,
    output_dir: &Path,
) -> AppResult<Vec<WorkUnit>> {
    let ext = Path::new(&task.path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if ext == "cue" {
        build_cue_segments(&task.path, output_dir, &task.id)
    } else if ext == "iso" {
        build_sacd_segments(&task.path, output_dir, &task.id)
    } else {
        let stem = Path::new(&task.path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output")
            .to_string();
        Ok(vec![WorkUnit {
            task_id: task.id.clone(),
            input: PathBuf::from(&task.path),
            start_ms: None,
            end_ms: None,
            stem,
            temp_input: false,
        }])
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
/// 输出文件名预留集合（按批次重置）。
///
/// 并行转码时多个任务可能同名（同名文件、同一专辑多轨同名），
/// 若仅靠「磁盘上是否存在」判断，会出现两个任务拿到同一输出路径。
/// 分配时一并预留，保证同一批次内输出名唯一且绝不覆盖既有文件。
static OUTPUT_RESERVED: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

fn output_reserved() -> &'static Mutex<HashSet<PathBuf>> {
    OUTPUT_RESERVED.get_or_init(|| Mutex::new(HashSet::new()))
}

/// 批次开始时重置预留集合（同一时间仅允许一批转换，见 running 标志）
fn reset_output_reservations() {
    if let Ok(mut set) = output_reserved().lock() {
        set.clear();
    }
}

/// 分配唯一输出路径：磁盘已存在或本批次已被预留时顺延 " (n)" 编号，绝不覆盖。
fn unique_output(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut reserved = output_reserved().lock().unwrap_or_else(|e| e.into_inner());
    let mut candidate = dir.join(format!("{stem}.{ext}"));
    let mut n = 1;
    while candidate.exists() || reserved.contains(&candidate) {
        candidate = dir.join(format!("{stem} ({n}).{ext}"));
        n += 1;
    }
    reserved.insert(candidate.clone());
    candidate
}

/// 转换并发度：ffmpeg 自身已多线程，占满全部核心反而拖慢单任务；
/// 取「核数的一半、上限 4」，在整批吞吐与单任务速度之间取平衡。
fn convert_worker_count() -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);
    (cpus / 2).clamp(1, 4)
}

fn emit(app: &AppHandle, event: ConvertEvent) {
    let _ = app.emit("converter://event", &event);
}

// ===================== 实验室·CUE/SACD 分轨转换 =====================
//
// 把 CUE 镜像（含分轨时间信息）或 SACD ISO（含多音轨）按曲目切割并转码为指定格式。
// 与 start_batch 共享同一 running 锁：同一时间只允许一批（普通转码或分轨）。

/// 单个转换工作单元（普通文件为整文件转码；分轨为按起止截取的一段）。
/// 这是「统一转换流程」的基本执行单位：普通音频 / CUE / SACD ISO 经 `expand_input`
/// 展开为一组 WorkUnit 后由同一循环顺序执行，名称一律由后端真实 output_path 回填，
/// 确保任务名与实际文件始终一致。
struct WorkUnit {
    /// 任务 ID（前端事件关联）
    task_id: String,
    /// 输入文件：CUE → 镜像音频文件；SACD → 导出的临时 DSF
    input: PathBuf,
    /// 截取起点（毫秒），None 表示从头
    start_ms: Option<u64>,
    /// 截取终点（毫秒），None 表示到尾
    end_ms: Option<u64>,
    /// 输出文件名主干（不含扩展名）
    stem: String,
    /// 输入是否为导出产生的临时文件（任务完成后清理）
    temp_input: bool,
}

// CUE/SACD 分轨已并入统一转换流程（start_batch → expand_input → run_batch），
// 不再单独暴露 start_split 命令。

/// 解析 CUE：每条音轨一个分轨任务（按 cue_offset/duration 截取）。
///
/// `parse_cue` 返回的 `Track` 均为可播放音轨（DATA 数据轨不会生成 Track），
/// 其 `cue_offset` 为在镜像文件中的起始秒数，`duration` 为单轨时长。
fn build_cue_segments(source: &str, _output_dir: &Path, prefix: &str) -> AppResult<Vec<WorkUnit>> {
    let tracks = cue::parse_cue(Path::new(source));
    let mut segments = Vec::new();
    for (i, t) in tracks.iter().enumerate() {
        if !t.is_cue || t.file_path.is_empty() {
            continue;
        }
        let input = PathBuf::from(&t.file_path);
        if !input.is_file() {
            continue;
        }
        let offset = t.cue_offset.unwrap_or(0.0);
        let start_ms = if offset > 0.0 {
            Some((offset * 1000.0) as u64)
        } else {
            None
        };
        let end_ms = if t.duration > 0.0 {
            Some(((offset + t.duration) * 1000.0) as u64)
        } else {
            None
        };
        let title = t.title.clone();
        let stem = sanitize_stem(&format!("{:02} - {}", i + 1, title));
        segments.push(WorkUnit {
            task_id: format!("{prefix}-c{i:02}"),
            input,
            start_ms,
            end_ms,
            stem,
            temp_input: false,
        });
    }
    Ok(segments)
}

/// 解析 SACD ISO：每轨导出为临时 DSF 再转码（内置裸 DSD 导出失败则回退 sacd_extract）。
fn build_sacd_segments(source: &str, output_dir: &Path, prefix: &str) -> AppResult<Vec<WorkUnit>> {
    let Some(disc) = SacdDisc::open(Path::new(source)) else {
        return Err(AppError::Converter("无法打开 SACD ISO".into()));
    };
    let tracks = disc.to_tracks();
    if tracks.is_empty() {
        return Ok(Vec::new());
    }
    let mut segments = Vec::new();
    for (i, t) in tracks.iter().enumerate() {
        let idx = t.sacd_track_index as usize;
        let title = if t.title.trim().is_empty() {
            format!("Track {:02}", i + 1)
        } else {
            t.title.clone()
        };
        let stem = sanitize_stem(&format!("{:02} - {}", i + 1, title));
        let dsf = output_dir.join(format!(".split-{:02}.dsf", i + 1));
        // 优先内置裸 DSD 导出；DST 压缩 / 多声道回退到 sacd_extract（1-based 曲目号）
        let exported = disc
            .export_track_to_dsf(idx, &dsf)
            .map_err(|e| e.to_string())
            .or_else(|_| disc.extract_track_via_external(idx as u32 + 1, false, &dsf).map_err(|e| e.to_string()));
        match exported {
            Ok(()) => segments.push(WorkUnit {
                task_id: format!("{prefix}-s{i:02}"),
                input: dsf,
                start_ms: None,
                end_ms: None,
                stem,
                temp_input: true,
            }),
            Err(_) => {
                // 该轨解码失败：登记为失败任务（输入置为不存在，convert_segment 会报文件不存在）
                segments.push(WorkUnit {
                    task_id: format!("{prefix}-s{i:02}"),
                    input: PathBuf::from(format!("__sacd_missing_{i}")),
                    start_ms: None,
                    end_ms: None,
                    stem,
                    temp_input: false,
                });
            }
        }
    }
    Ok(segments)
}

// run_split 逻辑已并入 run_batch（统一展开 + 顺序执行），保留 convert_segment 作为
// 单/分轨通用的执行单元。

/// 单个分轨任务：按起点/终点截取并转码。
fn convert_segment(
    app: &AppHandle,
    seg: &WorkUnit,
    options: &ConvertOptions,
    format: OutputFormat,
    output_dir: &Path,
    cancel: &AtomicBool,
) -> Result<(), TaskError> {
    if !seg.input.is_file() {
        return Err(TaskError::Failed(format!("文件不存在: {}", seg.input.display())));
    }

    let output = unique_output(output_dir, &seg.stem, format.extension());

    // 截取时长（用于进度换算）：优先用起止差，否则用探测总时长扣除起点
    let segment_duration_ms: u64 = match (seg.start_ms, seg.end_ms) {
        (Some(s), Some(e)) if e > s => e - s,
        _ => {
            let total = (probe_duration(&seg.input).unwrap_or(0.0) * 1000.0) as u64;
            match (seg.start_ms, seg.end_ms) {
                (Some(s), None) => total.saturating_sub(s),
                (None, Some(e)) => e,
                _ => total,
            }
        }
    };
    // 写入 -t 的截取时长（None = 到尾）
    let duration_ms: Option<u64> = match (seg.start_ms, seg.end_ms) {
        (Some(s), Some(e)) if e > s => Some(e - s),
        (None, Some(e)) => Some(e),
        _ => None,
    };

    emit(
        app,
        ConvertEvent::Started {
            task_id: seg.task_id.clone(),
            output_path: output.display().to_string(),
        },
    );

    let ffmpeg = resolve_ffmpeg().expect("start_split 已校验 ffmpeg 存在");
    let mut cmd = crate::util::hidden_command(ffmpeg);
    cmd.arg("-y")
        .arg("-v")
        .arg("error");
    if let Some(s) = seg.start_ms {
        cmd.arg("-ss").arg((s as f64 / 1000.0).to_string());
    }
    cmd.arg("-i")
        .arg(&seg.input)
        .arg("-vn")
        .arg("-map_metadata")
        .arg("0")
        .arg("-ar")
        .arg(options.sample_rate.to_string())
        .args(format.codec_args(options.bit_depth));
    if let Some(d) = duration_ms {
        cmd.arg("-t").arg((d as f64 / 1000.0).to_string());
    }
    cmd.arg("-progress")
        .arg("pipe:1")
        .arg("-nostats")
        .arg(&output)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| TaskError::Failed(format!("启动 ffmpeg 失败: {e}")))?;

    let out_time_ms = Arc::new(AtomicU64::new(0));
    let stderr_text = Arc::new(Mutex::new(String::new()));
    let readers = spawn_readers(&mut child, out_time_ms.clone(), stderr_text.clone());

    let mut last_progress = 0u8;
    let status = loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            join_readers(readers);
            let _ = std::fs::remove_file(&output);
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
        if segment_duration_ms > 0 {
            let played = out_time_ms.load(Ordering::Relaxed);
            let progress = ((played.min(segment_duration_ms) * 100) / segment_duration_ms).min(99) as u8;
            if progress > last_progress {
                last_progress = progress;
                emit(
                    app,
                    ConvertEvent::Progress {
                        task_id: seg.task_id.clone(),
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
                task_id: seg.task_id.clone(),
                progress: 100,
            },
        );
        emit(
            app,
            ConvertEvent::Completed {
                task_id: seg.task_id.clone(),
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
            detail.lines().last().unwrap_or("未知错误").to_string()
        };
        let _ = std::fs::remove_file(&output);
        Err(TaskError::Failed(message))
    }
}

/// 清洗文件名主干中的非法字符（Windows/Unix 通用）
fn sanitize_stem(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        "track".to_string()
    } else {
        trimmed.to_string()
    }
}
