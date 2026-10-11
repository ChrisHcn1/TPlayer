//! ReplayGain 响度分析与增益建议
//!
//! 增益来源分两级：
//! 1. 优先读取音频文件内嵌 ReplayGain 标签（见 library/scan.rs，入库时已解析）；
//! 2. 无标签的曲目在首次播放时，用 ffmpeg 的 EBU R128（ebur128）滤波器测量
//!    综合响度（Integrated LUFS）与峰值，按目标响度换算建议增益（dB），
//!    并写回曲库持久化，此后播放直接复用。
//!
//! 说明：本项目本地闭环、无在线依赖，ffmpeg 已随 Tauri 打包可用（同 M0 时长探测）。

use std::process::Command;

/// 无标签曲目在播放时触发的后台响度分析（ffmpeg ebur128）
/// 返回 `(建议增益 dB, 峰值线性)`；静音/失败返回 None。
pub fn analyze_loudness(path: &str) -> Option<(f32, f32)> {
    let output = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-nostats",
            "-i",
            path,
            "-af",
            "ebur128=framelog=quiet",
            "-f",
            "null",
            "-",
        ])
        .output()
        .ok()?;

    // ebur128 将统计输出写到 stderr
    let stderr = String::from_utf8_lossy(&output.stderr);
    let integrated = parse_lufs(&stderr)?;
    let peak_db = parse_peak_dbfs(&stderr);

    // 建议增益 = 目标响度 - 实测综合响度（目标默认 -14 LUFS，与主流流媒体一致）
    let gain_db = TARGET_LUFS - integrated;
    if gain_db < -24.0 || gain_db > 24.0 {
        // 明显异常（静音或失真数据）不给建议，避免把音量推到极端
        return None;
    }

    let peak = match peak_db {
        Some(db) => (10.0_f32).powf(db / 20.0).clamp(0.0, 1.0),
        None => 1.0,
    };
    Some((gain_db, peak))
}

/// 目标综合响度（LUFS），与设置中的默认一致
pub const TARGET_LUFS: f32 = -14.0;

/// 从 ebur128 stderr 文本中解析综合响度（兼容新旧 ffmpeg 输出格式）
fn parse_lufs(stderr: &str) -> Option<f32> {
    for line in stderr.lines() {
        let line = line.trim();
        if let Some(value) = line
            .strip_prefix("I:")
            .or_else(|| line.strip_prefix("Integrated loudness (I):"))
        {
            // 形如 "I: -14.2 LUFS" / "Integrated loudness (I): -14.2 LUFS"
            if let Some(num) = value
                .trim()
                .trim_end_matches("LUFS")
                .trim_end_matches("lufs")
                .trim()
                .parse::<f32>()
                .ok()
            {
                return Some(num);
            }
        }
    }
    None
}

/// 从 ebur128 stderr 文本中解析采样峰值（dBFS）
fn parse_peak_dbfs(stderr: &str) -> Option<f32> {
    for line in stderr.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("Peak:").or_else(|| {
            line.strip_prefix("Peak (sample):")
        }) {
            if let Some(num) = value
                .trim()
                .trim_end_matches("dBFS")
                .trim_end_matches("dbfs")
                .trim()
                .parse::<f32>()
                .ok()
            {
                return Some(num);
            }
        }
    }
    None
}
