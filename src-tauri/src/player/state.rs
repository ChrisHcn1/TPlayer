//! 播放状态模型（与前端 src/types/player.ts 一一对应）

use serde::{Deserialize, Serialize};

/// 播放状态机状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackStatus {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopped,
    Error,
}

impl PlaybackStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Loading => "loading",
            Self::Playing => "playing",
            Self::Paused => "paused",
            Self::Stopped => "stopped",
            Self::Error => "error",
        }
    }
}

/// 播放模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlayMode {
    Order,
    RepeatAll,
    RepeatOne,
    Shuffle,
}

/// 播放上下文：决定"下一首"的语义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayContext {
    /// 上下文类型
    pub source: PlayContextSource,
    /// 上下文标识（专辑 ID / 歌单 ID 等，library 与 search 为 null）
    #[serde(rename = "sourceId")]
    pub source_id: Option<String>,
    /// 上下文曲目 ID 有序列表
    #[serde(rename = "trackIds")]
    pub track_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayContextSource {
    Library,
    Album,
    Artist,
    Playlist,
    Search,
}

/// 播放快照（后端为唯一真相源，前端只读）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackSnapshot {
    pub status: PlaybackStatus,
    /// 当前曲目 ID
    #[serde(rename = "trackId")]
    pub track_id: Option<String>,
    /// 当前播放位置（毫秒）
    #[serde(rename = "positionMs")]
    pub position_ms: u64,
    /// 总时长（毫秒，未知为 0）
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    /// 音量 0.0 ~ 1.0
    pub volume: f32,
    pub muted: bool,
    pub mode: PlayMode,
    /// 播放错误信息
    pub error: Option<String>,
}

impl Default for PlaybackSnapshot {
    fn default() -> Self {
        Self {
            status: PlaybackStatus::Idle,
            track_id: None,
            position_ms: 0,
            duration_ms: 0,
            volume: 1.0,
            muted: false,
            mode: PlayMode::Order,
            error: None,
        }
    }
}

/// 播放事件（后端 -> 前端推送）
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "lowercase")]
pub enum PlaybackEvent {
    Status {
        status: PlaybackStatus,
    },
    Track {
        #[serde(rename = "trackId")]
        track_id: String,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },
    Progress {
        #[serde(rename = "positionMs")]
        position_ms: u64,
    },
    Volume {
        volume: f32,
        muted: bool,
    },
    Mode {
        mode: PlayMode,
    },
    Ended {
        #[serde(rename = "trackId")]
        track_id: String,
    },
    Error {
        message: String,
        #[serde(rename = "trackId")]
        track_id: Option<String>,
    },
}
