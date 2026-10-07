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
    Ok(())
}
