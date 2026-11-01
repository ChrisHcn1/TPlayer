//! 播放相关命令
//!
//! 每个命令对应 src/services/player.ts 中的一个方法（IPlayer 实现）。
//! 前端以 camelCase 传参，Tauri 自动映射到 snake_case 参数。

use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::player::engine::{EngineCapabilities, EqConfig};
use crate::player::state::{PlayContext, PlayMode, PlaybackSnapshot, SleepTimerStatus};
use crate::AppState;

/// 初始化播放器（恢复曲目缓存、音量等）
///
/// 返回上次会话恢复出的播放上下文（断点续播），无恢复内容时为 None。
/// 断点曲目为原生可播的非 CUE 曲目时自动从断点位置续播；需前端 WASM 兜底
/// 解码的冷门格式（FFmpeg 缺失）与 CUE 分轨在 restore 阶段已跳过定位——
/// 避免启动即触发整文件慢解码或错误定位，用户可手动选择播放。
/// 自动续播失败（文件缺失等）不影响初始化，按队列顺序自动跳过尝试。
#[tauri::command]
pub async fn player_init(app: AppHandle) -> AppResult<Option<PlayContext>> {
    // 初始化会读全库并可能加载当前曲目（引擎打开/ffmpeg/SACD 提取会阻塞），
    // 放到阻塞线程池，避免卡住主线程。
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        let tracks = state.library.lock().unwrap_or_else(|e| e.into_inner()).tracks();
        let mut player = state.player.lock().unwrap();
        player.set_tracks(tracks);
        let restored = player.apply_pending_restore();
        // 断点存在且已定位：从断点位置自动续播；失败则从当前断点曲目起按队列尝试
        if player.snapshot.track_id.is_some() && player.resume(&app).is_err() {
            let _ = player.play_current_or_skip(&app);
        }
        Ok(restored)
    })
    .await
    .map_err(|e| AppError::Player(format!("播放器初始化任务失败: {e}")))?
}

/// 查询当前引擎能力
#[tauri::command]
pub async fn player_capabilities(app: AppHandle) -> AppResult<EngineCapabilities> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        Ok(state.player.lock().unwrap().capabilities())
    })
    .await
    .map_err(|e| AppError::Player(format!("查询引擎能力任务执行失败: {e}")))?
}

/// 获取完整播放快照
///
/// 该命令在切歌后会被前端立即调用（`await playerService.snapshot()`），
/// 是最容易撞上 `player_play` 持锁窗口的热点路径。历史上它是同步命令，
/// 会在主线程 `lock()` 阻塞整个 WebView——这正是"点下一首后界面假死"
/// 的直接原因。现统一改走阻塞线程池。
#[tauri::command]
pub async fn player_snapshot(app: AppHandle) -> AppResult<PlaybackSnapshot> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        Ok(state.player.lock().unwrap().snapshot.clone())
    })
    .await
    .map_err(|e| AppError::Player(format!("获取播放快照任务执行失败: {e}")))?
}

/// 播放指定曲目
#[tauri::command]
pub async fn player_play(app: AppHandle, track_id: String, context: Option<PlayContext>) -> AppResult<()> {
    // player.play 内部会同步等引擎打开音频（ffmpeg 探测 / SACD 提取可能很慢），
    // 放到阻塞线程池，绝不能阻塞主线程导致「未响应」。
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        let track = state
            .library
            .lock()
            .unwrap()
            .get_track(&track_id)
            .ok_or_else(|| AppError::Player(format!("曲目不存在: {}", track_id)))?;

        let mut player = state.player.lock().unwrap();
        player.play(&track, context, &app)
    })
    .await
    .map_err(|e| AppError::Player(format!("播放任务执行失败: {e}")))?
}

#[tauri::command]
pub async fn player_pause(app: AppHandle) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        state.player.lock().unwrap().pause(&app)
    })
    .await
    .map_err(|e| AppError::Player(format!("暂停任务执行失败: {e}")))?
}

#[tauri::command]
pub async fn player_stop(app: AppHandle) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        state.player.lock().unwrap().stop(&app)
    })
    .await
    .map_err(|e| AppError::Player(format!("停止任务执行失败: {e}")))?
}

