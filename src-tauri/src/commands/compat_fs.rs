//! 兼容层（前端 CompatPlayer）文件读取命令
//!
//! 背景：前端读文件的两条通道（plugin-fs / assetProtocol）scope 静态限定在
//! capabilities 与 tauri.conf.json（$AUDIO、$HOME/Music 等），无法覆盖用户
//! 任意盘符的曲库目录；而曲库扫描、原生播放都在 Rust 侧读文件。本模块把
//! 「读库内文件字节」挪到 Rust 侧执行，并沿用同一安全边界：任何路径参数
//! 必须通过 `security::ensure_allowed`（白名单 = config.libraryDirs）。
//!
//! 大文件返回走 `tauri::ipc::Response`（二进制 IPC 通道），避免 `Vec<u8>`
//! JSON 序列化带来的性能与内存问题。

use std::io::Read;
use std::path::Path;

use tauri::ipc::Response;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::security;
use crate::AppState;

/// 校验路径位于曲库白名单内（与 capabilities / assetProtocol 语义一致的第三层防线）。
///
/// 白名单口径：① 路径已登记在曲库内（权威来源——config.library_dirs 可能被重置，
/// 而曲库数据持久化于 SQLite，扫描产物仍在）；② 退回 config.libraryDirs 前缀匹配
/// （大小写不敏感 + 分隔符边界）。基础规范化沿用 security::normalize（拒绝 .. 越权）。
fn ensure_library_path(state: &AppState, path: &str) -> AppResult<std::path::PathBuf> {
    let normalized = security::normalize(Path::new(path))?;
    let norm = normalize_path_str(path);

    if state.library.lock().unwrap().has_file_path(path) {
        return Ok(normalized);
    }

    let roots: Vec<String> = state.config.lock().unwrap().library_dirs.clone();
    let allowed = roots.iter().any(|root| {
        let r = normalize_path_str(root);
        !r.is_empty()
            && norm.starts_with(&r)
            && (norm.len() == r.len() || norm.as_bytes()[r.len()] == b'\\')
    });
    if allowed {
        return Ok(normalized);
    }

    Err(AppError::Library(format!(
        "compat 白名单拒绝: path={path} roots={roots:?} inLibrary=false"
    )))
}

/// Windows 路径归一化（小写 + 统一分隔符），仅用于白名单前缀比较
fn normalize_path_str(s: &str) -> String {
    s.replace('/', "\\").to_lowercase()
}

/// 读曲库内音频文件头（前 len 字节，1..=4096）——兼容层魔数预判用
#[tauri::command]
pub fn compat_read_head(state: State<'_, AppState>, path: String, len: u32) -> AppResult<Response> {
    let path = ensure_library_path(&state, &path)?;
    let take = len.clamp(1, 4096) as usize;
    let mut file = std::fs::File::open(&path)
        .map_err(|e| AppError::Library(format!("无法打开音频文件 {}: {}", path.display(), e)))?;
    let mut buf = vec![0u8; take];
    let mut total = 0usize;
    while total < take {
        match file.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(AppError::Library(format!("读取文件头失败: {}", e))),
        }
    }
    buf.truncate(total);
    Ok(Response::new(buf))
}

/// 全量读曲库内音频文件——兼容层 L1/L2 解码用
#[tauri::command]
pub fn compat_read_full(state: State<'_, AppState>, path: String) -> AppResult<Response> {
    let path = ensure_library_path(&state, &path)?;
    let bytes = std::fs::read(&path)
        .map_err(|e| AppError::Library(format!("无法读取音频文件 {}: {}", path.display(), e)))?;
    Ok(Response::new(bytes))
}
