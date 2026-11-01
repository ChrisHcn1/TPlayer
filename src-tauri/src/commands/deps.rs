//! 解码组件（第三方依赖）管理命令
//!
//! 对应前端 `src/services/deps.ts`：
//!   - `deps_status`        → 检测本地依赖状态（工具、来源、版本）
//!   - `deps_install`       → 下载并安装依赖包（远端）
//!   - `deps_install_local` → 从本地 zip 文件安装依赖包（离线 / 手动下载场景）
//!   - `deps_open_folder`   → 打开公共依赖目录（用户手动放置文件）
//!   - `deps_remove`        → 卸载已安装的依赖包
//!
//! 设计约束：
//!   - 命令只做「参数校验 + 调用 `crate::deps` 领域层 + 结构转换」，不写业务实现；
//!   - 安装是长耗时操作（数百 MB 下载），必须走异步命令 + 进度事件，
//!     否则前端 IPC 超时（默认 10s）会先于安装结束；
//!   - 安装并发互斥：同一时刻只允许一个安装任务，重复调用直接返回错误而非排队。

use tauri::{AppHandle, Emitter, State};

use crate::deps::{self, DepsStatus, InstallProgress, InstallResult};
use crate::error::{AppError, AppResult};
use crate::AppState;

/// 安装进度事件名（前端 `listen('deps://progress')` 订阅）
const PROGRESS_EVENT: &str = "deps://progress";

/// 检测依赖状态
#[tauri::command]
pub fn deps_status() -> AppResult<DepsStatus> {
    Ok(deps::status())
}

/// 下载并安装依赖包（远端）
#[tauri::command]
pub async fn deps_install(app: AppHandle, state: State<'_, AppState>) -> AppResult<InstallResult> {
    let _guard = InstallGuard::acquire(&state)?;

    let app_for_progress = app.clone();
    let result = deps::install_from_remote(move |progress| {
        // 进度事件发送失败不阻断安装（前端可能已关闭窗口）
        let _ = app_for_progress.emit(PROGRESS_EVENT, &progress);
    })
    .await;

    match result {
        Ok(installed) => Ok(installed),
        Err(err) => {
            let _ = app.emit(
                PROGRESS_EVENT,
                &InstallProgress {
                    phase: "failed".into(),
                    downloaded: 0,
                    total: 0,
                    message: err.to_string(),
                },
            );
            Err(err)
        }
    }
}

/// 从本地 zip 文件安装依赖包
///
/// 供「离线安装」与「用户自行下载后手动选择」场景使用：依赖包内含
/// `deps.json` 清单，逐文件 SHA256 校验通过后才会被启用。
#[tauri::command]
pub async fn deps_install_local(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> AppResult<InstallResult> {
    let _guard = InstallGuard::acquire(&state)?;

    let bytes = std::fs::read(&path)
        .map_err(|e| AppError::Io(std::io::Error::other(format!("读取依赖包文件失败：{e}"))))?;

    let app_for_progress = app.clone();
    let result = deps::install_from_zip(&bytes, move |progress| {
        let _ = app_for_progress.emit(PROGRESS_EVENT, &progress);
    });

    match result {
        Ok(installed) => Ok(installed),
        Err(err) => {
            let _ = app.emit(
                PROGRESS_EVENT,
                &InstallProgress {
                    phase: "failed".into(),
                    downloaded: 0,
                    total: 0,
                    message: err.to_string(),
                },
            );
            Err(err)
        }
    }
}

/// 打开公共依赖目录（用户手动放置 ffmpeg / ffprobe / sacd_extract）
#[tauri::command]
pub fn deps_open_folder() -> AppResult<()> {
    deps::open_deps_dir()
}

/// 卸载已安装的依赖包（清空公共依赖目录下的 packs；主程序内置副本不受影响）
#[tauri::command]
pub fn deps_remove() -> AppResult<DepsStatus> {
    deps::remove_packs()?;
    Ok(deps::status())
}

/// 安装任务互斥守卫
///
/// 依赖：
///   - 并发安装会同时写同一个 `packs/` 目录，可能产生半截目录；
///   - 数百 MB 下载期间用户可能反复点「安装」。
/// 因此用原子标志位互斥：抢不到锁直接报错「已有安装任务在进行中」。
///
/// 持有 `Arc<AtomicBool>` 的克隆（而非借用 `State`）：异步命令的 future 必须
/// 自持有全部数据，不能借用命令参数里的引用。
struct InstallGuard {
    flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl InstallGuard {
    fn acquire(state: &State<'_, AppState>) -> AppResult<Self> {
        use std::sync::atomic::Ordering;
        let flag = state.deps_installing.clone();
        if flag
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(AppError::Config("已有安装任务在进行中，请稍候".into()));
        }
        Ok(InstallGuard { flag })
    }
}

impl Drop for InstallGuard {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering;
        self.flag.store(false, Ordering::Release);
    }
}
