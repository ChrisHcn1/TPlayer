//! Linux 曲库扫描平台适配（仅 `target_os = "linux"` 编译，Windows 构建不包含本模块）
//!
//! 背景：大量用户的曲库源文件位于 Windows NTFS 分区（双系统共用盘 / 外置硬盘）。
//! 与 Windows 盘符路径相比，Linux 侧遍历这些分区有三类差异必须处理：
//!
//! 1. **路径**：NTFS 在 Linux 上以挂载点暴露（`/mnt/...`、`/media/<用户>/...`），
//!    路径含空格、中文、符号链接均属常态；符号链接成环还会让栈式遍历死循环。
//!
//! 2. **编码**：NTFS 文件名以 UTF-16 存储。挂载 locale 非 UTF-8（或文件名本身
//!    含非常规字符）时，`read_dir` 返回的 `OsString` 不是合法 UTF-8——
//!    旧逻辑用 `extension().and_then(|e| e.to_str())` 匹配扩展名，
//!    `to_str()` 返回 `None` 会导致这些文件**直接被漏掉、无法入库**。
//!
//! 3. **权限**：ntfs-3g / 内核 ntfs3 驱动按挂载选项决定文件归属，
//!    当前用户无读权限时 `read_dir` 返回 `EACCES`。旧逻辑对错误一律
//!    `Err(_) => continue` 静默跳过，表现为「扫完 0 文件 / 大量曲目缺失」
//!    且界面无任何提示，用户无法判断是分区问题还是程序问题。
//!
//! 本模块提供的能力：
//!  - [`discover_files_checked`]：递归发现 + 目录级错误收集（不静默吞错），
//!    扩展名按**原始字节**匹配（容忍非 UTF-8 文件名），并以 (dev, ino) 去重
//!    防御符号链接成环；
//!  - [`scan_error_summary`]：把目录错误聚合为面向用户的 `ScanProgress.error` 文案；
//!  - [`resolve_path_str`]：把入库时写入 DB 的容错路径字符串还原为可 I/O 的
//!    真实路径（非 UTF-8 文件名场景），供播放 / 转码 / 封面 / 歌词读取使用。
//!
//! 所有能力仅服务 Linux 目标；共享逻辑（Windows 路径语义）仍在
//! [`super::scan`] 中保持原样。

use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// 一次目录遍历的结果
pub struct Discovery {
    /// 发现的音频（或 CUE）文件，路径为文件系统真实路径
    pub files: Vec<PathBuf>,
    /// 无法读取的目录：(展示用路径, 原因说明)——需要向用户明示的问题
    pub errors: Vec<(String, String)>,
}

/// 扩展名匹配：对 `OsStr` 的原始字节做 ASCII 大小写无关比较。
///
/// 不经过 `to_str()`：NTFS 上的非常规文件名（非 UTF-8）同样能正确识别，
/// 保证「完整发现并入库」。
fn ext_matches(path: &Path, exts: &[&str]) -> bool {
    let Some(ext) = path.extension() else {
        return false;
    };
    let bytes = ext.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    exts.iter().any(|want| {
        bytes.len() == want.len()
            && bytes
                .iter()
                .zip(want.as_bytes())
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
    })
}

/// `fs::read_dir` 错误的用户可读描述（含 NTFS 挂载权限提示）
fn describe_read_dir_error(path: &Path, err: &std::io::Error) -> String {
    let shown = path.display().to_string();
    match err.kind() {
        // NTFS 分区最常见形态：ntfs-3g/ntfs3 以挂载选项决定归属，
        // 默认可能只有 root 可读；用户侧表现为「目录扫不进」。
        ErrorKind::PermissionDenied => format!(
            "{shown}：无读取权限（NTFS 分区常见原因：挂载时未把当前用户映射为属主。\
             可改用 root 挂载并指定 uid/gid，或为 ntfs-3g 启用 permissions 选项后 chmod）"
        ),
        ErrorKind::NotFound => format!("{shown}：目录不存在（可能尚未挂载或已被移除）"),
        _ => format!("{shown}：{err}"),
    }
}

