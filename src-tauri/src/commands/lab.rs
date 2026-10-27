//! 实验室（Lab）功能开关命令。
//!
//! 与主设置**解耦**：只读写 config.json 的 `lab` 段，由前端「实验室」面板驱动。
//! 功能本体完全隔离在各自模块（见前端 `src/lab/features/*` 与后端按需接入），
//! 此处仅切换「启用开关」，绝不触碰既有功能代码路径。

use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::commands::library::library_scan;
use crate::config::LabConfig;
use crate::error::{AppError, AppResult};
use crate::AppState;

/// `WindowEffectsConfig` 未在 `tauri::window` 公开再导出，需经 `tauri::utils::config` 取用。
use tauri::utils::config::WindowEffectsConfig;

/// 读取当前实验室配置（各功能开关 + 生命周期状态）
#[tauri::command]
pub fn lab_get(state: State<'_, AppState>) -> AppResult<LabConfig> {
    Ok(state.config.lock().unwrap().lab.clone())
}

/// 设置某个实验功能的启用 / 关闭（默认关闭）
#[tauri::command]
pub fn lab_set_feature(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> AppResult<()> {
    let changed = {
        let config = state.config.lock().unwrap();
        config.lab.features.get(&id).copied() != Some(enabled)
    };
    let mut config = state.config.lock().unwrap();
    config.lab.features.insert(id.clone(), enabled);
    config.save(&state.config_path)?;
    // enhanced-metadata：扫描解析用的原子开关。切换后若已扫描曲目需要按新模式重新
    // 解析（mtime 跳过不会触发），故在「值发生变化」时强制全量重扫一次，否则开启后
    // 旧曲目解析结果不变、用户会误判功能失效。
    if id == "enhanced-metadata" {
        crate::library::scan::ENHANCED_METADATA.store(enabled, std::sync::atomic::Ordering::SeqCst);
        if changed {
            drop(config);
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = library_scan(app2, true).await;
            });
        }
    }
    Ok(())
}

/// 实验室·窗口底纹：设置主窗口的**原生材质**（运行时切换，无需重启）。
///
/// `mode` 取值：
/// - `native` → 应用 OS 原生质感：
///   - Windows：先试 Mica（Win11），`apply_mica` 在不支持的版本返回 Err → 退 Acrylic（Win10）；
///   - macOS：`HudWindow` + radius（需 `app.macOSPrivateApi`）；
///   - Linux：无原生特效，直接返回 false（由前端 CSS 磨砂层兜底）。
/// - 其它（`off` / `solid` / `wallpaper` / `gradient` / `texture`）→ **清除**原生特效，
///   改由前端 CSS 底纹层呈现。
///
/// 返回值：**原生特效是否真正生效**。前端据此决策——生效则用透明表层让 OS 材质透出；
/// 未生效（Win7 / Linux / 系统关闭透明效果 / DWM 拒绝）则降级到 CSS 纯色基底，
/// 任何情况下都不会出现全透明裸窗。
#[tauri::command]
pub fn lab_window_set_effect(app: AppHandle, mode: String) -> AppResult<bool> {
    // 注：`Manager::get_window` 在 Tauri 中受 `unstable` feature 门控（本项目未启用），
    // 故用未门控的 `get_webview_window`；`WebviewWindow` 同样提供 set_effects()。
    let Some(window) = app.get_webview_window("main") else {
        return Ok(false);
    };

    if mode != "native" {
        window
            .set_effects(None::<WindowEffectsConfig>)
            .map_err(|e| AppError::Config(format!("清除窗口特效失败: {e}")))?;
        return Ok(false);
    }

    #[cfg(target_os = "windows")]
    {
        // Win11 → Mica；Win10 / 老版本 apply_mica 返回 Err → 退 Acrylic
        let applied = if window_vibrancy::apply_mica(&window, None).is_ok() {
            true
        } else {
            window_vibrancy::apply_acrylic(&window, None).is_ok()
        };
        // apply_* 返回 Ok 也可能被 DWM 静默忽略（系统关闭透明效果 / 节能模式），
        // 回读 DWM 属性确认真实状态，未生效则让前端降级到 CSS 纯色基底。
        return Ok(applied && windows_native_effect_active(&window));
    }

    #[cfg(target_os = "macos")]
    {
        window
            .set_effects(
                tauri::window::EffectsBuilder::new()
                    .effect(tauri::window::Effect::HudWindow)
                    .radius(8.0)
                    .build(),
            )
            .map_err(|e| AppError::Config(format!("应用窗口特效失败: {e}")))?;
        // macOS 无简便的生效回读手段，按已生效处理（HudWindow 在 10.14+ 可用）。
        return Ok(true);
    }

    // Linux：无原生窗口特效，前端 CSS 磨砂层兜底（见 assets/styles/lab-backdrop.css）
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = window;
        Ok(false)
    }
}

