//! 播放状态机：管理当前播放状态、队列上下文、自动连播、事件推送
//!
//! 位置追踪由本状态机负责（引擎线程不回传位置），通过 Instant 计时。

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use rand::Rng;
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{AppError, AppResult};
use crate::library::models::Track;

use super::engine::{spawn_engine, EngineCapabilities, EngineHandle};
use super::persist::{self, PersistedPlayback};
use super::state::{
    PlayContext, PlayMode, PlaybackEvent, PlaybackSnapshot, PlaybackStatus, SleepTimerStatus,
};

/// 播放中持久化节流的间隔（毫秒）：最多丢失 5s 进度
const STATE_SAVE_INTERVAL_MS: u64 = 5000;

/// 一次播放操作中连续跳过无法播放曲目的上限（避免全队列不可播时长时间静默尝试）
const MAX_AUTO_SKIP: usize = 20;

/// 连续"异常秒切"多少首后判定解码链路不可用并停止（解码器缺失/批量损坏时避免无声快进）
const ABRUPT_END_LIMIT: usize = 8;

/// 自动连播预加载提前量（毫秒）：当前曲剩余时长小于该值时提前预解码下一曲
const PRELOAD_LEAD_MS: u64 = 3000;

/// 睡眠定时器内部状态
struct SleepTimer {
    /// 到期时刻（Unix epoch 毫秒）
    ends_at_epoch_ms: u64,
    /// 到期后播完当前曲目才停止（否则到期立即暂停）
    wait_track_end: bool,
    /// 已到期：wait_track_end 模式下等待当前曲播完
    fired: bool,
}

fn now_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 播放器句柄：封装引擎与状态机，命令层通过它操作播放
pub struct PlayerHandle {
    engine: EngineHandle,
    pub snapshot: PlaybackSnapshot,
    context: Option<PlayContext>,
    index: usize,
    /// 全部曲目缓存（按 ID 索引），供 next/previous/自动连播查找文件路径
    track_map: HashMap<String, Track>,
    /// 位置计时
    position_offset_ms: u64,
    play_started_at: Option<Instant>,
    is_paused: bool,
    /// 播放状态持久化文件路径（playback_state.json）
    persist_path: Option<PathBuf>,
    /// 待恢复的断点（set_persist_path 时加载，apply_pending_restore 消费）
    pending_restore: Option<PersistedPlayback>,
    /// 上次持久化时间（tick 节流用）
    last_state_save: Instant,
    /// 恢复断点后引擎尚未装载曲目：首次 resume/seek 需先装载再定位
    needs_engine_load: bool,
    /// 连续"异常秒切"次数（sink 提前排空，如解码器缺失）；达到阈值停止并报错，
    /// 避免整张专辑的曲目被无声快速跳过
    abrupt_end_streak: usize,
    /// 已预解码到引擎待命链路的下一曲 id（P2 自动连播预加载）
    preloaded_track_id: Option<String>,
    /// 最近一次尝试预加载的曲目 id（成功/失败都记录，避免 250ms 时钟重复尝试）
    preload_attempted: Option<String>,
    /// 睡眠定时器（None = 未启用）
    sleep_timer: Option<SleepTimer>,
}

impl PlayerHandle {
    pub fn new(engine: EngineHandle) -> Self {
        Self {
            engine,
            snapshot: PlaybackSnapshot::default(),
            context: None,
            index: 0,
            track_map: HashMap::new(),
            position_offset_ms: 0,
            play_started_at: None,
            is_paused: false,
            persist_path: None,
            pending_restore: None,
            last_state_save: Instant::now(),
            needs_engine_load: false,
            abrupt_end_streak: 0,
            preloaded_track_id: None,
            preload_attempted: None,
            sleep_timer: None,
        }
    }

    /// 设置持久化路径并加载上次会话的播放状态（待 player_init 应用）
    pub fn set_persist_path(&mut self, path: PathBuf) {
        self.pending_restore = persist::load(&path);
        self.persist_path = Some(path);
    }

    /// 刷新曲目缓存（曲库扫描完成后调用）
    pub fn set_tracks(&mut self, tracks: Vec<Track>) {
        self.track_map = tracks.into_iter().map(|t| (t.id.clone(), t)).collect();
    }

