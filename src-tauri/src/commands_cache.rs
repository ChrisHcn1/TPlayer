use std::fs;
use std::path::{Component, Path, PathBuf};
use tauri::{AppHandle, Manager};

/// 保存文件到缓存目录
#[tauri::command]
pub async fn save_to_cache(
    app_handle: AppHandle,
    category: String,
    filename: String,
    data: Vec<u8>,
) -> Result<String, String> {
    let cache_dir = get_cache_dir_path(&app_handle, &category)?;
    let file_path = validate_cache_path(&cache_dir, &filename)?;

    fs::write(&file_path, &data)
        .map_err(|e| format!("写入缓存失败：{}", e))?;

    Ok(file_path.to_string_lossy().to_string())
}

/// 从缓存读取文件
#[tauri::command]
pub async fn get_cached_file(
    app_handle: AppHandle,
    category: String,
    filename: String,
) -> Result<Vec<u8>, String> {
    let cache_dir = get_cache_dir_path(&app_handle, &category)?;
    let file_path = validate_cache_path(&cache_dir, &filename)?;

    fs::read(&file_path)
        .map_err(|e| format!("读取缓存失败：{}", e))
}

/// 验证缓存路径安全：防止路径遍历攻击
///
/// 确保传入的 `filename` 无法逃逸出 `cache_dir`：
/// 1. 拒绝空文件名、绝对路径、以及包含 `..` 等可疑组件的路径
/// 2. 通过 `canonicalize()` 规范化缓存目录
/// 3. 校验拼接后的最终路径仍位于缓存目录之内
fn validate_cache_path(cache_dir: &PathBuf, filename: &str) -> Result<PathBuf, String> {
    if filename.is_empty() {
        return Err("文件名不能为空".to_string());
    }

    let path = Path::new(filename);

    // 1. 拒绝绝对路径（Unix 形式 `/...` 或 Windows 形式 `C:\...`）
    if path.is_absolute() {
        return Err("文件名不能为绝对路径".to_string());
    }

    // 2. 逐个检查路径组件，拒绝 `..`、根目录、Windows 前缀（如 `C:`）
    for component in path.components() {
        match component {
            Component::ParentDir => {
                return Err("文件名包含非法路径组件 '..'".to_string());
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("文件名不能为绝对路径".to_string());
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    // 3. 规范化缓存目录（目录已由 `get_cache_dir_path` 创建）
    let canonical_cache_dir = cache_dir
        .canonicalize()
        .map_err(|e| format!("无法规范化缓存目录：{}", e))?;

    // 4. 拼接得到最终路径
    let file_path = canonical_cache_dir.join(filename);

    // 5. 双重校验：最终路径必须仍位于缓存目录之内
    if !file_path.starts_with(&canonical_cache_dir) {
        return Err("文件路径越界，拒绝访问缓存目录之外的位置".to_string());
    }

    Ok(file_path)
}

/// 获取缓存目录路径
#[tauri::command]
pub async fn get_cache_dir(
    app_handle: AppHandle,
    category: String,
) -> Result<String, String> {
    let cache_dir = get_cache_dir_path(&app_handle, &category)?;
    Ok(cache_dir.to_string_lossy().to_string())
}

/// 清理过期缓存
#[tauri::command]
pub async fn clear_cache(
    app_handle: AppHandle,
    category: String,
    older_than_days: u64,
) -> Result<usize, String> {
    let cache_dir = get_cache_dir_path(&app_handle, &category)?;
    
    if !cache_dir.exists() {
        return Ok(0);
    }
    
    let mut cleaned = 0;
    let now = std::time::SystemTime::now();
    
    if let Ok(entries) = fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age.as_secs() > older_than_days * 86400 {
                            let _ = fs::remove_file(entry.path());
                            cleaned += 1;
                        }
                    }
                }
            }
        }
    }
    
    Ok(cleaned)
}

/// 辅助函数：获取缓存目录路径
fn get_cache_dir_path(app_handle: &AppHandle, category: &str) -> Result<PathBuf, String> {
    // 使用 Tauri 的 app_data_dir 方法
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法获取应用数据目录：{}", e))?;
    
    let base_dir = app_data_dir
        .join("cache")
        .join(category);
    
    fs::create_dir_all(&base_dir)
        .map_err(|e| format!("创建缓存目录失败：{}", e))?;
    
    Ok(base_dir)
}
