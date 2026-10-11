//! 目录自动监控：notify 监听曲库目录，文件变化增量同步进曲库
//!
//! 方案说明（评估结论）：采用 Rust 侧 notify crate + Tauri 事件推送的实时监听方案。
//! notify 为桌面级跨平台文件监听事实标准（内部按平台选择 ReadDirectoryChangesW /
//! kqueue / inotify），曲库目录变化毫秒级感知；监听线程通过 mpsc 通道把事件汇聚到
//! 工作线程，以 600ms 窗口去抖批量处理（避免批量复制/下载时逐事件刷库）。
//!
//! 处理规则（以磁盘实况为准）：
//!  - 事件路径存在且为音频 → 库中无则入库，已有则重新解析元数据（覆盖重命名/修改）；
//!  - 事件路径为目录 → 递归发现其中音频文件入库（新建目录里已存在的文件不会单独触发文件事件）；
//!  - 事件路径已不存在且库中有记录 → 移除（覆盖删除与重命名的旧路径）。
//!
//! 每次批量同步后更新 WatchStatus 并推送 `watcher://event` 事件（前端据此刷新曲库与状态展示）。

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::library::scan::{discover_audio_files, parse_track, SUPPORTED_EXT};
use crate::library::store::LibraryStore;
use crate::AppState;

/// 事件去抖窗口（毫秒）：窗口内到达的事件合并为一批处理
const FLUSH_WINDOW_MS: u64 = 600;
/// 事件汇聚线程轮询间隔
const POLL_INTERVAL_MS: u64 = 150;

/// 监控状态（供前端设置页展示与命令返回）
#[derive(Debug, Clone, Default, Serialize)]
pub struct WatchStatus {
    /// 监控开关状态
    pub enabled: bool,
    /// 当前实际监控的目录（仅保留存在的有效目录）
    pub dirs: Vec<String>,
    /// 最近一次增量同步时间（本地时间 "YYYY-MM-DD HH:MM:SS"）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event_at: Option<String>,
    /// 最近一次增量同步描述（如"新增/更新 3 首，移除 1 首"）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event_desc: Option<String>,
}

/// 共享监控状态（原子读写，供命令与工作线程共享）
#[derive(Default)]
pub struct WatchState {
    pub status: Mutex<WatchStatus>,
}

/// 监控管理器：持有当前监听线程的停止信号，支持配置变更时平滑重启
pub struct WatchManager {
    stop: Mutex<Option<Arc<AtomicBool>>>,
    state: Arc<WatchState>,
}

impl Default for WatchManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WatchManager {
    pub fn new() -> Self {
        Self {
            stop: Mutex::new(None),
            state: Arc::new(WatchState::default()),
        }
    }