    /// 应用启动恢复：音量/静音/播放模式始终恢复；
    /// 曲目仍在曲库中时恢复为 paused 态并定位到中断位置（引擎待首次 resume 装载）。
    /// 返回恢复出的播放上下文（供前端还原队列显示）。
    pub fn apply_pending_restore(&mut self) -> Option<PlayContext> {
        let state = self.pending_restore.take()?;

        // 播放习惯：音量/静音/播放模式
        self.snapshot.volume = state.volume.clamp(0.0, 1.0);
        self.snapshot.muted = state.muted;
        self.snapshot.mode = state.mode;
        let _ = self.engine.set_volume(self.snapshot.volume);
        let _ = self.engine.set_muted(self.snapshot.muted);

        // 断点续播：曲目需仍在曲库中
        let track_id = state.track_id?;
        let track = self.track_map.get(&track_id)?.clone();
        let mut position_ms = state.position_ms;
        let duration_ms = (track.duration * 1000.0) as u64;
        // 已播到结尾的曲目从头开始；时长未知（0）时按原样恢复
        if duration_ms > 0 && position_ms >= duration_ms {
            position_ms = 0;
        }

        self.context = state.context;
        self.index = state.index;
        if let Some(ctx) = &self.context {
            if let Some(idx) = ctx.track_ids.iter().position(|id| id == &track_id) {
                self.index = idx;
            }
        }

        self.snapshot.status = PlaybackStatus::Paused;
        self.snapshot.track_id = Some(track_id);
        self.snapshot.position_ms = position_ms;
        self.snapshot.duration_ms = duration_ms;
        self.position_offset_ms = position_ms;
        self.play_started_at = None;
        self.is_paused = true;
        self.needs_engine_load = true;

        self.context.clone()
    }

    /// 持久化当前播放状态（断点 + 播放习惯）；停止/空闲时不记录曲目
    pub fn save_state(&mut self) {
        let Some(path) = &self.persist_path else { return };
        let playing_track = match self.snapshot.status {
            PlaybackStatus::Playing | PlaybackStatus::Paused | PlaybackStatus::Loading => {
                self.snapshot.track_id.clone()
            }
            _ => None,
        };
        let state = PersistedPlayback {
            track_id: playing_track,
            position_ms: if self.snapshot.status == PlaybackStatus::Playing {
                self.current_position()
            } else {
                self.position_offset_ms
            },
            context: self.context.clone(),
            index: self.index,
            volume: self.snapshot.volume,
            muted: self.snapshot.muted,
            mode: self.snapshot.mode,
        };
        persist::save(path, &state);
        self.last_state_save = Instant::now();
    }

    pub fn capabilities(&self) -> EngineCapabilities {
        let mut formats = vec![
            "mp3".to_string(),
            "flac".to_string(),
            "wav".to_string(),
            "ogg".to_string(),
            "oga".to_string(),
            "opus".to_string(),
            "aac".to_string(),
            "m4a".to_string(),
        ];
        if super::engine::ffmpeg_available() {
            formats.extend(super::engine::ffmpeg_formats());
        }
        EngineCapabilities {
            seek: true,
            volume_control: true,
            rate_control: false,
            supported_formats: formats,
        }
    }

    fn current_position(&self) -> u64 {
        if self.is_paused {
            return self.position_offset_ms;
        }
        if let Some(started) = self.play_started_at {
            let elapsed = started.elapsed().as_millis() as u64;
            self.position_offset_ms + elapsed
        } else {
            self.position_offset_ms
        }
    }

    /// 播放指定曲目；无法播放（缺解码器/文件损坏或缺失）时自动顺序跳过后续曲目
    pub fn play(
        &mut self,
        track: &Track,
        context: Option<PlayContext>,
        app: &AppHandle,
    ) -> AppResult<()> {
        self.play_or_skip(track, context, app)
    }

