//! 设置命令：加载/保存 config.json

use tauri::State;

use crate::config::AppConfig;
use crate::error::AppResult;
use crate::AppState;

/// 加载完整配置
#[tauri::command]
pub fn settings_load(state: State<'_, AppState>) -> AppResult<AppConfig> {
    Ok(state.config.lock().unwrap().clone())
}

/// 保存完整配置（整体替换）
#[tauri::command]
pub fn settings_save(state: State<'_, AppState>, config: AppConfig) -> AppResult<()> {
    let mut current = state.config.lock().unwrap();
    *current = config;
    current.save(&state.config_path)?;

    // 配置落盘后按新配置重启目录监控（目录白名单 / 开关变化即时生效）
    let watch_dirs = current.library_dirs.clone();
    let watch_enabled = current.watch.enabled;
    drop(current);
    state
        .watch
        .reconfigure(state.app.clone(), &watch_dirs, watch_enabled);
    Ok(())
}
