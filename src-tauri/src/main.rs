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

fn main() {
    #[cfg(windows)]
    suppress_system_error_dialogs();
    tplayer_next_lib::run()
}