/// Windows：回读 DWM 属性判断原生底纹是否真正生效。
///
/// `DWMWA_SYSTEMBACKDROP_TYPE`(38) 非 0 表示 DWM 正在渲染 Mica / Acrylic / Tabbed；
/// 另外兼容 Win11 早期未文档化的 `DWMWA_MICA_EFFECT`(1029) 路径。
/// 任一被置位即认为生效；读取失败按未生效处理（安全侧，前端会降级到纯色）。
#[cfg(target_os = "windows")]
fn windows_native_effect_active(window: &tauri::webview::WebviewWindow) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWINDOWATTRIBUTE};

    let Ok(foreign_hwnd) = window.hwnd() else {
        return false;
    };
    // tauri 内部依赖的 windows crate 版本与本 crate 直接依赖的 0.54 可能不同，
    // `window.hwnd()` 返回的是 tauri 那版的 HWND，无法直接传给本版的 DWM 绑定。
    // 两者均为 `#[repr(transparent)] struct HWND(pub isize)`，取原始句柄重建即可。
    let hwnd = HWND(foreign_hwnd.0 as isize);
    // DWMWINDOWATTRIBUTE 取值（未在 windows 0.54 绑定中导出，直接用文档值）
    const DWMWA_SYSTEMBACKDROP_TYPE: i32 = 38;
    const DWMWA_MICA_EFFECT: i32 = 1029;

    for attr in [DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_MICA_EFFECT] {
        let mut value: i32 = 0;
        let size = std::mem::size_of::<i32>() as u32;
        // SAFETY: value 为栈上 i32，size 与之匹配；hwnd 来自当前窗口
        let ret = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(attr),
                &mut value as *mut i32 as *mut std::ffi::c_void,
                size,
            )
        };
        if ret.is_ok() && value != 0 {
            return true;
        }
    }
    false
}

/// 壁纸目录名（位于 app 数据目录下）：`%APPDATA%\com.tplayer\lab-wallpaper\`
const WALLPAPER_DIR: &str = "lab-wallpaper";
/// 本地壁纸文件名前缀（后缀保留原图扩展名，如 wallpaper.jpg / wallpaper.png）
const WALLPAPER_PREFIX: &str = "wallpaper.";

/// 查找当前已导入的本地壁纸（wallpaper.<ext>），返回首个匹配的绝对路径。
fn find_wallpaper_file(dir: &PathBuf) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(rest) = name.strip_prefix(WALLPAPER_PREFIX) {
            if !rest.is_empty() {
                return Some(entry.path().to_string_lossy().to_string());
            }
        }
    }
    None
}

/// 删除目录下所有 wallpaper.* 本地壁纸副本（避免重复导入堆积）
fn remove_wallpaper_files(dir: &PathBuf) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(rest) = name.strip_prefix(WALLPAPER_PREFIX) {
                if !rest.is_empty() {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }
}

