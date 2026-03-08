use base64::Engine;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;
use walkdir::WalkDir;

mod audio_decoder;
mod special_decoder;
mod ffmpeg_decoder;
mod equalizer;
mod transcoder;

// 存储启动参数
static STARTUP_ARGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Track {
  pub id: String,
  pub title: String,
  pub artist: String,
  pub album: String,
  pub duration: String,
  pub path: String,
  pub lyrics: Option<String>,
  pub album_art: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlayerState {
  pub tracks: Vec<Track>,
  pub current_track_index: i32,
  pub current_time: f64,
  pub is_playing: bool,
  pub play_mode: i32,
}

#[derive(Debug, Serialize)]
pub struct PlaybackInfo {
  pub is_playing: bool,
  pub is_empty: bool,
  pub current_time: f64,
  pub duration: f64,
}

pub struct AudioPlayer {
  _stream: Option<OutputStream>,
  _stream_handle: Option<OutputStreamHandle>,
  sink: Option<Sink>,
  current_path: Option<String>,
  duration: f64,
  start_time: Option<std::time::Instant>,
  paused_at: f64,
  equalizer_bands: Vec<f32>, // 均衡器频段增益
  volume: f32, // 音量 (0.0-1.0)
  repeat_one: bool, // 单曲循环
  playback_speed: f32, // 播放速度 (0.5-2.0)
  muted: bool, // 静音
  mute_volume: f32, // 静音前的音量
  ffmpeg_enabled: bool, // 是否启用 ffmpeg 引擎
  equalizer_enabled: bool, // 是否启用均衡器
  crossfade_duration: u64, // 交叉淡入淡出时长（毫秒）
  save_playback_progress: bool, // 是否保存播放进度
  default_play_mode: i32, // 默认播放模式
  transcoder: Option<transcoder::Transcoder>, // 转码管理器
  transcoder_enabled: bool, // 是否启用转码
}

impl AudioPlayer {
  fn new() -> Self {
    // 创建转码缓存目录
    let cache_dir = std::env::temp_dir().join("tplayer_transcode_cache");
    let mut transcoder = transcoder::Transcoder::new(cache_dir);
    
    // 默认启用转码
    transcoder.set_enabled(true);
    
    AudioPlayer {
      _stream: None,
      _stream_handle: None,
      sink: None,
      current_path: None,
      duration: 0.0,
      start_time: None,
      paused_at: 0.0,
      equalizer_bands: vec![0.0; 10], // 默认 10 频段均衡器
      volume: 0.7, // 默认音量 70%
      repeat_one: false, // 默认关闭单曲循环
      playback_speed: 1.0, // 默认播放速度 1.0
      muted: false, // 默认不静音
      mute_volume: 0.7, // 默认静音前音量 70%
      ffmpeg_enabled: false, // 默认关闭 ffmpeg 引擎
      equalizer_enabled: true, // 默认启用均衡器
      crossfade_duration: 3000, // 默认交叉淡入淡出时长 3000ms
      save_playback_progress: true, // 默认保存播放进度
      default_play_mode: 1, // 默认随机播放
      transcoder: Some(transcoder),
      transcoder_enabled: true, // 默认启用转码
    }
  }

  fn play(&mut self, path: &str) -> Result<f64, String> {
    if self.current_path.as_deref() == Some(path) && self.sink.as_ref().map_or(false, |s| !s.empty()) {
      if let Some(sink) = &self.sink {
        sink.play();
        self.start_time = Some(std::time::Instant::now());
        return Ok(self.duration);
      }
    }

    // 第一步：尝试直接播放（使用 FFmpeg 引擎或 rodio）
    let direct_play_result = self.try_play_direct(path);
    
    match direct_play_result {
      Ok(duration) => return Ok(duration),
      Err(direct_error) => {
        log::warn!("直接播放失败: {} - {}", path, direct_error);
        
        // 检查是否是格式不支持错误
        let is_unsupported_format = direct_error.starts_with("UNSUPPORTED_FORMAT:");
        let format_ext = if is_unsupported_format {
          direct_error.strip_prefix("UNSUPPORTED_FORMAT:").unwrap_or("")
        } else {
          ""
        };
        
        // 第二步：检查是否需要转码
        if let Some(ref transcoder) = self.transcoder {
          if transcoder.needs_transcode(path) {
            if self.transcoder_enabled {
              // 转码已启用，检查转码状态
              if let Some(transcoded_path) = transcoder.get_transcoded_path(path) {
                log::info!("使用转码后的文件播放: {}", transcoded_path);
                return self.try_play_direct(&transcoded_path);
              } else {
                // 检查转码状态
                match transcoder.get_task_status(path) {
                  Some(transcoder::TranscodeStatus::InProgress) => {
                    return Err("正在转码中，请稍后再试".to_string());
                  }
                  Some(transcoder::TranscodeStatus::Pending) => {
                    return Err("等待转码中，请稍后再试".to_string());
                  }
                  _ => {
                    // 开始转码
                    if let Err(e) = transcoder.start_transcode(path.to_string()) {
                      return Err(format!("无法开始转码: {}", e));
                    }
                    return Err("开始转码，请稍后再试".to_string());
                  }
                }
              }
            } else {
              // 需要转码但未启用转码功能
              return Err(format!("此格式需要转码后才能播放: {}。请在设置中启用'转码功能'，系统将自动转码为FLAC格式。", format_ext));
            }
          }
        }
        
        // 第三步：如果启用了 FFmpeg 但播放失败，且未启用转码或不需要转码，返回跳过错误
        if self.ffmpeg_enabled {
          return Err(format!("SKIP:{}", direct_error));
        }
        
        // 返回原始错误（如果不是格式不支持错误）
        if is_unsupported_format {
          return Err(format!("此格式需要转码后才能播放: {}。请在设置中启用'转码功能'，系统将自动转码为FLAC格式。", format_ext));
        }
        Err(direct_error)
      }
    }
  }
  
  /// 尝试直接播放音频文件（不经过转码）
  fn try_play_direct(&mut self, path: &str) -> Result<f64, String> {
    // 捕获解码过程中可能的 panic
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      // 使用自定义音频解码器
      let source = audio_decoder::try_open_audio(path);
      match source {
        Err(e) => Err(e),
        Ok(source) => {
          // 应用均衡器
          let mut equalized_source = equalizer::Equalizer::new(source);
          equalized_source.set_bands(&self.equalizer_bands);
          
          let duration = equalized_source.total_duration().map(|d| d.as_secs_f64()).unwrap_or(0.0);
          
          // 使用默认输出流配置
          let stream_result = OutputStream::try_default();
          if let Err(e) = stream_result {
            return Err(e.to_string());
          }
          let (stream, stream_handle) = stream_result.unwrap();
          
          let sink_result = Sink::try_new(&stream_handle);
          if let Err(e) = sink_result {
            return Err(e.to_string());
          }
          let sink = sink_result.unwrap();
          
          // 设置音量
          if self.muted {
              sink.set_volume(0.0);
          } else {
              sink.set_volume(self.volume);
          }
          
          // 添加淡入效果（使用配置的交叉淡入淡出时长）
          let fade_in_duration = Duration::from_millis(self.crossfade_duration);
          let faded_source = equalized_source.fade_in(fade_in_duration);
          
          // 应用播放速度
          let speed_adjusted_source = faded_source.speed(self.playback_speed);
          
          sink.append(speed_adjusted_source);
          sink.play();
          
          self._stream = Some(stream);
          self._stream_handle = Some(stream_handle);
          self.sink = Some(sink);
          self.current_path = Some(path.to_string());
          self.duration = duration;
          self.start_time = Some(std::time::Instant::now());
          self.paused_at = 0.0;
          
          Ok(duration)
        }
      }
    }));

    match result {
      Ok(Ok(duration)) => Ok(duration),
      Ok(Err(e)) => Err(e),
      Err(panic) => {
        // 处理 panic，返回错误信息
        let error_msg = match panic.downcast_ref::<String>() {
          Some(s) => s.to_string(),
          None => match panic.downcast_ref::<&str>() {
            Some(s) => s.to_string(),
            None => "解码器崩溃".to_string(),
          },
        };
        Err(format!("播放失败: {}", error_msg))
      }
    }
  }

  fn pause(&mut self) -> Result<(), String> {
    if let Some(sink) = &self.sink {
      sink.pause();
      if let Some(start) = self.start_time {
        self.paused_at += start.elapsed().as_secs_f64();
      }
      self.start_time = None;
      Ok(())
    } else {
      Err("没有正在播放的音频".to_string())
    }
  }

  fn resume(&mut self) -> Result<(), String> {
    if let Some(sink) = &self.sink {
      sink.play();
      self.start_time = Some(std::time::Instant::now());
      Ok(())
    } else {
      Err("没有正在播放的音频".to_string())
    }
  }

  fn stop(&mut self) -> Result<(), String> {
    if let Some(sink) = &self.sink {
      sink.stop();
      self._stream = None;
      self._stream_handle = None;
      self.sink = None;
      self.current_path = None;
      self.duration = 0.0;
      self.start_time = None;
      self.paused_at = 0.0;
      Ok(())
    } else {
      Err("没有正在播放的音频".to_string())
    }
  }

  #[allow(dead_code)]
  fn fade_out_and_stop(&mut self) -> Result<(), String> {
    if let Some(sink) = &self.sink {
      // 立即停止，不使用阻塞的淡出效果
      sink.stop();
      self._stream = None;
      self._stream_handle = None;
      self.sink = None;
      self.current_path = None;
      self.duration = 0.0;
      self.start_time = None;
      self.paused_at = 0.0;
      Ok(())
    } else {
      Err("没有正在播放的音频".to_string())
    }
  }

  #[allow(dead_code)]
  fn set_volume(&self, volume: f32) -> Result<(), String> {
    if let Some(sink) = &self.sink {
      sink.set_volume(volume);
      Ok(())
    } else {
      Err("没有正在播放的音频".to_string())
    }
  }

  fn seek(&mut self, _position: f64) -> Result<(), String> {
    if let Some(_sink) = &self.sink {
      // 在 rodio 0.17 中，Sink 没有 try_seek 方法
      // 我们需要重新实现播放逻辑来支持跳转
      Err("跳转功能暂不支持".to_string())
    } else {
      Err("没有正在播放的音频".to_string())
    }
  }

  fn get_current_time(&self) -> f64 {
    if self.is_playing() {
      if let Some(start) = self.start_time {
        let elapsed = start.elapsed().as_secs_f64();
        return (self.paused_at + elapsed).min(self.duration);
      }
    }
    self.paused_at
  }

  fn is_playing(&self) -> bool {
    self.sink.as_ref().map_or(false, |s| !s.is_paused())
  }

  fn is_empty(&self) -> bool {
    self.sink.as_ref().map_or(true, |s| s.empty())
  }

  fn get_info(&self) -> PlaybackInfo {
    PlaybackInfo {
      is_playing: self.is_playing(),
      is_empty: self.is_empty(),
      current_time: self.get_current_time(),
      duration: self.duration,
    }
  }

  fn toggle_repeat_one(&mut self) -> bool {
    self.repeat_one = !self.repeat_one;
    self.repeat_one
  }

  fn set_repeat_one(&mut self, repeat: bool) {
    self.repeat_one = repeat;
  }

  fn get_repeat_one(&self) -> bool {
    self.repeat_one
  }

  fn toggle_ffmpeg(&mut self) -> bool {
    self.ffmpeg_enabled = !self.ffmpeg_enabled;
    self.ffmpeg_enabled
  }

  fn set_ffmpeg(&mut self, enabled: bool) {
    self.ffmpeg_enabled = enabled;
  }

  fn get_ffmpeg(&self) -> bool {
    self.ffmpeg_enabled
  }

  fn toggle_equalizer(&mut self) -> bool {
    self.equalizer_enabled = !self.equalizer_enabled;
    self.equalizer_enabled
  }

  fn set_equalizer(&mut self, enabled: bool) {
    self.equalizer_enabled = enabled;
  }

  fn get_equalizer(&self) -> bool {
    self.equalizer_enabled
  }

  fn set_playback_speed(&mut self, speed: f32) -> Result<(), String> {
    // 限制播放速度在 0.5-2.0 之间
    let speed = speed.clamp(0.5, 2.0);
    self.playback_speed = speed;
    
    // 如果有正在播放的音频，重新应用播放速度
    if let Some(path) = self.current_path.clone() {
      // 保存当前播放位置和状态
      let _current_time = self.get_current_time();
      let is_playing = self.is_playing();
      
      // 停止当前播放
      self.stop()?;
      
      // 重新播放音频，应用新的播放速度
      self.play(&path)?;
      
      // 恢复播放状态
      if !is_playing {
        self.pause()?;
      }
    }
    
    Ok(())
  }

  fn get_playback_speed(&self) -> f32 {
    self.playback_speed
  }

  fn toggle_mute(&mut self) -> bool {
    self.muted = !self.muted;
    if self.muted {
      // 保存当前音量
      self.mute_volume = self.volume;
      // 设置音量为 0
      if let Some(sink) = &self.sink {
        sink.set_volume(0.0);
      }
    } else {
      // 恢复之前的音量
      self.volume = self.mute_volume;
      if let Some(sink) = &self.sink {
        sink.set_volume(self.volume);
      }
    }
    self.muted
  }

  fn set_mute(&mut self, mute: bool) {
    if self.muted != mute {
      self.toggle_mute();
    }
  }

  fn get_mute(&self) -> bool {
    self.muted
  }
  
  // 转码相关方法
  fn set_transcoder_enabled(&mut self, enabled: bool) {
    self.transcoder_enabled = enabled;
    if let Some(ref mut transcoder) = self.transcoder {
      transcoder.set_enabled(enabled);
    }
    log::info!("转码功能已{}", if enabled { "启用" } else { "禁用" });
  }
  
  fn is_transcoder_enabled(&self) -> bool {
    self.transcoder_enabled
  }
  
  fn set_transcoder_preload_seconds(&mut self, seconds: u64) {
    if let Some(ref mut transcoder) = self.transcoder {
      transcoder.set_preload_seconds(seconds);
    }
  }
  
  fn preload_next_track(&self, next_path: &str) {
    if !self.transcoder_enabled {
      return;
    }
    if let Some(ref transcoder) = self.transcoder {
      transcoder.preload_next_track(next_path);
    }
  }
  
  #[allow(dead_code)]
  fn get_transcode_status(&self, path: &str) -> Option<transcoder::TranscodeStatus> {
    if let Some(ref transcoder) = self.transcoder {
      transcoder.get_task_status(path)
    } else {
      None
    }
  }
  
  fn check_ffmpeg_available(&self) -> bool {
    transcoder::Transcoder::check_ffmpeg()
  }
}

