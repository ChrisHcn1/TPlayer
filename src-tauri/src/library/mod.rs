//! 曲库模块：模型、SQLite 存储、增量扫描

// CUE 分轨解析：仅开发版本编译。分轨功能尚未完成，Release 构建不包含其解析逻辑，
// 避免未实现功能随安装包上线；开发态保留以便继续开发（见 scan.rs 的 cfg 门控）。
#[cfg(debug_assertions)]
pub mod cue;
pub mod db;
pub mod models;
pub mod scan;
pub mod store;

pub use models::{Album, Artist, LibraryStats, Playlist, RecentEntry, ScanProgress, ScanStage, Track};
pub use scan::{scan_directory, ScanResult};
pub use store::LibraryStore;
#[cfg(debug_assertions)]
pub use cue::parse_cue;