/// 实验室·窗口底纹：导入自定义壁纸。
///
/// 由对话框选中的本地图片路径复制到 app 数据目录（Rust 侧直接文件 IO，**不依赖**
/// fs 插件权限，也无需扩大 asset 协议 scope），返回可供前端加载的完整路径。
/// 前端通过 `convertFileSrc` 把它转成 `asset://`/`http://asset.localhost` URL 使用。
///
/// 复制失败（源不可读 / 目标目录不可写）返回错误，由前端提示并保持原状。
#[tauri::command]
pub fn lab_import_wallpaper(app: AppHandle, source_path: String) -> AppResult<String> {
    let src = PathBuf::from(&source_path);
    if !src.is_file() {
        return Err(AppError::Config(format!("壁纸文件不存在: {source_path}")));
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .filter(|e| !e.is_empty())
        .unwrap_or_else(|| "jpg".to_string());
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Config(format!("无法定位应用数据目录: {e}")))?
        .join(WALLPAPER_DIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::Config(format!("创建壁纸目录失败: {e}")))?;
    // 清掉旧的 wallpaper.* 副本，避免堆积
    remove_wallpaper_files(&dir);
    let dest = dir.join(format!("{WALLPAPER_PREFIX}{ext}"));
    std::fs::copy(&src, &dest)
        .map_err(|e| AppError::Config(format!("复制壁纸失败: {e}")))?;
    Ok(dest.to_string_lossy().to_string())
}

/// 实验室·窗口底纹：查询已导入的壁纸路径；未导入返回 `None`。
#[tauri::command]
pub fn lab_wallpaper_path(app: AppHandle) -> AppResult<Option<String>> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Config(format!("无法定位应用数据目录: {e}")))?
        .join(WALLPAPER_DIR);
    Ok(find_wallpaper_file(&dir))
}

/// 实验室·窗口底纹：移除已导入的壁纸（删除本地副本）。
#[tauri::command]
pub fn lab_remove_wallpaper(app: AppHandle) -> AppResult<()> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Config(format!("无法定位应用数据目录: {e}")))?
        .join(WALLPAPER_DIR);
    remove_wallpaper_files(&dir);
    Ok(())
}

/* ===================== 每日壁纸（Bing 每日图像，与 Edge 同源） ===================== */

/// 每日壁纸缓存图片文件名（固定，覆盖写入）
const DAILY_FILE: &str = "daily.jpg";
/// 每日壁纸元数据（日期 + 版权 + 标题）缓存文件名
const DAILY_META: &str = "daily.json";

/// 每日壁纸返回给前端的数据
#[derive(Serialize)]
pub struct DailyWallpaper {
    /// 本地缓存图片完整路径（前端经 convertFileSrc 加载）
    pub path: String,
    /// 本地日期 YYYYMMDD（用作「今日是否已缓存」的判据）
    pub date: String,
    /// 图片版权信息
    pub copyright: String,
    /// 图片标题
    pub title: String,
}

#[derive(Deserialize)]
struct BingResponse {
    images: Vec<BingImage>,
}

#[derive(Deserialize)]
struct BingImage {
    #[serde(default)]
    url: String,
    #[serde(default)]
    urlbase: String,
    #[serde(default)]
    copyright: String,
    #[serde(default)]
    title: String,
}

#[derive(Serialize, Deserialize)]
struct DailyMeta {
    date: String,
    copyright: String,
    title: String,
}