/// 递归发现目录下指定扩展名的文件（Linux 适配版）。
///
/// 与共享版 `discover_audio_files` 的差异：
///  - 扩展名按字节匹配（容忍非 UTF-8 文件名）；
///  - 子目录不可读时**记录错误**而非静默跳过，其余分支继续完整遍历；
///  - 以 (dev, ino) 去重已访问目录，防御符号链接成环导致的无限递归。
pub fn discover_files_checked(root: &Path, exts: &[&str]) -> Discovery {
    let mut files = Vec::new();
    let mut errors = Vec::new();
    let mut visited: HashSet<(u64, u64)> = HashSet::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        // 成环防护：目录（含符号链接指向的目录）按 (dev, ino) 只访问一次
        if let Ok(meta) = fs::metadata(&dir) {
            use std::os::unix::fs::MetadataExt;
            let key = (meta.dev(), meta.ino());
            if !visited.insert(key) {
                continue;
            }
        }

        let rd = match fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) => {
                // 根目录不可读：必须上报，否则用户看到「0 曲目」且无从判断原因；
                // 子目录不可读（EACCES 等）：记录后继续遍历其它分支，
                // 保证其余目录的文件仍能完整入库。
                if e.kind() != ErrorKind::NotFound {
                    errors.push((dir.display().to_string(), describe_read_dir_error(&dir, &e)));
                }
                continue;
            }
        };

        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if ext_matches(&path, exts) {
                files.push(path);
            }
        }
    }

    Discovery { files, errors }
}

/// 把目录错误聚合为 `ScanProgress.error` 的用户可见文案；无错误返回 `None`。
///
/// 文案上限取前 3 条明细 + 汇总计数，避免超长目录树把事件撑爆。
pub fn scan_error_summary(errors: &[(String, String)]) -> Option<String> {
    if errors.is_empty() {
        return None;
    }
    let mut msg = format!(
        "{} 个目录无法读取（已跳过，其余目录正常入库）：",
        errors.len()
    );
    for (_path, why) in errors.iter().take(3) {
        msg.push_str(&format!(" [{why}]"));
    }
    if errors.len() > 3 {
        msg.push_str(&format!(" 等 {} 个目录（详见完整日志）", errors.len()));
    }
    Some(msg)
}

