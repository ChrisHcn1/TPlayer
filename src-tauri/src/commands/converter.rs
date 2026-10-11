//! 音频转换命令：启动批次 / 取消 / 探测文件信息
//!
//! 命令与前端 services/converter.ts 一一对应：
//!   - converter_start   启动一批转换任务（专用线程顺序执行，事件经 converter://event 推送）
//!   - converter_cancel  取消当前批次（任务间跳过，任务中 kill ffmpeg）
//!   - converter_probe_files  探测待转文件大小（添加文件时展示）

use tauri::{AppHandle, State};

use crate::converter::{self, ConvertOptions, ConvertTaskInput, FileProbe};
use crate::error::AppResult;
use crate::AppState;

/// 启动一批转换任务，返回接受的任务数
#[tauri::command]
pub fn converter_start(
    state: State<'_, AppState>,
    app: AppHandle,
    tasks: Vec<ConvertTaskInput>,
    options: ConvertOptions,
) -> AppResult<usize> {
    converter::start_batch(&state.converter, &app, tasks, options)
}

/// 取消当前转换批次
#[tauri::command]
pub fn converter_cancel(state: State<'_, AppState>) {
    state.converter.request_cancel();
}

/// 探测文件信息（大小 / 存在性）
#[tauri::command]
pub fn converter_probe_files(paths: Vec<String>) -> Vec<FileProbe> {
    converter::probe_files(paths)
}
