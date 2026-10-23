//! 曲库相关命令
//!
//! 安全约定：凡接收路径参数的命令，都必须经 `security::ensure_allowed` 校验
//! （白名单 = config.library_dirs）。library_add_dir 的路径来自 dialog 选择器，可信。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::cover;
use crate::error::{AppError, AppResult};
use crate::library::models::{Album, Artist, LibraryStats, Playlist, RecentEntry, Track};
use crate::library::scan::{collect_import_tracks, discover_cue_files, parse_track, scan_directory_fast};
use crate::library::sacd::SacdDisc;
use crate::library::cue::parse_cue;
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

    // 扫描前先校验每个根目录是否可达：网络盘未挂载 / 凭据缺失 / 路径非法（如
    // SMB 共享写成 172.18.0.21/songs 而非常规 \\\\host\\share，或相对路径）都会被
    // 静默吞掉导致「扫完 0 文件」。这里显式报错让用户看到真实原因。
    //
    // Linux 补充：NTFS 分区（ntfs-3g / ntfs3）按挂载选项决定归属，目录可 stat
    // 但当前用户无读权限时 read_dir 返回 EACCES——metadata 探测不到这一点，
    // 会在扫描阶段被逐目录跳过。Linux 追加一次 read_dir 预检，把「挂载点存在
    // 但不可读」与「挂载点不存在」区分开；错误文案按平台分支，Windows 保持
    // 原有 SMB 提示逐字不变。
    let mut inaccessible = Vec::new();
    for root in &roots {
        if std::fs::metadata(root).is_err() {
            inaccessible.push(root.to_string_lossy().to_string());
            continue;
        }
        #[cfg(target_os = "linux")]
        if let Err(e) = std::fs::read_dir(root) {
            inaccessible.push(format!(
                "{}：目录不可读（{}）。NTFS 分区请检查挂载选项：\
                 以 root 挂载并指定 uid/gid（当前用户），或为 ntfs-3g 启用 permissions 选项",
                root.display(),
                e
            ));
        }
    }
    if !inaccessible.is_empty() {
        #[cfg(target_os = "linux")]
        let message = format!(
            "以下曲库目录无法访问（未挂载 / 无读取权限 / 路径非法；\
             请确认分区已挂载且当前用户有读取权限）：{}",
            inaccessible.join("；")
        );
        #[cfg(not(target_os = "linux"))]
        let message = format!(
            "以下曲库目录无法访问（网络盘未挂载 / 凭据缺失 / 路径非法；Windows SMB 共享需以 \\\\host\\share 形式，如 \\\\172.18.0.21\\songs，或将共享映射为盘符如 Y:\\songs）：{}",
            inaccessible.join("；")
        );
        return Err(AppError::Library(message));
    }

    // 增量扫描基线（id/path/mtime）。v6 起使用两阶段懒加载快速扫描：
    // 扫描阶段只做「前期加载」——目录遍历 + 文件名/路径/大小轻量信息占位入库，
    // 不做逐文件元数据解析（lofty/ffprobe）、不读封面、不解析 CUE/SACD，
    // 因此网络共享大目录（如 \\172.18.0.21\sound，54GB/1864 文件）加载不再卡死 UI。
    state.scan_cancel.store(false, std::sync::atomic::Ordering::SeqCst);
    let snapshot = state.library.lock().unwrap().scan_snapshot();
    let result = scan_directory_fast(&roots, &snapshot, &app);
    let changed = result.changed_count;

    let mut library = state.library.lock().unwrap();
    library.apply_scan(&result.upserts, &result.removed_ids)?;

    // 本次扫描成功提交 → 记为「好状态」快照（崩溃回退的还原点，内部 5 分钟节流）
    library.commit_good_snapshot();

    // 刷新播放器曲目缓存（占位轨道同样进入缓存，列表立即可渲染）
    let tracks = library.tracks();
    drop(library);
    state.player.lock().unwrap().set_tracks(tracks);

    // 变更/删除曲目封面内存缓存失效（磁盘缓存保留，文件恢复后仍可复用）
    if changed > 0 || !result.removed_ids.is_empty() {
        cover_cache().lock().unwrap().clear();
    }

    // 后台补解析：仅对「本地磁盘 + 较小文件」先行解析元数据（不解析 CUE/SACD，
    // 网络目录/大文件不阻塞等待，留待播放时按需解析）。全程异步、不阻塞 UI。
    // 带整体时间预算（避免慢盘/海量小文件长期占用）与取消标志（前端 cancelScan）。
    spawn_lazy_backfill(
        &app,
        state.scan_cancel.clone(),
        std::time::Duration::from_secs(120),
    );

    Ok(())
}

