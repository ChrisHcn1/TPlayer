//! 第三方解码依赖（FFmpeg / FFprobe / SACD Extract）的独立化管理
//!
//! # 为什么要有这个模块
//!
//! 这三个可执行文件原先通过 `bundle.resources` 打进主安装包，导致：
//!   - NSIS 安装包约 470 MB，MSIX 约 196 MB，**每次主程序更新都要重下**；
//!   - 覆盖安装时 NSIS 的 `RMDir /r` 会连同旧版内置副本一起清掉，
//!     用户升级后必须重新下载数百 MB 才能继续播放 DSD / SACD 等格式。
//!
//! 本模块把三者改为**独立可选依赖**：
//!   - 存放位置：**公共依赖目录**（用户级，跨安装实例共享，位于安装目录之外），
//!     目录结构 `<deps>/packs/<版本>/`，按版本并存，便于独立升级与回滚；
//!   - 主程序**不内置**这些二进制（发行包瘦身），改为「检测 → 引导安装」；
//!   - 覆盖安装 / 主程序升级**不触碰**公共依赖目录 → 已装依赖直接复用。
//!
//! # 目录布局
//!
//! | 平台 | 公共依赖目录 |
//! |------|--------------|
//! | Windows | `%APPDATA%\com.tplayer\deps`（= `app_data_dir()`） |
//! | macOS | `~/Library/Application Support/com.tplayer/deps` |
//! | Linux | `$XDG_DATA_HOME/com.tplayer/deps`（缺省 `~/.local/share/com.tplayer/deps`） |
//!
//! **该目录必须位于安装目录之外**（安装目录为 `%LOCALAPPDATA%\TPlayer`，
//! 覆盖安装时会被 `RMDir /r` 清空）—— 详见 `deps_root()` 的注释。
//!
//! ```
//! <deps>/packs/1.0.0/ffmpeg.exe
//! <deps>/packs/1.0.0/ffprobe.exe
//! <deps>/packs/1.0.0/sacd_extract.exe
//! <deps>/packs/1.0.0/deps.json      # 逐文件 SHA256 清单（安装时强制校验）
//! ```
//!
//! # 安装流程（原子）
//!
//! 1. 下载依赖包 zip（远端 URL 来自内置清单，或用户本地选中的 zip 文件）；
//! 2. 解压到 `packs/.staging-<pid>`，**拒绝路径穿越**（Zip Slip）与未知文件；
//! 3. 按包内 `deps.json` 逐文件校验 SHA256，任一不符即失败并清理暂存目录；
//! 4. 校验通过后 `rename` 为 `packs/<版本>`（旧版本改名让位，随后清理）——
//!    任何时刻 `packs/<版本>` 要么不存在，要么是一份完整可用的依赖。
//!
//! # 安全
//!
//! - 只接受清单里列出的文件名，多余条目一律丢弃；
//! - 解压目标路径必须是文件名本身，不允许 `..` / 绝对路径 / 分隔符；
//! - 校验失败绝不落盘为生效版本。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

/// FFmpeg（主解码器）
pub const TOOL_FFMPEG: &str = "ffmpeg";
/// FFprobe（时长 / 采样率探测）
pub const TOOL_FFPROBE: &str = "ffprobe";
/// SACD Extract（SACD / DST 音轨导出）
pub const TOOL_SACD_EXTRACT: &str = "sacd_extract";

/// 受管工具清单（顺序即前端展示顺序）
pub const MANAGED_TOOLS: [&str; 3] = [TOOL_FFMPEG, TOOL_FFPROBE, TOOL_SACD_EXTRACT];

/// 包内清单文件名（逐文件 SHA256）
const PACK_MANIFEST_NAME: &str = "deps.json";

/// 内置依赖清单（编译期嵌入；URL 指向独立的依赖包 Release）
const MANIFEST_JSON: &str = include_str!("../deps-manifest.json");

// ---------------------------------------------------------------------------
// 清单
// ---------------------------------------------------------------------------

/// 内置依赖包清单
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepsManifest {
    /// 依赖包版本（与应用版本解耦，可单独升级）
    pub pack_version: String,
    /// 各平台的依赖包
    #[serde(default)]
    pub packs: Vec<PackEntry>,
}

