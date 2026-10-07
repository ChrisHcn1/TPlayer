//! 播放状态持久化：断点续播 + 播放习惯（音量/静音/播放模式）记忆
//!
//! 数据落盘到 app_data_dir/playback_state.json：
//!   - 保存时机：播放/暂停/切歌/seek/音量与模式变更、进度 tick 节流（5s）、窗口关闭；
//!   - 恢复时机：player_init 刷新曲目缓存后，若曲目仍在曲库中，
//!     定位到中断位置并自动开始播放（播放列表为空则不恢复）。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::state::{PlayContext, PlayMode};

/// 持久化的播放状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedPlayback {
    /// 上次播放的曲目（None 表示退出时无播放内容）
    #[serde(rename = "trackId")]
    pub track_id: Option<String>,
    /// 中断位置（毫秒）
    #[serde(rename = "positionMs")]
    pub position_ms: u64,
    /// 播放上下文（恢复队列与"下一首"语义）
    pub context: Option<PlayContext>,
    /// 曲目在上下文中的下标
    pub index: usize,
    pub volume: f32,
    pub muted: bool,
    pub mode: PlayMode,
}

impl Default for PersistedPlayback {
    fn default() -> Self {
        Self {
            track_id: None,
            position_ms: 0,
            context: None,
            index: 0,
            volume: 1.0,
            muted: false,
            mode: PlayMode::Order,
        }
    }
}

/// 从磁盘加载；文件缺失或损坏时返回 None（按首次启动处理）
pub fn load(path: &Path) -> Option<PersistedPlayback> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// 写入磁盘（失败静默：持久化降级不影响播放主流程）
pub fn save(path: &PathBuf, state: &PersistedPlayback) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(path, json);
    }
}
