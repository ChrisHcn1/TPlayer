//! 曲库相关命令
//!
//! 安全约定：凡接收路径参数的命令，都必须经 `security::ensure_allowed` 校验
//! （白名单 = config.library_dirs）。library_add_dir 的路径来自 dialog 选择器，可信。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use sha2::{Digest, Sha256};
use tauri::{AppHandle, State};

use crate::cover;
use crate::error::{AppError, AppResult};
use crate::library::models::{Album, Artist, LibraryStats, Playlist, RecentEntry, Track};
use crate::library::scan::scan_directory;
use crate::AppState;

/// 启动一次曲库扫描（增量）
#[tauri::command]
pub fn library_scan(state: State<'_, AppState>, app: AppHandle) -> AppResult<()> {
    let roots: Vec<PathBuf> = {
        let config = state.config.lock().unwrap();
        config
            .library_dirs
            .iter()
            .map(|s| PathBuf::from(s))
            .collect()
    };

    if roots.is_empty() {
        return Err(AppError::Library("未配置曲库目录，请先在设置中添加".into()));
    }

    // 增量扫描基线（id/path/mtime），扫描为同步阻塞操作，在命令线程中执行
    let snapshot = state.library.lock().unwrap().scan_snapshot();
    let result = scan_directory(&roots, &snapshot, &app);
    let changed = result.changed_count;

    let mut library = state.library.lock().unwrap();
    library.apply_scan(&result.upserts, &result.removed_ids)?;

    // 刷新播放器曲目缓存
    let tracks = library.tracks();
    drop(library);
    state.player.lock().unwrap().set_tracks(tracks);

    // 变更/删除曲目封面内存缓存失效（磁盘缓存保留，文件恢复后仍可复用）
    if changed > 0 || !result.removed_ids.is_empty() {
        cover_cache().lock().unwrap().clear();
    }

    Ok(())
}

/// 请求取消当前扫描（M2 简化：扫描是同步的，此命令为占位兼容）
#[tauri::command]
pub fn library_cancel_scan(_state: State<'_, AppState>) -> AppResult<()> {
    Ok(())
}

#[tauri::command]
pub fn library_tracks(state: State<'_, AppState>) -> AppResult<Vec<Track>> {
    Ok(state.library.lock().unwrap().tracks())
}

// ---- 封面（内嵌 → 同名边车 → 在线 → 前端默认图） ------------------------

/// 内存封面缓存：track_id → data URL；值为 None 表示本会话已确认无封面（负缓存）
fn cover_cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 磁盘缓存可能出现的扩展名（与 cover::resolve_cover 嗅探结果对齐）
const CACHE_EXTS: &[&str] = &["jpg", "png", "webp", "gif", "bmp"];