/// 单个平台的依赖包描述
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackEntry {
    /// 平台键（如 `windows-x86_64`）
    pub platform: String,
    /// 下载地址（GitHub Release 资产）
    pub url: String,
    /// zip 整体 SHA256（可选；缺失时仅依赖包内逐文件校验）
    #[serde(default)]
    pub sha256: Option<String>,
    /// zip 字节数（可选，用于展示与进度）
    #[serde(default)]
    pub size: Option<u64>,
}

/// 解析内置清单
pub fn manifest() -> DepsManifest {
    serde_json::from_str(MANIFEST_JSON).unwrap_or(DepsManifest {
        pack_version: "0.0.0".to_string(),
        packs: Vec::new(),
    })
}

/// 当前平台的键（清单索引用）
pub fn platform_key() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "windows-x86_64"
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        "windows-aarch64"
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "macos-aarch64"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "macos-x86_64"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "linux-x86_64"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "linux-aarch64"
    }
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    )))]
    {
        "unsupported"
    }
}

// ---------------------------------------------------------------------------
// 目录布局
// ---------------------------------------------------------------------------

/// 公共依赖根目录（用户级；**必须在安装目录之外**）
///
/// # 为什么不能放在安装目录里
///
/// NSIS 安装目录为 `%LOCALAPPDATA%\TPlayer`，而覆盖安装时安装器会
/// `RMDir /r` 整个旧安装目录（见 `nsis/hooks.nsh` 的 `TPN_ForceCleanup`
/// 与 `TPN_RemoveLeftoverDir`）。若依赖放在其内部，**每次主程序更新都会
/// 把已装依赖一起删掉**，用户必须重新下载数百 MB —— 这正是本方案要消除的问题。
///
/// 因此放在应用数据目录（与 `config.json` / `library.db` 同源，即
/// `app_data_dir()` 对应的 `%APPDATA%\com.tplayer`）：
///   - 与安装目录分离，覆盖安装 / 卸载均不触碰 → 已装依赖直接复用；
///   - 与用户数据同源，卸载主程序时依赖仍在（用户可在设置页显式卸载）；
///   - 无需管理员权限即可读写。
///
/// 目录布局见模块文档。
pub fn deps_root() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        // 与 Tauri app_data_dir() 一致：%APPDATA%\<identifier>
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("AppData\\Roaming"))
            })
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("com.tplayer").join("deps")
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        home.join("Library")
            .join("Application Support")
            .join("com.tplayer")
            .join("deps")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
            })
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("tplayer").join("deps")
    }
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_family = "unix"
    )))]
    {
        PathBuf::from("deps")
    }
}

/// 依赖包存放目录（按版本并存）
pub fn packs_dir() -> PathBuf {
    deps_root().join("packs")
}

/// 某个版本的依赖包目录
pub fn pack_dir(version: &str) -> PathBuf {
    packs_dir().join(version)
}

/// 列出已安装的依赖包版本，按版本从新到旧排序
pub fn list_pack_versions() -> Vec<String> {
    let mut versions: Vec<String> = Vec::new();
    let Ok(entries) = std::fs::read_dir(packs_dir()) else {
        return versions;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        // 跳过暂存目录与临时让位目录
        if name.starts_with('.') || name.ends_with(".old") {
            continue;
        }
        if parse_version(&name).is_none() {
            continue;
        }
        versions.push(name);
    }
    versions.sort_by(|a, b| compare_versions(b, a));
    versions
}

/// 当前生效的依赖包版本（已安装的最高版本）
pub fn installed_pack_version() -> Option<String> {
    list_pack_versions().into_iter().next()
}

/// 解析版本号为数字段（`1.2.3` → `[1,2,3]`）
fn parse_version(version: &str) -> Option<Vec<u64>> {
    if version.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for seg in version.split('.') {
        let n: u64 = seg.parse().ok()?;
        parts.push(n);
    }
    Some(parts)
}

/// 比较两个版本号（`a` > `b` 时返回 Ordering::Greater）
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (pa, pb) = (parse_version(a), parse_version(b));
    match (pa, pb) {
        (Some(x), Some(y)) => x.cmp(&y),
        // 无法解析时按字符串比较，保证排序稳定（不 panic）
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => a.cmp(b),
    }
}

// ---------------------------------------------------------------------------
// 状态
// ---------------------------------------------------------------------------

/// 工具来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolSource {
    /// 公共依赖目录（独立依赖包）
    Deps,
    /// 主程序安装目录内（旧版随包内置，兼容期保留）
    ProgramDir,
    /// 系统 PATH / 固定安装路径
    System,
    /// 未找到
    Missing,
}

