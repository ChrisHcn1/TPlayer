//! 外部工具（FFmpeg / FFprobe / SACD Extract）的定位与调用辅助
//!
//! # 查找顺序（自高到低）
//!
//! 1. **公共依赖目录**（`%LOCALAPPDATA%\TPlayer\deps\packs\<版本>\` 等）
//!    —— 用户通过「设置 → 解码组件」安装的独立依赖包。
//!    该目录位于**安装目录之外**，主程序覆盖安装 / 卸载都不会动它，
//!    因此更新场景下检测到即可直接复用，无需重新下载。
//! 2. **主程序安装目录内**（`bin/`、`resources/bin/`）—— 旧版随包内置副本（兼容期）。
//! 3. **系统固定路径**（`/usr/lib/tplayer/bin` 等）。
//! 4. **系统 PATH**（Debian/Ubuntu 的 `ffmpeg` 包）。
//!
//! Linux 侧所有候选都经 ELF 魔数校验（见 `tool_runnable`），Windows 版 `.exe`
//! 不会被误用；Windows / macOS 侧只判存在性。

use std::path::{Path, PathBuf};

use crate::deps::{self, ToolSource};

/// 外部工具的可执行文件名候选：Windows 带 `.exe`，Linux/macOS 不带。
/// 两个候选都保留，便于同一 `bin/` 目录跨平台复用。
pub fn tool_names(base: &str) -> Vec<String> {
    if cfg!(windows) {
        vec![format!("{base}.exe"), base.to_string()]
    } else {
        vec![base.to_string(), format!("{base}.exe")]
    }
}

/// 候选外部工具是否可在本平台执行（平台隔离）。
///
/// Linux：必须是 ELF 可执行文件。仓库 `bin/ffmpeg.exe` / `bin/ffprobe.exe` 是
/// Windows 程序（随 Windows 安装包分发），而查找逻辑会沿可执行文件祖先目录
/// 查找 `bin/`——开发期（源码树内运行）与任何把 Windows 版工具放在查找路径内的
/// 布局都会命中它。直接 spawn 一个 PE 文件会得到 `ExecFormatError`（表现为
/// "无法启动 ffmpeg"而非"未找到 ffmpeg"），且可用性判定会误报 true。
/// 读取文件头魔数 `\x7fELF` 甄别（不依赖 file 命令）。
/// Windows / macOS：保持原语义（候选存在即可）。
#[cfg(target_os = "linux")]
pub fn tool_runnable(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 4];
    if file.read_exact(&mut magic).is_err() {
        return false;
    }
    magic == *b"\x7fELF"
}

#[cfg(not(target_os = "linux"))]
pub fn tool_runnable(_path: &Path) -> bool {
    true
}

/// Linux 追加的发行版分发布局：`bin/linux`（Linux 版 ffmpeg/ffprobe 的存放位；
/// Windows 侧不识别该子目录，行为不变）。
#[cfg(target_os = "linux")]
const EXTRA_TOOL_SUBDIRS: &[&str] = &["bin/linux"];

#[cfg(not(target_os = "linux"))]
const EXTRA_TOOL_SUBDIRS: &[&str] = &[];

/// 工具命中位置（含来源），供前端区分「独立依赖包 / 随包内置 / 系统安装」
#[derive(Debug, Clone)]
pub struct ToolLocation {
    /// 命中路径
    pub path: PathBuf,
    /// 来源
    pub source: ToolSource,
    /// 所属依赖包版本（仅 `ToolSource::Deps` 有值）
    pub pack_version: Option<String>,
}