    /// 尝试播放一首曲目。
    /// `quiet=true` 时不向前端推送任何事件（自动跳过链中的试探性播放），
    /// 由调用方在最终成功/失败时统一播报，避免错误 toast 风暴与中间态闪烁。
    fn play_once(
        &mut self,
        track: &Track,
        context: Option<PlayContext>,
        app: &AppHandle,
        quiet: bool,
    ) -> AppResult<()> {
        // 任何主动播放都会改变"下一曲"计划：使既有预加载失效
        // （引擎 play() 会按路径匹配自动复用同曲预加载链路，其余情况丢弃）
        self.preloaded_track_id = None;
        self.preload_attempted = None;

        if let Some(ctx) = context {
            self.index = ctx
                .track_ids
                .iter()
                .position(|id| id == &track.id)
                .unwrap_or(0);
            self.context = Some(ctx);
        } else if self.context.is_none() {
            let ids: Vec<String> = self.track_map.keys().cloned().collect();
            self.context = Some(PlayContext {
                source: super::state::PlayContextSource::Library,
                source_id: None,
                track_ids: ids,
            });
            self.index = self
                .context
                .as_ref()
                .unwrap()
                .track_ids
                .iter()
                .position(|id| id == &track.id)
                .unwrap_or(0);
        }

        self.snapshot.status = PlaybackStatus::Loading;
        self.snapshot.track_id = Some(track.id.clone());
        // 切歌时同步重置进度，避免旧曲目的 position/duration 残留到 Loading 窗口期
        self.snapshot.position_ms = 0;
        self.snapshot.duration_ms = 0;
        self.position_offset_ms = 0;
        self.play_started_at = None;
        self.snapshot.error = None;
        self.needs_engine_load = false;
        if !quiet {
            emit(app, PlaybackEvent::Status { status: PlaybackStatus::Loading });
        }

        // 引擎线程播放（异步），UI 立即切到 playing 态
        let gain = self.replaygain_of(track);
        match self.engine.play(std::path::PathBuf::from(&track.file_path), gain) {
            Ok(_) => {
                self.snapshot.status = PlaybackStatus::Playing;
                self.snapshot.duration_ms = (track.duration * 1000.0) as u64;
                // 时长兜底：曲库中时长为 0（lofty 无法解析的旧数据）时用 ffprobe 探测，
                // 否则进度条不推进、自动连播失效
                if self.snapshot.duration_ms == 0 {
                    if let Some(secs) =
                        super::engine::probe_duration(std::path::Path::new(&track.file_path))
                    {
                        self.snapshot.duration_ms = (secs * 1000.0) as u64;
                    } else {
                        eprintln!("[play] ffprobe 时长探测失败: {}", track.file_path);
                    }
                }
                self.play_started_at = Some(Instant::now());
                self.is_paused = false;
                // ReplayGain 兜底：曲目无内嵌/已算增益时，后台用 ffmpeg ebur128
                // 分析响度并写回曲库（本次播放仍按 1.0 增益；此后播放即生效）
                if track.replaygain_db.is_none() || track.replaygain_peak.is_none() {
                    self.ensure_replaygain_analyzed(app, track);
                }
                if !quiet {
                    emit(
                        app,
                        PlaybackEvent::Track {
                            track_id: track.id.clone(),
                            duration_ms: self.snapshot.duration_ms,
                        },
                    );
                    emit(app, PlaybackEvent::Status { status: PlaybackStatus::Playing });
                    self.save_state();
                }
            }
            Err(e) => {
                // 回收半初始化的播放链路，避免残留 sink 影响下一次尝试
                let _ = self.engine.stop();
                self.snapshot.status = PlaybackStatus::Error;
                self.snapshot.error = Some(e.to_string());
                self.play_started_at = None;
                self.is_paused = true;
                if !quiet {
                    emit(
                        app,
                        PlaybackEvent::Error {
                            message: e.to_string(),
                            track_id: Some(track.id.clone()),
                        },
                    );
                }
                return Err(e);
            }
        }
        Ok(())
    }