/// 单个工具的状态
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    /// 工具名（ffmpeg / ffprobe / sacd_extract）
    pub name: String,
    /// 是否为播放必需（ffmpeg 必需；ffprobe / sacd_extract 缺失仅部分功能降级）
    pub required: bool,
    /// 是否可用
    pub installed: bool,
    /// 命中路径
    pub path: Option<String>,
    /// 来源
    pub source: ToolSource,
    /// 所属依赖包版本（仅 Deps 来源有值）
    pub pack_version: Option<String>,
}

/// 依赖整体状态
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepsStatus {
    /// 公共依赖目录
    pub deps_dir: String,
    /// 当前平台键
    pub platform: String,
    /// 已安装的依赖包版本
    pub installed_pack: Option<String>,
    /// 应用期望的依赖包版本
    pub expected_pack: String,
    /// 已安装版本是否落后于期望版本
    pub outdated: bool,
    /// 各工具状态
    pub tools: Vec<ToolStatus>,
    /// 缺失的工具名
    pub missing: Vec<String>,
    /// 必需工具是否齐备（可正常播放受支持格式）
    pub ok: bool,
    /// 是否可从网络安装（清单里有当前平台的包）
    pub installable: bool,
}

/// 工具是否为播放必需
pub fn tool_required(name: &str) -> bool {
    name == TOOL_FFMPEG
}

/// 检测依赖状态
pub fn status() -> DepsStatus {
    let manifest = manifest();
    let installed_pack = installed_pack_version();
    let deps_dir = deps_root();
    let deps_dir_str = deps_dir.to_string_lossy().to_string();

    let mut tools = Vec::new();
    let mut missing = Vec::new();
    for name in MANAGED_TOOLS {
        let located = crate::tools::locate_tool(name);
        let (installed, path, source, pack_version) = match located {
            Some(found) => (true, Some(found.path.to_string_lossy().to_string()), found.source, found.pack_version),
            None => (false, None, ToolSource::Missing, None),
        };
        if !installed {
            missing.push(name.to_string());
        }
        tools.push(ToolStatus {
            name: name.to_string(),
            required: tool_required(name),
            installed,
            path,
            source,
            pack_version,
        });
    }

    let ok = tools.iter().all(|t| t.installed || !t.required);
    let outdated = match (&installed_pack, parse_version(&manifest.pack_version)) {
        (Some(installed), Some(expected)) => parse_version(installed)
            .map(|v| v < expected)
            .unwrap_or(false),
        _ => false,
    };
    let installable = manifest
        .packs
        .iter()
        .any(|p| p.platform == platform_key() && !p.url.is_empty());

    DepsStatus {
        deps_dir: deps_dir_str,
        platform: platform_key().to_string(),
        installed_pack,
        expected_pack: manifest.pack_version,
        outdated,
        tools,
        missing,
        ok,
        installable,
    }
}

// ---------------------------------------------------------------------------
// 安装
// ---------------------------------------------------------------------------

/// 安装阶段（用于向前端回报进度）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    /// 阶段：downloading / verifying / extracting / finalizing / done / failed
    pub phase: String,
    /// 已下载字节
    pub downloaded: u64,
    /// 总字节（0 = 未知）
    pub total: u64,
    /// 面向用户的中文提示
    pub message: String,
}

/// 安装结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    /// 安装后的版本
    pub version: String,
    /// 是否为新安装（false = 已是最新，未做改动）
    pub changed: bool,
    /// 安装后的完整状态
    pub status: DepsStatus,
}