/// 两阶段懒加载 - 阶段二「后台补解析」（异步线程，不阻塞 UI）。
///
/// 扫描完成后在后台只对「本地磁盘且体积较小」的占位曲目先行解析元数据：
///  - 网络共享路径（UNC `\\host\share`、映射的网络盘符，或 Linux 非本地挂载）
///    一律跳过，避免逐文件网络 IO 拖慢体验；
///  - 大文件（默认 > 32MB，典型 APE 整碟 / SACD ISO / 高码率 DSF）跳过，
///    其元数据解析留待播放该曲目时按需触发；
///  - 本函数**不解析** CUE 分轨与 SACD ISO（sacd_extract），这两类由
///    `library_resolve_track` 在播放时补做。
///
/// 时间预算 `budget`：整体耗时超限即停止，剩余占位留待播放时按需解析——
/// 慢盘 / 海量小文件不会让后台线程长期不退出。`cancel` 由 `library_cancel_scan`
/// 置位，用户主动取消时立即中止。
///
/// 已解析完成的曲目通过 `library://tracks-resolved` 事件推送前端增量更新。
fn spawn_lazy_backfill(app: &AppHandle, cancel: std::sync::Arc<std::sync::atomic::AtomicBool>, budget: std::time::Duration) {
    let handle = app.clone();
    std::thread::spawn(move || {
        let Some(app_state) = handle.try_state::<AppState>() else {
            return;
        };
        let deadline = std::time::Instant::now() + budget;
        let pending = {
            match app_state.library.lock() {
                Ok(g) => g.pending_tracks(),
                Err(_) => return,
            }
        };
        if pending.is_empty() {
            return;
        }
        const SIZE_THRESHOLD: u64 = 32 * 1024 * 1024; // 32MB
        let mut upserts: Vec<Track> = Vec::new();
        let mut resolved: Vec<Track> = Vec::new();
        for track in pending {
            // 取消 / 超时预算：立即中止，剩余占位留待播放时按需解析
            if cancel.load(std::sync::atomic::Ordering::SeqCst)
                || std::time::Instant::now() >= deadline
            {
                break;
            }
            // 只处理本地磁盘 + 较小文件；网络路径 / ISO 占位 / 大文件跳过
            if track.iso_placeholder || track.file_size > SIZE_THRESHOLD {
                continue;
            }
            if is_network_path(&track.file_path) {
                continue;
            }
            if let Some(mut full) = parse_track(std::path::Path::new(&track.file_path)) {
                full.id = track.id;
                // 完整元数据入库（metadata_pending 由 parse_track 置 false）
                upserts.push(full.clone());
                resolved.push(full);
            }
            // 每 64 条提交一批，避免长时间占用库锁
            if upserts.len() >= 64 {
                let batch: Vec<Track> = std::mem::take(&mut upserts);
                if let Ok(mut library) = app_state.library.lock() {
                    let _ = library.apply_scan(&batch, &[]);
                }
                let _ = handle.emit(
                    "library://tracks-resolved",
                    serde_json::json!({ "tracks": &resolved }),
                );
                resolved.clear();
            }
        }
        if !upserts.is_empty() {
            if let Ok(mut library) = app_state.library.lock() {
                let _ = library.apply_scan(&upserts, &[]);
            }
            let _ = handle.emit(
                "library://tracks-resolved",
                serde_json::json!({ "tracks": &resolved }),
            );
        }
    });
}

/// 判断路径是否为网络共享路径。
///
/// - UNC：`\\host\share` 或 `//host/share`；
/// - Windows 映射盘符（`Y:\music` 实际指向网络共享）——按 `GetDriveTypeW`
///   返回 `DRIVE_REMOTE` 判定，否则「把共享映射成盘符」的用户会被当成本地盘，
///   后台补解析会逐文件走网络 IO，重新引入卡顿；
/// - Linux：`/mnt/`、`/run/media/` 等非常驻挂载前缀。
fn is_network_path(path: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        path.starts_with("//") || path.starts_with("/mnt/") || path.starts_with("/run/media/")
    }
    #[cfg(not(target_os = "linux"))]
    {
        if path.starts_with("\\\\") || path.starts_with("//") {
            return true;
        }
        is_windows_remote_drive(path)
    }
}

/// Windows：路径带盘符且该盘是「网络驱动器」时为 true。
#[cfg(target_os = "windows")]
fn is_windows_remote_drive(path: &str) -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDriveTypeW;

    let bytes = path.as_bytes();
    if bytes.len() < 2 || bytes[1] != b':' || !bytes[0].is_ascii_alphabetic() {
        return false;
    }
    let root: Vec<u16> = format!("{}:\\", (bytes[0] as char).to_ascii_uppercase())
        .encode_utf16()
        .collect();
    // DRIVE_REMOTE = 3
    unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) == 3 }
}

