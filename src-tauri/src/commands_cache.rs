use std::fs;
use std::path::PathBuf;
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
    let file_path = cache_dir.join(&filename);
    
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
    let file_path = get_cache_dir_path(&app_handle, &category)?
        .join(&filename);
    
    fs::read(&file_path)
        .map_err(|e| format!("读取缓存失败：{}", e))
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
