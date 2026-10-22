//! TPlayer 后端 lib 入口
//!
//! 业务装配（状态注入、命令注册、插件初始化）集中于此，main.rs 仅调用 run()。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Listener, Manager, RunEvent};

pub mod commands;
pub mod config;
pub mod converter;
pub mod cover;
pub mod error;
pub mod library;
pub mod lyrics;
pub mod player;
pub mod replaygain;
pub mod security;
#[cfg(windows)]
pub mod smtc;
pub mod watch;

use config::AppConfig;
use library::store::LibraryStore;
use player::{spawn_progress_clock, PlaybackStatus, PlayerHandle};

/// 应用全局状态：命令通过 `State<AppState>` 访问
pub struct AppState {
    /// 播放器句柄（Arc<Mutex> 因进度时钟线程需要访问）
    pub player: Arc<Mutex<PlayerHandle>>,
    /// 曲库存储
    pub library: Mutex<LibraryStore>,
    /// 应用配置
    pub config: Mutex<AppConfig>,
    /// 配置文件路径
    pub config_path: PathBuf,
    /// 曲库数据文件路径
    pub library_path: PathBuf,
    /// 歌词偏移持久化文件路径
    pub lyrics_offsets_path: PathBuf,
    /// 目录自动监控管理器
    pub watch: watch::WatchManager,
    /// 音频转换器状态（running/cancel 原子标志）
    pub converter: converter::ConverterState,
    /// AppHandle（用于事件推送，命令层也可直接从参数获取）
    pub app: AppHandle,
}

