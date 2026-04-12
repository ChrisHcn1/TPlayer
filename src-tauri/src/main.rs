#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use std::sync::Mutex;

mod commands;
mod commands_cue;
mod cue_parser;
mod equalizer;
mod ffmpeg_transcoder;
mod http_server;
use commands::PlayerState;
use tauri_plugin_dialog;
use tauri_plugin_fs;
use tauri::{Emitter, Listener, Manager, tray::{TrayIconBuilder, TrayIconEvent}};

// 日志开关：设置为 false 可禁用所有日志输出
const ENABLE_LOGS: bool = true;

// 条件性日志宏
macro_rules! log_info {
    ($($arg:tt)*) => {
        if ENABLE_LOGS {
            println!($($arg)*);
        }
    };
}

macro_rules! log_error {
    ($($arg:tt)*) => {
        if ENABLE_LOGS {
            eprintln!($($arg)*);
        }
    };
}

fn main() {
    // 创建播放器状态
    let player_state = Arc::new(Mutex::new(PlayerState::default()));

    // 初始化转码缓存
    match ffmpeg_transcoder::init_transcode_cache() {
        Ok(_) => {
            log_info!("转码缓存初始化成功");
            
            // 检查FFmpeg是否可用
            if ffmpeg_transcoder::TranscodeCache::get_ffmpeg_path().is_some() {
                log_info!("FFmpeg检测成功，转码功能已启用");
            } else {
                log_info!("FFmpeg未检测到，转码功能不可用");
            }
            
            // 检查FFplay是否可用
            if ffmpeg_transcoder::TranscodeCache::get_ffplay_path().is_some() {
                log_info!("FFplay检测成功，无损音频播放功能已启用");
            } else {
                log_info!("FFplay未检测到，无损音频播放功能不可用");
            }
        }
        Err(e) => {
            log_error!("初始化转码缓存失败: {}", e);
        }
    }

    // 初始化HTTP服务器
    match http_server::init_http_server() {
        Ok(_) => {
            log_info!("HTTP服务器初始化成功");
        }
        Err(e) => {
            log_error!("初始化HTTP服务器失败: {}", e);
        }
    }

    tauri::Builder::default()
        // 注册命令
        .invoke_handler(tauri::generate_handler![
            commands::scan_directory,
            commands::get_audio_duration,
            commands::minimize_window,
            commands::toggle_maximize_window,
            commands::close_window,
            commands::toggle_window_visibility,
            commands::open_readme,
            commands_cue::scan_cue_files,
            commands_cue::parse_cue_file_command,
            ffmpeg_transcoder::check_needs_transcode,
            ffmpeg_transcoder::pretranscode_audio,
            ffmpeg_transcoder::get_transcoded_path,
            ffmpeg_transcoder::play_with_ffplay,
            ffmpeg_transcoder::stop_ffplay,
            ffmpeg_transcoder::pause_ffplay,
            ffmpeg_transcoder::resume_ffplay,
            ffmpeg_transcoder::seek_ffplay,
            ffmpeg_transcoder::set_ffplay_volume,
            ffmpeg_transcoder::get_ffplay_status,
            http_server::get_file_http_url
        ])
        // 注册插件
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        // 系统托盘
        .setup(|app| {
            let app_handle = app.handle();
            
            // 监听应用退出事件，清理FFplay资源
            let app_handle_for_cleanup = app_handle.clone();
            app.listen("tauri://close-requested", move |_| {
                println!("[应用] 收到退出请求，清理FFplay资源");
                ffmpeg_transcoder::cleanup_ffplay();
            });
            
            // 创建托盘菜单
            let menu = tauri::menu::Menu::with_items(
                app,
                &[
                    &tauri::menu::MenuItem::with_id(app, "show", "显示", true, None::<&str>)?,
                    &tauri::menu::PredefinedMenuItem::separator(app)?,
                    &tauri::menu::MenuItem::with_id(app, "next", "下一首", true, None::<&str>)?,
                    &tauri::menu::MenuItem::with_id(app, "play_pause", "播放/暂停", true, None::<&str>)?,
                    &tauri::menu::MenuItem::with_id(app, "previous", "上一首", true, None::<&str>)?,
                    &tauri::menu::PredefinedMenuItem::separator(app)?,
                    &tauri::menu::MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?,
                ],
            )?;

            // 创建托盘图标并保存，防止被释放
            let tray_icon = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(move |app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "next" => {
                            // 触发下一首事件
                            let _ = app.emit("tray-next-song", ());
                        }
                        "play_pause" => {
                            // 触发播放/暂停事件
                            let _ = app.emit("play-pause", ());
                        }
                        "previous" => {
                            // 触发上一首事件
                            let _ = app.emit("tray-previous-song", ());
                        }
                        "quit" => {
                            // 清理FFplay资源
                            ffmpeg_transcoder::cleanup_ffplay();
                            // 退出应用
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(move |tray, event| {
                    match event {
                        TrayIconEvent::Click { button, .. } => {
                            log_info!("系统托盘点击事件: {:?}", button);
                            // 只有左键点击才显示/隐藏窗口
                            if button == tauri::tray::MouseButton::Left {
                                if let Some(window) = tray.app_handle().get_webview_window("main") {
                                    // 尝试获取窗口可见性
                                    match window.is_visible() {
                                        Ok(is_visible) => {
                                            log_info!("窗口当前可见性: {}", is_visible);
                                            if is_visible {
                                                log_info!("隐藏窗口");
                                                let _ = window.hide();
                                            } else {
                                                log_info!("显示窗口并设置焦点");
                                                let _ = window.show();
                                                let _ = window.set_focus();
                                            }
                                        }
                                        Err(e) => {
                                            log_error!("获取窗口可见性失败: {}", e);
                                            // 窗口可能已损坏，尝试重新显示
                                            let _ = window.show();
                                            let _ = window.set_focus();
                                        }
                                    }
                                } else {
                                    log_error!("窗口不存在，无法显示");
                                }
                            }
                            // 右键点击会自动显示菜单，不需要处理
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // 保存托盘图标到应用状态
            let tray_icon_handle = app_handle.clone();
            app.manage(tray_icon);

            // 监听更新托盘菜单事件
            let _ = app_handle.listen("update-tray-menu", move |event| {
                let payload_str = event.payload();
                if let Ok(payload) = serde_json::from_str::<serde_json::Value>(payload_str) {
                    // 获取托盘图标实例
                    let tray_icon = tray_icon_handle.state::<tauri::tray::TrayIcon>();
                    // 创建新的菜单
                    if let Ok(menu) = tauri::menu::Menu::with_items(
                        &tray_icon_handle,
                        &[
                            &tauri::menu::MenuItem::with_id(
                                &tray_icon_handle,
                                "show",
                                payload.get("show").and_then(|v| v.as_str()).unwrap_or("显示"),
                                true,
                                None::<&str>,
                            ).unwrap(),
                            &tauri::menu::PredefinedMenuItem::separator(&tray_icon_handle).unwrap(),
                            &tauri::menu::MenuItem::with_id(
                                &tray_icon_handle,
                                "next",
                                payload.get("next").and_then(|v| v.as_str()).unwrap_or("下一首"),
                                true,
                                None::<&str>,
                            ).unwrap(),
                            &tauri::menu::MenuItem::with_id(
                                &tray_icon_handle,
                                "play_pause",
                                payload.get("play_pause").and_then(|v| v.as_str()).unwrap_or("播放/暂停"),
                                true,
                                None::<&str>,
                            ).unwrap(),
                            &tauri::menu::MenuItem::with_id(
                                &tray_icon_handle,
                                "previous",
                                payload.get("previous").and_then(|v| v.as_str()).unwrap_or("上一首"),
                                true,
                                None::<&str>,
                            ).unwrap(),
                            &tauri::menu::PredefinedMenuItem::separator(&tray_icon_handle).unwrap(),
                            &tauri::menu::MenuItem::with_id(
                                &tray_icon_handle,
                                "quit",
                                payload.get("quit").and_then(|v| v.as_str()).unwrap_or("退出"),
                                true,
                                None::<&str>,
                            ).unwrap(),
                        ],
                    ) {
                        // 更新托盘菜单
                        let _ = tray_icon.set_menu(Some(menu));
                    }
                }
            });

            Ok(())
        })
        // 提供状态给命令
        .manage(player_state)
        // 运行应用
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