#[tauri::command]
pub async fn player_resume(app: AppHandle) -> AppResult<()> {
    // resume 可能触发引擎加载当前曲目（阻塞），移出主线程
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        state.player.lock().unwrap().resume(&app)
    })
    .await
    .map_err(|e| AppError::Player(format!("继续播放任务执行失败: {e}")))?
}

#[tauri::command]
pub async fn player_next(app: AppHandle) -> AppResult<()> {
    // 先预览下一首 id（锁外预取曲目），再在同一 player 锁内完成推进 + 起播，
    // 避免时钟线程在窗口期内二次推进造成跳两首/双链路。起播会等引擎 ack，移出主线程。
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        let next_id = state
            .player
            .lock()
            .unwrap()
            .peek_next_id()
            .ok_or_else(|| AppError::Player("队列已到末尾或队列为空".into()))?;
        let track = state
            .library
            .lock()
            .unwrap()
            .get_track(&next_id)
            .ok_or_else(|| AppError::Player(format!("曲目不存在: {}", next_id)))?;
        state
            .player
            .lock()
            .unwrap()
            .advance_and_play(false, &track, &app)
    })
    .await
    .map_err(|e| AppError::Player(format!("下一曲任务执行失败: {e}")))?
}

#[tauri::command]
pub async fn player_previous(app: AppHandle) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        let prev_id = state
            .player
            .lock()
            .unwrap()
            .peek_previous_id()
            .ok_or_else(|| AppError::Player("队列为空".into()))?;
        let track = state
            .library
            .lock()
            .unwrap()
            .get_track(&prev_id)
            .ok_or_else(|| AppError::Player(format!("曲目不存在: {}", prev_id)))?;
        state
            .player
            .lock()
            .unwrap()
            .advance_and_play(true, &track, &app)
    })
    .await
    .map_err(|e| AppError::Player(format!("上一曲任务执行失败: {e}")))?
}

/// 跳转到指定毫秒位置
#[tauri::command]
pub async fn player_seek(app: AppHandle, position_ms: u64) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        state.player.lock().unwrap().seek(position_ms)
    })
    .await
    .map_err(|e| AppError::Player(format!("跳转任务执行失败: {e}")))?
}

/// 设置音量（0.0 - 1.0）
///
/// 全部播放命令统一 `async + spawn_blocking`：`state.player` 是普通 `Mutex`，
/// 而 `player_play` 会在持锁期间完成打开文件 / 启动 ffmpeg / SACD 提取等
/// 数十毫秒到数秒的操作。若本命令以**同步** Tauri 命令实现，它会跑在
/// WebView 主线程上，`lock()` 一等就把整个窗口冻结（表现为"程序未响应"）。
#[tauri::command]
pub async fn player_set_volume(app: AppHandle, volume: f32) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        state.player.lock().unwrap().set_volume(volume, &app)
    })
    .await
    .map_err(|e| AppError::Player(format!("设置音量任务执行失败: {e}")))?
}

#[tauri::command]
pub async fn player_set_muted(app: AppHandle, muted: bool) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        state.player.lock().unwrap().set_muted(muted, &app)
    })
    .await
    .map_err(|e| AppError::Player(format!("设置静音任务执行失败: {e}")))?
}

/// 设置播放模式
#[tauri::command]
pub async fn player_set_mode(app: AppHandle, mode: PlayMode) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        state.player.lock().unwrap().set_mode(mode, &app)
    })
    .await
    .map_err(|e| AppError::Player(format!("设置播放模式任务执行失败: {e}")))?
}

/// 释放播放器资源
#[tauri::command]
pub async fn player_dispose(app: AppHandle) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        state.player.lock().unwrap().dispose()
    })
    .await
    .map_err(|e| AppError::Player(format!("释放播放器任务执行失败: {e}")))?
}

/// 设置交叉淡化（启用开关 + 时长毫秒，0~3000）
///
/// 写入 config.json 持久化，并立即下发引擎线程生效。
#[tauri::command]
pub async fn player_set_crossfade(
    app: AppHandle,
    enabled: bool,
    duration_ms: u64,
) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        let duration_ms = duration_ms.min(3000);
        {
            let mut config = state.config.lock().unwrap();
            config.crossfade.enabled = enabled;
            config.crossfade.duration_ms = duration_ms;
            config.save(&state.config_path)?;
        }
        state.player.lock().unwrap().set_crossfade(enabled, duration_ms)
    })
    .await
    .map_err(|e| AppError::Player(format!("设置交叉淡化任务执行失败: {e}")))?
}

