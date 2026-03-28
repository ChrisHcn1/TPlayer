const fs = require('fs');
const path = 'e:\\TPlayer\\src-tauri\\src\\lib.rs';

let content = fs.readFileSync(path, 'utf8');

// 修改play_track函数，添加对wav文件异常的处理
const oldPlayTrack = `    // 第一步：尝试直接播放（使用 FFmpeg 引擎或 rodio）
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
              return Err(format!("此格式需要转码后才能播放: {}\. 请在设置中启用'转码功能'，系统将自动转码为FLAC格式。", format_ext));
            }
          }
        }`;

const newPlayTrack = `    // 第一步：尝试直接播放（使用 FFmpeg 引擎或 rodio）
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
        
        // 检查是否是wav文件且解码失败
        let is_wav_file = path.to_lowercase().ends_with(".wav");
        let is_decode_failure = direct_error.contains("无法解码音频") || direct_error.contains("解码失败") || direct_error.contains("decoder");
        
        // 第二步：检查是否需要转码
        if let Some(ref transcoder) = self.transcoder {
          let needs_transcode = transcoder.needs_transcode(path) || (is_wav_file && is_decode_failure);
          
          if needs_transcode {
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
                    log::info!("开始转码文件: {}（{}", path, if is_wav_file && is_decode_failure { "WAV文件异常" } else { "需要转码" });
                    if let Err(e) = transcoder.start_transcode(path.to_string()) {
                      return Err(format!("无法开始转码: {}", e));
                    }
                    return Err("开始转码，请稍后再试".to_string());
                  }
                }
              }
            } else {
              // 需要转码但未启用转码功能
              let message = if is_wav_file && is_decode_failure {
                "此WAV文件可能有元数据或头文件异常，需要转码后才能播放。请在设置中启用'转码功能'，系统将自动转码为FLAC格式。"
              } else {
                format!("此格式需要转码后才能播放: {}\. 请在设置中启用'转码功能'，系统将自动转码为FLAC格式。", format_ext)
              };
              return Err(message);
            }
          }
        }`;

content = content.replace(oldPlayTrack, newPlayTrack);

fs.writeFileSync(path, content, 'utf8');
console.log('WAV file error handling added');