/// 两阶段懒加载 - 阶段二「播放时按需解析」。
///
/// 播放某曲目（或显式点击 SACD/CUE 条目）前调用：把占位轨道解析为完整轨道
/// （含元数据 / 封面 / CUE 分轨 / SACD 分轨），解析结果写入曲库并通过
/// `library://tracks-resolved` 事件推送前端增量更新。
///
/// - 普通音频占位：parse_track 解析完整元数据；若同目录存在引用该文件的 CUE，
///   一并解析出 CUE 分轨并移除镜像占位；
/// - SACD ISO 占位（iso_placeholder）：SacdDisc::open 完整解析，虚拟分轨入库、
///   占位移除；解析失败保留占位（返回原占位，前端提示暂不可解析）。
///
/// 返回解析后的完整曲目列表（前端可据此渲染 / 播放）。
#[tauri::command]
pub async fn library_resolve_track(
    state: State<'_, AppState>,
    app: AppHandle,
    track_id: String,
) -> AppResult<Vec<Track>> {
    let initial = state
        .library
        .lock()
        .unwrap()
        .get_track(&track_id)
        .ok_or_else(|| AppError::Library(format!("曲目不存在: {}", track_id)))?;

    // 已完成解析：直接返回（含占位解析失败等场景的幂等路径）
    if !initial.metadata_pending {
        return Ok(vec![initial]);
    }

    // 解析可能耗时（网络文件 / SACD ISO / CUE probe），放到阻塞池异步执行。
    // 注意：State 是函数级借用，不能跨 spawn_blocking；闭包内从 owned AppHandle 取状态。
    let handle = app.clone();
    let path = initial.file_path.clone();
    let is_iso = initial.iso_placeholder;
    let id = initial.id.clone();

    // 解析可能耗时（网络文件 / SACD ISO / CUE probe），放到阻塞池异步执行
    let resolved: Vec<Track> = tauri::async_runtime::spawn_blocking(move || {
        let app_state = handle.state::<AppState>().inner();
        let path = std::path::Path::new(&path);
        if is_iso {
            resolve_sacd_iso(&app_state, &id, path)
        } else {
            resolve_audio(&app_state, &id, path)
        }
    })
    .await
    .unwrap_or_else(|_| vec![initial.clone()]);

    let _ = app.emit(
        "library://tracks-resolved",
        serde_json::json!({ "tracks": &resolved }),
    );
    Ok(resolved)
}