/// 定位外部工具并给出命中来源
pub fn locate_tool(base: &str) -> Option<ToolLocation> {
    let names = tool_names(base);
    let found = |cand: &Path| (cand.is_file() && tool_runnable(cand)).then(|| cand.to_path_buf());

    // 1) 公共依赖目录（独立依赖包，跨安装复用）
    for version in deps::list_pack_versions() {
        let dir = deps::pack_dir(&version);
        for name in &names {
            if let Some(hit) = found(&dir.join(name)) {
                return Some(ToolLocation {
                    path: hit,
                    source: ToolSource::Deps,
                    pack_version: Some(version.clone()),
                });
            }
        }
    }

    // 2) 可执行文件祖先目录 / exe 同级（开发期、安装目录内的内置副本）
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors().skip(1) {
            for sub in ["bin", "lib/tplayer/bin", "libexec/tplayer"]
                .iter()
                .chain(EXTRA_TOOL_SUBDIRS)
            {
                for name in &names {
                    if let Some(hit) = found(&ancestor.join(sub).join(name)) {
                        return Some(ToolLocation {
                            path: hit,
                            source: ToolSource::ProgramDir,
                            pack_version: None,
                        });
                    }
                }
            }
        }
        if let Some(parent) = exe.parent() {
            for rel in ["", "bin", "resources/bin", "../lib/tplayer/bin", "../libexec/tplayer"]
                .iter()
                .chain(EXTRA_TOOL_SUBDIRS)
            {
                for name in &names {
                    if let Some(hit) = found(&parent.join(rel).join(name)) {
                        return Some(ToolLocation {
                            path: hit,
                            source: ToolSource::ProgramDir,
                            pack_version: None,
                        });
                    }
                }
            }
        }
    }

    // 3) 安装包固定位置（系统级安装的依赖）
    for root in [
        "/usr/lib/tplayer/bin",
        "/usr/local/lib/tplayer/bin",
        "/usr/libexec/tplayer",
    ] {
        for name in &names {
            if let Some(hit) = found(Path::new(root).join(name).as_path()) {
                return Some(ToolLocation {
                    path: hit,
                    source: ToolSource::System,
                    pack_version: None,
                });
            }
        }
    }

    // 4) 系统 PATH
    resolve_on_path(&names).map(|path| ToolLocation {
        path,
        source: ToolSource::System,
        pack_version: None,
    })
}

/// 定位外部工具（只关心路径时的便捷封装）
pub fn resolve_bin_tool(base: &str) -> Option<PathBuf> {
    locate_tool(base).map(|found| found.path)
}

/// 在 PATH 中按候选顺序查找可执行文件（跳过空目录项）
fn resolve_on_path(names: &[String]) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        for name in names {
            let cand = dir.join(name);
            // 与 locate_tool 同一判定：Linux 校验 ELF 魔数
            if cand.is_file() && tool_runnable(&cand) {
                return Some(cand.to_path_buf());
            }
        }
    }
    None
}

/// 定位 FFmpeg
pub fn resolve_ffmpeg() -> Option<PathBuf> {
    resolve_bin_tool("ffmpeg")
}

/// 定位 FFprobe
pub fn resolve_ffprobe() -> Option<PathBuf> {
    resolve_bin_tool("ffprobe")
}

/// FFmpeg 是否可用
pub fn ffmpeg_available() -> bool {
    resolve_ffmpeg().is_some()
}

/// 用系统文件管理器打开目录（供用户手动放置依赖文件）
pub fn open_directory(dir: &Path) -> crate::error::AppResult<()> {
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|e| {
            crate::error::AppError::Io(std::io::Error::other(format!("创建目录失败：{e}")))
        })?;
    }
    spawn_open_dir(dir).map_err(|e| {
        crate::error::AppError::Io(std::io::Error::other(format!("打开目录失败：{e}")))
    })
}

#[cfg(target_os = "windows")]
fn spawn_open_dir(dir: &Path) -> std::io::Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let path = dir.to_string_lossy().to_string();
    let wide = path.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let verb: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
    let instance = unsafe {
        ShellExecuteW(
            HWND(0),
            PCWSTR(verb.as_ptr()),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if instance.0 as isize <= 32 {
        return Err(std::io::Error::other(format!(
            "ShellExecuteW 打开目录失败（代码 {}）",
            instance.0
        )));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn spawn_open_dir(dir: &Path) -> std::io::Result<()> {
    std::process::Command::new("open").arg(dir).spawn().map(|_| ())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_open_dir(dir: &Path) -> std::io::Result<()> {
    std::process::Command::new("xdg-open")
        .arg(dir)
        .spawn()
        .map(|_| ())
}

#[cfg(not(any(target_os = "windows", target_family = "unix")))]
fn spawn_open_dir(_dir: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("当前平台暂不支持打开目录"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_names_prefer_platform_convention() {
        let names = tool_names("ffmpeg");
        assert_eq!(names.len(), 2);
        assert!(names.iter().any(|n| n == "ffmpeg"));
        assert!(names.iter().any(|n| n == "ffmpeg.exe"));
    }

    #[test]
    fn locating_unknown_tool_returns_none() {
        // 仓库里不存在该工具，任何平台都应返回 None（不 panic）
        assert!(locate_tool("tplayer-nonexistent-tool").is_none());
    }
}
