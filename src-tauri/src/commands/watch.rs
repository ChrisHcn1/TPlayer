//! 目录自动监控命令：查询监控状态

use tauri::State;

use crate::watch::WatchStatus;
use crate::AppState;

/// 查询目录自动监控状态（开关、监控目录、最近一次增量同步）
#[tauri::command]
pub fn watcher_status(state: State<'_, AppState>) -> WatchStatus {
    state.watch.snapshot()
}