    /// 播放；失败时自动按队列顺序跳过无法播放的曲目（忽略单曲循环/随机模式）。
    /// 最多尝试 MAX_AUTO_SKIP 首，全部失败时只播报一次最终错误。
    fn play_or_skip(
        &mut self,
        track: &Track,
        context: Option<PlayContext>,
        app: &AppHandle,
    ) -> AppResult<()> {
        let mut tried: HashSet<String> = HashSet::new();
        let mut current = track.clone();
        let mut pending_ctx = context;
        let mut last_err: Option<AppError> = None;
        let mut skipped = 0usize;

        loop {
            tried.insert(current.id.clone());
            match self.play_once(&current, pending_ctx.take(), app, true) {
                Ok(()) => {
                    if skipped > 0 {
                        eprintln!("[play] 已自动跳过 {skipped} 首无法播放的曲目");
                    }
                    // 成功：统一播报最终曲目
                    emit(
                        app,
                        PlaybackEvent::Track {
                            track_id: current.id.clone(),
                            duration_ms: self.snapshot.duration_ms,
                        },
                    );
                    emit(app, PlaybackEvent::Status { status: PlaybackStatus::Playing });
                    self.save_state();
                    return Ok(());
                }
                Err(e) => {
                    last_err = Some(e);
                    skipped += 1;
                    match self.next_skippable_id(&tried, skipped) {
                        Some(next_id) => {
                            match self.track_map.get(&next_id).cloned() {
                                Some(next_track) => current = next_track,
                                // 队列引用了已从曲库移除的 id：计入尝试并继续
                                None => {
                                    tried.insert(next_id);
                                    skipped += 1;
                                    continue;
                                }
                            }
                        }
                        None => {
                            // 候选耗尽：错误态只播报一次
                            let message = last_err
                                .as_ref()
                                .map(|e| e.to_string())
                                .unwrap_or_else(|| "无法播放".to_string());
                            self.snapshot.status = PlaybackStatus::Error;
                            self.snapshot.error = Some(message.clone());
                            emit(
                                app,
                                PlaybackEvent::Error {
                                    message: format!(
                                        "{message}（连续 {skipped} 首曲目无法播放，已停止）"
                                    ),
                                    track_id: Some(current.id.clone()),
                                },
                            );
                            return Err(last_err.unwrap_or_else(|| {
                                AppError::Player("没有可播放的曲目".to_string())
                            }));
                        }
                    }
                }
            }
        }
    }

    /// 从播放队列中移除指定下标的曲目（右面板队列操作）。
    /// 移除当前播放曲目不打断播放（结束后从后继继续）；移除后推送 Queue
    /// 事件同步前端队列，并立即持久化断点。
    pub fn remove_from_queue(&mut self, index: usize, app: &AppHandle) -> AppResult<()> {
        let Some(ctx) = self.context.as_mut() else {
            return Err(AppError::Player("没有可操作的播放队列".into()));
        };
        if index >= ctx.track_ids.len() {
            return Err(AppError::Player(format!("队列下标越界: {}", index)));
        }
        let removed_id = ctx.track_ids.remove(index);

        // 修正当前曲目下标：移除当前曲之前的项则前移；移除当前曲本身时
        // 下标自然指向原后继（next/previous 语义基于该下标继续）
        if index < self.index && self.index > 0 {
            self.index -= 1;
        }
        if self.index >= ctx.track_ids.len() {
            self.index = ctx.track_ids.len().saturating_sub(1);
        }

        // 被移除项正是预加载的下一曲：标记失效（切歌时 play 因路径不一致自动回收）
        if self.preloaded_track_id.as_deref() == Some(removed_id.as_str()) {
            self.preloaded_track_id = None;
            self.preload_attempted = None;
        }

        let context = ctx.clone();
        emit(app, PlaybackEvent::Queue { context });
        self.save_state();
        Ok(())
    }

    /// 断点续播失败后调用：从当前断点曲目开始尝试，不可播则自动跳过
    pub fn play_current_or_skip(&mut self, app: &AppHandle) -> AppResult<()> {
        let track_id = self
            .snapshot
            .track_id
            .clone()
            .ok_or_else(|| AppError::Player("没有可播放的曲目".into()))?;
        let track = self
            .track_map
            .get(&track_id)
            .cloned()
            .ok_or_else(|| AppError::Player(format!("曲目不存在: {}", track_id)))?;
        // context 已由 apply_pending_restore 恢复，传 None 沿用
        self.play_or_skip(&track, None, app)
    }

    /// 自动跳过专用：始终按队列顺序向后取未尝试过的下一首索引
    fn next_skippable_id(
        &self,
        tried: &HashSet<String>,
        skipped: usize,
    ) -> Option<String> {
        if skipped >= MAX_AUTO_SKIP {
            return None;
        }
        let ctx = self.context.as_ref()?;
        let n = ctx.track_ids.len();
        if n == 0 {
            return None;
        }
        for step in 1..=n {
            let idx = (self.index + step) % n;
            let id = ctx.track_ids.get(idx)?;
            if !tried.contains(id) {
                return Some(id.clone());
            }
        }
        None
    }

    pub fn pause(&mut self, app: &AppHandle) -> AppResult<()> {
        if self.snapshot.status != PlaybackStatus::Playing {
            return Ok(());
        }
        self.engine.pause()?;
        self.position_offset_ms = self.current_position();
        self.play_started_at = None;
        self.is_paused = true;
        self.snapshot.status = PlaybackStatus::Paused;
        emit(app, PlaybackEvent::Status { status: PlaybackStatus::Paused });
        self.save_state();
        Ok(())
    }