/// 把入库时写入 DB 的路径字符串还原为真实可 I/O 的路径。
///
/// 非 UTF-8 文件名在扫描入库时使用了容错编码（`to_string_lossy()`，
/// 非法字节替换为 U+FFFD），直接按该字符串打开会 ENOENT。此处：
///  - 路径本身可访问 → 原样返回（常见路径零成本，仅一次 `exists` 探测）；
///  - 直连失败 → 回到父目录做一次「按容错名反查真实条目」，用 `read_dir`
///    返回的原始字节路径（`OsString`）替换，使播放 / 转码 / 封面 / 歌词
///    都能打开这类文件；
///  - 反查也失败 → 原样返回，由调用方的打开错误链路给出明确提示。
pub fn resolve_path_str(stored: &str) -> PathBuf {
    let direct = PathBuf::from(stored);
    if direct.exists() {
        return direct;
    }
    let (Some(parent), Some(want)) = (direct.parent(), direct.file_name()) else {
        return direct;
    };
    let want_lossy = want.to_string_lossy();
    let Ok(rd) = fs::read_dir(parent) else {
        return direct;
    };
    for entry in rd.flatten() {
        if entry.file_name().to_string_lossy() == want_lossy {
            return entry.path();
        }
    }
    direct
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;

    fn temp_dir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "tplayer-scan-linux-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn ext_matches_bytes_not_str() {
        // 合法 UTF-8：常规匹配 + 大小写无关
        let p = Path::new("/mnt/music/周杰伦/晴天.FLAC");
        assert!(ext_matches(p, &["mp3", "flac", "wav"]));
        // 非 UTF-8 文件名：旧 to_str() 逻辑会漏掉，字节匹配必须命中
        let mut os = std::ffi::OsString::new();
        os.push("/mnt/music/");
        os.push(std::ffi::OsString::from_vec(vec![0x80, 0xFF, 0xFE]));
        os.push(".ape");
        let weird = PathBuf::from(os);
        assert!(ext_matches(&weird, &["ape", "wv"]));
        assert!(!ext_matches(&weird, &["mp3"]));
        // 无扩展名 / 非音频扩展名不命中
        assert!(!ext_matches(Path::new("/mnt/music/noext"), &["mp3"]));
        assert!(!ext_matches(Path::new("/mnt/music/a.txt"), &["mp3"]));
    }

    #[test]
    fn discover_collects_files_and_errors() {
        let root = temp_dir("discover");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("a.flac"), b"fLaC").unwrap();
        std::fs::write(root.join("sub/b.mp3"), b"ID3").unwrap();
        std::fs::write(root.join("c.txt"), b"nope").unwrap();
        // 不可读子目录：记录错误但其余文件仍完整发现
        let denied = root.join("denied");
        std::fs::create_dir_all(&denied).unwrap();
        std::fs::write(denied.join("hidden.wav"), b"RIFF").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o000)).unwrap();
        }

        let d = discover_files_checked(&root, &["mp3", "flac", "wav"]);
        let names: Vec<String> = d
            .files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"a.flac".to_string()));
        assert!(names.contains(&"b.mp3".to_string()));
        assert!(!names.contains(&"c.txt".to_string()));
        #[cfg(unix)]
        {
            // root 用户不受权限位约束（000 目录仍可读），此时跳过不可读断言
            let still_readable = std::fs::read_dir(&denied).is_ok();
            if !still_readable {
                assert_eq!(d.errors.len(), 1, "不可读目录应被记录：{:?}", d.errors);
                assert!(d.errors[0].1.contains("无读取权限"));
            }
            // 恢复权限以便清理
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn symlink_loop_does_not_hang() {
        let root = temp_dir("loop");
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::write(root.join("a/x.mp3"), b"ID3").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&root, root.join("a/loop")).unwrap();

        let d = discover_files_checked(&root, &["mp3"]);
        assert_eq!(d.files.len(), 1, "成环符号链接不应导致重复/死循环");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_path_str_roundtrip() {
        // 合法 UTF-8 路径：直连命中，原样返回
        let root = temp_dir("resolve");
        let real = root.join("正常名字.flac");
        std::fs::write(&real, b"fLaC").unwrap();
        assert_eq!(resolve_path_str(real.to_str().unwrap()), real);

        // 非 UTF-8 文件名：容错编码（U+FFFD）存库后反查真实路径
        let mut os = std::ffi::OsString::new();
        os.push(root.as_os_str());
        os.push("/");
        os.push(std::ffi::OsString::from_vec(vec![0x80, 0xFF, 0xFE]));
        os.push(".ape");
        let real_weird = PathBuf::from(os);
        std::fs::write(&real_weird, b"MAC ").unwrap();
        let stored = real_weird.to_string_lossy().to_string();
        assert!(!PathBuf::from(&stored).exists(), "容错串直连应失败");
        assert!(stored.contains('\u{FFFD}'), "容错串应含 U+FFFD");
        assert_eq!(resolve_path_str(&stored), real_weird);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn summary_none_when_no_errors() {
        assert!(scan_error_summary(&[]).is_none());
        let s = scan_error_summary(&[("/mnt/a".into(), "无读取权限".into())]).unwrap();
        assert!(s.contains("/mnt/a"));
        assert!(s.contains("正常入库"));
    }
}
