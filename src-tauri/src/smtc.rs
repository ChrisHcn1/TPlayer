#![cfg(windows)]
//! Windows 系统媒体控制（SMTC / 任务栏 / 锁屏 / 耳机线控）
//!
//! 方案：创建一个内存 `MediaPlayer` 作为 SMTC 代理，通过它的
//! `SystemMediaTransportControls` 上报元数据/播放状态并接收按钮事件。
//! 所有 WinRT 调用都集中在专用 STA 线程（搭配消息泵），避免跨 apartment 调用错误；
//! 按钮事件经 Tauri `app.emit("smtc://button", {action})` 转发到前端 player store 处理，
//! 复用前端的播放控制逻辑（play/pause/next/previous/stop）。

use std::sync::mpsc;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

use tauri::AppHandle;
use tauri::Emitter;
use windows::core::*;
use windows::Foundation::TypedEventHandler;
use windows::Media::MediaPlaybackStatus;
use windows::Media::MediaPlaybackType;
use windows::Media::Playback::MediaPlayer;
use windows::Media::SystemMediaTransportControls;
use windows::Media::SystemMediaTransportControlsButton;
use windows::Media::SystemMediaTransportControlsButtonPressedEventArgs;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, PeekMessageW, PM_REMOVE, TranslateMessage, MSG,
};

/// 主线程/任意线程发给 SMTC 线程的更新指令
enum SmtcCmd {
    Metadata { title: String, artist: String, album: String },
    Status(MediaPlaybackStatus),
}

/// 跨线程发送到 SMTC 线程的指令通道
static TX: OnceLock<mpsc::Sender<SmtcCmd>> = OnceLock::new();

/// 初始化 SMTC（仅 Windows 调用）。spawn 一个 STA 线程持有 MediaPlayer 并泵消息。
pub fn init(app: AppHandle) {
    let (tx, rx) = mpsc::channel();
    let _ = TX.set(tx);

    thread::spawn(move || {
        unsafe {
            // MediaPlayer 需要 COM 已初始化；若当前线程已是 STA 则忽略错误
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }

        let player = match MediaPlayer::new() {
            Ok(p) => p,
            // 创建失败（极少见：系统媒体服务不可用）时静默退出——
            // SMTC 是可选集成，失败不影响播放，也不向控制台刷诊断输出
            Err(_) => return,
        };
        let controls = match player.SystemMediaTransportControls() {
            Ok(c) => c,
            Err(_) => return,
        };

        let _ = controls.SetIsEnabled(true);
        let _ = controls.SetIsPlayEnabled(true);
        let _ = controls.SetIsPauseEnabled(true);
        let _ = controls.SetIsNextEnabled(true);
        let _ = controls.SetIsPreviousEnabled(true);
        let _ = controls.SetIsStopEnabled(true);
        if let Ok(updater) = controls.DisplayUpdater() {
            let _ = updater.SetType(MediaPlaybackType::Music);
        }

        // 按钮回调：仅转发到前端，不在此处直接操控播放（复用前端控制逻辑）
        let app_for_btn = app.clone();
        let handler = TypedEventHandler::<
            SystemMediaTransportControls,
            SystemMediaTransportControlsButtonPressedEventArgs,
        >::new(move |_sender, args| {
            if let Some(args) = args.as_ref() {
                if let Ok(btn) = args.Button() {
                    let action = match btn {
                        SystemMediaTransportControlsButton::Play => "play",
                        SystemMediaTransportControlsButton::Pause => "pause",
                        SystemMediaTransportControlsButton::Next => "next",
                        SystemMediaTransportControlsButton::Previous => "previous",
                        SystemMediaTransportControlsButton::Stop => "stop",
                        _ => return Ok(()),
                    };
                    let _ = app_for_btn.emit(
                        "smtc://button",
                        serde_json::json!({ "action": action }),
                    );
                }
            }
            Ok(())
        });
        // 注册失败时媒体键不可用，静默降级（不影响应用内播放控制）
        let _ = controls.ButtonPressed(Some(&handler));

        // 消息泵 + 指令处理：所有 WinRT 调用都在本 STA 线程
        let mut msg = MSG::default();
        loop {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    SmtcCmd::Metadata {
                        title,
                        artist,
                        album,
                    } => {
                        if let Ok(updater) = controls.DisplayUpdater() {
                            if let Ok(music) = updater.MusicProperties() {
                                let _ = music.SetTitle(&HSTRING::from(title));
                                let _ = music.SetArtist(&HSTRING::from(artist));
                                let _ = music.SetAlbumTitle(&HSTRING::from(album));
                            }
                            let _ = updater.Update();
                        }
                    }
                    SmtcCmd::Status(s) => {
                        let _ = controls.SetPlaybackStatus(s);
                    }
                }
            }
            unsafe {
                if PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    let _ = DispatchMessageW(&msg);
                }
            }
            thread::sleep(Duration::from_millis(16));
        }
    });
}

/// 由播放状态事件驱动：更新播放状态与当前曲目元数据
pub fn update_playback(status: &str, title: &str, artist: &str, album: &str) {
    let smtc_status = match status {
        "playing" => MediaPlaybackStatus::Playing,
        "paused" => MediaPlaybackStatus::Paused,
        "stopped" | "error" => MediaPlaybackStatus::Stopped,
        _ => MediaPlaybackStatus::Closed,
    };
    if let Some(tx) = TX.get() {
        let _ = tx.send(SmtcCmd::Status(smtc_status));
        let _ = tx.send(SmtcCmd::Metadata {
            title: title.to_string(),
            artist: artist.to_string(),
            album: album.to_string(),
        });
    }
}