/// lib 入口：构建 Tauri 应用并运行
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        // 应用自动升级（签名校验由 plugins.updater.pubkey 保证，配置见 tauri.conf.json）
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // 数据目录：Windows 上为 %APPDATA%\com.tplayer（由 identifier 决定）
            let app_data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

            // 彻底改名（com.tplayer.next -> com.tplayer）后，一次性迁移旧数据目录的配置与曲库，
            // 避免用户升级后丢失已扫描的曲目与设置（旧目录保留，旧版若仍安装可继续使用）。
            migrate_legacy_app_data(&app_data_dir);

            let config_path = app_data_dir.join("config.json");
            let library_path = app_data_dir.join("library.db");
            let legacy_library_path = app_data_dir.join("library.json");
            let playback_path = app_data_dir.join("playback_state.json");
            let lyrics_offsets_path = app_data_dir.join("lyrics_offsets.json");

            let config = AppConfig::load(&config_path);
            // 首次启动自动把旧 library.json 导入 SQLite 并备份为 library.json.bak
            let library = LibraryStore::open(library_path.clone(), legacy_library_path)?;

            // 启动引擎线程，返回可发送命令的句柄
            let engine_handle = player::spawn_engine();

            let player_handle = Arc::new(Mutex::new(PlayerHandle::new(engine_handle.clone())));
            // 退出标志：托盘"退出"置为 true，窗口 CloseRequested 据此区分"隐藏到托盘"与"真正退出"
            let should_quit = Arc::new(AtomicBool::new(false));
            // 加载上次会话的播放状态（断点 + 播放习惯），待 player_init 应用
            player_handle
                .lock()
                .unwrap()
                .set_persist_path(playback_path);

            // 从配置下发交叉淡化参数（引擎线程 + 状态机内部配置都要同步：
            // 状态机的 crossfade_enabled/ms 决定连播时是否提前换曲，
            // 只同步引擎会导致"配置已关、状态机仍按 1s 提前切走"的截断）
            {
                let cfg = &config;
                let mut player = player_handle.lock().unwrap();
                player
                    .set_crossfade(cfg.crossfade.enabled, cfg.crossfade.duration_ms)
                    .ok(); // 启动期同步失败不阻断（前端 restore 会再次下发）
            }
            // 从配置下发音量归一化 ReplayGain 开关
            {
                let cfg = &config;
                engine_handle.set_replaygain(cfg.replaygain.enabled);
            }

            let state = AppState {
                player: player_handle.clone(),
                library: Mutex::new(library),
                config: Mutex::new(config),
                config_path,
                library_path,
                lyrics_offsets_path,
                watch: watch::WatchManager::new(),
                converter: converter::ConverterState::new(),
                app: app_handle.clone(),
            };

            app.manage(state);

            // 目录自动监控：按配置启动监听（默认开启，曲库目录白名单变化时增量同步）
            // 注意：config 两项必须先取出再调用（同一表达式里两次 lock 同一 Mutex
            // 会因临时 MutexGuard 存活到语句结束而自死锁，卡死主线程）
            {
                let state = app.state::<AppState>();
                let (dirs, watch_enabled) = {
                    let config = state.config.lock().unwrap();
                    (config.library_dirs.clone(), config.watch.enabled)
                };
                state.watch.reconfigure(app_handle.clone(), &dirs, watch_enabled);
            }

            // 启动期 SACD 分轨时长校正（后台线程，不阻塞首屏）：
            // 时长只能由 sacd_extract -P 给出（DST 压缩区数据量 ≠ 播放时长），旧版本
            // 按字节推算入库的值会一直留在库里，必须重扫才会更新。这里在启动后
            // 重新探测并就地改 duration，升级即修正，无需用户手动重扫。
            // 探测在曲库锁外进行（每碟几十毫秒），只在收集/写回时短暂持锁。
            {
                let app_bg = app_handle.clone();
                std::thread::spawn(move || {
                    let state = app_bg.state::<AppState>();
                    let plan = {
                        let library = state.library.lock().unwrap();
                        library::scan::sacd_duration_plan(&library)
                    };
                    if plan.is_empty() {
                        return;
                    }
                    let fixes = library::scan::sacd_duration_fixes(&plan);
                    if fixes.is_empty() {
                        return;
                    }
                    let updated = state
                        .library
                        .lock()
                        .unwrap()
                        .update_durations(&fixes)
                        .unwrap_or(0);
                    if updated > 0 {
                        eprintln!("[library] 启动期校正 {updated} 条 SACD 分轨时长");
                        // 复用目录监控事件通知前端刷新曲库视图（时长列即时更新）
                        let _ = app_bg.emit(
                            "watcher://event",
                            serde_json::json!({ "changes": [], "reason": "sacd-duration" }),
                        );
                    }
                });
            }

            // 显式给主窗口设置图标：Linux/X11·Wayland 下若不设置，窗口与任务栏
            // 会回退为系统默认（齿轮）图标；安装版另由 .desktop 提供图标。
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(icon) =
                    tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))
                {
                    let _ = window.set_icon(icon);
                }
            }

            // ---- 系统托盘 ----
            // 菜单：播放/暂停、上一首、下一首 | 显示主窗口、退出
            let toggle_item = MenuItem::with_id(app, "tray_toggle", "播放", true, None::<&str>)?;
            let prev_item = MenuItem::with_id(app, "tray_prev", "上一首", true, None::<&str>)?;
            let next_item = MenuItem::with_id(app, "tray_next", "下一首", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let show_item =
                MenuItem::with_id(app, "tray_show", "显示主窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "tray_quit", "退出", true, None::<&str>)?;
            let tray_menu = Menu::with_items(
                app,
                &[
                    &toggle_item,
                    &prev_item,
                    &next_item,
                    &separator,
                    &show_item,
                    &quit_item,
                ],
            )?;

            // 托盘图标直接使用应用自身的 32x32 PNG，避免任何情况下回退到系统默认图标。
            let tray_icon =
                tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))
                    .expect("加载托盘图标失败");

            let should_quit_for_tray = should_quit.clone();
            let tray = TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon)
                .menu(&tray_menu)
                .tooltip("TPlayer")
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "tray_toggle" => {
                        let state = app.state::<AppState>();
                        let mut player = state.player.lock().unwrap();
                        match player.snapshot.status {
                            PlaybackStatus::Playing => {
                                let _ = player.pause(app);
                            }
                            PlaybackStatus::Paused => {
                                let _ = player.resume(app);
                            }
                            _ => {}
                        }
                    }
                    "tray_next" | "tray_prev" => {
                        let state = app.state::<AppState>();
                        let target_id = {
                            let mut player = state.player.lock().unwrap();
                            if event.id().as_ref() == "tray_next" {
                                player.next()
                            } else {
                                player.previous()
                            }
                        };
                        if let Some(id) = target_id {
                            let track = state.library.lock().unwrap().get_track(&id);
                            if let Some(track) = track {
                                let _ = state.player.lock().unwrap().play(&track, None, app);
                            }
                        }
                    }
                    "tray_show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                    "tray_quit" => {
                        let state = app.state::<AppState>();
                        if let Ok(mut player) = state.player.lock() {
                            player.save_state();
                        }
                        // 置位退出标志，避免窗口 CloseRequested 的 prevent_close() 拦截退出
                        should_quit_for_tray.store(true, Ordering::SeqCst);
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    // 左键单击：显示并聚焦主窗口
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // 托盘状态同步：切歌时更新 tooltip（当前曲目名），播放状态变化时更新"播放/暂停"文案
            let tray_for_events = tray.clone();
            let toggle_for_events = toggle_item.clone();
            let app_for_events = app_handle.clone();
            #[cfg(windows)]
            let smtc_status = std::cell::RefCell::new(String::new());
            #[cfg(windows)]
            let smtc_track_id = std::cell::RefCell::new(String::new());
            app_handle.listen("playback://event", move |event| {
                let Ok(value) = serde_json::from_str::<serde_json::Value>(event.payload())
                else {
                    return;
                };
                match value.get("type").and_then(|t| t.as_str()) {
                    Some("track") => {
                        let track_id = value
                            .get("payload")
                            .and_then(|p| p.get("trackId"))
                            .and_then(|t| t.as_str())
                            .unwrap_or_default()
                            .to_string();
                        let state = app_for_events.state::<AppState>();
                        let track = state.library.lock().unwrap().get_track(&track_id);
                        let tooltip = match &track {
                            Some(t) => format!("TPlayer - {}", t.title),
                            None => "TPlayer".to_string(),
                        };
                        let _ = tray_for_events.set_tooltip(Some(&tooltip));
                        #[cfg(windows)]
                        {
                            *smtc_track_id.borrow_mut() = track_id;
                            if let Some(t) = track {
                                smtc::update_playback(
                                    &smtc_status.borrow(),
                                    &t.title,
                                    &t.artists.join(", "),
                                    &t.album,
                                );
                            }
                        }
                    }
                    Some("status") => {
                        let status = value
                            .get("payload")
                            .and_then(|p| p.get("status"))
                            .and_then(|s| s.as_str())
                            .unwrap_or("");
                        let text = if status == "playing" { "暂停" } else { "播放" };
                        let _ = toggle_for_events.set_text(text);
                        #[cfg(windows)]
                        {
                            *smtc_status.borrow_mut() = status.to_string();
                            let tid = smtc_track_id.borrow().clone();
                            if !tid.is_empty() {
                                if let Some(t) = app_for_events
                                    .state::<AppState>()
                                    .library
                                    .lock()
                                    .unwrap()
                                    .get_track(&tid)
                                {
                                    smtc::update_playback(
                                        status,
                                        &t.title,
                                        &t.artists.join(", "),
                                        &t.album,
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                }
            });

            // 窗口关闭时保存播放状态，随后按设置决定"隐藏到托盘驻留"还是"直接退出"
            // （托盘菜单"退出"始终真正退出，置位 should_quit 后不拦截）。
            if let Some(window) = app.get_webview_window("main") {
                let player_for_close = player_handle.clone();
                let window_for_hide = window.clone();
                let should_quit_for_close = should_quit.clone();
                let app_for_close = app_handle.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        if let Ok(mut player) = player_for_close.lock() {
                            player.save_state();
                        }
                        // 托盘"退出"已置位：不拦截，让应用真正退出
                        if should_quit_for_close.load(Ordering::SeqCst) {
                            return;
                        }
                        // 开启"关闭即退出"：读取实时配置，直接退出，不驻留托盘
                        let quit_on_close = app_for_close
                            .state::<AppState>()
                            .config
                            .lock()
                            .map(|c| c.quit_on_close)
                            .unwrap_or(false);
                        if quit_on_close {
                            app_for_close.exit(0);
                            return;
                        }
                        api.prevent_close();
                        let _ = window_for_hide.hide();
                    }
                });
            }

            // Windows 系统媒体控制（任务栏/锁屏/耳机线控）
            #[cfg(windows)]
            smtc::init(app_handle.clone());

            // 启动进度时钟
            spawn_progress_clock(player_handle, app_handle);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 兼容层文件读取（路径白名单同 security::ensure_allowed）
            commands::compat_fs::compat_read_head,
            commands::compat_fs::compat_read_full,
            // 播放
            commands::player::player_init,
            commands::player::player_capabilities,
            commands::player::player_snapshot,
            commands::player::player_play,
            commands::player::player_pause,
            commands::player::player_resume,
            commands::player::player_stop,
            commands::player::player_next,
            commands::player::player_previous,
            commands::player::player_seek,
            commands::player::player_set_volume,
            commands::player::player_set_muted,
            commands::player::player_set_mode,
            commands::player::player_set_crossfade,
            commands::player::player_set_replaygain,
            commands::player::player_set_rate,
            commands::player::player_set_sleep_timer,
            commands::player::player_cancel_sleep_timer,
            commands::player::player_sleep_timer_status,
            commands::player::player_remove_from_queue,
            commands::player::player_dispose,
            // 曲库
            commands::library::library_scan,
            commands::library::library_cancel_scan,
            commands::library::library_tracks,
            commands::library::library_cover,
            commands::library::library_album_cover,
            commands::library::library_artist_cover,
            commands::library::library_albums,
            commands::library::library_artists,
            commands::library::library_stats,
            commands::library::library_cue_albums,
            commands::library::library_sacd_albums,
            commands::library::library_favorite_ids,
            commands::library::library_set_favorite,
            commands::library::library_recent,
            commands::library::library_record_played,
            commands::library::library_remove_tracks,
            commands::library::library_add_dir,
            commands::library::library_remove_dir,
            commands::library::library_import_files,
            commands::library::library_track_info,
            commands::library::library_list_playlists,
            commands::library::library_create_playlist,
            commands::library::library_rename_playlist,
            commands::library::library_delete_playlist,
            commands::library::library_set_playlist_tracks,
            commands::library::library_add_tracks_to_playlist,
            commands::library::library_remove_track_from_playlist,
            // 歌词
            commands::lyrics::lyrics_load,
            commands::lyrics::lyrics_save_offset,
            commands::lyrics::lyrics_clear_cache,
            // 设置
            commands::settings::settings_load,
            commands::settings::settings_save,
            // 目录监控
            commands::watch::watcher_status,
            // 音频转换
            commands::converter::converter_start,
            commands::converter::converter_cancel,
            commands::converter::converter_probe_files,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // 进程退出前的最后兜底：无论窗口关闭、托盘退出还是系统关机/注销
            // 走到运行循环收尾，都把播放状态（毫秒断点 + 队列 + 播放习惯）落盘。
            // 强杀（taskkill /F、断电）无法拦截，由播放中 5s 节流保存兜底。
            if let RunEvent::Exit = event {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    if let Ok(mut player) = state.player.lock() {
                        player.save_state();
                    }
                }
            }
        });
}