// 扩展 PlaybackInfo 结构体，添加更多信息
#[derive(Debug, Serialize)]
pub struct ExtendedPlaybackInfo {
  pub is_playing: bool,
  pub is_empty: bool,
  pub current_time: f64,
  pub duration: f64,
  pub repeat_one: bool,
  pub playback_speed: f32,
  pub muted: bool,
  pub volume: f32,
}

unsafe impl Send for AudioPlayer {}

#[tauri::command]
async fn scan_dir(directory: String) -> Result<serde_json::Value, String> {
  let mut tracks = Vec::new();
  let mut id = 0;
  
  for entry in WalkDir::new(directory).into_iter().filter_map(|e| e.ok()) {
    let path = entry.path();
    if path.is_file() {
      let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
      if is_audio_file(ext) {
        // 使用 panic 捕获来保护文件解析
        match std::panic::catch_unwind(|| {
          parse_audio_file(path, id)
        }) {
          Ok(track) => {
            tracks.push(track);
            id += 1;
          }
          Err(_) => {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("未知");
            log::error!("解析文件时发生崩溃: {}", file_name);
            // 跳过这个文件，继续处理下一个
          }
        }
      }
    }
  }
  
  Ok(serde_json::json!({
    "tracks": tracks
  }))
}

#[tauri::command]
async fn load_state() -> Result<serde_json::Value, String> {
  // 使用应用数据目录保存状态文件
  let app_dir = std::env::current_dir().map_err(|e| e.to_string())?;
  let state_path = app_dir.join("player_state.json");
  
  if let Ok(content) = fs::read_to_string(&state_path) {
    if let Ok(state) = serde_json::from_str::<PlayerState>(&content) {
      return Ok(serde_json::json!(state));
    }
  }
  
  Ok(serde_json::json!({
    "tracks": [], 
    "current_track_index": -1, 
    "current_time": 0.0, 
    "is_playing": false,
    "play_mode": 0
  }))
}

