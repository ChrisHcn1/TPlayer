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
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

/// LRC 头部元数据标签正则（`[ar:...]` / `[offset:...]` 等）：编译一次后全局复用。
///
/// 性能：原实现每次解析都重新编译正则（正则编译是 LRC 解析里最贵的固定开销），
/// 现改为 `OnceLock` 静态缓存，多次解析零编译开销。
fn meta_tag_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"^\s*\[\s*([a-zA-Z#]+)\s*:(.*?)\]\s*$")
            .expect("LRC 元数据标签正则应当有效")
    })
}

/// LRC 时间标签正则（`[mm:ss]` / `[mm:ss.xx]` / `[mm:ss.xxx]`）：编译一次后全局复用。
fn time_tag_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"\[\s*(\d+)\s*:\s*(\d+(?:[.,]\d+)?)\s*\]")
            .expect("LRC 时间标签正则应当有效")
    })
}

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

/// 边车歌词元数据（LRC 头部 `[ar:]/[al:]/[ti:]/[au:]/[by:]` 标签解析结果）。
///
/// 用于「歌词元数据补全」实验室功能：当曲库的艺术家/专辑为占位（未知艺术家/未知专辑）时，
/// 从同名 `.lrc` 边车里提取真实信息回填。仅提取标签、不解析时间轴（见 `parse`）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SidecarMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}

/// 解析 LRC 头部的元数据标签（不解析时间轴）。
/// 覆盖常见字段：`ar`=艺术家、`al`=专辑、`ti`=标题、`au`=作者、`by`=制作者；
/// 大小写不敏感，空值忽略；其余标签（如 `offset`）忽略。
pub fn parse_sidecar_metadata(content: &str) -> SidecarMeta {
    let meta_re = meta_tag_re();
    let mut meta = SidecarMeta::default();
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(cap) = meta_re.captures(line) {
            let key = cap[1].to_ascii_lowercase();
            let value = cap[2].trim().to_string();
            if value.is_empty() {
                continue;
            }
            match key.as_str() {
                "ar" => meta.artist = Some(value),
                "al" => meta.album = Some(value),
                "ti" => meta.title = Some(value),
                "au" => meta.author = Some(value),
                "by" => meta.by = Some(value),
                _ => {}
            }
        }
    }
    meta
}

/// 时间标签：`[mm:ss]` `[mm:ss.xx]` `[mm:ss.xxx]`（小数按秒的小数部分解释）
fn time_tag_ms(minutes: &str, seconds: &str) -> Option<u64> {
    let m = minutes.trim().parse::<u64>().ok()?;
    let s = seconds.trim().parse::<f64>().ok()?;
    Some(((m as f64 * 60.0 + s) * 1000.0) as u64)
}

/// 解析 LRC 文本为歌词行；无任何时间标签时返回 None（调用方按纯文本处理）
fn parse_timed_lines(content: &str) -> Option<(Vec<LyricsLine>, i64)> {
    let re = time_tag_re();
    let meta_re = meta_tag_re();

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

/// 偏移表内存缓存：`(文件路径, 偏移表)`；路径变化时自动重新加载。
///
/// 性能：原实现每次读 / 写偏移都全量读 + 解析 JSON 文件（IO 与解析开销随条目数
/// 线性增长）；现改为内存缓存——读偏移零磁盘 IO，写偏移只做一次序列化落盘。
/// 该文件由本模块独占写入，缓存与磁盘内容始终一致。
static OFFSETS_CACHE: OnceLock<Mutex<Option<(PathBuf, HashMap<String, i64>)>>> = OnceLock::new();

fn offsets_cell() -> &'static Mutex<Option<(PathBuf, HashMap<String, i64>)>> {
    OFFSETS_CACHE.get_or_init(|| Mutex::new(None))
}

/// 从磁盘加载偏移表；文件缺失/损坏返回空表
pub fn load_offsets(path: &Path) -> HashMap<String, i64> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

/// 取偏移（命中内存缓存，避免每次全量读盘）
pub fn get_offset(path: &Path, track_id: &str) -> i64 {
    let mut cell = offsets_cell().lock().unwrap_or_else(|e| e.into_inner());
    let stale = match cell.as_ref() {
        Some((p, _)) => p != path,
        None => true,
    };
    if stale {
        *cell = Some((path.to_path_buf(), load_offsets(path)));
    }
    cell.as_ref()
        .and_then(|(_, map)| map.get(track_id))
        .copied()
        .unwrap_or(0)
}

/// 写入单个曲目的偏移（内存改写 + 落盘；偏移为 0 时移除条目保持文件干净）
pub fn save_offset(path: &PathBuf, track_id: &str, offset_ms: i64) {
    let mut cell = offsets_cell().lock().unwrap_or_else(|e| e.into_inner());
    let stale = match cell.as_ref() {
        Some((p, _)) => p != path,
        None => true,
    };
    if stale {
        *cell = Some((path.to_path_buf(), load_offsets(path)));
    }
    let Some((_, offsets)) = cell.as_mut() else {
        return;
    };
    if offset_ms == 0 {
        offsets.remove(track_id);
    } else {
        offsets.insert(track_id.to_string(), offset_ms);
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(offsets) {
        let _ = std::fs::write(path, json);
    }
}

/// 清空全部偏移
pub fn clear_offsets(path: &PathBuf) {
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    // 同步失效内存缓存（下次访问重新从磁盘加载）
    let mut cell = offsets_cell().lock().unwrap_or_else(|e| e.into_inner());
    if matches!(cell.as_ref(), Some((p, _)) if p == path) {
        *cell = None;
    }
}
