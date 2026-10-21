//! 曲库模块：模型、SQLite 存储、增量扫描

// CUE 分轨解析：Release 与开发构建均编译。分轨功能已可用，
// 扫描 / 拖拽 / 播放全链路支持（与 SACD ISO 分轨一致）。
pub mod cue;
pub mod db;
pub mod models;
pub mod scan;
// Linux 扫描平台适配（NTFS 分区路径/编码/权限）：仅 Linux 目标编译，
// Windows 构建完全不包含本模块及其依赖。
#[cfg(target_os = "linux")]
pub mod scan_linux;
pub mod store;

pub mod sacd;

pub use models::{Album, Artist, LibraryStats, Playlist, RecentEntry, ScanProgress, ScanStage, Track};
pub use scan::{scan_directory, ScanResult};
pub use store::LibraryStore;
pub use cue::parse_cue;
