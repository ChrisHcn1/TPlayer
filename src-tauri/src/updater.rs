use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use once_cell::sync::Lazy;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub release_notes: String,
    pub download_url: String,
    pub sha256_hash: String,
    pub size: u64,
    pub release_date: String,
}

#[derive(Debug, Deserialize)]
struct GitHubReleaseAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: String,
    published_at: String,
    assets: Vec<GitHubReleaseAsset>,
}

#[derive(Debug, Serialize)]
pub struct UpdateStatus {
    pub status: UpdateStatusType,
    pub progress: u64,
    pub total: u64,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub enum UpdateStatusType {
    Checking,
    Available,
    Downloading,
    Verifying,
    Installing,
    Completed,
    Error,
    NoUpdate,
}

#[derive(Debug, Serialize)]
pub struct CheckResult {
    pub has_update: bool,
    pub update_info: Option<UpdateInfo>,
    pub current_version: String,
}

static GITHUB_REPO_OWNER: Lazy<String> = Lazy::new(|| {
    std::env::var("GITHUB_REPO_OWNER")
        .unwrap_or_else(|_| "ChrisHcn1".to_string())
});

static GITHUB_REPO_NAME: Lazy<String> = Lazy::new(|| {
    std::env::var("GITHUB_REPO_NAME")
        .unwrap_or_else(|_| "TPlayer".to_string())
});

static UPDATE_CHECK_INTERVAL: Lazy<Duration> = Lazy::new(|| {
    let hours: u64 = std::env::var("UPDATE_CHECK_HOURS")
        .unwrap_or_else(|_| "24".to_string())
        .parse()
        .unwrap_or(24);
    Duration::from_secs(hours * 3600)
});

static LAST_CHECK_TIME: Lazy<Mutex<SystemTime>> = Lazy::new(|| Mutex::new(SystemTime::UNIX_EPOCH));
static DOWNLOAD_PROGRESS: Lazy<Mutex<HashMap<String, (u64, u64)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub async fn check_for_updates() -> Result<CheckResult, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let client = Client::new();

    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        *GITHUB_REPO_OWNER, *GITHUB_REPO_NAME
    );

    let response = client
        .get(&url)
        .header("User-Agent", format!("TPlayer/{}", current_version))
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {}", e))?;

    if response.status().is_success() {
        let release: GitHubRelease = response
            .json()
            .await
            .map_err(|e| format!("解析更新信息失败: {}", e))?;

        let tag_version = release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name);
        
        if compare_versions(tag_version, &current_version) > 0 {
            let asset = release
                .assets
                .into_iter()
                .find(|a| a.name.to_lowercase().ends_with(".msix"))
                .ok_or_else(|| "未找到MSIX安装包".to_string())?;

            let update_info = UpdateInfo {
                version: tag_version.to_string(),
                release_notes: release.body,
                download_url: asset.browser_download_url,
                sha256_hash: String::new(),
                size: asset.size,
                release_date: release.published_at,
            };

            Ok(CheckResult {
                has_update: true,
                update_info: Some(update_info),
                current_version,
            })
        } else {
            Ok(CheckResult {
                has_update: false,
                update_info: None,
                current_version,
            })
        }
    } else {
        Ok(CheckResult {
            has_update: false,
            update_info: None,
            current_version,
        })
    }
}

