#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use std::sync::Mutex;

mod commands;
mod commands_cache;
mod commands_cue;
mod cue_parser;
mod equalizer;
mod ffmpeg_transcoder;
mod http_server;
mod audio_converter;
mod updater;
use commands::PlayerState;
use tauri_plugin_dialog;
use tauri_plugin_fs;
use tauri::{Emitter, Listener, Manager, tray::{TrayIconBuilder, TrayIconEvent}, WindowEvent};

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
            commands::check_audio_environment,
            audio_converter::convert_audio,
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
            ffmpeg_transcoder::play_audio_with_ffmpeg,
            ffmpeg_transcoder::set_ffplay_volume,
            ffmpeg_transcoder::get_ffplay_status,
            ffmpeg_transcoder::get_ffplay_path,
            ffmpeg_transcoder::get_custom_ffmpeg_path_command,
            http_server::get_file_http_url,
            http_server::check_http_server_status,
            http_server::open_devtools,
            commands_cache::save_to_cache,
            commands_cache::get_cached_file,
            commands_cache::get_cache_dir,
            commands_cache::clear_cache,
            updater::check_update_manual,
            updater::check_update_auto,
            updater::download_update_command,
            updater::verify_update_command,
            updater::install_update_command,
            updater::get_current_version
        ])
        // 注册插件
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        // 系统托盘
        .setup(|app| {
            let app_handle = app.handle();

            // 监听窗口关闭事件（Tauri v2 正确 API）
            let app_handle_for_close = app_handle.clone();
            if let Some(window) = app_handle.get_webview_window("main") {
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { .. } = event {
                        println!("[应用] 收到窗口关闭请求，清理资源");
                        commands::cleanup_player_resources();
                        ffmpeg_transcoder::cleanup_ffplay();
                        let _ = app_handle_for_close.exit(0);
                    }
                });
            }

            // 创建托盘菜单 — 失败时不阻止应用启动
            let menu_result = tauri::menu::Menu::with_items(
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
            );

            // 如果托盘菜单创建失败，记录错误但不阻止应用启动
            let menu = match menu_result {
                Ok(m) => m,
                Err(e) => {
                    log_error!("创建托盘菜单失败（应用将继续启动）: {}", e);
                    return Ok(());
                }
            };

            // 获取默认窗口图标 — 使用安全的方式，避免 unwrap() panic
            let icon = app.default_window_icon().cloned().unwrap_or_else(|| {
                log_error!("默认窗口图标未找到，使用空图标继续");
                tauri::image::Image::new(&[], 0, 0)
            });

            // 创建托盘图标 — 失败时不阻止应用启动
            let tray_result = TrayIconBuilder::new()
                .icon(icon)
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
                            let _ = app.emit("tray-next-song", ());
                        }
                        "play_pause" => {
                            let _ = app.emit("play-pause", ());
                        }
                        "previous" => {
                            let _ = app.emit("tray-previous-song", ());
                        }
                        "quit" => {
                            println!("[应用] 用户点击托盘退出");
                            commands::cleanup_player_resources();
                            ffmpeg_transcoder::cleanup_ffplay();
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(move |tray, event| {
                    match event {
                        TrayIconEvent::Click { button, .. } => {
                            log_info!("系统托盘点击事件: {:?}", button);
                            if button == tauri::tray::MouseButton::Left {
                                if let Some(window) = tray.app_handle().get_webview_window("main") {
                                    match window.is_visible() {
                                        Ok(is_visible) => {
                                            log_info!("窗口当前可见性: {}", is_visible);
                                            if is_visible {
                                                let _ = window.hide();
                                            } else {
                                                let _ = window.show();
                                                let _ = window.set_focus();
                                            }
                                        }
                                        Err(e) => {
                                            log_error!("获取窗口可见性失败: {}", e);
                                            let _ = window.show();
                                            let _ = window.set_focus();
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                })
                .build(app);

            // 如果托盘图标创建失败，记录错误但不阻止应用启动
            match tray_result {
                Ok(tray_icon) => {
                    let tray_icon_handle = app_handle.clone();
                    app.manage(tray_icon);

                    // 监听更新托盘菜单事件
                    let _ = app_handle.listen("update-tray-menu", move |event| {
                        let payload_str = event.payload();
                        if let Ok(payload) = serde_json::from_str::<serde_json::Value>(payload_str) {
                            let tray_icon = tray_icon_handle.state::<tauri::tray::TrayIcon>();
                            if let Ok(menu) = tauri::menu::Menu::with_items(
                                &tray_icon_handle,
                                &[
                                    &tauri::menu::MenuItem::with_id(
                                        &tray_icon_handle,
                                        "show",
                                        payload.get("show").and_then(|v| v.as_str()).unwrap_or("显示"),
                                        true,
                                        None::<&str>,
                                    ).unwrap_or_else(|_| tauri::menu::MenuItem::with_id(&tray_icon_handle, "show", "显示", true, None::<&str>).unwrap()),
                                    &tauri::menu::PredefinedMenuItem::separator(&tray_icon_handle).unwrap_or_else(|_| tauri::menu::PredefinedMenuItem::separator(&tray_icon_handle).unwrap()),
                                    &tauri::menu::MenuItem::with_id(
                                        &tray_icon_handle,
                                        "next",
                                        payload.get("next").and_then(|v| v.as_str()).unwrap_or("下一首"),
                                        true,
                                        None::<&str>,
                                    ).unwrap_or_else(|_| tauri::menu::MenuItem::with_id(&tray_icon_handle, "next", "下一首", true, None::<&str>).unwrap()),
                                    &tauri::menu::MenuItem::with_id(
                                        &tray_icon_handle,
                                        "play_pause",
                                        payload.get("play_pause").and_then(|v| v.as_str()).unwrap_or("播放/暂停"),
                                        true,
                                        None::<&str>,
                                    ).unwrap_or_else(|_| tauri::menu::MenuItem::with_id(&tray_icon_handle, "play_pause", "播放/暂停", true, None::<&str>).unwrap()),
                                    &tauri::menu::MenuItem::with_id(
                                        &tray_icon_handle,
                                        "previous",
                                        payload.get("previous").and_then(|v| v.as_str()).unwrap_or("上一首"),
                                        true,
                                        None::<&str>,
                                    ).unwrap_or_else(|_| tauri::menu::MenuItem::with_id(&tray_icon_handle, "previous", "上一首", true, None::<&str>).unwrap()),
                                    &tauri::menu::PredefinedMenuItem::separator(&tray_icon_handle).unwrap_or_else(|_| tauri::menu::PredefinedMenuItem::separator(&tray_icon_handle).unwrap()),
                                    &tauri::menu::MenuItem::with_id(
                                        &tray_icon_handle,
                                        "quit",
                                        payload.get("quit").and_then(|v| v.as_str()).unwrap_or("退出"),
                                        true,
                                        None::<&str>,
                                    ).unwrap_or_else(|_| tauri::menu::MenuItem::with_id(&tray_icon_handle, "quit", "退出", true, None::<&str>).unwrap()),
                                ],
                            ) {
                                let _ = tray_icon.set_menu(Some(menu));
                            }
                        }
                    });
                }
                Err(e) => {
                    log_error!("创建托盘图标失败（应用将继续启动）: {}", e);
                }
            }

            Ok(())
        })
        // 提供状态给命令
        .manage(player_state)
        // 运行应用
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            log_error!("应用启动失败: {}", e);
            // 在 MSIX 环境中，将错误写入事件日志以便诊断
            std::process::exit(1);
        });
}
