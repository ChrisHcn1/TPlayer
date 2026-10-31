//! 应用级命令：打开外部网址（官网入口）

use crate::error::{AppError, AppResult};
use crate::website;

/// 用系统默认浏览器打开外部网址（仅允许 http / https）。
///
/// 供前端「设置 → 更新 → 官方网站」入口调用；与首次安装后自动打开官网
/// 复用同一套校验与打开实现（见 `crate::website`）。
#[tauri::command]
pub fn open_external_url(url: String) -> AppResult<()> {
    website::open_external_url(&url).map_err(AppError::Io)
}