    pub fn resume(&mut self, app: &AppHandle) -> AppResult<()> {
        if self.snapshot.status != PlaybackStatus::Paused {
            return Ok(());
        }
        if self.needs_engine_load {
            // 断点恢复后的首次播放：引擎尚未装载曲目，先装载再定位到中断位置
            let track_id = self
                .snapshot
                .track_id
                .clone()
                .ok_or_else(|| AppError::Player("没有可恢复的曲目".into()))?;
            let track = self
                .track_map
                .get(&track_id)
                .cloned()
                .ok_or_else(|| AppError::Player(format!("曲目不存在: {}", track_id)))?;
            self.engine.play_at(
                std::path::PathBuf::from(&track.file_path),
                self.position_offset_ms,
                self.replaygain_of(&track),
            )?;
            self.needs_engine_load = false;
        } else {
            self.engine.resume()?;
        }
        self.play_started_at = Some(Instant::now());
        self.is_paused = false;
        self.snapshot.status = PlaybackStatus::Playing;
        emit(app, PlaybackEvent::Status { status: PlaybackStatus::Playing });
        Ok(())
    }

    pub fn stop(&mut self, app: &AppHandle) -> AppResult<()> {
        self.engine.stop()?;
        self.preloaded_track_id = None;
        self.preload_attempted = None;
        self.snapshot.status = PlaybackStatus::Stopped;
        self.snapshot.position_ms = 0;
        self.position_offset_ms = 0;
        self.play_started_at = None;
        self.is_paused = false;
        self.needs_engine_load = false;
        emit(app, PlaybackEvent::Status { status: PlaybackStatus::Stopped });
        self.save_state();
        Ok(())
    }

    pub fn seek(&mut self, position_ms: u64) -> AppResult<()> {
        if !self.capabilities().seek {
            return Err(AppError::Player("当前引擎不支持 seek".into()));
        }
        if self.needs_engine_load {
            // 断点恢复后直接拖动进度：先装载曲目再定位
            let track_id = self
                .snapshot
                .track_id
                .clone()
                .ok_or_else(|| AppError::Player("没有可恢复的曲目".into()))?;
            let track = self
                .track_map
                .get(&track_id)
                .cloned()
                .ok_or_else(|| AppError::Player(format!("曲目不存在: {}", track_id)))?;
            self.engine
                .play(std::path::PathBuf::from(&track.file_path), self.replaygain_of(&track))?;
            self.needs_engine_load = false;
        }
        self.engine.seek(position_ms)?;
        self.position_offset_ms = position_ms;
        if !self.is_paused {
            self.play_started_at = Some(Instant::now());
        }
        self.snapshot.position_ms = position_ms;
        self.save_state();
        Ok(())
    }

    pub fn set_volume(&mut self, volume: f32, app: &AppHandle) -> AppResult<()> {
        let v = volume.clamp(0.0, 1.0);
        self.engine.set_volume(v)?;
        self.snapshot.volume = v;
        emit(
            app,
            PlaybackEvent::Volume {
                volume: v,
                muted: self.snapshot.muted,
            },
        );
        self.save_state();
        Ok(())
    }

    pub fn set_muted(&mut self, muted: bool, app: &AppHandle) -> AppResult<()> {
        self.engine.set_muted(muted)?;
        self.snapshot.muted = muted;
        emit(
            app,
            PlaybackEvent::Volume {
                volume: self.snapshot.volume,
                muted,
            },
        );
        self.save_state();
        Ok(())
    }

    pub fn set_mode(&mut self, mode: PlayMode, app: &AppHandle) -> AppResult<()> {
        self.snapshot.mode = mode;
        emit(app, PlaybackEvent::Mode { mode });
        self.save_state();
        Ok(())
    }

    /// 下发交叉淡化参数到引擎线程（启用开关 + 时长毫秒）
    pub fn set_crossfade(&mut self, enabled: bool, duration_ms: u64) -> AppResult<()> {
        self.engine.set_crossfade(enabled, duration_ms);
        Ok(())
    }

    /// 下发音量归一化 ReplayGain 开关到引擎线程（开启时当前曲目立即重算增益）
    pub fn set_replaygain(&mut self, enabled: bool) {
        self.engine.set_replaygain(enabled);
    }