/// 设置音量归一化 ReplayGain 开关
///
/// 写入 config.json 持久化，并立即下发引擎线程生效（当前曲目按新策略重算增益）。
#[tauri::command]
pub async fn player_set_replaygain(app: AppHandle, enabled: bool) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        {
            let mut config = state.config.lock().unwrap();
            config.replaygain.enabled = enabled;
            config.save(&state.config_path)?;
        }
        state.player.lock().unwrap().set_replaygain(enabled);
        Ok(())
    })
    .await
    .map_err(|e| AppError::Player(format!("设置 ReplayGain 任务执行失败: {e}")))?
}

/// 设置播放倍速（有效区间 0.25 ~ 4.0）；引擎立即生效并向前端广播 Rate 事件
#[tauri::command]
pub async fn player_set_rate(app: AppHandle, rate: f32) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        state.player.lock().unwrap().set_rate(rate, &app)
    })
    .await
    .map_err(|e| AppError::Player(format!("设置倍速任务执行失败: {e}")))?
}

/// 设置均衡器配置（频段增益 + 总开关 + 输出增益）；写入 config.json 持久化，
/// 并立即下发引擎线程生效（下一首起沿用新配置）。
#[tauri::command]
pub async fn player_set_eq(
    app: AppHandle,
    enabled: bool,
    bands: Vec<crate::player::engine::EqBand>,
    output_gain_db: f32,
) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        {
            let mut config = state.config.lock().unwrap();
            config.eq.enabled = enabled;
            config.eq.bands = bands.clone();
            config.eq.output_gain_db = output_gain_db;
            config.save(&state.config_path)?;
        }
        state.player.lock().unwrap().set_eq(EqConfig {
            enabled,
            bands,
            output_gain_db,
        });
        Ok(())
    })
    .await
    .map_err(|e| AppError::Player(format!("设置均衡器任务执行失败: {e}")))?
}

/// 读取当前实验室均衡器配置（供「实验室」面板回显已保存的频段增益）。
#[tauri::command]
pub async fn player_get_eq(app: AppHandle) -> AppResult<EqConfig> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        Ok(state.config.lock().unwrap().eq.clone())
    })
    .await
    .map_err(|e| AppError::Player(format!("读取均衡器任务执行失败: {e}")))?
}

/// 设置睡眠定时器（delay_ms 毫秒后到期；wait_track_end=true 时到期后播完当前曲目才停止）
#[tauri::command]
pub async fn player_set_sleep_timer(
    app: AppHandle,
    delay_ms: u64,
    wait_track_end: bool,
) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        state
            .player
            .lock()
            .unwrap()
            .set_sleep_timer(delay_ms, wait_track_end, &app);
        Ok(())
    })
    .await
    .map_err(|e| AppError::Player(format!("设置睡眠定时器任务执行失败: {e}")))?
}

/// 取消睡眠定时器
#[tauri::command]
pub async fn player_cancel_sleep_timer(app: AppHandle) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        state.player.lock().unwrap().cancel_sleep_timer(&app);
        Ok(())
    })
    .await
    .map_err(|e| AppError::Player(format!("取消睡眠定时器任务执行失败: {e}")))?
}

/// 查询睡眠定时器状态
#[tauri::command]
pub async fn player_sleep_timer_status(app: AppHandle) -> AppResult<SleepTimerStatus> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        Ok(state.player.lock().unwrap().sleep_timer_status())
    })
    .await
    .map_err(|e| AppError::Player(format!("查询睡眠定时器任务执行失败: {e}")))?
}

/// 从播放队列中移除指定下标的曲目（移除当前播放曲目不打断播放）
#[tauri::command]
pub async fn player_remove_from_queue(app: AppHandle, index: usize) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        state.player.lock().unwrap().remove_from_queue(index, &app)
    })
    .await
    .map_err(|e| AppError::Player(format!("移除队列项任务执行失败: {e}")))?
}
