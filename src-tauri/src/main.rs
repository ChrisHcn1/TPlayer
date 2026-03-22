#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use std::sync::Mutex;

mod commands;
mod equalizer;
mod ffmpeg_transcoder;
use commands::PlayerState;
use tauri_plugin_dialog;
use tauri_plugin_fs;
use tauri::{Emitter, Manager, tray::{TrayIconBuilder, TrayIconEvent}};

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
        }
        Err(e) => {
            log_error!("初始化转码缓存失败: {}", e);
        }
    }

    tauri::Builder::default()
        // 注册命令
        .invoke_handler(tauri::generate_handler![
            commands::scan_directory,
            commands::play_song,
            commands::pause_song,
            commands::resume_song,
            commands::stop_song,
            commands::set_volume,
            commands::seek_song,
            commands::set_equalizer,
            commands::apply_equalizer_preset,
            commands::get_position,
            commands::cleanup_player,
            commands::minimize_window,
            commands::toggle_maximize_window,
            commands::close_window,
            commands::toggle_window_visibility,
            ffmpeg_transcoder::check_needs_transcode,
            ffmpeg_transcoder::pretranscode_audio,
            ffmpeg_transcoder::get_transcoded_path
        ])
        // 注册插件
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        // 系统托盘
        .setup(|app| {
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

            // 忽略 unused 警告，我们需要保持 tray_icon 的生命周期
            std::mem::forget(tray_icon);

            Ok(())
        })
        // 提供状态给命令
        .manage(player_state)
        // 运行应用
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
