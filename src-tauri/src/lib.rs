//! TPlayer Next 后端 lib 入口
//!
//! 业务装配（状态注入、命令注册、插件初始化）集中于此，main.rs 仅调用 run()。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Listener, Manager};

pub mod commands;
pub mod config;
pub mod cover;
pub mod error;
pub mod library;
pub mod lyrics;
pub mod player;
pub mod security;

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

            // 数据目录：Windows 上为 %APPDATA%\TPlayer Next
            let app_data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

            let config_path = app_data_dir.join("config.json");
            let library_path = app_data_dir.join("library.db");
            let legacy_library_path = app_data_dir.join("library.json");
            let playback_path = app_data_dir.join("playback_state.json");
            let lyrics_offsets_path = app_data_dir.join("lyrics_offsets.json");

            let config = AppConfig::load(&config_path);
            // 首次启动自动把旧 library.json 导入 SQLite 并备份为 library.json.bak
            let library = LibraryStore::open(library_path.clone(), legacy_library_path)?;

            let player_handle = Arc::new(Mutex::new(PlayerHandle::new()));
            // 退出标志：托盘"退出"置为 true，窗口 CloseRequested 据此区分"隐藏到托盘"与"真正退出"
            let should_quit = Arc::new(AtomicBool::new(false));
            // 加载上次会话的播放状态（断点 + 播放习惯），待 player_init 应用
            player_handle
                .lock()
                .unwrap()
                .set_persist_path(playback_path);

            let state = AppState {
                player: player_handle.clone(),
                library: Mutex::new(library),
                config: Mutex::new(config),
                config_path,
                library_path,
                lyrics_offsets_path,
                app: app_handle.clone(),
            };

            app.manage(state);

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

            let tray_icon = app
                .default_window_icon()
                .cloned()
                .unwrap_or_else(|| {
                    tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))
                        .expect("加载托盘图标失败")
                });

            let should_quit_for_tray = should_quit.clone();
            let tray = TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon)
                .menu(&tray_menu)
                .tooltip("TPlayer Next")
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
                        let title = state
                            .library
                            .lock()
                            .unwrap()
                            .get_track(&track_id)
                            .map(|t| t.title.clone());
                        let tooltip = match title {
                            Some(title) => format!("TPlayer Next - {}", title),
                            None => "TPlayer Next".to_string(),
                        };
                        let _ = tray_for_events.set_tooltip(Some(&tooltip));
                    }
                    Some("status") => {
                        let status = value
                            .get("payload")
                            .and_then(|p| p.get("status"))
                            .and_then(|s| s.as_str())
                            .unwrap_or("");
                        let text = if status == "playing" { "暂停" } else { "播放" };
                        let _ = toggle_for_events.set_text(text);
                    }
                    _ => {}
                }
            });

            // 窗口关闭时保存播放状态，并隐藏到托盘而非退出（托盘菜单"退出"才真正退出）
            if let Some(window) = app.get_webview_window("main") {
                let player_for_close = player_handle.clone();
                let window_for_hide = window.clone();
                let should_quit_for_close = should_quit.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        if let Ok(mut player) = player_for_close.lock() {
                            player.save_state();
                        }
                        // 托盘"退出"已置位：不拦截，让应用真正退出
                        if should_quit_for_close.load(Ordering::SeqCst) {
                            return;
                        }
                        api.prevent_close();
                        let _ = window_for_hide.hide();
                    }
                });
            }

            // 启动进度时钟
            spawn_progress_clock(player_handle, app_handle);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
            commands::library::library_favorite_ids,
            commands::library::library_set_favorite,
            commands::library::library_recent,
            commands::library::library_record_played,
            commands::library::library_remove_tracks,
            commands::library::library_add_dir,
            commands::library::library_remove_dir,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
