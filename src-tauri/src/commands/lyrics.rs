//! 歌词相关命令
//!
//! 来源优先级链（本地优先）：
//!   1. 内嵌标签（lofty 读取 Lyrics 字段）；
//!   2. 同目录同名 .lrc 边车文件；
//!   3. 在线匹配（受设置 online.enabled 双闸控制，默认关闭；
//!      成功后写为 .lrc 边车，下次本地命中不再请求网络）。
//! 用户手动校正的偏移持久化在 lyrics_offsets.json。

use std::path::PathBuf;

use tauri::State;

use crate::error::AppResult;
use crate::lyrics::online;
use crate::lyrics::parser::{self, Lyrics, LyricsSource};
use crate::AppState;

/// 读取音频内嵌歌词文本（Lyrics 标签，如 ID3v2 USLT 帧）
fn read_embedded_lyrics(path: &std::path::Path) -> Option<String> {
    use lofty::file::TaggedFileExt;
    use lofty::tag::ItemKey;

    let tagged = lofty::read_from_path(path).ok()?;
    // 主标签优先，其次遍历全部标签（多标签文件如 ID3v1+ID3v2 并存）
    let mut tags: Vec<&lofty::tag::Tag> = Vec::new();
    if let Some(primary) = tagged.primary_tag() {
        tags.push(primary);
    }
    tags.extend(tagged.tags().iter());
    for tag in tags {
        if let Some(text) = tag.get_string(&ItemKey::Lyrics) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// 同名 .lrc 边车文件路径（song.flac → song.lrc）
fn sidecar_path(track: &crate::library::models::Track) -> PathBuf {
    PathBuf::from(&track.file_path).with_extension("lrc")
}

/// 读取指定曲目的歌词；三个来源均无歌词时返回 None
#[tauri::command]
pub async fn lyrics_load(
    state: State<'_, AppState>,
    track_id: String,
) -> AppResult<Option<Lyrics>> {
    // 取出所需数据后立即释放锁（不得跨 await 持锁）
    let (track, online_enabled) = {
        let library = state.library.lock().unwrap();
        let config = state.config.lock().unwrap();
        let track = library.get_track(&track_id);
        let enabled = config.online.enabled && !config.online.provider.is_empty() && config.online.provider != "none";
        (track, enabled)
    };
    let Some(track) = track else { return Ok(None) };
    let audio_path = PathBuf::from(&track.file_path);

    // 1. 内嵌标签
    if let Some(content) = read_embedded_lyrics(&audio_path) {
        let mut lyrics = parser::parse(&track_id, LyricsSource::Embedded, &content);
        lyrics.offset_ms = parser::get_offset(&state.lyrics_offsets_path, &track_id);
        return Ok(Some(lyrics));
    }

    // 2. 同名 .lrc 边车
    let sidecar = sidecar_path(&track);
    if let Ok(content) = std::fs::read_to_string(&sidecar) {
        if !content.trim().is_empty() {
            let mut lyrics = parser::parse(&track_id, LyricsSource::Sidecar, &content);
            lyrics.offset_ms = parser::get_offset(&state.lyrics_offsets_path, &track_id);
            return Ok(Some(lyrics));
        }
    }

    // 3. 在线匹配（默认关闭；成功后写为边车供下次本地命中）
    if online_enabled {
        let duration_ms = (track.duration * 1000.0) as u64;
        if let Some(content) =
            online::fetch_lyrics(&track.title, &track.artists, duration_ms).await
        {
            // 落盘边车失败不阻断返回（目录只读等场景降级为纯在线歌词）
            let _ = std::fs::write(&sidecar, &content);
            let mut lyrics = parser::parse(&track_id, LyricsSource::Online, &content);
            lyrics.offset_ms = parser::get_offset(&state.lyrics_offsets_path, &track_id);
            return Ok(Some(lyrics));
        }
    }

    Ok(None)
}

/// 保存用户手动校正的歌词偏移（毫秒），持久化到 lyrics_offsets.json
#[tauri::command]
pub fn lyrics_save_offset(
    state: State<'_, AppState>,
    track_id: String,
    offset_ms: i64,
) -> AppResult<()> {
    parser::save_offset(&state.lyrics_offsets_path, &track_id, offset_ms);
    Ok(())
}

/// 清空歌词缓存（当前为全部曲目的手动偏移记录）
#[tauri::command]
pub fn lyrics_clear_cache(state: State<'_, AppState>) -> AppResult<()> {
    parser::clear_offsets(&state.lyrics_offsets_path);
    Ok(())
}
