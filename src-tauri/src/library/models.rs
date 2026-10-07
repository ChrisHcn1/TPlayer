//! 曲库领域模型（与前端 src/types/track.ts 一一对应，camelCase 序列化）

use serde::{Deserialize, Serialize};

/// 单曲
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    #[serde(rename = "albumId")]
    pub album_id: Option<String>,
    /// 时长（秒）
    pub duration: f64,
    #[serde(rename = "trackNumber")]
    pub track_number: Option<u32>,
    pub year: Option<u32>,
    #[serde(rename = "filePath")]
    pub file_path: String,
    #[serde(rename = "fileSize")]
    pub file_size: u64,
    pub format: String,
    pub bitrate: Option<u32>,
    #[serde(rename = "sampleRate")]
    pub sample_rate: Option<u32>,
    #[serde(rename = "hasCover")]
    pub has_cover: bool,
    /// 入库时间（Unix 秒）
    #[serde(rename = "addedAt")]
    pub added_at: i64,
    /// 文件修改时间（Unix 秒），用于增量扫描
    #[serde(rename = "modifiedAt")]
    pub modified_at: i64,
}

///
/// 专辑稳定 ID：`album:<专辑名>:<主艺术家>`。
/// 扫描入库与专辑聚合必须共用此规则，保证 Track.album_id 与 Album.id 一致、
/// 前端按 albumId 过滤曲目与后端封面聚合都能命中。
pub fn album_id_of(album: &str, first_artist: &str) -> String {
    format!("album:{}:{}", album, first_artist)
}

/// 专辑（聚合视图）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub id: String,
    pub name: String,
    pub artists: Vec<String>,
    pub year: Option<u32>,
    #[serde(rename = "trackCount")]
    pub track_count: u32,
    #[serde(rename = "hasCover")]
    pub has_cover: bool,
    /// 是否为 CUE 分轨专辑
    #[serde(rename = "isCue")]
    pub is_cue: bool,
}

/// 歌手（聚合视图）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
    #[serde(rename = "albumCount")]
    pub album_count: u32,
    #[serde(rename = "trackCount")]
    pub track_count: u32,
}

/// 歌单
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "trackIds")]
    pub track_ids: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
    #[serde(rename = "isSystem")]
    pub is_system: bool,
}

/// 扫描进度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub stage: ScanStage,
    pub processed: u32,
    pub total: u32,
    #[serde(rename = "currentPath")]
    pub current_path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanStage {
    Idle,
    Discovering,
    Parsing,
    Writing,
    Done,
    Failed,
}

/// 曲库统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryStats {
    #[serde(rename = "trackCount")]
    pub track_count: u32,
    #[serde(rename = "albumCount")]
    pub album_count: u32,
    #[serde(rename = "artistCount")]
    pub artist_count: u32,
    /// 曲库总时长（秒）
    #[serde(rename = "totalDuration")]
    pub total_duration: f64,
    #[serde(rename = "lastScanAt")]
    pub last_scan_at: Option<i64>,
}

/// 最近播放条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentEntry {
    #[serde(rename = "trackId")]
    pub track_id: String,
    /// 最近一次播放时间（Unix 秒）
    #[serde(rename = "playedAt")]
    pub played_at: i64,
}