    /// 设置睡眠定时器：delay_ms 毫秒后到期；wait_track_end=true 时到期后
    /// 播完当前曲目才停止（delay_ms=0 表示立即进入"播完当前曲"等待）。
    pub fn set_sleep_timer(&mut self, delay_ms: u64, wait_track_end: bool, app: &AppHandle) {
        self.sleep_timer = Some(SleepTimer {
            ends_at_epoch_ms: now_epoch_ms() + delay_ms.max(1000),
            wait_track_end,
            fired: wait_track_end && delay_ms == 0,
        });
        self.emit_sleep_timer(app);
    }

    /// 取消睡眠定时器
    pub fn cancel_sleep_timer(&mut self, app: &AppHandle) {
        if self.sleep_timer.take().is_some() {
            self.emit_sleep_timer(app);
        }
    }

    /// 查询睡眠定时器状态
    pub fn sleep_timer_status(&self) -> SleepTimerStatus {
        match &self.sleep_timer {
            Some(st) => SleepTimerStatus {
                active: true,
                wait_track_end: st.wait_track_end,
                ends_at_ms: Some(st.ends_at_epoch_ms),
            },
            None => SleepTimerStatus::default(),
        }
    }

    fn emit_sleep_timer(&self, app: &AppHandle) {
        let status = self.sleep_timer_status();
        emit(
            app,
            PlaybackEvent::SleepTimer {
                active: status.active,
                wait_track_end: status.wait_track_end,
                ends_at_ms: status.ends_at_ms,
            },
        );
    }

    /// 取曲目的 ReplayGain 数据：标签/已算增益都持久化在曲库，两者齐备才下发
    fn replaygain_of(&self, track: &Track) -> Option<(f32, f32)> {
        match (track.replaygain_db, track.replaygain_peak) {
            (Some(db), Some(peak)) => Some((db, peak)),
            _ => None,
        }
    }

    /// 无标签曲目后台分析响度并写回曲库（仅开关开启时执行；写库+内存缓存双更新，
    /// 后续播放/预加载直接复用，避免重复分析）。
    fn ensure_replaygain_analyzed(&self, app: &AppHandle, track: &Track) {
        let replaygain_enabled = app
            .try_state::<crate::AppState>()
            .map(|st| st.config.lock().unwrap().replaygain.enabled)
            .unwrap_or(true);
        if !replaygain_enabled {
            return;
        }
        let track_id = track.id.clone();
        let file_path = track.file_path.clone();
        let app = app.clone();
        std::thread::spawn(move || {
            let Some((gain_db, peak)) = crate::replaygain::analyze_loudness(&file_path) else {
                return;
            };
            if let Some(state) = app.try_state::<crate::AppState>() {
                if let Ok(mut library) = state.library.lock() {
                    let _ = library.update_replaygain(&track_id, Some(gain_db), Some(peak));
                }
                if let Ok(mut player) = state.player.lock() {
                    if let Some(t) = player.track_map.get_mut(&track_id) {
                        t.replaygain_db = Some(gain_db);
                        t.replaygain_peak = Some(peak);
                    }
                }
            }
        });
    }

