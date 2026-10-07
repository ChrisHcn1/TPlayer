//! 播放相关命令
//!
//! 每个命令对应 src/services/player.ts 中的一个方法（IPlayer 实现）。
//! 前端以 camelCase 传参，Tauri 自动映射到 snake_case 参数。

use tauri::{AppHandle, State};

use crate::error::{AppError, AppResult};
use crate::player::engine::EngineCapabilities;
use crate::player::state::{PlayContext, PlayMode, PlaybackSnapshot};
use crate::AppState;

/// 初始化播放器（恢复曲目缓存、音量等）
///
/// 返回上次会话恢复出的播放上下文（断点续播），无恢复内容时为 None。
/// 恢复出曲目且播放列表（上下文）不为空时自动从断点位置继续播放；
/// 自动续播失败（文件缺失等）不影响初始化，UI 停留在错误态可手动重试。
#[tauri::command]
pub fn player_init(state: State<'_, AppState>, app: AppHandle) -> AppResult<Option<PlayContext>> {
    let tracks = state.library.lock().unwrap().tracks();
    let mut player = state.player.lock().unwrap();
    player.set_tracks(tracks);
    let restored = player.apply_pending_restore();
    // 断点存在即自动续播（播放列表为空时 apply_pending_restore 已返回 None）
    if player.snapshot.track_id.is_some() {
        // 断点曲目无法播放（文件缺失/解码器不可用）时，自动顺序跳过到可播放曲目
        if player.resume(&app).is_err() {
            let _ = player.play_current_or_skip(&app);
        }
    }
    Ok(restored)
}

/// 查询当前引擎能力
#[tauri::command]
pub fn player_capabilities(state: State<'_, AppState>) -> AppResult<EngineCapabilities> {
    Ok(state.player.lock().unwrap().capabilities())
}

/// 获取完整播放快照
#[tauri::command]
pub fn player_snapshot(state: State<'_, AppState>) -> AppResult<PlaybackSnapshot> {
    Ok(state.player.lock().unwrap().snapshot.clone())
}

/// 播放指定曲目
#[tauri::command]
pub fn player_play(
    state: State<'_, AppState>,
    app: AppHandle,
    track_id: String,
    context: Option<PlayContext>,
) -> AppResult<()> {
    let track = state
        .library
        .lock()
        .unwrap()
        .get_track(&track_id)
        .ok_or_else(|| AppError::Player(format!("曲目不存在: {}", track_id)))?;

    let mut player = state.player.lock().unwrap();
    player.play(&track, context, &app)
}

#[tauri::command]
pub fn player_pause(state: State<'_, AppState>, app: AppHandle) -> AppResult<()> {
    state.player.lock().unwrap().pause(&app)
}

#[tauri::command]
pub fn player_resume(state: State<'_, AppState>, app: AppHandle) -> AppResult<()> {
    state.player.lock().unwrap().resume(&app)
}

#[tauri::command]
pub fn player_stop(state: State<'_, AppState>, app: AppHandle) -> AppResult<()> {
    state.player.lock().unwrap().stop(&app)
}

#[tauri::command]
pub fn player_next(state: State<'_, AppState>, app: AppHandle) -> AppResult<()> {
    let next_id = {
        let mut player = state.player.lock().unwrap();
        player.next()
    };
    if let Some(id) = next_id {
        let track = state
            .library
            .lock()
            .unwrap()
            .get_track(&id)
            .ok_or_else(|| AppError::Player(format!("曲目不存在: {}", id)))?;
        state.player.lock().unwrap().play(&track, None, &app)?;
    }
    Ok(())
}

#[tauri::command]
pub fn player_previous(state: State<'_, AppState>, app: AppHandle) -> AppResult<()> {
    let prev_id = {
        let mut player = state.player.lock().unwrap();
        player.previous()
    };
    if let Some(id) = prev_id {
        let track = state
            .library
            .lock()
            .unwrap()
            .get_track(&id)
            .ok_or_else(|| AppError::Player(format!("曲目不存在: {}", id)))?;
        state.player.lock().unwrap().play(&track, None, &app)?;
    }
    Ok(())
}

/// 跳转到指定毫秒位置
#[tauri::command]
pub fn player_seek(state: State<'_, AppState>, position_ms: u64) -> AppResult<()> {
    state.player.lock().unwrap().seek(position_ms)
}

/// 设置音量（0.0 - 1.0）
#[tauri::command]
pub fn player_set_volume(
    state: State<'_, AppState>,
    app: AppHandle,
    volume: f32,
) -> AppResult<()> {
    state.player.lock().unwrap().set_volume(volume, &app)
}

#[tauri::command]
pub fn player_set_muted(
    state: State<'_, AppState>,
    app: AppHandle,
    muted: bool,
) -> AppResult<()> {
    state.player.lock().unwrap().set_muted(muted, &app)
}

/// 设置播放模式
#[tauri::command]
pub fn player_set_mode(
    state: State<'_, AppState>,
    app: AppHandle,
    mode: PlayMode,
) -> AppResult<()> {
    state.player.lock().unwrap().set_mode(mode, &app)
}

/// 释放播放器资源
#[tauri::command]
pub fn player_dispose(state: State<'_, AppState>) -> AppResult<()> {
    state.player.lock().unwrap().dispose()
}