/// 实验室·窗口底纹：拉取「每日壁纸」（Bing 每日图像，与 Edge「每日图像」同源）。
///
/// `idx`：Bing 归档偏移（0 = 今日，1..7 = 往日）；`换一张` 用随机往日制造变化。
/// 本地按「当天」缓存一次：命中当日缓存且非往日请求直接返回，避免每日重复下载；
/// 断网 / 接口异常时回退到本地已有缓存（若有），保证可用性与零额外开销。
#[tauri::command]
pub async fn lab_fetch_daily_wallpaper(
    app: AppHandle,
    idx: Option<u32>,
) -> AppResult<Option<DailyWallpaper>> {
    let idx = idx.unwrap_or(0).min(7);
    let today = Local::now().format("%Y%m%d").to_string();

    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Config(format!("无法定位应用数据目录: {e}")))?
        .join(WALLPAPER_DIR);
    let img_path = dir.join(DAILY_FILE);
    let meta_path = dir.join(DAILY_META);

    // 今日缓存命中且不是「换一张」往日请求 → 直接返回，零网络开销
    if idx == 0 && img_path.is_file() {
        if let Ok(raw) = std::fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<DailyMeta>(&raw) {
                if meta.date == today {
                    return Ok(Some(DailyWallpaper {
                        path: img_path.to_string_lossy().to_string(),
                        date: meta.date,
                        copyright: meta.copyright,
                        title: meta.title,
                    }));
                }
            }
        }
    }

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) TPlayer/2.0")
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| AppError::Config(format!("创建 HTTP 客户端失败: {e}")))?;

    let idx_s = idx.to_string();
    let archive: BingResponse = client
        .get("https://www.bing.com/HPImageArchive.aspx")
        .query(&[("format", "js"), ("idx", idx_s.as_str()), ("n", "1"), ("mkt", "zh-CN")])
        .send()
        .await
        .map_err(|e| AppError::Config(format!("请求 Bing 接口失败: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Config(format!("解析 Bing 接口失败: {e}")))?;

    let Some(img) = archive.images.into_iter().next() else {
        // 接口无图：回退到本地已有缓存（若有），否则 None
        return Ok(local_daily_fallback(&img_path, &today));
    };

    // 候选直链：优先 4K（UHD）；构造失败则退回接口给的 1920x1080 直链
    let candidates: Vec<String> = if !img.urlbase.is_empty() {
        let uhd = format!("https://www.bing.com{}_UHD.jpg", img.urlbase);
        let full = if img.url.starts_with("http") {
            img.url.clone()
        } else {
            format!("https://www.bing.com{}", img.url)
        };
        vec![uhd, full]
    } else if img.url.starts_with("http") {
        vec![img.url.clone()]
    } else {
        vec![format!("https://www.bing.com{}", img.url)]
    };

    let mut bytes: Option<Vec<u8>> = None;
    for u in &candidates {
        if let Ok(resp) = client.get(u).send().await {
            if let Ok(b) = resp.bytes().await {
                if !b.is_empty() {
                    bytes = Some(b.to_vec());
                    break;
                }
            }
        }
    }
    let bytes = match bytes {
        Some(b) => b,
        None => return Ok(local_daily_fallback(&img_path, &today)),
    };

    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::Config(format!("创建壁纸目录失败: {e}")))?;
    std::fs::write(&img_path, &bytes)
        .map_err(|e| AppError::Config(format!("写入每日壁纸失败: {e}")))?;

    let meta = DailyMeta {
        date: today.clone(),
        copyright: img.copyright.clone(),
        title: img.title.clone(),
    };
    if let Ok(raw) = serde_json::to_string(&meta) {
        let _ = std::fs::write(&meta_path, raw);
    }

    Ok(Some(DailyWallpaper {
        path: img_path.to_string_lossy().to_string(),
        date: today,
        copyright: img.copyright,
        title: img.title,
    }))
}

/// 每日壁纸拉取失败时的回退：若本地已有缓存则用它（沿用上次图片），否则 None。
fn local_daily_fallback(img_path: &PathBuf, today: &str) -> Option<DailyWallpaper> {
    if img_path.is_file() {
        Some(DailyWallpaper {
            path: img_path.to_string_lossy().to_string(),
            date: today.to_string(),
            copyright: String::new(),
            title: String::new(),
        })
    } else {
        None
    }
}

/// 实验室·窗口底纹：清除每日壁纸缓存（daily.jpg + daily.json）。
///
/// 每日壁纸本身「按天覆盖、最多只存 1 张」（约 1–2MB，非 4K 时也仅数百 KB），
/// 不会长期堆积；本命令提供手动清除入口，清除后下次启动 / 点击「换一张」会重新下载。
#[tauri::command]
pub fn lab_clear_daily_wallpaper(app: AppHandle) -> AppResult<()> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Config(format!("无法定位应用数据目录: {e}")))?
        .join(WALLPAPER_DIR);
    for name in [DAILY_FILE, DAILY_META] {
        let p = dir.join(name);
        if p.is_file() {
            let _ = std::fs::remove_file(p);
        }
    }
    Ok(())
}