/// track_id → 磁盘缓存文件名（sha256 十六进制，避免非法路径字符）
fn cache_stem(track_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(track_id.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 读磁盘缓存原始字节，命中返回 (扩展名, 字节)
fn read_disk_cache_bytes(
    cache_dir: &std::path::Path,
    cache_key: &str,
) -> Option<(String, Vec<u8>)> {
    let stem = cache_stem(cache_key);
    for ext in CACHE_EXTS {
        let path = cache_dir.join(format!("{}.{}", stem, ext));
        if let Ok(data) = std::fs::read(&path) {
            if !data.is_empty() {
                return Some((ext.to_string(), data));
            }
        }
    }
    None
}

/// 读磁盘缓存（应用数据目录 cover-cache/<sha>.<ext>），命中返回 data URL
fn read_disk_cache(cache_dir: &std::path::Path, cache_key: &str) -> Option<String> {
    let (_, data) = read_disk_cache_bytes(cache_dir, cache_key)?;
    let mime = cover::sniff_image_mime(&data);
    Some(cover::to_data_url(mime, &data))
}

/// 写磁盘缓存（内嵌/边车/在线任一来源成功后都落盘，下次启动免解析免联网）
fn write_disk_cache(cache_dir: &std::path::Path, track_id: &str, mime: &str, data: &[u8]) {
    if let Err(err) = std::fs::create_dir_all(cache_dir) {
        eprintln!("[cover] 创建缓存目录失败: {err}");
        return;
    }
    let path = cache_dir.join(format!("{}.{}", cache_stem(track_id), cover::ext_for_mime(mime)));
    if let Err(err) = std::fs::write(&path, data) {
        eprintln!("[cover] 写入缓存失败: {err}");
    }
}

///
/// 读取曲目封面，返回可直接用于 `<img src>` 的 data URL；整条链无封面返回 None
/// （由前端回落到内置默认封面）。
///
/// 来源顺序：内嵌标签 → 同名边车图片 → 在线匹配（online.enabled 时）。
/// 内存缓存（含负缓存）+ 应用数据目录磁盘缓存两级，同一曲目只解析/联网一次。
#[tauri::command]
pub async fn library_cover(
    state: State<'_, AppState>,
    track_id: String,
) -> AppResult<Option<String>> {
    {
        let cache = cover_cache().lock().unwrap();
        if let Some(hit) = cache.get(&track_id) {
            return Ok(hit.clone());
        }
    }

    // 磁盘缓存目录：%APPDATA%/TPlayer Next/cover-cache
    let cache_dir = cover_cache_dir(&state);

    if let Some(url) = read_disk_cache(&cache_dir, &track_id) {
        cover_cache()
            .lock()
            .unwrap()
            .insert(track_id, Some(url.clone()));
        return Ok(Some(url));
    }

    // 取曲目与在线开关后立即释放锁（不得跨 await 持锁）
    let (track, online_enabled) = {
        let library = state.library.lock().unwrap();
        let config = state.config.lock().unwrap();
        let enabled = config.online.enabled
            && !config.online.provider.is_empty()
            && config.online.provider != "none";
        (library.get_track(&track_id), enabled)
    };
    let Some(track) = track else {
        cover_cache()
            .lock()
            .unwrap()
            .insert(track_id, None);
        return Ok(None);
    };

    let result = cover::resolve_cover(&track, online_enabled).await;
    let url = match result {
        Some((mime, data)) => {
            write_disk_cache(&cache_dir, &track_id, mime, &data);
            Some(cover::to_data_url(mime, &data))
        }
        None => None,
    };

    cover_cache()
        .lock()
        .unwrap()
        .insert(track_id, url.clone());
    Ok(url)
}

/// 封面磁盘缓存目录：%APPDATA%/TPlayer Next/cover-cache
fn cover_cache_dir(state: &State<'_, AppState>) -> PathBuf {
    state
        .config_path
        .parent()
        .map(|dir| dir.join("cover-cache"))
        .unwrap_or_else(|| PathBuf::from("cover-cache"))
}

/// 分组封面路径内存缓存（含负缓存）；分组封面走 asset 文件 URL，不占 base64 内存
fn group_cover_cache() -> &'static Mutex<HashMap<String, Option<PathBuf>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<PathBuf>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 找到分组/单曲键对应的磁盘缓存文件路径（按扩展名探测）
fn find_disk_cache_path(cache_dir: &std::path::Path, cache_key: &str) -> Option<PathBuf> {
    let stem = cache_stem(cache_key);
    CACHE_EXTS
        .iter()
        .map(|ext| cache_dir.join(format!("{}.{}", stem, ext)))
        .find(|path| path.is_file())
}

///
/// 分组（专辑/艺术家）封面加载，返回**磁盘缓存文件绝对路径**（前端经 convertFileSrc
/// 以 asset 协议加载，避免 base64 data URL 长驻 JS 内存）：
///   内存缓存（含负缓存）→ 分组磁盘缓存 → 成员曲目磁盘缓存（复制一份到分组键）
///   → 重新解析（候选已由 SQL 按内嵌封面优先、音轨号升序取前 30 首；
///     至多一次在线匹配）→ 落盘。
///
/// 负缓存仅存内存：下次启动自动重试（用户可能补了 folder.jpg 或开启在线）。
async fn load_group_cover_path(
    state: &State<'_, AppState>,
    group_key: &str,
    tracks: Vec<Track>,
    online_enabled: bool,
) -> AppResult<Option<String>> {
    // 1. 内存缓存
    {
        let cache = group_cover_cache().lock().unwrap();
        if let Some(hit) = cache.get(group_key) {
            return Ok(hit.clone().map(|path| path.to_string_lossy().to_string()));
        }
    }

    let cache_dir = cover_cache_dir(state);

    // 2. 分组磁盘缓存（上一次启动已落盘）
    if let Some(path) = find_disk_cache_path(&cache_dir, group_key) {
        group_cover_cache()
            .lock()
            .unwrap()
            .insert(group_key.to_string(), Some(path.clone()));
        return Ok(Some(path.to_string_lossy().to_string()));
    }

    if tracks.is_empty() {
        group_cover_cache()
            .lock()
            .unwrap()
            .insert(group_key.to_string(), None);
        return Ok(None);
    }

    // 3. 成员曲目磁盘缓存：单曲播放/查看时已解析过（可能来自在线），复制镜像到分组键
    for track in tracks.iter().take(30) {
        if let Some(src) = find_disk_cache_path(&cache_dir, &track.id) {
            if let Some(ext) = src.extension().and_then(|ext| ext.to_str()) {
                let dest = cache_dir.join(format!("{}.{}", cache_stem(group_key), ext));
                if std::fs::copy(&src, &dest).is_ok() {
                    group_cover_cache()
                        .lock()
                        .unwrap()
                        .insert(group_key.to_string(), Some(dest.clone()));
                    return Ok(Some(dest.to_string_lossy().to_string()));
                }
            }
        }
    }

    // 4. 完整解析（逐曲目本地链 + 至多一次在线匹配）
    let resolved = cover::resolve_group_cover(&tracks, online_enabled).await;
    let path = match resolved {
        Some((mime, data)) => {
            write_disk_cache(&cache_dir, group_key, mime, &data);
            find_disk_cache_path(&cache_dir, group_key)
        }
        None => None,
    };

    group_cover_cache()
        .lock()
        .unwrap()
        .insert(group_key.to_string(), path.clone());
    Ok(path.map(|path| path.to_string_lossy().to_string()))
}

/// 专辑封面（含 CUE 分轨专辑）：组内任一曲目有封面即共用。
/// 返回缓存文件绝对路径（前端 convertFileSrc 为 asset URL）；无封面返回 null。
#[tauri::command]
pub async fn library_album_cover(
    state: State<'_, AppState>,
    album_id: String,
) -> AppResult<Option<String>> {
    let (tracks, online_enabled) = {
        let library = state.library.lock().unwrap();
        let config = state.config.lock().unwrap();
        let enabled = config.online.enabled
            && !config.online.provider.is_empty()
            && config.online.provider != "none";
        (library.cover_candidates_of_album(&album_id, 30), enabled)
    };
    load_group_cover_path(&state, &album_id, tracks, online_enabled).await
}

/// 艺术家封面：候选曲目（任一阵位艺术家匹配）的专辑封面为代表
#[tauri::command]
pub async fn library_artist_cover(
    state: State<'_, AppState>,
    artist: String,
) -> AppResult<Option<String>> {
    let (tracks, online_enabled) = {
        let library = state.library.lock().unwrap();
        let config = state.config.lock().unwrap();
        let enabled = config.online.enabled
            && !config.online.provider.is_empty()
            && config.online.provider != "none";
        (library.cover_candidates_of_artist(&artist, 30), enabled)
    };
    load_group_cover_path(&state, &format!("artist:{artist}"), tracks, online_enabled).await
}

#[tauri::command]
pub fn library_albums(state: State<'_, AppState>) -> AppResult<Vec<Album>> {
    Ok(state.library.lock().unwrap().albums())
}

#[tauri::command]
pub fn library_artists(state: State<'_, AppState>) -> AppResult<Vec<Artist>> {
    Ok(state.library.lock().unwrap().artists())
}

#[tauri::command]
pub fn library_stats(state: State<'_, AppState>) -> AppResult<LibraryStats> {
    Ok(state.library.lock().unwrap().stats())
}

/// CUE 分轨专辑（M2 返回空）
#[tauri::command]
pub fn library_cue_albums(state: State<'_, AppState>) -> AppResult<Vec<Album>> {
    Ok(state.library.lock().unwrap().cue_albums())
}

#[tauri::command]
pub fn library_favorite_ids(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    Ok(state.library.lock().unwrap().favorite_ids())
}

#[tauri::command]
pub fn library_set_favorite(
    state: State<'_, AppState>,
    track_id: String,
    favorite: bool,
) -> AppResult<()> {
    let mut library = state.library.lock().unwrap();
    library.set_favorite(&track_id, favorite)?;
    Ok(())
}

#[tauri::command]
pub fn library_recent(state: State<'_, AppState>) -> AppResult<Vec<RecentEntry>> {
    Ok(state.library.lock().unwrap().recent())
}

#[tauri::command]
pub fn library_record_played(state: State<'_, AppState>, track_id: String) -> AppResult<()> {
    let mut library = state.library.lock().unwrap();
    // 自动记录，失败（如曲目刚被删除导致外键不匹配）不应打断播放流程
    let _ = library.record_played(&track_id);
    Ok(())
}

#[tauri::command]
pub fn library_remove_tracks(state: State<'_, AppState>, ids: Vec<String>) -> AppResult<()> {
    let mut library = state.library.lock().unwrap();
    library.remove_tracks(&ids)?;
    Ok(())
}

/// 把目录加入曲库白名单
#[tauri::command]
pub fn library_add_dir(state: State<'_, AppState>, path: String) -> AppResult<()> {
    // 路径来自 dialog 选择器，可信；直接加入白名单
    let mut config = state.config.lock().unwrap();
    if !config.library_dirs.contains(&path) {
        config.library_dirs.push(path.clone());
        config.save(&state.config_path)?;
    }
    Ok(())
}

/// 从曲库白名单移除目录
#[tauri::command]
pub fn library_remove_dir(state: State<'_, AppState>, path: String) -> AppResult<()> {
    let mut config = state.config.lock().unwrap();
    config.library_dirs.retain(|p| p != &path);
    config.save(&state.config_path)?;
    Ok(())
}

#[tauri::command]
pub fn library_list_playlists(state: State<'_, AppState>) -> AppResult<Vec<Playlist>> {
    Ok(state.library.lock().unwrap().playlists())
}

#[tauri::command]
pub fn library_create_playlist(
    state: State<'_, AppState>,
    name: String,
) -> AppResult<Playlist> {
    let mut library = state.library.lock().unwrap();
    let playlist = library.create_playlist(name);
    Ok(playlist)
}

#[tauri::command]
pub fn library_rename_playlist(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> AppResult<()> {
    let mut library = state.library.lock().unwrap();
    library.rename_playlist(&id, name)?;
    Ok(())
}

/// 删除歌单
#[tauri::command]
pub fn library_delete_playlist(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let mut library = state.library.lock().unwrap();
    library.delete_playlist(&id)?;
    Ok(())
}

#[tauri::command]
pub fn library_set_playlist_tracks(
    state: State<'_, AppState>,
    id: String,
    track_ids: Vec<String>,
) -> AppResult<()> {
    let mut library = state.library.lock().unwrap();
    library.set_playlist_tracks(&id, track_ids)?;
    Ok(())
}

#[tauri::command]
pub fn library_add_tracks_to_playlist(
    state: State<'_, AppState>,
    id: String,
    track_ids: Vec<String>,
) -> AppResult<()> {
    let mut library = state.library.lock().unwrap();
    library.add_tracks_to_playlist(&id, track_ids)?;
    Ok(())
}

#[tauri::command]
pub fn library_remove_track_from_playlist(
    state: State<'_, AppState>,
    id: String,
    track_id: String,
) -> AppResult<()> {
    let mut library = state.library.lock().unwrap();
    library.remove_track_from_playlist(&id, &track_id)?;
    Ok(())
}
