//! 歌词解析：LRC 时间轴解析 + 用户手动偏移的持久化
//!
//! LRC 解析要点：
//!   - 支持一行多个时间标签（`[00:01.00][00:05.00]重复句` 展开为多行）；
//!   - 支持分/秒/小数的常见写法（`[mm:ss]` `[mm:ss.xx]` `[mm:ss.xxx]`）；
//!   - 元数据行（`[ti:]` `[ar:]` 等）跳过，其中 `[offset:±ms]` 应用到全部时间轴
//!     （惯例：正偏移使歌词提前，即有效时间 = 标签时间 − offset）；
//!   - 相邻两行时间相同（合并式双语 LRC）时，后一行作为前一行的翻译；
//!   - 全文无任何时间标签时按纯文本歌词处理（synced = false）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 歌词数据结构（与前端 types/lyrics.ts 对应）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lyrics {
    #[serde(rename = "trackId")]
    pub track_id: String,
    pub source: LyricsSource,
    pub synced: bool,
    pub lines: Vec<LyricsLine>,
    #[serde(rename = "offsetMs")]
    pub offset_ms: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LyricsSource {
    Embedded,
    Sidecar,
    Online,
    Manual,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsLine {
    #[serde(rename = "timeMs")]
    pub time_ms: u64,
    pub text: String,
    pub translation: Option<String>,
}

/// 时间标签：`[mm:ss]` `[mm:ss.xx]` `[mm:ss.xxx]`（小数按秒的小数部分解释）
fn time_tag_ms(minutes: &str, seconds: &str) -> Option<u64> {
    let m = minutes.trim().parse::<u64>().ok()?;
    let s = seconds.trim().parse::<f64>().ok()?;
    Some(((m as f64 * 60.0 + s) * 1000.0) as u64)
}

/// 解析 LRC 文本为歌词行；无任何时间标签时返回 None（调用方按纯文本处理）
fn parse_timed_lines(content: &str) -> Option<(Vec<LyricsLine>, i64)> {
    let re = regex::Regex::new(r"\[\s*(\d+)\s*:\s*(\d+(?:[.,]\d+)?)\s*\]").ok()?;
    let meta_re = regex::Regex::new(r"^\s*\[\s*([a-zA-Z#]+)\s*:(.*?)\]\s*$").ok()?;

    let mut offset_ms: i64 = 0;
    let mut lines: Vec<LyricsLine> = Vec::new();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let matches: Vec<_> = re.captures_iter(line).collect();
        if matches.is_empty() {
            // 元数据行：仅处理 offset
            if let Some(cap) = meta_re.captures(line) {
                if cap[1].eq_ignore_ascii_case("offset") {
                    if let Ok(v) = cap[2].trim().parse::<i64>() {
                        offset_ms = v;
                    }
                }
            }
            continue;
        }

        // 去掉所有 [...] 标签后的歌词文本
        let text = re.replace_all(line, "").trim().to_string();
        for cap in matches {
            let Some(ms) = time_tag_ms(&cap[1], &cap[2].replace(',', ".")) else {
                continue;
            };
            lines.push(LyricsLine {
                time_ms: ms,
                text: text.clone(),
                translation: None,
            });
        }
    }

    if lines.is_empty() {
        return None;
    }

    // 应用 LRC 自带偏移（正偏移 = 歌词提前）
    if offset_ms != 0 {
        for l in &mut lines {
            let adjusted = l.time_ms as i64 - offset_ms;
            l.time_ms = adjusted.max(0) as u64;
        }
    }

    // 稳定排序后合并同时间行：第二行作为翻译
    lines.sort_by_key(|l| l.time_ms);
    let mut merged: Vec<LyricsLine> = Vec::with_capacity(lines.len());
    for line in lines {
        if let Some(last) = merged.last_mut() {
            if last.time_ms == line.time_ms && !line.text.is_empty() && last.translation.is_none()
            {
                last.translation = Some(line.text);
                continue;
            }
        }
        merged.push(line);
    }

    Some((merged, offset_ms))
}

/// 解析歌词内容（内嵌标签 / .lrc 边车 / 在线下载的统一入口）
pub fn parse(track_id: &str, source: LyricsSource, content: &str) -> Lyrics {
    match parse_timed_lines(content) {
        Some((lines, _)) => Lyrics {
            track_id: track_id.to_string(),
            source,
            synced: true,
            lines,
            offset_ms: 0,
        },
        None => {
            // 纯文本歌词：无时间轴
            let lines = content
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .map(|text| LyricsLine {
                    time_ms: 0,
                    text: text.to_string(),
                    translation: None,
                })
                .collect();
            Lyrics {
                track_id: track_id.to_string(),
                source,
                synced: false,
                lines,
                offset_ms: 0,
            }
        }
    }
}

/// 毫秒 → LRC 时间标签字符串（[mm:ss.xx]）
pub fn format_lrc_time(ms: u64) -> String {
    let total_seconds = ms / 1000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    let centis = (ms % 1000) / 10;
    format!("[{:02}:{:02}.{:02}]", minutes, seconds, centis)
}

// ---- 用户手动偏移（持久化到 lyrics_offsets.json）----

/// 从磁盘加载偏移表；文件缺失/损坏返回空表
pub fn load_offsets(path: &Path) -> HashMap<String, i64> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

pub fn get_offset(path: &Path, track_id: &str) -> i64 {
    load_offsets(path).get(track_id).copied().unwrap_or(0)
}

/// 写入单个曲目的偏移（读-改-写；偏移为 0 时移除条目保持文件干净）
pub fn save_offset(path: &PathBuf, track_id: &str, offset_ms: i64) {
    let mut offsets = load_offsets(path);
    if offset_ms == 0 {
        offsets.remove(track_id);
    } else {
        offsets.insert(track_id.to_string(), offset_ms);
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(&offsets) {
        let _ = std::fs::write(path, json);
    }
}

/// 清空全部偏移
pub fn clear_offsets(path: &PathBuf) {
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
}