/// 从远端下载并安装依赖包
///
/// `on_progress` 用于把下载 / 校验 / 解压进度回报给前端。
pub async fn install_from_remote<F>(on_progress: F) -> AppResult<InstallResult>
where
    F: Fn(InstallProgress),
{
    let manifest = manifest();
    let entry = manifest
        .packs
        .iter()
        .find(|p| p.platform == platform_key())
        .ok_or_else(|| {
            AppError::Config(format!(
                "当前平台（{}）暂未提供依赖包，无法自动安装",
                platform_key()
            ))
        })?;

    let total = entry.size.unwrap_or(0);
    on_progress(InstallProgress {
        phase: "downloading".into(),
        downloaded: 0,
        total,
        message: "正在下载解码组件…".into(),
    });

    let client = reqwest::Client::builder()
        .user_agent("TPlayerNext/deps-installer")
        .build()
        .map_err(|e| AppError::Config(format!("创建下载客户端失败：{e}")))?;

    let response = client
        .get(&entry.url)
        .send()
        .await
        .map_err(|e| AppError::Io(std::io::Error::other(format!("下载解码组件失败：{e}"))))?;

    if !response.status().is_success() {
        return Err(AppError::Config(format!(
            "下载解码组件失败：服务器返回 HTTP {}",
            response.status()
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::Io(std::io::Error::other(format!("读取解码组件数据失败：{e}"))))?;

    if let Some(expected) = entry.sha256.as_deref() {
        on_progress(InstallProgress {
            phase: "verifying".into(),
            downloaded: bytes.len() as u64,
            total: bytes.len() as u64,
            message: "正在校验下载完整性…".into(),
        });
        let actual = sha256_hex(&bytes);
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(AppError::Config(
                "解码组件校验失败：文件已损坏或被篡改，请重试".into(),
            ));
        }
    }

    install_from_zip(&bytes, on_progress)
}

/// 从本地 zip 文件安装依赖包（离线 / 用户手动下载场景）
pub fn install_from_zip<F>(bytes: &[u8], on_progress: F) -> AppResult<InstallResult>
where
    F: Fn(InstallProgress),
{
    on_progress(InstallProgress {
        phase: "verifying".into(),
        downloaded: bytes.len() as u64,
        total: bytes.len() as u64,
        message: "正在校验解码组件…".into(),
    });

    // 先在内存里解析包内清单，拿到版本号
    let version = read_pack_version(bytes)?;
    let target = pack_dir(&version);
    if target.exists() && !is_pack_complete(&target) {
        // 半截的旧目录（上次安装失败残留）先清掉，避免 rename 冲突
        let _ = std::fs::remove_dir_all(&target);
    }
    if target.exists() {
        // 已有完整版本 → 无需改动
        on_progress(InstallProgress {
            phase: "done".into(),
            downloaded: bytes.len() as u64,
            total: bytes.len() as u64,
            message: "解码组件已是最新版本".into(),
        });
        return Ok(InstallResult {
            version,
            changed: false,
            status: status(),
        });
    }

    on_progress(InstallProgress {
        phase: "extracting".into(),
        downloaded: bytes.len() as u64,
        total: bytes.len() as u64,
        message: "正在解压解码组件…".into(),
    });

    let staging = packs_dir().join(format!(".staging-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)
        .map_err(|e| AppError::Io(std::io::Error::other(format!("创建暂存目录失败：{e}"))))?;

    let extracted = extract_pack(bytes, &staging);
    if let Err(err) = extracted {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(err);
    }

    // 逐文件 SHA256 校验
    if let Err(err) = verify_pack_files(&staging) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(err);
    }

    on_progress(InstallProgress {
        phase: "finalizing".into(),
        downloaded: bytes.len() as u64,
        total: bytes.len() as u64,
        message: "正在启用解码组件…".into(),
    });

    // 原子启用：旧版本改名让位 → 新目录 rename 到位 → 清理旧版本
    let mut displaced: Option<PathBuf> = None;
    if target.exists() {
        let old = packs_dir().join(format!("{}.old", version));
        let _ = std::fs::remove_dir_all(&old);
        std::fs::rename(&target, &old).map_err(|e| {
            AppError::Io(std::io::Error::other(format!("替换旧版依赖包失败：{e}")))
        })?;
        displaced = Some(old);
    }
    if let Err(e) = std::fs::rename(&staging, &target) {
        // 回滚：把旧版本挪回来
        if let Some(old) = &displaced {
            let _ = std::fs::rename(old, &target);
        }
        let _ = std::fs::remove_dir_all(&staging);
        return Err(AppError::Io(std::io::Error::other(format!(
            "启用解码组件失败：{e}"
        ))));
    }
    if let Some(old) = displaced {
        let _ = std::fs::remove_dir_all(old);
    }

    on_progress(InstallProgress {
        phase: "done".into(),
        downloaded: bytes.len() as u64,
        total: bytes.len() as u64,
        message: "解码组件安装完成".into(),
    });

    Ok(InstallResult {
        version,
        changed: true,
        status: status(),
    })
}

/// 从包内 `deps.json` 读取版本号
fn read_pack_version(bytes: &[u8]) -> AppResult<String> {
    let manifest = read_pack_manifest(bytes)?;
    Ok(manifest.pack_version)
}

/// 包内清单结构
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackManifest {
    pack_version: String,
    #[serde(default)]
    files: std::collections::HashMap<String, PackFile>,
}

/// 包内单个文件条目
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackFile {
    sha256: String,
    #[serde(default)]
    size: u64,
}

/// 读取包内 `deps.json`
fn read_pack_manifest(bytes: &[u8]) -> AppResult<PackManifest> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| AppError::Config(format!("解码组件包已损坏（无法打开）：{e}")))?;
    let mut entry = archive
        .by_name(PACK_MANIFEST_NAME)
        .map_err(|_| AppError::Config("解码组件包缺少 deps.json 清单，已拒绝安装".into()))?;
    let mut text = String::new();
    use std::io::Read;
    entry
        .read_to_string(&mut text)
        .map_err(|e| AppError::Config(format!("读取依赖包清单失败：{e}")))?;
    serde_json::from_str(&text)
        .map_err(|e| AppError::Config(format!("依赖包清单格式错误：{e}")))
}

/// 允许写入的包内文件名（白名单，杜绝任意文件落盘）
fn allowed_entry_name(name: &str) -> Option<String> {
    if name.eq_ignore_ascii_case(PACK_MANIFEST_NAME) {
        return Some(PACK_MANIFEST_NAME.to_string());
    }
    for tool in MANAGED_TOOLS {
        for candidate in tool_file_names(tool) {
            if name.eq_ignore_ascii_case(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// 工具在各平台下的文件名候选
pub fn tool_file_names(tool: &str) -> Vec<String> {
    if cfg!(windows) {
        vec![format!("{tool}.exe"), tool.to_string()]
    } else {
        vec![tool.to_string(), format!("{tool}.exe")]
    }
}

/// 解压依赖包到暂存目录（拒绝路径穿越与未知文件）
fn extract_pack(bytes: &[u8], dest: &Path) -> AppResult<()> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| AppError::Config(format!("解码组件包已损坏（无法打开）：{e}")))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| AppError::Config(format!("读取依赖包条目失败：{e}")))?;
        let raw_name = entry.name().to_string();

        // 只取文件名部分：包内路径一律忽略（防止 ../ 穿越）
        let file_name = match raw_name.rsplit(['/', '\\']).next() {
            Some(n) if !n.is_empty() && n != "." && n != ".." => n.to_string(),
            _ => continue,
        };
        let Some(allowed) = allowed_entry_name(&file_name) else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }

        let out_path = dest.join(allowed);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::Io(std::io::Error::other(format!("创建目录失败：{e}"))))?;
        }
        let mut out = std::fs::File::create(&out_path)
            .map_err(|e| AppError::Io(std::io::Error::other(format!("写入依赖文件失败：{e}"))))?;
        std::io::copy(&mut entry, &mut out)
            .map_err(|e| AppError::Io(std::io::Error::other(format!("解压依赖文件失败：{e}"))))?;
    }
    Ok(())
}