    /// 由时钟线程调用：同步位置并决定是否切歌
    pub fn tick(&mut self, app: &AppHandle) {
        if self.snapshot.status == PlaybackStatus::Playing {
            let pos = self.current_position();
            self.snapshot.position_ms = pos;
            emit(app, PlaybackEvent::Progress { position_ms: pos });

            // 播放习惯与断点的节流持久化（崩溃/强退时最多丢失一个间隔的进度）
            if self.last_state_save.elapsed().as_millis() as u64 >= STATE_SAVE_INTERVAL_MS {
                self.save_state();
            }

            // 睡眠定时器：到期立即暂停；"播完当前曲"模式只标记，停止动作在 on_track_ended 执行
            let fire_pause = match &self.sleep_timer {
                Some(st) if !st.fired && now_epoch_ms() >= st.ends_at_epoch_ms => {
                    if st.wait_track_end {
                        if let Some(st) = self.sleep_timer.as_mut() {
                            st.fired = true;
                        }
                        false
                    } else {
                        true
                    }
                }
                _ => false,
            };
            if fire_pause {
                self.sleep_timer = None;
                let _ = self.pause(app);
                self.emit_sleep_timer(app);
            }

            let dur = self.snapshot.duration_ms;

            // P2 自动连播预加载：剩余时长 < 提前量且尚未预加载下一曲时提前解码
            if self.preloaded_track_id.is_none() {
                let remaining = dur.saturating_sub(pos);
                if remaining <= PRELOAD_LEAD_MS {
                    self.maybe_preload_next(app);
                }
            }

            let reached_duration = dur > 0 && pos + 300 >= dur;
            // 兜底：时长未知（0）或解码器中途退出（如 ffmpeg 缺失/秒退）时，
            // sink 已排空即视为结束。600ms 宽限避免起流瞬间误判。
            let grace_passed = self
                .play_started_at
                .map(|t| t.elapsed().as_millis() > 600)
                .unwrap_or(false);
            let sink_drained = grace_passed && self.engine.is_current_ended();

            if reached_duration {
                // 正常播完
                self.abrupt_end_streak = 0;
                self.on_track_ended(app);
            } else if sink_drained {
                // 提前排空：解码器缺失/文件损坏的典型表现。连续多首即停止，
                // 避免整张专辑被无声快速跳过
                self.abrupt_end_streak += 1;
                if self.abrupt_end_streak >= ABRUPT_END_LIMIT {
                    let _ = self.engine.stop();
                    self.play_started_at = None;
                    self.snapshot.status = PlaybackStatus::Error;
                    let message = format!(
                        "连续 {ABRUPT_END_LIMIT} 首曲目无法播放（解码器缺失或文件损坏），已停止自动连播"
                    );
                    self.snapshot.error = Some(message.clone());
                    emit(
                        app,
                        PlaybackEvent::Error {
                            message,
                            track_id: self.snapshot.track_id.clone(),
                        },
                    );
                } else {
                    self.on_track_ended(app);
                }
            }

            // 稳定播放超过 5s 视为解码链路健康，清除异常计数
            if pos > 5000 {
                self.abrupt_end_streak = 0;
            }
        }
    }

    fn on_track_ended(&mut self, app: &AppHandle) {
        let track_id = self.snapshot.track_id.clone().unwrap_or_default();
        emit(app, PlaybackEvent::Ended { track_id: track_id.clone() });

        // 睡眠定时器"播完当前曲目"已到期：停止而不连播
        if let Some(st) = &self.sleep_timer {
            if st.fired && st.wait_track_end {
                self.sleep_timer = None;
                let _ = self.engine.stop();
                self.preloaded_track_id = None;
                self.preload_attempted = None;
                self.snapshot.status = PlaybackStatus::Stopped;
                self.position_offset_ms = 0;
                self.play_started_at = None;
                self.is_paused = false;
                emit(app, PlaybackEvent::Status { status: PlaybackStatus::Stopped });
                self.emit_sleep_timer(app);
                self.save_state();
                return;
            }
        }

        // P2 无缝衔接：已预加载下一曲时直接接管（无 Loading 窗口、解码已在后台完成）
        if let Some(preloaded_id) = self.preloaded_track_id.take() {
            // 把 index 定位到预加载曲目（Order/RepeatAll 与 peek 一致；Shuffle 以预加载时选定的为准）
            if let Some(idx) = self
                .context
                .as_ref()
                .and_then(|c| c.track_ids.iter().position(|id| id == &preloaded_id))
            {
                self.index = idx;
            }
            if let Some(track) = self.track_map.get(&preloaded_id).cloned() {
                if self.engine.swap_to_next().is_ok() {
                    // 接管成功：直接进入 playing（跳过 Loading，避免切歌闪烁）
                    self.snapshot.status = PlaybackStatus::Playing;
                    self.snapshot.track_id = Some(track.id.clone());
                    self.snapshot.position_ms = 0;
                    self.snapshot.duration_ms = (track.duration * 1000.0) as u64;
                    if self.snapshot.duration_ms == 0 {
                        if let Some(secs) =
                            super::engine::probe_duration(std::path::Path::new(&track.file_path))
                        {
                            self.snapshot.duration_ms = (secs * 1000.0) as u64;
                        }
                    }
                    self.position_offset_ms = 0;
                    self.play_started_at = Some(Instant::now());
                    self.is_paused = false;
                    self.snapshot.error = None;
                    self.needs_engine_load = false;
                    self.preload_attempted = None;
                    emit(
                        app,
                        PlaybackEvent::Track {
                            track_id: track.id.clone(),
                            duration_ms: self.snapshot.duration_ms,
                        },
                    );
                    emit(app, PlaybackEvent::Status { status: PlaybackStatus::Playing });
                    self.save_state();
                    return;
                }
            }
            // 接管失败或曲目已不存在：清空尝试记录，回退普通播放路径
            self.preload_attempted = None;
        }

        if let Some(next_id) = self.next_track_id() {
            if let Some(track) = self.track_map.get(&next_id).cloned() {
                let _ = self.play(&track, None, app);
            }
        } else {
            let _ = self.engine.stop();
            self.snapshot.status = PlaybackStatus::Stopped;
            self.position_offset_ms = 0;
            self.play_started_at = None;
            self.is_paused = false;
            emit(app, PlaybackEvent::Status { status: PlaybackStatus::Stopped });
        }
    }