/// 一次性迁移旧版本（identifier = com.tplayer.next，数据目录 %APPDATA%\TPlayer Next）的数据到当前目录。
///
/// 触发条件：当前数据目录（%APPDATA%\com.tplayer）尚无 config.json 且旧目录存在。
/// 仅做增量拷贝（不删除旧目录、不覆盖新数据），失败仅记录日志、不影响启动。
fn migrate_legacy_app_data(current: &PathBuf) {
    let Some(parent) = current.parent() else {
        return;
    };
    let legacy = parent.join("TPlayer Next");
    if !legacy.exists() {
        return;
    }
    // 已持有配置则跳过，避免覆盖当前版本数据
    if current.join("config.json").exists() {
        return;
    }
    if let Err(e) = std::fs::create_dir_all(current) {
        eprintln!("[migrate] 创建数据目录失败：{e}");
        return;
    }
    match copy_dir_all(&legacy, current) {
        Ok(_) => eprintln!(
            "[migrate] 已从 %APPDATA%\\TPlayer Next 迁移配置与曲库到新数据目录 {}",
            current.display()
        ),
        Err(e) => eprintln!("[migrate] 迁移旧数据失败（不影响启动）：{e}"),
    }
}

/// 递归拷贝目录（仅复制、不删除目标中已存在的其它文件）。
fn copy_dir_all(src: &PathBuf, dst: &PathBuf) -> std::io::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            std::fs::create_dir_all(&target)?;
            copy_dir_all(&src_path, &target)?;
        } else {
            std::fs::copy(&src_path, &target)?;
        }
    }
    Ok(())
}