/// 校验暂存目录内每个文件的 SHA256
fn verify_pack_files(dir: &Path) -> AppResult<()> {
    let manifest_path = dir.join(PACK_MANIFEST_NAME);
    let text = std::fs::read_to_string(&manifest_path).map_err(|e| {
        AppError::Config(format!("依赖包缺少 deps.json 清单，已拒绝安装：{e}"))
    })?;
    let manifest: PackManifest = serde_json::from_str(&text)
        .map_err(|e| AppError::Config(format!("依赖包清单格式错误：{e}")))?;

    if manifest.files.is_empty() {
        return Err(AppError::Config("依赖包清单为空，已拒绝安装".into()));
    }

    for (name, expect) in &manifest.files {
        let path = dir.join(name);
        let bytes = std::fs::read(&path).map_err(|_| {
            AppError::Config(format!("依赖包缺少文件 {name}，已拒绝安装"))
        })?;
        if expect.size > 0 && bytes.len() as u64 != expect.size {
            return Err(AppError::Config(format!(
                "依赖文件 {name} 大小校验失败，已拒绝安装"
            )));
        }
        let actual = sha256_hex(&bytes);
        if !actual.eq_ignore_ascii_case(&expect.sha256) {
            return Err(AppError::Config(format!(
                "依赖文件 {name} 校验失败（可能已损坏），已拒绝安装"
            )));
        }
    }
    Ok(())
}