    /// 预加载下一曲：仅读取下一曲 id（不推进 index），成功则记录 preloaded_track_id 并推送事件。
    /// preload_attempted 记录最近一次尝试（含失败），避免 250ms 时钟重复尝试同一曲目。
    fn maybe_preload_next(&mut self, app: &AppHandle) {
        let Some(next_id) = self.peek_next_track_id() else {
            return;
        };
        if self.preload_attempted.as_deref() == Some(next_id.as_str()) {
            return;
        }
        let Some(track) = self.track_map.get(&next_id).cloned() else {
            self.preload_attempted = Some(next_id);
            return;
        };
        if track.file_path.is_empty() {
            self.preload_attempted = Some(next_id);
            return;
        }
        self.preload_attempted = Some(next_id.clone());
        match self.engine.preload(
            std::path::PathBuf::from(&track.file_path),
            self.replaygain_of(&track),
        ) {
            Ok(()) => {
                self.preloaded_track_id = Some(next_id.clone());
                emit(
                    app,
                    PlaybackEvent::NextPreloaded {
                        track_id: next_id,
                    },
                );
            }
            Err(e) => {
                // 预加载失败：切歌时走普通 play（play_or_skip 会处理跳过）
                eprintln!("[preload] 预加载下一曲失败 {}: {}", track.file_path, e);
            }
        }
    }

    /// 只读地取下一曲 id（不推进 index）。Shuffle 下预加载时随机选定，
    /// on_track_ended 以 preloaded_track_id 为准定位 index，保证两者一致。
    fn peek_next_track_id(&self) -> Option<String> {
        let idx = self.next_index()?;
        self.context
            .as_ref()
            .and_then(|c| c.track_ids.get(idx).cloned())
    }

    fn next_index(&self) -> Option<usize> {
        let ctx = self.context.as_ref()?;
        if ctx.track_ids.is_empty() {
            return None;
        }
        match self.snapshot.mode {
            PlayMode::RepeatOne => Some(self.index),
            PlayMode::Order => {
                if self.index + 1 < ctx.track_ids.len() {
                    Some(self.index + 1)
                } else {
                    None
                }
            }
            PlayMode::RepeatAll => Some((self.index + 1) % ctx.track_ids.len()),
            PlayMode::Shuffle => {
                if ctx.track_ids.len() == 1 {
                    Some(0)
                } else {
                    let mut rng = rand::thread_rng();
                    let mut n = rng.gen_range(0..ctx.track_ids.len());
                    while n == self.index {
                        n = rng.gen_range(0..ctx.track_ids.len());
                    }
                    Some(n)
                }
            }
        }
    }

    fn next_track_id(&mut self) -> Option<String> {
        let idx = self.next_index()?;
        self.index = idx;
        self.context
            .as_ref()
            .and_then(|c| c.track_ids.get(idx).cloned())
    }

    pub fn next(&mut self) -> Option<String> {
        self.next_track_id()
    }

    pub fn previous(&mut self) -> Option<String> {
        let ctx = self.context.as_ref()?;
        if ctx.track_ids.is_empty() {
            return None;
        }
        let idx = if self.index == 0 {
            ctx.track_ids.len() - 1
        } else {
            self.index - 1
        };
        self.index = idx;
        ctx.track_ids.get(idx).cloned()
    }

    pub fn dispose(&mut self) -> AppResult<()> {
        self.engine.stop()?;
        Ok(())
    }
}

impl Default for PlayerHandle {
    fn default() -> Self {
        Self::new(spawn_engine())
    }
}

/// 启动 250ms 进度时钟线程
pub fn spawn_progress_clock(
    handle: Arc<Mutex<PlayerHandle>>,
    app: AppHandle,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(250));
            {
                if let Ok(mut h) = handle.lock() {
                    h.tick(&app);
                }
            }
            // 应用退出时进程终止，线程随之结束
        }
    })
}

fn emit(app: &AppHandle, event: PlaybackEvent) {
    let _ = app.emit("playback://event", &event);
}
