//! 实验室·悬浮歌词：独立窗口命令。
//!
//! 悬浮歌词窗口是一个**独立**的 Tauri WebView（`label = "floating-lyrics"`），与主窗口
//! 互不依附：它有自己的渲染上下文，通过 IPC 连到同一个后端（AppState 为唯一真相源），
//! 订阅 `playback://event` 实时同步播放状态与歌词进度。主窗口最小化 / 隐藏 / 关闭（托盘驻留）
//! 都不会让该窗口消失——只要应用进程存活，悬浮窗口始终独立存在于桌面之上。
//!
//! 窗口特性（满足「脱离主窗口」诉求）：
//!  - `transparent`: 无边框半透明（如需不透明由前端 CSS 控制）；
//!  - `decorations(false)`: 无原生标题栏 / 边框；
//!  - `always_on_top(true)`: 置顶显示，不被其它窗口遮挡；
//!  - 拖动由前端 `getCurrentWindow().startDragging()` 驱动。

use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

/// 悬浮歌词窗口标签（全局唯一）
pub const FLOATING_LYRICS_LABEL: &str = "floating-lyrics";

/// 打开悬浮歌词窗口（已存在则视为已打开，幂等）。
///
/// 仅在实验室·悬浮歌词功能开启时被前端调用。窗口加载 `floating-lyrics.html`
/// （dev 下映射到 devUrl，构建后映射到 frontendDist），挂载独立的精简 Vue 应用。
/// 注意：必须是 `async fn`。Tauri 2 的**同步命令跑在主线程（事件循环线程）**上，
/// 而 `WebviewWindowBuilder::build()` 需要把创建请求投递给事件循环并阻塞等待；
/// 若在主线程上同步调用，等待的正是自己 → **永久死锁**，会导致界面上所有 IPC
/// （曲库扫描、歌单刷新、退出请求）全部无响应。改为 async 后命令在异步线程池执行，
/// `build()` 可安全投递到主线程完成创建。
#[tauri::command]
pub async fn floating_lyrics_open(app: AppHandle) -> AppResult<()> {
    if app.get_webview_window(FLOATING_LYRICS_LABEL).is_some() {
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        &app,
        FLOATING_LYRICS_LABEL,
        tauri::WebviewUrl::App("floating-lyrics.html".into()),
    )
    .title("TPlayer 悬浮歌词")
    .inner_size(440.0, 120.0)
    .min_inner_size(260.0, 64.0)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .resizable(false)
    .shadow(false)
    .skip_taskbar(true)
    .center()
    .build()
    .map_err(|e| AppError::Config(format!("创建悬浮歌词窗口失败: {e}")))?;
    Ok(())
}

/// 关闭悬浮歌词窗口（不存在则幂等）。同样用 `async fn` 避免在主线程上做窗口操作。
#[tauri::command]
pub async fn floating_lyrics_close(app: AppHandle) -> AppResult<()> {
    if let Some(window) = app.get_webview_window(FLOATING_LYRICS_LABEL) {
        window
            .close()
            .map_err(|e| AppError::Config(format!("关闭悬浮歌词窗口失败: {e}")))?;
    }
    Ok(())
}

/// 查询悬浮歌词窗口当前是否处于打开状态（供前端回显开关）。
#[tauri::command]
pub fn floating_lyrics_is_open(app: AppHandle) -> bool {
    app.get_webview_window(FLOATING_LYRICS_LABEL).is_some()
}