    /// 配置变更时调用：停掉旧监听线程，按新配置启动（目录列表/开关）
    pub fn reconfigure(&self, app: AppHandle, dirs: &[String], enabled: bool) {
        // 停止旧线程（置停止标志后旧线程下一轮退出）
        if let Some(old_stop) = self.stop.lock().unwrap().take() {
            old_stop.store(true, Ordering::SeqCst);
        }
        {
            let mut st = self.state.status.lock().unwrap();
            st.enabled = enabled;
            st.dirs = dirs.to_vec();
        }
        if !enabled || dirs.is_empty() {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        *self.stop.lock().unwrap() = Some(stop.clone());

        let handle = app.clone();
        let state = self.state.clone();
        let dirs = dirs.to_vec();
        std::thread::spawn(move || spawn_watch_loop(handle, dirs, state, stop));
    }

    /// 当前监控状态快照（供命令返回）
    pub fn snapshot(&self) -> WatchStatus {
        self.state.status.lock().unwrap().clone()
    }
}

/// 监听线程主循环：notify 事件 → 通道 → 去抖批量处理
fn spawn_watch_loop(
    handle: AppHandle,
    dirs: Vec<String>,
    state: Arc<WatchState>,
    stop: Arc<AtomicBool>,
) {
    let (ev_tx, ev_rx): (Sender<Vec<PathBuf>>, Receiver<Vec<PathBuf>>) = channel();

    // notify 回调把事件路径投递到通道；watcher 被本线程持有，退出即停止监听
    let mut watcher: RecommendedWatcher = match notify::recommended_watcher(
        move |res: notify::Result<notify::Event>| {
            if let Ok(ev) = res {
                let _ = ev_tx.send(ev.paths);
            }
        },
    ) {
        Ok(w) => w,
        Err(e) => {
            let mut st = state.status.lock().unwrap();
            st.last_event_desc = Some(format!("文件监听启动失败: {e}"));
            return;
        }
    };

    let roots: Vec<PathBuf> = dirs
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect();
    if roots.is_empty() {
        return;
    }
    for root in &roots {
        let _ = watcher.watch(root, RecursiveMode::Recursive);
    }
    // 只保留实际监听成功的有效目录
    {
        let mut st = state.status.lock().unwrap();
        st.dirs = roots.iter().map(|p| p.to_string_lossy().to_string()).collect();
    }

    let mut pending: HashSet<PathBuf> = HashSet::new();
    let mut last_flush = Instant::now();
    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        match ev_rx.recv_timeout(Duration::from_millis(POLL_INTERVAL_MS)) {
            Ok(paths) => {
                for p in paths {
                    pending.insert(p);
                }
                last_flush = Instant::now();
            }
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        if !pending.is_empty() && last_flush.elapsed() >= Duration::from_millis(FLUSH_WINDOW_MS) {
            let batch: Vec<PathBuf> = pending.drain().collect();
            flush_batch(&handle, &state, &batch);
        }
    }
}

/// 批量同步：以磁盘实况为准，入库新增/更新、移除消失的曲目
fn flush_batch(handle: &AppHandle, state: &Arc<WatchState>, batch: &[PathBuf]) {
    let Some(app_state) = handle.try_state::<AppState>() else {
        return;
    };
    let mut library: std::sync::MutexGuard<'_, LibraryStore> = match app_state.library.lock() {
        Ok(g) => g,
        Err(_) => return,
    };

    // 库内路径索引（Windows 路径大小写不敏感，统一小写比较）
    let by_path: HashMap<String, String> = library
        .tracks()
        .iter()
        .map(|t| (t.file_path.to_lowercase(), t.id.clone()))
        .collect();

    let mut upserts: Vec<crate::library::models::Track> = Vec::new();
    let mut removed_ids: Vec<String> = Vec::new();

    for path in batch {
        if path.is_dir() {
            // 新建目录：递归发现其中音频入库（新目录内已有文件不单独触发文件事件）
            for f in discover_audio_files(path) {
                let key = f.to_string_lossy().to_lowercase();
                if !by_path.contains_key(&key) {
                    if let Some(mut track) = parse_track(&f) {
                        // 复用库内已有 id（若大小写变体存在），避免重复入库
                        if let Some(id) = by_path.get(&key) {
                            track.id = id.clone();
                        }
                        upserts.push(track);
                    }
                }
            }
            continue;
        }

        let Some(ext) = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
        else {
            continue;
        };
        if !SUPPORTED_EXT.contains(&ext.as_str()) {
            continue;
        }

        let key = path.to_string_lossy().to_lowercase();
        if path.exists() {
            // 新增 / 修改 / 重命名新路径：入库或更新元数据
            if let Some(mut track) = parse_track(path) {
                if let Some(id) = by_path.get(&key) {
                    track.id = id.clone();
                }
                upserts.push(track);
            }
        } else if let Some(id) = by_path.get(&key) {
            // 删除 / 重命名旧路径：从库中移除
            removed_ids.push(id.clone());
        }
    }

    if !upserts.is_empty() || !removed_ids.is_empty() {
        let desc = if let Err(e) = library.apply_scan(&upserts, &removed_ids) {
            format!("增量同步失败: {e}")
        } else {
            format!("新增/更新 {} 首，移除 {} 首", upserts.len(), removed_ids.len())
        };
        set_status(state, &desc);
        let _ = handle.emit("watcher://event", serde_json::json!({ "desc": desc, "at": now_str() }));
    }
}

fn set_status(state: &Arc<WatchState>, desc: &str) {
    let mut st = state.status.lock().unwrap();
    st.last_event_desc = Some(desc.to_string());
    st.last_event_at = Some(now_str());
}

fn now_str() -> String {
    use chrono::Local;
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}
