//! 可执行入口（M0 骨架）
//!
//! 这里刻意只做一件事：调用 lib 中的 `run()`。
//! 业务装配（插件注册、命令注册、状态注入）全部在 src/lib.rs，便于后续接入测试与移动端目标。

// 阻止 Windows 上随 release 构建弹出额外的控制台窗口（debug 构建保留控制台便于查看日志）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
fn suppress_system_error_dialogs() {
    // SEM_FAILCRITICALERRORS (0x0001)：子进程缺少 DLL（如打包不全的 ffmpeg.exe）时，
    //   Windows 默认弹模态"系统错误"框并挂起进程；设置后直接以失败退出，由播放器自动跳过。
    // SEM_NOGPFAULTERRORBOX (0x0002)：崩溃时不弹 Windows 错误报告框。
    // 错误模式会被子进程继承，ffmpeg/ffprobe 均受保护。
    extern "system" {
        fn SetErrorMode(uMode: u32) -> u32;
    }
    const SEM_FAILCRITICALERRORS: u32 = 0x0001;
    const SEM_NOGPFAULTERRORBOX: u32 = 0x0002;
    unsafe {
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
    }
}

/// 单实例：用命名互斥体确保系统中仅有一个常驻进程。
///
/// 返回 true 表示已存在另一实例（调用方应聚焦旧窗口并退出新进程）；
/// 返回 false 表示本进程为首实例（已持有互斥体直至退出）。
/// 首实例持有的互斥体句柄（进程级保活，确保互斥体在进程存活期间不被释放）
#[cfg(windows)]
static SINGLE_INSTANCE: std::sync::OnceLock<windows::Win32::Foundation::HANDLE> =
    std::sync::OnceLock::new();

#[cfg(windows)]
fn ensure_single_instance() -> bool {
    use windows::Win32::Foundation::{BOOL, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND};
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };
    use windows::core::PCWSTR;

    // 跨进程唯一命名：首个实例创建并持有；后续实例打开同名得到 ERROR_ALREADY_EXISTS。
    let name: Vec<u16> = "TPlayer_SingleInstance"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let handle: HANDLE =
        unsafe { CreateMutexW(None, BOOL(0), PCWSTR(name.as_ptr())) }.expect("创建单实例互斥体失败");
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        // 已有实例：聚焦其主窗口（标题 "TPlayer"），最多等待约 2s 让其完成启动
        let title: Vec<u16> = "TPlayer"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        for _ in 0..40 {
            unsafe {
                let hwnd = FindWindowW(None, PCWSTR(title.as_ptr()));
                if hwnd != HWND::default() {
                    ShowWindow(hwnd, SW_RESTORE);
                    let _ = SetForegroundWindow(hwnd);
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        true
    } else {
        // 首实例：将句柄存入进程级 static 保活（HANDLE 为 Copy，std::mem::forget 无效；
        // 若不保活，作用域结束释放互斥体，单实例约束随即失效）
        let _ = SINGLE_INSTANCE.set(handle);
        false
    }
}

fn main() {
    #[cfg(windows)]
    suppress_system_error_dialogs();
    // 单实例：已有实例在运行时聚焦它并退出新进程，确保系统仅一个常驻进程
    #[cfg(windows)]
    if ensure_single_instance() {
        return;
    }
    tplayer_next_lib::run()
}