pub async fn download_update(
    update_info: &UpdateInfo,
    progress_callback: impl Fn(u64, u64) + Send + Sync + 'static,
) -> Result<PathBuf, String> {
    let app_dirs = tauri::api::path::app_data_dir(&Default::default())
        .ok_or_else(|| "无法获取应用数据目录".to_string())?;
    let download_dir = app_dirs.join("updates");
    fs::create_dir_all(&download_dir).map_err(|e| format!("创建下载目录失败: {}", e))?;

    let file_name = PathBuf::from(update_info.download_url.clone())
        .file_name()
        .ok_or_else(|| "无法解析下载文件名".to_string())?
        .to_string_lossy()
        .to_string();
    let file_path = download_dir.join(&file_name);

    let client = Client::new();
    let mut response: Response = client
        .get(&update_info.download_url)
        .send()
        .await
        .map_err(|e| format!("下载请求失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("下载失败，HTTP状态码: {}", response.status()));
    }

    let total_size = response
        .content_length()
        .unwrap_or(update_info.size);

    let mut file = if file_path.exists() {
        let existing_size = fs::metadata(&file_path)
            .map(|m| m.len())
            .unwrap_or(0);
        if existing_size > 0 && existing_size < total_size {
            let f = File::options()
                .write(true)
                .append(true)
                .open(&file_path)
                .map_err(|e| format!("打开已存在文件失败: {}", e))?;
            let _ = response
                .headers_mut()
                .insert("Range", format!("bytes={}-", existing_size).parse().unwrap());
            existing_size
        } else {
            fs::remove_file(&file_path).ok();
            File::create(&file_path).map_err(|e| format!("创建文件失败: {}", e))?;
            0
        }
    } else {
        File::create(&file_path).map_err(|e| format!("创建文件失败: {}", e))?;
    };

    let mut bytes_downloaded = file.seek(SeekFrom::End(0)).map_err(|e| format!("定位文件失败: {}", e))?;
    let mut buf_writer = BufWriter::new(&mut file);

    while let Ok(Some(chunk)) = response.chunk().await {
        bytes_downloaded += chunk.len() as u64;
        buf_writer
            .write_all(&chunk)
            .map_err(|e| format!("写入文件失败: {}", e))?;
        progress_callback(bytes_downloaded, total_size);
    }

    buf_writer.flush().map_err(|e| format!("刷新缓冲区失败: {}", e))?;

    Ok(file_path)
}

pub fn verify_update_package(file_path: &Path, expected_hash: &str) -> bool {
    if expected_hash.is_empty() {
        return true;
    }
    
    let file = match File::open(file_path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut hasher = sha2::Sha256::new();
    let mut reader = BufReader::new(file);
    let mut buffer = [0u8; 8192];

    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buffer[..n]),
            Err(_) => return false,
        }
    }

    let computed_hash = hex::encode(hasher.finalize());
    computed_hash.eq_ignore_ascii_case(expected_hash)
}

pub fn install_update(app_handle: &AppHandle, file_path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::process::Command;

        let exe_path = app_handle
            .path()
            .resolve_resource("bin\\updater.exe")
            .ok_or_else(|| "无法找到更新器程序".to_string())?;

        let command = Command::new(&exe_path)
            .arg("/install")
            .arg(file_path)
            .spawn()
            .map_err(|e| format!("启动更新器失败: {}", e))?;

        log::info!("更新器已启动，进程ID: {}", command.id());
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("更新功能仅支持Windows平台".to_string())
    }
}

pub fn compare_versions(v1: &str, v2: &str) -> i32 {
    let v1_parts: Vec<u32> = v1.split('.').filter_map(|s| s.parse().ok()).collect();
    let v2_parts: Vec<u32> = v2.split('.').filter_map(|s| s.parse().ok()).collect();

    for (a, b) in v1_parts.iter().zip(v2_parts.iter()) {
        if a > b {
            return 1;
        } else if a < b {
            return -1;
        }
    }

    v1_parts.len() as i32 - v2_parts.len() as i32
}

pub fn should_check_for_updates() -> bool {
    let last_check = *LAST_CHECK_TIME.lock().unwrap();
    let now = SystemTime::now();

    match now.duration_since(last_check) {
        Ok(duration) => duration >= *UPDATE_CHECK_INTERVAL,
        Err(_) => true,
    }
}

pub fn update_last_check_time() {
    *LAST_CHECK_TIME.lock().unwrap() = SystemTime::now();
}

#[tauri::command]
pub async fn check_update_manual() -> Result<CheckResult, String> {
    update_last_check_time();
    check_for_updates().await
}

#[tauri::command]
pub async fn check_update_auto() -> Result<CheckResult, String> {
    if !should_check_for_updates() {
        return Ok(CheckResult {
            has_update: false,
            update_info: None,
            current_version: env!("CARGO_PKG_VERSION").to_string(),
        });
    }
    update_last_check_time();
    check_for_updates().await
}

#[tauri::command]
pub async fn download_update_command(update_info: UpdateInfo) -> Result<PathBuf, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    let progress_tx = Arc::new(Mutex::new(tx));

    let handle = tokio::spawn(async move {
        let result = download_update(&update_info, move |downloaded, total| {
            if let Ok(mut tx) = progress_tx.lock().unwrap().try_send((downloaded, total)) {
                let _ = tx.send(());
            }
        }).await;

        result
    });

    tauri::async_runtime::spawn(async move {
        while let Ok((downloaded, total)) = rx.await {
            let _ = tauri::emit_all("update-progress", (downloaded, total));
        }
    });

    handle.await.unwrap()
}

#[tauri::command]
pub fn verify_update_command(file_path: String, expected_hash: String) -> Result<bool, String> {
    let path = PathBuf::from(file_path);
    Ok(verify_update_package(&path, &expected_hash))
}

#[tauri::command]
pub fn install_update_command(app_handle: AppHandle, file_path: String) -> Result<(), String> {
    let path = PathBuf::from(file_path);
    install_update(&app_handle, &path)
}

#[tauri::command]
pub fn get_current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}