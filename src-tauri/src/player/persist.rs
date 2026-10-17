//! 播放状态持久化：断点续播 + 播放习惯（音量/静音/播放模式）记忆 + 按曲目续播位置
//!
//! 数据落盘到 app_data_dir/playback_state.json：
//!   - 保存时机：播放/暂停/切歌/seek/音量与模式变更、进度 tick 节流（5s）、窗口关闭；
//!   - 恢复时机：player_init 刷新曲目缓存后，若曲目仍在曲库中，
//!     定位到中断位置并自动开始播放（播放列表为空则不恢复）。
//!
//! 另落盘 app_data_dir/track_positions.json（trackId → 毫秒位置表）：
//!   - 记录时机与上同（随 save_state 一并落盘）；自然播完的曲目回读时视为无效；
//!   - 消费时机：用户切换播放任一曲目时，命中有效记录（剩余 ≥3s）即精准接续。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::state::{PlayContext, PlayMode};

/// 按曲目续播位置存储文件名（与 playback_state.json 同目录）：
/// trackId → 上次中断位置（毫秒），供"再次播放同一曲目时精准接续"使用
pub const POSITIONS_FILE: &str = "track_positions.json";

/// 末尾保护：记录位置距曲目末尾不足该值时视为已播完，下次从头播放
pub const RESUME_END_GUARD_MS: u64 = 3000;

/// 续播位置有效性判定（纯函数，便于单测）：
/// - 位置为 0 或时长未知（0）：无效，从头播放
/// - 位置越界（≥ 时长）或剩余时长 < 3s：视为已播完，从头播放
/// - 其余：返回原位置
pub fn sanitize_resume_position(position_ms: u64, duration_ms: u64) -> u64 {
    if position_ms == 0 || duration_ms == 0 {
        return 0;
    }
    if position_ms >= duration_ms {
        return 0;
    }
    if duration_ms - position_ms < RESUME_END_GUARD_MS {
        return 0;
    }
    position_ms
}

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

/// 读取按曲目续播位置表；文件缺失或损坏时返回空表（按无历史处理）
pub fn load_positions(path: &Path) -> HashMap<String, u64> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

/// 写入按曲目续播位置表（失败静默，不影响播放主流程）
pub fn save_positions(path: &Path, positions: &HashMap<String, u64>) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(positions) {
        let _ = std::fs::write(path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_zero_position() {
        assert_eq!(sanitize_resume_position(0, 200_000), 0);
    }

    #[test]
    fn sanitize_rejects_unknown_duration() {
        assert_eq!(sanitize_resume_position(60_000, 0), 0);
    }

    #[test]
    fn sanitize_rejects_position_beyond_duration() {
        assert_eq!(sanitize_resume_position(200_000, 180_000), 0);
        assert_eq!(sanitize_resume_position(180_000, 180_000), 0);
    }

    #[test]
    fn sanitize_rejects_near_end_within_3s() {
        assert_eq!(sanitize_resume_position(178_000, 180_000), 0);
        assert_eq!(sanitize_resume_position(177_001, 180_000), 0);
    }

    #[test]
    fn sanitize_accepts_valid_position() {
        assert_eq!(sanitize_resume_position(177_000, 180_000), 177_000);
        assert_eq!(sanitize_resume_position(1, 180_000), 1);
    }

    #[test]
    fn positions_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "tplayer-positions-test-{}",
            std::process::id()
        ));
        let path = dir.join(POSITIONS_FILE);
        let mut map = HashMap::new();
        map.insert("abc".to_string(), 61_500u64);
        map.insert("def".to_string(), 0u64);
        save_positions(&path, &map);
        let loaded = load_positions(&path);
        assert_eq!(loaded.get("abc"), Some(&61_500));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn positions_missing_file_returns_empty() {
        let loaded = load_positions(Path::new("Z:/nonexistent/dir/track_positions.json"));
        assert!(loaded.is_empty());
    }
}