/// 解析普通音频占位：完整元数据 + 同目录 CUE 分轨（若引用该文件）
fn resolve_audio(app_state: &AppState, placeholder_id: &str, path: &std::path::Path) -> Vec<Track> {
    let Some(mut full) = parse_track(path) else {
        // 解析失败：保留占位，交由前端提示
        let placeholder = app_state
            .library
            .lock()
            .unwrap()
            .get_track(placeholder_id);
        return placeholder.into_iter().collect();
    };
    full.id = placeholder_id.to_string();

    // 尝试同目录 CUE 分轨解析（播放整碟镜像时把占位替换为分轨）
    let cue_tracks = if let Some(dir) = path.parent() {
        discover_cue_files(dir)
            .into_iter()
            .flat_map(|cue| parse_cue(&cue))
            .filter(|t| t.file_path.eq_ignore_ascii_case(&full.file_path))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    if cue_tracks.is_empty() {
        // 无 CUE 引用：单曲完整入库，替换占位
        let upserts = vec![full.clone()];
        let removed = vec![placeholder_id.to_string()];
        let mut library = app_state.library.lock().unwrap();
        let _ = library.apply_scan(&upserts, &removed);
        vec![full]
    } else {
        // 有 CUE 分轨：分轨入库，镜像占位移除（分轨 file_path 指向镜像，播放可 seek）
        let upserts = cue_tracks.clone();
        let removed = vec![placeholder_id.to_string()];
        let mut library = app_state.library.lock().unwrap();
        let _ = library.apply_scan(&upserts, &removed);
        cue_tracks
    }
}

/// 解析 SACD ISO 占位：完整 sacd_extract 解析，虚拟分轨入库、占位移除
fn resolve_sacd_iso(app_state: &AppState, placeholder_id: &str, path: &std::path::Path) -> Vec<Track> {
    let mut disc = match SacdDisc::open(path) {
        Some(d) => d,
        None => {
            let placeholder = app_state
                .library
                .lock()
                .unwrap()
                .get_track(placeholder_id);
            return placeholder.into_iter().collect();
        }
    };
    disc.probe_durations();
    let tracks = disc.to_tracks();
    if tracks.is_empty() {
        let placeholder = app_state
            .library
            .lock()
            .unwrap()
            .get_track(placeholder_id);
        return placeholder.into_iter().collect();
    }
    let mut library = app_state.library.lock().unwrap();
    let _ = library.apply_scan(&tracks, &[placeholder_id.to_string()]);
    tracks
}

/// 请求取消当前扫描：置位取消标志，快扫占位循环与后台补解析随即中止
/// （已写入的批次保留，剩余文件保持占位，下次扫描/播放时再处理）。
#[tauri::command]
pub fn library_cancel_scan(state: State<'_, AppState>) -> AppResult<()> {
    state
        .scan_cancel
        .store(true, std::sync::atomic::Ordering::SeqCst);
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
///
/// 缓存是纯优化：写失败（目录不可写 / 磁盘满）时静默返回，不影响本次取封面，
/// 也不向控制台刷诊断输出。
fn write_disk_cache(cache_dir: &std::path::Path, track_id: &str, mime: &str, data: &[u8]) {
    if std::fs::create_dir_all(cache_dir).is_err() {
        return;
    }
    let path = cache_dir.join(format!("{}.{}", cache_stem(track_id), cover::ext_for_mime(mime)));
    let _ = std::fs::write(&path, data);
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

    // 磁盘缓存目录：%APPDATA%/com.tplayer/cover-cache
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

/// 封面磁盘缓存目录：%APPDATA%/com.tplayer/cover-cache
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

/// SACD ISO 分轨专辑（整张 ISO 拆出的虚拟多轨专辑）
#[tauri::command]
pub fn library_sacd_albums(state: State<'_, AppState>) -> AppResult<Vec<Album>> {
    Ok(state.library.lock().unwrap().sacd_albums())
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
    {
        let mut library = state.library.lock().unwrap();
        library.remove_tracks_under_dir(&path)?;
    }
    {
        let mut config = state.config.lock().unwrap();
        config.library_dirs.retain(|p| p != &path);
        config.save(&state.config_path)?;
    }
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
    let playlist = library.create_playlist(name)?;
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

/// 拖拽导入：将给定路径（文件或目录）中的音频文件直接解析入库，
/// 绕过白名单目录扫描（等价于用户显式把文件"放入"曲库）。
///
/// 安全约定：路径来自 Tauri 拖放事件（用户主动拖入窗口），视为可信，
/// 与 library_add_dir（dialog 选择器）同源，不强制白名单校验。
/// 返回成功入库的曲目 ID 列表；无可识别音频时返回空列表。
#[tauri::command]
pub fn library_import_files(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> AppResult<Vec<String>> {
    let path_bufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let upserts = collect_import_tracks(&path_bufs);
    if upserts.is_empty() {
        return Ok(Vec::new());
    }

    // 写入曲库并刷新播放器曲目缓存（与 library_scan 同构）
    let mut library = state.library.lock().unwrap();
    library.apply_scan(&upserts, &[])?;
    let tracks = library.tracks();
    drop(library);
    state.player.lock().unwrap().set_tracks(tracks);

    // 新导入曲目可能带来新封面，失效内存封面缓存（磁盘缓存保留）
    cover_cache().lock().unwrap().clear();

    Ok(upserts.into_iter().map(|t| t.id).collect())
}

// ---------------- 曲目详情（track_info） ----------------

/// 曲目详情返回模型：面向前端 TrackInfoPanel 的全量元数据聚合。
/// 所有字段均 Option 化，lofty 读不到、文件缺失、解码失败一律降级为 None，
/// 严禁因单个字段缺失让整个接口报错导致前端崩溃。
#[derive(Debug, Clone, serde::Serialize)]
pub struct TrackInfo {
    // ---- 基础信息 ----
    pub title: String,
    #[serde(rename = "artists")]
    pub artists: Vec<String>,
    pub album: String,
    #[serde(rename = "trackNumber")]
    pub track_number: Option<u32>,
    pub year: Option<u32>,
    /// 时长（秒），时长探测失败时为 0
    pub duration: f64,

    // ---- 技术信息 ----
    /// 音频格式（小写扩展名）
    pub format: String,
    /// 比特率（bps）
    pub bitrate: Option<u32>,
    /// 采样率（Hz）
    #[serde(rename = "sampleRate")]
    pub sample_rate: Option<u32>,
    /// 位深（bit）
    #[serde(rename = "bitDepth")]
    pub bit_depth: Option<u8>,
    /// 声道数
    pub channels: Option<u8>,

    // ---- 存储信息 ----
    #[serde(rename = "filePath")]
    pub file_path: String,
    #[serde(rename = "fileSize")]
    pub file_size: u64,
    #[serde(rename = "modifiedAt")]
    pub modified_at: i64,
    #[serde(rename = "addedAt")]
    pub added_at: i64,

    // ---- 封面（data URL，与 library_cover 同链路；无封面为 None）----
    pub cover: Option<String>,

    // ---- 内嵌歌词（纯文本；无歌词为 None）----
    #[serde(rename = "embeddedLyrics")]
    pub embedded_lyrics: Option<String>,

    // ---- ReplayGain ----
    #[serde(rename = "replaygainDb")]
    pub replaygain_db: Option<f32>,
    #[serde(rename = "replaygainPeak")]
    pub replaygain_peak: Option<f32>,
}

/// 读音频内嵌歌词（lofty Lyrics 标签）；失败或空文本一律返回 None
fn read_embedded_lyrics(path: &std::path::Path) -> Option<String> {
    use lofty::file::TaggedFileExt;
    use lofty::tag::ItemKey;
    let tagged = lofty::read_from_path(path).ok()?;
    let mut tags: Vec<&lofty::tag::Tag> = Vec::new();
    if let Some(primary) = tagged.primary_tag() {
        tags.push(primary);
    }
    tags.extend(tagged.tags().iter());
    for tag in tags {
        if let Some(text) = tag.get_string(&ItemKey::Lyrics) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// 读取曲目元数据：先读曲库缓存（快），再用 lofty + ffprobe 补齐 bit_depth/channels 等技术字段
fn enrich_track(track: &Track) -> TrackInfo {
    use lofty::file::AudioFile;
    // Linux：还原非 UTF-8 文件名的容错编码路径（NTFS 场景；合法路径零成本直连）
    #[cfg(target_os = "linux")]
    let pb = crate::library::scan_linux::resolve_path_str(&track.file_path);
    #[cfg(target_os = "linux")]
    let path = pb.as_path();
    #[cfg(not(target_os = "linux"))]
    let path = std::path::Path::new(&track.file_path);
    let mut bit_depth = None;
    let mut channels = None;
    let mut duration = track.duration;

    // 尝试 lofty 解析完整属性（bit_depth/channels 在 Track 模型中未持久化，每次实时解析）
    if let Ok(file) = lofty::read_from_path(path) {
        let props = file.properties();
        bit_depth = props.bit_depth();
        channels = props.channels();
        // lofty 读取的 duration 更精确时以它为准
        let lofty_dur = props.duration().as_secs_f64();
        if lofty_dur > 0.0 && (duration <= 0.0 || (lofty_dur - duration).abs() > 0.5) {
            duration = lofty_dur;
        }
    }

    // 内嵌歌词
    let embedded_lyrics = read_embedded_lyrics(path);

    TrackInfo {
        title: track.title.clone(),
        artists: track.artists.clone(),
        album: track.album.clone(),
        track_number: track.track_number,
        year: track.year,
        duration,
        format: track.format.clone(),
        bitrate: track.bitrate,
        sample_rate: track.sample_rate,
        bit_depth,
        channels,
        file_path: track.file_path.clone(),
        file_size: track.file_size,
        modified_at: track.modified_at,
        added_at: track.added_at,
        cover: None, // 单独通过 library_cover 异步解析（含内存/磁盘两级缓存 + 在线回退）
        embedded_lyrics,
        replaygain_db: track.replaygain_db,
        replaygain_peak: track.replaygain_peak,
    }
}

/// 返回指定曲目的完整元数据（含技术参数、内嵌歌词、封面 data URL）。
/// 任何字段缺失或解析失败一律降级为 None / 默认值，不向上抛错。
#[tauri::command]
pub async fn library_track_info(
    state: State<'_, AppState>,
    track_id: String,
) -> AppResult<Option<TrackInfo>> {
    // 1. 查曲库缓存（路径 / 基础字段）
    let track = match state.library.lock().unwrap().get_track(&track_id) {
        Some(t) => t,
        None => return Ok(None),
    };

    // 2. 技术参数 + 内嵌歌词（同步解析，lofty 读取极快）
    let mut info = enrich_track(&track);

    // 3. 封面：复用 library_cover 的完整解析链（含缓存 + 在线匹配）
    if let Ok(Some(url)) = library_cover(state.clone(), track_id.clone()).await {
        info.cover = Some(url);
    }

    Ok(Some(info))
}
