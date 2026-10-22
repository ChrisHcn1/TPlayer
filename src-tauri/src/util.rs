//! 跨平台小工具：构造“不弹出控制台窗口”的子进程命令。
//!
//! Tauri 是 GUI 应用，在 Windows 上直接 spawn ffmpeg / ffprobe / sacd_extract 这类
//! 控制台 EXE 会闪出一个黑框（每首歌解码、探测、校验都会闪）。设置
//! `CREATE_NO_WINDOW (0x08000000)` 即可抑制；Linux / macOS 无此概念，原样返回。

use std::ffi::OsStr;
use std::process::Command;

/// 创建一个默认隐藏控制台窗口的 `Command`（仅 Windows 生效）。
pub fn hidden_command(program: impl AsRef<OsStr>) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new(program);
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        cmd
    }
    #[cfg(not(windows))]
    {
        Command::new(program)
    }
}