/// 目录内是否是一份完整的依赖包（含清单且清单内文件齐备）
fn is_pack_complete(dir: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(dir.join(PACK_MANIFEST_NAME)) else {
        return false;
    };
    let Ok(manifest) = serde_json::from_str::<PackManifest>(&text) else {
        return false;
    };
    manifest
        .files
        .iter()
        .all(|(name, expect)| match std::fs::read(dir.join(name)) {
            Ok(bytes) => {
                expect.size == 0 || bytes.len() as u64 == expect.size
            }
            Err(_) => false,
        })
}

/// 计算字节的 SHA256（十六进制小写）
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

// ---------------------------------------------------------------------------
// 目录操作
// ---------------------------------------------------------------------------

/// 打开公共依赖目录（供用户手动放置文件）
pub fn open_deps_dir() -> AppResult<()> {
    let dir = deps_root();
    std::fs::create_dir_all(&dir).map_err(|e| {
        AppError::Io(std::io::Error::other(format!("创建依赖目录失败：{e}")))
    })?;
    crate::tools::open_directory(&dir)
}

/// 卸载已安装的依赖包（清空 packs 目录；主程序自带副本不受影响）
pub fn remove_packs() -> AppResult<()> {
    let dir = packs_dir();
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| AppError::Io(std::io::Error::other(format!("删除依赖包失败：{e}"))))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison_orders_newest_first() {
        let mut versions = vec!["1.0.0".to_string(), "1.2.0".to_string(), "1.10.0".to_string()];
        versions.sort_by(|a, b| compare_versions(b, a));
        assert_eq!(versions, vec!["1.10.0", "1.2.0", "1.0.0"]);
    }

    #[test]
    fn rejects_invalid_version_strings() {
        assert!(parse_version("").is_none());
        assert!(parse_version("abc").is_none());
        assert_eq!(parse_version("1.2.3"), Some(vec![1, 2, 3]));
    }

    #[test]
    fn manifest_parses_with_camel_case_keys() {
        let m = manifest();
        assert!(!m.pack_version.is_empty());
        // 清单至少应描述当前平台（构建期校验，避免发布出无法安装的清单）
        assert!(
            m.packs.iter().any(|p| p.platform == platform_key()),
            "内置清单缺少当前平台 {} 的依赖包",
            platform_key()
        );
    }

    #[test]
    fn entry_whitelist_rejects_unknown_and_traversal_names() {
        assert!(allowed_entry_name("ffmpeg.exe").is_some());
        assert!(allowed_entry_name("deps.json").is_some());
        assert!(allowed_entry_name("evil.dll").is_none());
        assert!(allowed_entry_name("../evil.dll").is_none());
    }

    #[test]
    fn sha256_matches_known_vector() {
        // 空输入的 SHA256 已知值
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn status_reports_all_managed_tools() {
        let s = status();
        assert_eq!(s.tools.len(), MANAGED_TOOLS.len());
        for tool in &s.tools {
            assert!(MANAGED_TOOLS.contains(&tool.name.as_str()));
        }
    }
}

#[cfg(test)]
mod install_tests {
    use super::*;
    use std::io::Write;

