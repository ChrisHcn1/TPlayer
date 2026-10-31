//! 官网入口与「首次安装完成后自动打开一次」
//!
//! 需求（三条）：
//!  1. 程序**首次安装完成后**，自动用系统默认浏览器打开官网首页；
//!  2. 之后每次启动都**不再**自动弹出（只此一次）；
//!  3. 程序内保留常驻入口（设置 → 更新 → 官方网站），用户随时可再打开。
//!
//! 「只弹一次」的实现：在应用数据目录（Windows 为 `%APPDATA%\com.tplayer`）
//! 放一个标记文件 `first_run_welcome.done`：
//!  - 首次启动时标记不存在 → 打开官网并**立即**写入标记；
//!  - 之后启动标记已存在 → 直接返回，不再打扰。
//! 覆盖安装 / 升级不会清空应用数据目录，故升级后也不会重复弹。
//! 标记写在打开动作**之前**：宁可「打开失败也不重复骚扰」，用户仍可从
//! 「设置 → 更新 → 官方网站」手动打开（打开失败会返回错误给前端提示）。
//!
//! 打开方式：**绕开 WebView**（Tauri v2 会拦截 webview 内的外部导航），
//! 直接把 URL 交给系统默认浏览器：
//!  - Windows：`ShellExecuteW`（shell32，等价于资源管理器「打开方式」）；
//!  - macOS：`open`；Linux：`xdg-open`。
//! 因此不需要引入 `tauri-plugin-opener` 依赖，也不必改 CSP。

use std::path::Path;
// Windows 分支走 ShellExecuteW，不需要外部命令
#[cfg(not(target_os = "windows"))]
use std::process::Command;

/// 官方网站（应用内入口与首次安装后自动打开的目标）
pub const WEBSITE_URL: &str = "http://tplayer.gleckes.cn";

/// 首次运行标记文件名（位于应用数据目录）
const FIRST_RUN_MARKER: &str = "first_run_welcome.done";

/// 用系统默认浏览器打开外部网址。
///
/// 安全约束：仅允许 `http` / `https`，并拒绝含引号、空白或控制字符的地址——
/// 命令行参数里的引号 / 空白可能被拆分或触发二次解析，不做校验等于把
/// 「拼命令」的能力交给调用方。
pub fn open_external_url(url: &str) -> std::io::Result<()> {
    let url = url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "仅允许打开 http / https 网址",
        ));
    }
    if url.contains('"') || url.contains('\'') || url.chars().any(char::is_whitespace) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "网址包含非法字符",
        ));
    }
    spawn_open(url)
}

/// 首次安装后自动打开官网一次；返回是否真的执行了打开动作。
///
/// 调用时机：应用启动后延迟一小段时间（让主窗口先呈现，避免浏览器窗口盖住应用）。
pub fn open_welcome_once(app_data_dir: &Path) -> bool {
    if !claim_welcome(app_data_dir) {
        return false;
    }
    open_external_url(WEBSITE_URL).is_ok()
}

/// 「本次启动是否该弹官网」的一次性认领。
///
/// 认领 = 检查标记文件 → 写入标记 → 返回是否放行。**先写标记再打开**：
/// 保证「至多一次」，不因打开失败而反复弹窗（用户仍可从程序内入口手动打开）。
/// 与打开动作分离，便于测试不真的拉起浏览器。
fn claim_welcome(app_data_dir: &Path) -> bool {
    let marker = welcome_marker_path(app_data_dir);
    if marker.exists() {
        return false;
    }
    if std::fs::create_dir_all(app_data_dir).is_err() {
        return false;
    }
    std::fs::write(&marker, b"seen").is_ok()
}

/// 标记文件是否已存在（供测试 / 诊断使用）
pub fn welcome_marker_path(app_data_dir: &Path) -> std::path::PathBuf {
    app_data_dir.join(FIRST_RUN_MARKER)
}

#[cfg(target_os = "windows")]
fn spawn_open(url: &str) -> std::io::Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    // ShellExecuteW 要求 UTF-16 宽字符串（以 NUL 结尾）
    let wide = url.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    // 返回值 > 32 表示成功（ShellExecute 的历史约定）
    let instance = unsafe {
        ShellExecuteW(
            HWND(0),
            PCWSTR::null(),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if instance.0 as isize <= 32 {
        return Err(std::io::Error::other(format!(
            "ShellExecuteW 打开网址失败（代码 {}）",
            instance.0
        )));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn spawn_open(url: &str) -> std::io::Result<()> {
    Command::new("open")
        .arg(url)
        .spawn()
        .map(|_| ())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_open(url: &str) -> std::io::Result<()> {
    Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_http_urls() {
        assert!(open_external_url("file:///C:/Windows/System32/cmd.exe").is_err());
        assert!(open_external_url("javascript:alert(1)").is_err());
        assert!(open_external_url("").is_err());
    }

    #[test]
    fn rejects_urls_with_shell_hazardous_chars() {
        assert!(open_external_url("http://a.com/\" & calc.exe").is_err());
        assert!(open_external_url("http://a.com/a b").is_err());
    }

    #[test]
    fn welcome_opens_only_once() {
        let dir = std::env::temp_dir().join(format!("tplayer-welcome-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // 首次：放行（认领）并写入标记
        assert!(claim_welcome(&dir), "首次应放行");
        assert!(
            welcome_marker_path(&dir).exists(),
            "首次调用后应写入标记文件"
        );
        // 第二次：因标记存在而跳过
        assert!(!claim_welcome(&dir), "标记存在时不应再次触发");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