#[tauri::command]
async fn save_state(state: PlayerState) -> Result<(), String> {
  let app_dir = std::env::current_dir().map_err(|e| e.to_string())?;
  let state_path = app_dir.join("player_state.json");
  
  if let Ok(content) = serde_json::to_string(&state) {
    if fs::write(&state_path, content).is_ok() {
      return Ok(());
    }
  }
  Err("保存状态失败".to_string())
}

#[tauri::command]
async fn play_track(path: String, player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<serde_json::Value, String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  let duration = player.play(&path)?;
  Ok(serde_json::json!({ "duration": duration }))
}

#[tauri::command]
async fn play_next_track(path: String, player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<serde_json::Value, String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  // 先停止当前曲目
  let _ = player.stop();
  // 播放新曲目（带淡入效果）
  let duration = player.play(&path)?;
  Ok(serde_json::json!({ "duration": duration }))
}

#[tauri::command]
async fn pause_track(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  player.pause()
}

#[tauri::command]
async fn resume_track(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  player.resume()
}

#[tauri::command]
async fn stop_track(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  player.stop()
}

#[tauri::command]
async fn seek_track(position: f64, player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  player.seek(position)
}

#[tauri::command]
async fn get_playback_info(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<serde_json::Value, String> {
  let player = player.lock().map_err(|e| e.to_string())?;
  let info = player.get_info();
  Ok(serde_json::json!(info))
}

#[tauri::command]
async fn set_equalizer_bands(bands: Vec<f32>, player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  player.equalizer_bands = bands;
  Ok(())
}

#[tauri::command]
async fn get_equalizer_bands(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<serde_json::Value, String> {
  let player = player.lock().map_err(|e| e.to_string())?;
  Ok(serde_json::json!(player.equalizer_bands))
}

#[tauri::command]
async fn set_volume(volume: f32, player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  // 确保音量在 0.0-1.0 范围内
  let volume = volume.clamp(0.0, 1.0);
  player.volume = volume;
  if let Some(sink) = &player.sink {
    sink.set_volume(volume);
  }
  Ok(())
}

#[tauri::command]
async fn get_volume(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<f32, String> {
  let player = player.lock().map_err(|e| e.to_string())?;
  Ok(player.volume)
}

#[tauri::command]
async fn toggle_repeat_one(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<bool, String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  Ok(player.toggle_repeat_one())
}

#[tauri::command]
async fn set_repeat_one(repeat: bool, player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  player.set_repeat_one(repeat);
  Ok(())
}

#[tauri::command]
async fn get_repeat_one(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<bool, String> {
  let player = player.lock().map_err(|e| e.to_string())?;
  Ok(player.get_repeat_one())
}

#[tauri::command]
async fn toggle_ffmpeg(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<bool, String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  Ok(player.toggle_ffmpeg())
}

#[tauri::command]
async fn set_ffmpeg(enabled: bool, player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  player.set_ffmpeg(enabled);
  Ok(())
}

#[tauri::command]
async fn get_ffmpeg(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<bool, String> {
  let player = player.lock().map_err(|e| e.to_string())?;
  Ok(player.get_ffmpeg())
}

#[tauri::command]
async fn toggle_equalizer(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<bool, String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  Ok(player.toggle_equalizer())
}

#[tauri::command]
async fn set_equalizer(enabled: bool, player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  player.set_equalizer(enabled);
  Ok(())
}

#[tauri::command]
async fn get_equalizer(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<bool, String> {
  let player = player.lock().map_err(|e| e.to_string())?;
  Ok(player.get_equalizer())
}

#[tauri::command]
async fn set_playback_speed(speed: f32, player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  player.set_playback_speed(speed)
}

#[tauri::command]
async fn get_playback_speed(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<f32, String> {
  let player = player.lock().map_err(|e| e.to_string())?;
  Ok(player.get_playback_speed())
}

#[tauri::command]
async fn toggle_mute(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<bool, String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  Ok(player.toggle_mute())
}

#[tauri::command]
async fn set_mute(mute: bool, player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  player.set_mute(mute);
  Ok(())
}

#[tauri::command]
async fn get_mute(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<bool, String> {
  let player = player.lock().map_err(|e| e.to_string())?;
  Ok(player.get_mute())
}

#[tauri::command]
async fn get_extended_playback_info(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<serde_json::Value, String> {
  let player = player.lock().map_err(|e| e.to_string())?;
  let info = ExtendedPlaybackInfo {
    is_playing: player.is_playing(),
    is_empty: player.is_empty(),
    current_time: player.get_current_time(),
    duration: player.duration,
    repeat_one: player.get_repeat_one(),
    playback_speed: player.get_playback_speed(),
    muted: player.get_mute(),
    volume: player.volume,
  };
  Ok(serde_json::json!(info))
}

// 转码相关命令
#[tauri::command]
async fn set_transcoder_enabled(enabled: bool, player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  player.set_transcoder_enabled(enabled);
  Ok(())
}

#[tauri::command]
async fn is_transcoder_enabled(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<bool, String> {
  let player = player.lock().map_err(|e| e.to_string())?;
  Ok(player.is_transcoder_enabled())
}

#[tauri::command]
async fn check_ffmpeg_available(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<bool, String> {
  let player = player.lock().map_err(|e| e.to_string())?;
  Ok(player.check_ffmpeg_available())
}

#[tauri::command]
async fn set_transcoder_preload_seconds(seconds: u64, player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  player.set_transcoder_preload_seconds(seconds);
  Ok(())
}

#[tauri::command]
async fn preload_next_track(next_path: String, player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let player = player.lock().map_err(|e| e.to_string())?;
  player.preload_next_track(&next_path);
  Ok(())
}

#[tauri::command]
async fn minimize_window(window: tauri::Window) -> Result<(), String> {
  window.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
async fn maximize_window(window: tauri::Window) -> Result<(), String> {
  if window.is_maximized().map_err(|e| e.to_string())? {
    window.unmaximize().map_err(|e| e.to_string())
  } else {
    window.maximize().map_err(|e| e.to_string())
  }
}

#[tauri::command]
async fn close_window(window: tauri::Window) -> Result<(), String> {
  window.close().map_err(|e| e.to_string())
}

// 应用设置结构体
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
  pub theme: String,
  pub visualizer_enabled: bool,
  pub sidebar_visible: bool,
  pub crossfade_duration: u64,
  pub save_playback_progress: bool,
  pub default_play_mode: i32,
  pub ffmpeg_enabled: bool,
  pub equalizer_enabled: bool,
  pub transcoder_enabled: bool,
  pub transcoder_preload_seconds: u64,
  pub playback_speed: f32,
  pub muted: bool,
  pub volume: f32,
}

impl Default for AppSettings {
  fn default() -> Self {
    AppSettings {
      theme: "default".to_string(),
      visualizer_enabled: true,
      sidebar_visible: true,
      crossfade_duration: 3000,
      save_playback_progress: true,
      default_play_mode: 1, // 默认随机播放
      ffmpeg_enabled: false, // 默认关闭 FFmpeg 引擎
      equalizer_enabled: true,
      transcoder_enabled: true, // 默认启用转码功能
      transcoder_preload_seconds: 15,
      playback_speed: 1.0,
      muted: false,
      volume: 0.7,
    }
  }
}

#[tauri::command]
async fn save_app_settings(settings: AppSettings) -> Result<(), String> {
  let app_dir = std::env::current_dir().map_err(|e| e.to_string())?;
  let settings_path = app_dir.join("app_settings.json");
  
  let content = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
  fs::write(&settings_path, content).map_err(|e| e.to_string())?;
  
  Ok(())
}

#[tauri::command]
async fn load_app_settings() -> Result<AppSettings, String> {
  let app_dir = std::env::current_dir().map_err(|e| e.to_string())?;
  let settings_path = app_dir.join("app_settings.json");
  
  if let Ok(content) = fs::read_to_string(&settings_path) {
    if let Ok(settings) = serde_json::from_str::<AppSettings>(&content) {
      return Ok(settings);
    }
  }
  
  // 如果文件不存在或解析失败，返回默认设置
  Ok(AppSettings::default())
}

#[tauri::command]
async fn set_crossfade_duration(duration: u64, player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  player.crossfade_duration = duration.clamp(0, 10000);
  Ok(())
}

#[tauri::command]
async fn get_crossfade_duration(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<u64, String> {
  let player = player.lock().map_err(|e| e.to_string())?;
  Ok(player.crossfade_duration)
}

#[tauri::command]
async fn set_save_playback_progress(save: bool, player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  player.save_playback_progress = save;
  Ok(())
}

#[tauri::command]
async fn get_save_playback_progress(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<bool, String> {
  let player = player.lock().map_err(|e| e.to_string())?;
  Ok(player.save_playback_progress)
}

#[tauri::command]
async fn set_default_play_mode(mode: i32, player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<(), String> {
  let mut player = player.lock().map_err(|e| e.to_string())?;
  player.default_play_mode = mode.clamp(0, 2);
  Ok(())
}

#[tauri::command]
async fn get_default_play_mode(player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<i32, String> {
  let player = player.lock().map_err(|e| e.to_string())?;
  Ok(player.default_play_mode)
}

fn is_audio_file(ext: &str) -> bool {
  audio_decoder::is_audio_file(ext)
}

fn parse_audio_file(path: &Path, id: i32) -> Track {
  let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("未知");
  let lyrics = load_lyrics(path);
  
  let (title, artist, album, duration, album_art) = parse_metadata(path);
  
  Track {
    id: id.to_string(),
    title: title.unwrap_or_else(|| file_name.to_string()),
    artist: artist.unwrap_or_else(|| "未知艺术家".to_string()),
    album: album.unwrap_or_else(|| "未知专辑".to_string()),
    duration,
    path: path.to_string_lossy().to_string(),
    lyrics,
    album_art,
  }
}

fn load_lyrics(path: &Path) -> Option<String> {
  let lrc_path = path.with_extension("lrc");
  if lrc_path.exists() {
    if let Ok(content) = fs::read_to_string(&lrc_path) {
      return Some(content);
    }
  }
  
  // 尝试查找同名歌词文件（不同扩展名）
  let file_stem = path.file_stem()?.to_str()?;
  let parent = path.parent()?;
  
  for entry in fs::read_dir(parent).ok()? {
    if let Ok(entry) = entry {
      let entry_path = entry.path();
      if let Some(ext) = entry_path.extension() {
        if ext == "lrc" || ext == "txt" {
          if let Some(stem) = entry_path.file_stem() {
            if stem.to_str() == Some(file_stem) {
              if let Ok(content) = fs::read_to_string(&entry_path) {
                return Some(content);
              }
            }
          }
        }
      }
    }
  }
  
  None
}

fn parse_metadata(path: &Path) -> (Option<String>, Option<String>, Option<String>, String, Option<String>) {
  use lofty::file::{AudioFile, TaggedFileExt};
  use lofty::probe::Probe;
  use lofty::tag::ItemKey;

  let mut title = None;
  let mut artist = None;
  let mut album = None;
  let mut duration = "0:00".to_string();
  let mut album_art = None;
  let mut duration_millis: u64 = 0;

  let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("未知");

  // 首先尝试使用 lofty 解析元数据（带 panic 保护）
  let lofty_result = std::panic::catch_unwind(|| {
    Probe::open(path).and_then(|p| p.read())
  });
  
  match lofty_result {
    Ok(Ok(tagged_file)) => {
      // 获取标签信息
      let tag = tagged_file.primary_tag().or_else(|| tagged_file.tags().first());
      
      if let Some(t) = tag {
        title = t.get_string(&ItemKey::TrackTitle).map(|s: &str| s.to_string());
        artist = t.get_string(&ItemKey::TrackArtist).map(|s: &str| s.to_string());
        album = t.get_string(&ItemKey::AlbumTitle).map(|s: &str| s.to_string());

        // 获取专辑封面
        if let Some(picture) = t.pictures().first() {
          let data = picture.as_ape_bytes();
          let mime_type = picture.mime_type().map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "image/jpeg".to_string());
          let base64_data = base64::engine::general_purpose::STANDARD.encode(data);
          album_art = Some(format!("data:{};base64, {}", mime_type, base64_data));
        }
      }

      // 获取歌曲时长 - 使用 as_millis() 获取毫秒数
      let properties = tagged_file.properties();
      let duration_obj = properties.duration();
      duration_millis = duration_obj.as_millis() as u64;
      log::info!("Lofty 解析 {}: 时长 = {} 毫秒 ({} 秒)", file_name, duration_millis, duration_obj.as_secs());
    }
    Ok(Err(e)) => {
      log::warn!("Lofty 无法解析文件 {}: {}", file_name, e);
    }
    Err(_) => {
      log::error!("Lofty 解析文件时发生 panic: {}", file_name);
    }
  }

  // 如果 lofty 没有获取到时长，尝试使用音频解码器（仅对特定格式）
  if duration_millis == 0 {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    // 只对可能不支持 duration 的格式尝试使用音频解码器
    let special_formats = ["ape", "tta", "wv", "mpc", "tak"];
    if special_formats.contains(&ext.as_str()) {
      log::info!("尝试使用音频解码器获取时长: {}", file_name);
      match std::panic::catch_unwind(|| {
        audio_decoder::try_open_audio(path.to_string_lossy().as_ref())
      }) {
        Ok(Ok(source)) => {
          if let Some(total_duration) = source.total_duration() {
            duration_millis = total_duration.as_millis() as u64;
            log::info!("音频解码器解析 {}: 时长 = {} 毫秒", file_name, duration_millis);
          } else {
            log::warn!("音频解码器无法获取时长: {}", file_name);
          }
        }
        Ok(Err(e)) => {
          log::error!("音频解码器打开失败 {}: {}", file_name, e);
        }
        Err(_) => {
          log::error!("音频解码器发生 panic: {}", file_name);
        }
      }
    }
  }

  // 格式化时长 - 将毫秒转换为 分:秒 格式
  if duration_millis > 0 {
    let total_seconds = duration_millis / 1000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    duration = format!("{}:{:02}", minutes, seconds);
  }

  log::info!("最终时长 {}: {}", file_name, duration);

  (title, artist, album, duration, album_art)
}

#[tauri::command]
async fn get_startup_args() -> Result<Vec<String>, String> {
  let args = STARTUP_ARGS.lock().map_err(|e| e.to_string())?;
  Ok(args.clone())
}

#[tauri::command]
async fn exit_app() -> Result<(), String> {
  std::process::exit(0);
}

// 检查文件是否需要转码
#[tauri::command]
async fn needs_transcode(path: &str, audio_player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<bool, String> {
  let audio_player = audio_player.lock().map_err(|e| e.to_string())?;
  if let Some(ref transcoder) = audio_player.transcoder {
    Ok(transcoder.needs_transcode(path))
  } else {
    Ok(false)
  }
}

// 检查文件是否已转码
#[tauri::command]
async fn is_transcoded(path: &str, audio_player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<bool, String> {
  let audio_player = audio_player.lock().map_err(|e| e.to_string())?;
  if let Some(ref transcoder) = audio_player.transcoder {
    Ok(transcoder.is_transcoded(path))
  } else {
    Ok(false)
  }
}

// 开始转码
#[tauri::command]
async fn start_transcode(path: &str, audio_player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<serde_json::Value, String> {
  let audio_player = audio_player.lock().map_err(|e| e.to_string())?;
  if let Some(ref transcoder) = audio_player.transcoder {
    match transcoder.start_transcode(path.to_string()) {
      Ok(_) => Ok(serde_json::json!({ "status": "started" })),
      Err(e) => Ok(serde_json::json!({ "status": "error", "message": e })),
    }
  } else {
    Ok(serde_json::json!({ "status": "error", "message": "转码器未初始化" }))
  }
}

// 获取转码状态
#[tauri::command]
async fn get_transcode_status(path: &str, audio_player: tauri::State<'_, Mutex<AudioPlayer>>) -> Result<serde_json::Value, String> {
  let audio_player = audio_player.lock().map_err(|e| e.to_string())?;
  if let Some(ref transcoder) = audio_player.transcoder {
    match transcoder.get_task_status(path) {
      Some(status) => {
        match status {
          transcoder::TranscodeStatus::Completed => {
            if let Some(transcoded_path) = transcoder.get_transcoded_path(path) {
              Ok(serde_json::json!({ "status": "completed", "path": transcoded_path }))
            } else {
              Ok(serde_json::json!({ "status": "completed" }))
            }
          },
          transcoder::TranscodeStatus::InProgress => {
            Ok(serde_json::json!({ "status": "in_progress" }))
          },
          transcoder::TranscodeStatus::Pending => {
            Ok(serde_json::json!({ "status": "pending" }))
          },
          transcoder::TranscodeStatus::Failed(msg) => {
            Ok(serde_json::json!({ "status": "failed", "message": msg }))
          },
        }
      },
      None => {
        Ok(serde_json::json!({ "status": "not_started" }))
      },
    }
  } else {
    Ok(serde_json::json!({ "status": "error", "message": "转码器未初始化" }))
  }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  // 获取命令行参数
  let args: Vec<String> = std::env::args().collect();
  if let Ok(mut startup_args) = STARTUP_ARGS.lock() {
    *startup_args = args;
  }
  
  tauri::Builder::default()
    .manage(Mutex::new(AudioPlayer::new()))
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_log::Builder::new().build())
    .invoke_handler(tauri::generate_handler!(
      scan_dir,
      play_track,
      play_next_track,
      pause_track,
      resume_track,
      stop_track,
      set_volume,
      get_volume,
      seek_track,
      get_playback_info,
      load_state, 
      save_state,
      set_equalizer_bands,
      get_equalizer_bands,
      toggle_repeat_one,
      set_repeat_one,
      get_repeat_one,
      toggle_ffmpeg,
      set_ffmpeg,
      get_ffmpeg,
      toggle_equalizer,
      set_equalizer,
      get_equalizer,
      set_playback_speed,
      get_playback_speed,
      toggle_mute,
      set_mute,
      get_mute,
      get_extended_playback_info,
      minimize_window,
      maximize_window,
      close_window,
      save_app_settings,
      load_app_settings,
      set_crossfade_duration,
      get_crossfade_duration,
      set_save_playback_progress,
      get_save_playback_progress,
      set_default_play_mode,
      get_default_play_mode,
      get_startup_args,
      set_transcoder_enabled,
      is_transcoder_enabled,
      check_ffmpeg_available,
      set_transcoder_preload_seconds,
      preload_next_track,
      needs_transcode,
      is_transcoded,
      start_transcode,
      get_transcode_status,
      exit_app,
    ))
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}