    /// 构造一个依赖包 zip：包含 ffmpeg.exe / ffprobe.exe / sacd_extract.exe + deps.json
    fn build_pack(version: &str, files: &[(&str, &[u8])], with_manifest: bool) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buf);
            let options: zip::write::FileOptions<'_, ()> =
                zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            for (name, data) in files {
                writer.start_file(*name, options).unwrap();
                writer.write_all(data).unwrap();
            }
            if with_manifest {
                // 清单里的 sha256 按实际内容计算
                let mut entries = serde_json::Map::new();
                for (name, data) in files {
                    entries.insert(
                        (*name).to_string(),
                        serde_json::json!({
                            "sha256": sha256_hex(data),
                            "size": data.len(),
                        }),
                    );
                }
                let manifest = serde_json::json!({
                    "packVersion": version,
                    "files": entries,
                });
                writer
                    .start_file(PACK_MANIFEST_NAME, options)
                    .unwrap();
                writer.write_all(manifest.to_string().as_bytes()).unwrap();
            }
            writer.finish().unwrap();
        }
        buf.into_inner()
    }


    #[test]
    fn pack_manifest_round_trip_reads_version() {
        let bytes = build_pack(
            "2.3.4",
            &[("ffmpeg.exe", b"fake-ffmpeg"), ("ffprobe.exe", b"fake-ffprobe")],
            true,
        );
        assert_eq!(read_pack_version(&bytes).unwrap(), "2.3.4");
    }

    #[test]
    fn pack_without_manifest_is_rejected() {
        let bytes = build_pack("1.0.0", &[("ffmpeg.exe", b"x")], false);
        let err = read_pack_version(&bytes).unwrap_err();
        assert!(
            err.to_string().contains("deps.json"),
            "应因缺少清单而拒绝，实际：{err}"
        );
    }

    #[test]
    fn zip_slip_entries_are_ignored() {
        // 条目名带 ../ 前缀：解包时只取文件名部分，穿越路径被丢弃
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buf);
            let options: zip::write::FileOptions<'_, ()> =
                zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            writer.start_file("../../evil.txt", options).unwrap();
            writer.write_all(b"pwned").unwrap();
            writer.finish().unwrap();
        }
        let bytes: Vec<u8> = buf.into_inner();
        let dest = std::env::temp_dir().join(format!("tplayer-zipslip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(&dest).unwrap();
        extract_pack(&bytes, &dest).unwrap();
        // 目标目录里不应出现 evil.txt，也不应写到父目录
        assert!(!dest.join("evil.txt").exists());
        assert!(!dest.parent().unwrap().join("evil.txt").exists());
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[test]
    fn unknown_files_are_not_extracted() {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buf);
            let options: zip::write::FileOptions<'_, ()> =
                zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            writer.start_file("evil.dll", options).unwrap();
            writer.write_all(b"pwned").unwrap();
            writer.start_file("ffmpeg.exe", options).unwrap();
            writer.write_all(b"ok").unwrap();
            writer.finish().unwrap();
        }
        let bytes: Vec<u8> = buf.into_inner();
        let dest = std::env::temp_dir().join(format!("tplayer-unknown-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(&dest).unwrap();
        extract_pack(&bytes, &dest).unwrap();
        assert!(dest.join("ffmpeg.exe").exists());
        assert!(!dest.join("evil.dll").exists());
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[test]
    fn corrupt_file_fails_verification() {
        // 清单里的 sha256 与实际内容不符 → 校验失败
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buf);
            let options: zip::write::FileOptions<'_, ()> =
                zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            writer.start_file("ffmpeg.exe", options).unwrap();
            writer.write_all(b"real-content").unwrap();
            writer.start_file(PACK_MANIFEST_NAME, options).unwrap();
            writer
                .write_all(
                    br#"{"packVersion":"1.0.0","files":{"ffmpeg.exe":{"sha256":"0000000000000000000000000000000000000000000000000000000000000000","size":12}}}"#,
                )
                .unwrap();
            writer.finish().unwrap();
        }
        let bytes: Vec<u8> = buf.into_inner();
        let dest = std::env::temp_dir().join(format!("tplayer-corrupt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(&dest).unwrap();
        extract_pack(&bytes, &dest).unwrap();
        let err = verify_pack_files(&dest).unwrap_err();
        assert!(
            err.to_string().contains("校验失败"),
            "应报校验失败，实际：{err}"
        );
        let _ = std::fs::remove_dir_all(&dest);
    }
    /// 端到端：真实依赖包 zip 能被安装逻辑接受（清单可解析、文件可解压、校验通过）
    ///
    /// 用最小的伪工具文件构造等价包（避免把 180MB 真实包塞进单测）：
    /// 结构与 build-deps-pack.ps1 产出一致 —— 三个工具 + deps.json 清单。
    #[test]
    fn install_flow_accepts_wellformed_pack() {
        let files = vec![
            ("ffmpeg.exe", b"fake-ffmpeg-binary".to_vec()),
            ("ffprobe.exe", b"fake-ffprobe-binary".to_vec()),
            ("sacd_extract.exe", b"fake-sacd-binary".to_vec()),
        ];
        let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, d)| (*n, d.as_slice())).collect();
        let bytes = build_pack("1.2.3", &refs, true);

        // 解压 + 校验（install_from_zip 的核心两步）
        let dest = std::env::temp_dir().join(format!("tplayer-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(&dest).unwrap();
        extract_pack(&bytes, &dest).unwrap();
        verify_pack_files(&dest).unwrap();

        assert!(dest.join("ffmpeg.exe").exists());
        assert!(dest.join("ffprobe.exe").exists());
        assert!(dest.join("sacd_extract.exe").exists());
        assert_eq!(read_pack_version(&bytes).unwrap(), "1.2.3");
        let _ = std::fs::remove_dir_all(&dest);
    }

    /// 清单缺失的文件 → 校验失败（防止装出「缺一个工具」的半残依赖）
    #[test]
    fn install_fails_when_a_listed_file_is_missing() {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buf);
            let options: zip::write::FileOptions<'_, ()> =
                zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            writer.start_file("ffmpeg.exe", options).unwrap();
            writer.write_all(b"only-ffmpeg").unwrap();
            writer.start_file(PACK_MANIFEST_NAME, options).unwrap();
            // ffmpeg.exe 在包内真实存在且数据自洽（size=11、sha256 匹配），
            // 仅 ffprobe.exe 被清单列出但包内缺失 —— 失败必须指向缺失的 ffprobe.exe。
            // 注意：manifest.files 是 HashMap，迭代顺序不确定；若存在的文件数据不自洽，
            // 可能先因「大小/校验失败」报错而漏掉对缺失文件的断言，故此处必须让存在项合法。
            writer
                .write_all(
                    br#"{"packVersion":"1.0.0","files":{"ffmpeg.exe":{"sha256":"3ed89210715bd80c05ce33f4a5aca3f39acf0611f756c7b1c14ee739cbe4d02c","size":11},"ffprobe.exe":{"sha256":"bb","size":11}}}"#,
                )
                .unwrap();
            writer.finish().unwrap();
        }
        let bytes: Vec<u8> = buf.into_inner();
        let dest = std::env::temp_dir().join(format!("tplayer-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(&dest).unwrap();
        extract_pack(&bytes, &dest).unwrap();
        let err = verify_pack_files(&dest).unwrap_err();
        assert!(
            err.to_string().contains("ffprobe.exe"),
            "应指出缺失的文件，实际：{err}"
        );
        let _ = std::fs::remove_dir_all(&dest);
    }
    /// 真实依赖包（scripts/build-deps-pack.ps1 产出）能被安装逻辑接受。
    ///
    /// 该测试仅在存在真实构建产物时运行（CI 不构建，故默认跳过）：
    ///   set TPLAYER_DEPS_PACK=<zip 路径>; cargo test --lib deps:: -- --ignored
    #[test]
    #[ignore = "需要真实依赖包产物"]
    fn install_real_pack_succeeds() {
        let Ok(zip_path) = std::env::var("TPLAYER_DEPS_PACK") else {
            panic!("请设置 TPLAYER_DEPS_PACK 指向真实依赖包 zip");
        };
        let bytes = std::fs::read(&zip_path).unwrap();
        let version = read_pack_version(&bytes).unwrap();
        let dest = std::env::temp_dir().join(format!("tplayer-real-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(&dest).unwrap();
        extract_pack(&bytes, &dest).unwrap();
        verify_pack_files(&dest).unwrap();
        assert!(dest.join("ffmpeg.exe").exists());
        assert!(dest.join("ffprobe.exe").exists());
        assert!(dest.join("sacd_extract.exe").exists());
        println!("真实依赖包 v{version} 校验通过");
        let _ = std::fs::remove_dir_all(&dest);
    }

    /// 真实依赖包 → 安装到真实公共目录（会写入 %APPDATA%\com.tplayer\deps）
    ///
    /// 仅手动运行（会改动本机依赖目录）：
    ///   set TPLAYER_DEPS_PACK=<zip>; cargo test --lib deps:: -- --ignored real
    #[test]
    #[ignore = "会写入真实公共依赖目录"]
    fn install_real_pack_into_public_dir() {
        let Ok(zip_path) = std::env::var("TPLAYER_DEPS_PACK") else {
            panic!("请设置 TPLAYER_DEPS_PACK 指向真实依赖包 zip");
        };
        let bytes = std::fs::read(&zip_path).unwrap();
        let result = install_from_zip(&bytes, |_| {}).unwrap();
        // 安装后状态应显示 ffmpeg 可用，且来源为独立依赖包
        let st = status();
        let ffmpeg = st.tools.iter().find(|t| t.name == TOOL_FFMPEG).unwrap();
        assert!(ffmpeg.installed, "安装后应检测到 ffmpeg");
        assert_eq!(ffmpeg.source, ToolSource::Deps);
        println!("已安装 v{}，依赖目录：{}", result.version, st.deps_dir);
        assert!(st.ok);
    }
}
