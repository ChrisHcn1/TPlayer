//! 曲库模块：模型、SQLite 存储、增量扫描

pub mod db;
pub mod models;
pub mod scan;
pub mod store;

pub use models::{Album, Artist, LibraryStats, Playlist, RecentEntry, ScanProgress, ScanStage, Track};
pub use scan::{scan_directory, ScanResult};
pub use store::LibraryStore;
