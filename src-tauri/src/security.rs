//! 路径与权限边界（M0 骨架，安全关键模块）
//!
//! 背景：旧版 `fs` / `assetProtocol` 使用 `**/*` 全盘授权，前端任意代码可读全盘文件。
//! v2 采用三层独立防线，任一层失效都不会直接放开全盘：
//!   1) Tauri capabilities 只授予音乐库目录范围（capabilities/default.json）；
//!   2) 前端 assetProtocol scope 同样限定在音乐库目录（tauri.conf.json）；
//!   3) Rust 侧**任何**来自前端的路径参数都必须经过本模块的 `ensure_allowed`，
//!      白名单 = 用户在设置中登记的曲库目录（AppConfig::library_dirs）。
//!
//! 约定：跨目录删除/移动类操作在 M1 起还需额外确认流程，本模块只负责"能不能读"。

use std::path::{Component, Path, PathBuf};

use crate::error::{AppError, AppResult};

/// 规范化路径：拒绝含 `..` 的越权路径。
///
/// M0 不做 canonicalize（避免符号链接在音乐库内的正常使用被误伤）；
/// M1 再评估是否对白名单根目录做一次 canonicalize 后比较，以防软链接绕过。
pub fn normalize(input: &Path) -> AppResult<PathBuf> {
    if input.components().any(|part| matches!(part, Component::ParentDir)) {
        return Err(AppError::PathNotAllowed(input.display().to_string()));
    }
    Ok(input.to_path_buf())
}

/// 校验路径是否位于音乐库白名单内，并返回规范化后的路径。
///
/// 默认拒绝：白名单为空时不放行任何路径（未配置曲库 = 不可读任何文件）。
pub fn ensure_allowed(input: &Path, library_dirs: &[PathBuf]) -> AppResult<PathBuf> {
    let path = normalize(input)?;

    if library_dirs.is_empty() {
        return Err(AppError::PathNotAllowed(path.display().to_string()));
    }

    let allowed = library_dirs
        .iter()
        .any(|root| !root.as_os_str().is_empty() && path.starts_with(root));

    if allowed {
        Ok(path)
    } else {
        Err(AppError::PathNotAllowed(path.display().to_string()))
    }
}

/// 批量校验（如歌单导入、多选播放）；任一路径越权则整体拒绝，避免部分放行。
pub fn ensure_all_allowed(
    inputs: &[PathBuf],
    library_dirs: &[PathBuf],
) -> AppResult<Vec<PathBuf>> {
    inputs
        .iter()
        .map(|path| ensure_allowed(path, library_dirs))
        .collect()
}
