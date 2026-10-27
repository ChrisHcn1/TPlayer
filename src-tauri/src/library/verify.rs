//! 实验室·元数据增强：在线校验编排
//!
//! 数据流：扫描期 `parse_track` 把本地拆分候选（见 `super::filename`）入队 →
//! 扫描 / 导入完成后 [`drain_and_spawn`] 后台批量校验 → 网易云搜索（复用
//! `lyrics::online::search_hits`）→ [`judge`] 纯函数裁决方向 → 命中则回写
//! 曲库（apply_scan + 播放器缓存 + `library://tracks-resolved` 增量事件）。
//!
//! 约束：
//!  - 仅在 `ENHANCED_METADATA`（实验室开关）开启时运行；开关关闭即中止；
//!  - **只补未知字段**：后台自动校验（[`VerifyMode::Auto`]）仅在「未知艺术家 /
//!    未知专辑」时写入，已有标注绝不覆盖；专辑在两种模式下都只补未知值；
//!  - **重试仅支持单曲**（[`retry_track`]，曲目右键菜单触发）：只重新处理用户
//!    指定的这一首，不影响其他曲目，也不受后台批量任务代际影响；每次重试结果
//!    追加写入 `app_data_dir/meta_retry_log.json`，供列表标记与事后核对；
//!  - 单实例：同一时刻至多一个批量校验任务，新任务令旧任务作废（generation）；
//!  - 限速：串行请求 + 批间 250ms 间隔，避免打爆网易云接口。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use super::filename::{self, Candidate};
use super::models::{album_id_of, Track};
use super::persist;
use super::scan::{self, ENHANCED_METADATA};
use crate::lyrics::online::{search_hits, SearchHit};
use crate::AppState;

/// 时长匹配容差（毫秒），与歌词在线匹配保持一致
const DURATION_TOLERANCE_MS: i64 = 3000;
/// 标题相似度阈值（归一化后 bigram Dice 系数）
const TITLE_SIM_THRESHOLD: f64 = 0.8;
/// 批大小：每批回写一次 + 批间限速
const BATCH_SIZE: usize = 50;

/// 待在线校验条目
#[derive(Debug, Clone)]
pub struct PendingVerify {
    pub track_id: String,
    /// 扫描期写入的本地标题（校验前复核用，防覆盖期间已变化的数据）
    pub local_title: String,
    /// 本地拆分候选（原串 + 两种方向）
    pub candidates: Vec<Candidate>,
    pub duration_ms: u64,
}

/// 入队（按 track_id 去重：强扫重复解析不产生重复校验）
pub fn enqueue(item: PendingVerify) {
    let mut q = VERIFY_QUEUE.lock().unwrap_or_else(|e| e.into_inner());
    if q.iter().any(|x| x.track_id == item.track_id) {
        return;
    }
    q.push(item);
}

static VERIFY_QUEUE: Mutex<Vec<PendingVerify>> = Mutex::new(Vec::new());
/// 当前批量校验任务代际：新任务启动即令旧任务作废
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// 是否有批量校验任务在运行
static RUNNING: AtomicBool = AtomicBool::new(false);

/// 单曲重试记录（追加式，完整保留全部历史；前端按 trackId 取最新一条展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryRecord {
    /// 曲目 ID（唯一标识被重试的那一首）
    pub track_id: String,
    /// 重试时间（本地时间 "YYYY-MM-DD HH:MM:SS"）
    pub at: String,
    /// 结果状态：ok（已更新）/ failed（无可信结果）/ skipped（无需变更）
    pub status: String,
    /// 中文说明：成功时列出变更字段，失败 / 跳过时给出原因
    pub detail: String,
    /// 重试后的标题（未变更时为原值）
    pub title: String,
    /// 重试后的艺术家（以 " / " 连接）
    pub artist: String,
    /// 重试后的专辑
    pub album: String,
    /// 识别结果的持久化位置：tag（文件标签）/ sidecar（附属文件）/ none（未保存）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved_to: Option<String>,
    /// 持久化说明（写入的标签类型 / 降级原因 / 失败原因）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_detail: Option<String>,
}

/// 重试日志内存副本（落盘于 app_data_dir/meta_retry_log.json）
static RETRY_LOG: Mutex<Vec<RetryRecord>> = Mutex::new(Vec::new());
/// 日志是否已从磁盘载入
static RETRY_LOG_LOADED: AtomicBool = AtomicBool::new(false);

/// 是否有批量校验任务正在后台运行（前端状态展示）
pub fn is_running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

/// 取出全部待校验条目并清空队列
fn take_queue() -> Vec<PendingVerify> {
    std::mem::take(&mut *VERIFY_QUEUE.lock().unwrap_or_else(|e| e.into_inner()))
}

/// 扫描 / 导入完成后调用：队列非空且开关开启时启动后台校验任务。
/// 已有任务在跑时也照常调用——新任务通过 generation 使旧任务尽快中止。
pub fn drain_and_spawn(app: &AppHandle) {
    if !ENHANCED_METADATA.load(Ordering::SeqCst) {
        // 开关已关：丢弃队列，绝不联网
        take_queue();
        return;
    }
    let queue = take_queue();
    if queue.is_empty() {
        return;
    }
    let gen = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        run(handle, queue, gen).await;
    });
}

/* ---------- 单曲重试：执行 + 追加式日志 ---------- */

fn retry_log_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join("meta_retry_log.json"))
}

fn ensure_log_loaded(app: &AppHandle) {
    if RETRY_LOG_LOADED.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some(path) = retry_log_path(app) else {
        return;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    match serde_json::from_str::<Vec<RetryRecord>>(&text) {
        Ok(list) => *RETRY_LOG.lock().unwrap_or_else(|e| e.into_inner()) = list,
        Err(e) => eprintln!("[verify] 重试记录解析失败（忽略旧文件）: {e}"),
    }
}

/// 读取完整重试日志（按时间正序，同一曲目可能有多条）
pub fn retry_log(app: &AppHandle) -> Vec<RetryRecord> {
    ensure_log_loaded(app);
    RETRY_LOG
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

fn append_log(app: &AppHandle, rec: RetryRecord) {
    ensure_log_loaded(app);
    // 持锁完成"追加 + 落盘"：避免并发重试时后写覆盖前一条
    let mut log = RETRY_LOG.lock().unwrap_or_else(|e| e.into_inner());
    log.push(rec);
    let Some(path) = retry_log_path(app) else {
        return;
    };
    // 重试为用户手动触发、量级很小，整文件重写即可（保证完整历史不丢）
    match serde_json::to_string_pretty(&*log) {
        Ok(text) => {
            if let Err(e) = std::fs::write(&path, text) {
                eprintln!("[verify] 重试记录写入失败: {e}");
            }
        }
        Err(e) => eprintln!("[verify] 重试记录序列化失败: {e}"),
    }
}

/// 单曲重试：重新解析文件名 + 联网查找，只处理 `track_id` 这一首。
///
/// 与后台批量校验的区别：
///  - 不受批量任务代际影响（用户操作不会被扫描打断）；
///  - [`VerifyMode::Manual`]：允许改写标题 / 艺术家（用户认为现有判断有误才会
///    重试），专辑仍只补未知值；
///  - 无论成功、失败、跳过都追加一条重试记录，便于核对与排查。
pub async fn retry_track(app: &AppHandle, track_id: &str) -> Result<RetryRecord, String> {
    let state = app.state::<AppState>();
    let track = {
        let lib = state.library.lock().unwrap_or_else(|e| e.into_inner());
        lib.get_track(track_id)
            .ok_or_else(|| "曲目不存在（可能已被移除）".to_string())?
    };

    let item = build_pending(&track);
    let outcome = verify_one(app, item, VerifyMode::Manual).await;
    let mut saved: Option<persist::PersistOutcome> = None;
    let (status, detail, updated) = match outcome {
        Outcome::Updated(updated) => {
            let updated = *updated;
            let detail = describe_changes(&track, &updated);
            commit_updates(app, vec![updated.clone()]);
            // 识别校正完成即自动保存：优先写音频标签，失败 / 不支持时降级附属文件
            saved = Some(persist::save_track(&updated));
            ("ok", detail, Some(updated))
        }
        Outcome::Failed(reason) => ("failed", reason, None),
        Outcome::Skip(reason) => ("skipped", reason, None),
    };

    let final_track = updated.as_ref().unwrap_or(&track);
    let rec = RetryRecord {
        track_id: track.id.clone(),
        at: now_str(),
        status: status.to_string(),
        detail,
        title: final_track.title.clone(),
        artist: final_track.artists.join(" / "),
        album: final_track.album.clone(),
        saved_to: saved.as_ref().map(|s| s.target.clone()),
        save_detail: saved.map(|s| s.detail),
    };
    append_log(app, rec.clone());
    Ok(rec)
}

/// 由曲目重建待校验条目：文件名拆分为主证据，当前标题作为兜底候选。
fn build_pending(track: &Track) -> PendingVerify {
    let path = Path::new(&track.file_path);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(track.title.as_str());
    let current_artist = track
        .artists
        .iter()
        .find(|a| !a.trim().is_empty() && a.trim() != "未知艺术家")
        .map(String::as_str)
        .unwrap_or("");

    let (dir_album, dir_artist) = scan::infer_album_artist(path);
    let ctx = filename::NameCtx {
        tag_artist: current_artist,
        parent_artist: (dir_artist != "未知艺术家").then_some(dir_artist.as_str()),
        parent_album: (dir_album != "未知专辑").then_some(dir_album.as_str()),
    };

    let mut candidates: Vec<Candidate> = Vec::new();
    if let Some(parsed) = filename::parse_stem(stem, &ctx) {
        candidates.extend(parsed.candidates);
    }
    // 当前标题兜底：标签标题可能本就正确，仅专辑 / 艺术家缺失
    if !track.title.trim().is_empty() && !candidates.iter().any(|c| c.title == track.title) {
        candidates.push(Candidate {
            title: track.title.clone(),
            artist: current_artist.to_string(),
        });
    }

    PendingVerify {
        track_id: track.id.clone(),
        local_title: track.title.clone(),
        candidates,
        duration_ms: (track.duration.max(0.0) * 1000.0) as u64,
    }
}

/// 变更摘要（用于重试日志 detail 与前端提示）
fn describe_changes(before: &Track, after: &Track) -> String {
    let mut parts: Vec<String> = Vec::new();
    if before.title != after.title {
        parts.push(format!("标题「{}」→「{}」", before.title, after.title));
    }
    if before.artists != after.artists {
        parts.push(format!(
            "艺术家「{}」→「{}」",
            before.artists.join(" / "),
            after.artists.join(" / ")
        ));
    }
    if before.album != after.album {
        parts.push(format!("专辑「{}」→「{}」", before.album, after.album));
    }
    if parts.is_empty() {
        "联网命中，但无需变更".to_string()
    } else {
        parts.join("；")
    }
}

fn now_str() -> String {
    chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

/// 后台校验主循环：串行请求、批间限速、逐批回写。
async fn run(app: AppHandle, queue: Vec<PendingVerify>, gen: u64) {
    RUNNING.store(true, Ordering::SeqCst);
    let mut verified = 0usize;
    let mut failed = 0usize;

    for batch in queue.chunks(BATCH_SIZE) {
        // 开关关闭 / 已被新任务取代 → 中止（未处理条目保留在队列外：
        // 本地结果已持久化，下次强扫会重新入队，不会丢数据）
        if !ENHANCED_METADATA.load(Ordering::SeqCst)
            || GENERATION.load(Ordering::SeqCst) != gen
        {
            RUNNING.store(false, Ordering::SeqCst);
            return;
        }

        let mut updates: Vec<Track> = Vec::new();
        for item in batch {
            if !ENHANCED_METADATA.load(Ordering::SeqCst)
                || GENERATION.load(Ordering::SeqCst) != gen
            {
                break;
            }
            match verify_one(&app, item.clone(), VerifyMode::Auto).await {
                Outcome::Updated(track) => updates.push(*track),
                Outcome::Failed(_) => failed += 1,
                Outcome::Skip(_) => {}
            }
        }

        if !updates.is_empty() {
            verified += updates.len();
            let persist_targets = updates.clone();
            commit_updates(&app, updates);
            // 自动保存：后台补全的识别结果同样落盘（标签优先，附属文件降级），
            // 重新导入 / 重扫时即可直接读取，无须再次联网识别。
            for track in &persist_targets {
                let outcome = persist::save_track(track);
                if outcome.target == "none" {
                    eprintln!(
                        "[verify] 识别结果未能持久化（{}）：{}",
                        track.id, outcome.detail
                    );
                }
            }
        }
        // 批间限速（放最后：即使中止也已回写本批）
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    RUNNING.store(false, Ordering::SeqCst);

    let _ = app.emit(
        "library://meta-verify-done",
        serde_json::json!({ "verified": verified, "failed": failed }),
    );
}

/// 校验模式：后台自动补全 vs 用户单曲重试
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerifyMode {
    /// 扫描期自动校验：仅艺术家 / 专辑为未知占位时才写入
    Auto,
    /// 用户单曲重试：允许改写标题 / 艺术家（专辑仍只补未知值）
    Manual,
}

enum Outcome {
    /// 裁决命中，曲目已更新待回写
    Updated(Box<Track>),
    /// 网络失败 / 无命中 / 裁决不通过（附原因）
    Failed(String),
    /// 曲目不存在、标题已变化、或无需变更（附原因）
    Skip(String),
}

fn artist_unknown(track: &Track) -> bool {
    track.artists.is_empty()
        || track
            .artists
            .iter()
            .all(|a| a.trim().is_empty() || a.trim() == "未知艺术家")
}

fn album_unknown(track: &Track) -> bool {
    track.album.trim().is_empty() || track.album.trim() == "未知专辑"
}

/// 校验单条：搜索 → 裁决 → 构造更新后的曲目（不写库）。
async fn verify_one(app: &AppHandle, item: PendingVerify, mode: VerifyMode) -> Outcome {
    let state = app.state::<AppState>();
    let track = {
        let lib = state.library.lock().unwrap_or_else(|e| e.into_inner());
        match lib.get_track(&item.track_id) {
            Some(t) => t,
            None => return Outcome::Skip("曲目已不存在".to_string()),
        }
    };
    // 复核：校验期间标题已被其它流程改写 → 跳过，避免覆盖
    if track.title != item.local_title {
        return Outcome::Skip("曲目已被其他流程改写".to_string());
    }

    let artist_unk = artist_unknown(&track);
    let album_unk = album_unknown(&track);
    // 自动补全只处理未知字段；两者都已标注时不做任何联网改动
    if mode == VerifyMode::Auto && !artist_unk && !album_unk {
        return Outcome::Skip("艺术家与专辑均已标注，无需补全".to_string());
    }

    let artist = track
        .artists
        .iter()
        .find(|a| !a.trim().is_empty() && a.trim() != "未知艺术家")
        .map(String::as_str)
        .unwrap_or("");
    let hits = search_hits(&item.local_title, artist).await;
    if hits.is_empty() {
        return Outcome::Failed("联网搜索无结果".to_string());
    }
    let Some((cand, hit)) = judge(&item.candidates, &hits, item.duration_ms) else {
        return Outcome::Failed(
            "未找到可信匹配（标题相似度 / 艺术家 / 时长均未确认）".to_string(),
        );
    };

    let mut updated = track.clone();
    // 标题 / 艺术家：自动模式只在艺术家未知时写入；手动重试允许纠正既有判断
    if mode == VerifyMode::Manual || artist_unk {
        if !cand.title.is_empty() {
            updated.title = cand.title.clone();
        }
        if !hit.artists.is_empty() {
            updated.artists = hit.artists.clone();
        } else if !cand.artist.is_empty() {
            updated.artists = super::scan::parse_artists_enhanced(&cand.artist);
        }
    }
    // 专辑：两种模式都只在未知占位时补，避免在线合辑名覆盖用户已整理的专辑
    if album_unk {
        if let Some(album) = hit.album.clone().filter(|a| !a.trim().is_empty()) {
            updated.album = album;
        }
    }
    if updated.title == track.title
        && updated.artists == track.artists
        && updated.album == track.album
    {
        return Outcome::Skip("联网结果与当前一致，无需变更".to_string());
    }
    let first = updated.artists.first().map(String::as_str).unwrap_or("");
    updated.album_id = Some(album_id_of(&updated.album, first));
    Outcome::Updated(Box::new(updated))
}

/// 回写一批校验结果：写库 + 播放器缓存 + 前端增量事件。
fn commit_updates(app: &AppHandle, upserts: Vec<Track>) {
    let state = app.state::<AppState>();
    {
        let mut lib = state.library.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = lib.apply_scan(&upserts, &[]) {
            eprintln!("[verify] 元数据校验写库失败: {e}");
            return;
        }
    }
    state
        .player
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .apply_track_changes(&upserts, &[]);
    let _ = app.emit(
        "library://tracks-resolved",
        serde_json::json!({ "tracks": &upserts, "removedIds": [] }),
    );
}

/// 标题归一化：小写 → 去括号段（含中英文括号/方括号）→ 保留字母数字与 CJK。
fn normalize_title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth: u32 = 0;
    for c in s.chars() {
        match c {
            '(' | '（' | '[' | '【' => depth += 1,
            ')' | '）' | ']' | '】' => depth = depth.saturating_sub(1),
            _ if depth == 0 && (c.is_alphanumeric() || c.is_whitespace()) => {
                out.extend(c.to_lowercase());
            }
            _ => {}
        }
    }
    // 去空白
    out.chars().filter(|c| !c.is_whitespace()).collect()
}

/// 归一化艺术家：小写 + 仅保留字母数字与空格（空白压缩为单空格）
fn normalize_artist(s: &str) -> String {
    let filtered: String = s
        .chars()
        .flat_map(|c| c.to_lowercase())
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    filtered.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 字符 bigram Dice 系数相似度（对 CJK 与英文均适用），范围 0..=1。
fn similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    if ac.len() < 2 || bc.len() < 2 {
        // 单字符串退化为包含判定
        return if a.contains(b) || b.contains(a) {
            0.85
        } else {
            0.0
        };
    }
    let mut a_bigrams: std::collections::HashMap<(char, char), i32> =
        std::collections::HashMap::new();
    for w in ac.windows(2) {
        *a_bigrams.entry((w[0], w[1])).or_insert(0) += 1;
    }
    let mut inter = 0i32;
    for w in bc.windows(2) {
        if let Some(n) = a_bigrams.get_mut(&(w[0], w[1])) {
            if *n > 0 {
                *n -= 1;
                inter += 1;
            }
        }
    }
    2.0 * inter as f64 / (ac.len() + bc.len() - 2) as f64
}

/// 艺术家是否命中：全串相等 / 互含，或分词后存在 ≥3 字符词互匹配——
/// 容忍 "Coheed & Cambria" vs "coheed and cambria"（& / and / 标点差异）。
fn artist_matches(candidate: &str, hit_artists: &[String]) -> bool {
    let c = normalize_artist(candidate);
    if c.is_empty() {
        return false;
    }
    hit_artists.iter().any(|h| {
        let n = normalize_artist(h);
        if n.is_empty() {
            return false;
        }
        if n == c || n.contains(&c) || c.contains(&n) {
            return true;
        }
        // 分词级匹配（≥3 字符词，避免 "a"/"x" 之类的噪音词误命中）
        let cws: Vec<&str> = c
            .split_whitespace()
            .filter(|w| w.chars().count() >= 3)
            .collect();
        let nws: Vec<&str> = n
            .split_whitespace()
            .filter(|w| w.chars().count() >= 3)
            .collect();
        !cws.is_empty() && cws.iter().any(|w| nws.contains(w))
    })
}

/// 裁决：在候选 × 命中里选出最可信的 (候选方向, 搜索命中)。
///
/// 规则：标题相似度 ≥ 0.8，且（艺术家命中 或 时长差 ≤ 3s）才算确认；
/// 两者方向都确认时取分高者。全部不通过 → None（调用方保留本地结果）。
pub fn judge(
    candidates: &[Candidate],
    hits: &[SearchHit],
    duration_ms: u64,
) -> Option<(Candidate, SearchHit)> {
    let mut best: Option<(f64, Candidate, SearchHit)> = None;
    for cand in candidates {
        let ct = normalize_title(&cand.title);
        for hit in hits {
            let ht = normalize_title(&hit.name);
            let sim = similarity(&ct, &ht);
            if sim < TITLE_SIM_THRESHOLD {
                continue;
            }
            let artist_ok = artist_matches(&cand.artist, &hit.artists);
            let dur_ok = duration_ms > 0
                && hit.duration_ms > 0
                && (hit.duration_ms - duration_ms as i64).abs() <= DURATION_TOLERANCE_MS;
            // 确认条件：艺术家或时长至少一项对上；两者皆无信号时不采纳
            if !artist_ok && !dur_ok {
                continue;
            }
            let score = sim + if artist_ok { 0.3 } else { 0.0 } + if dur_ok { 0.2 } else { 0.0 };
            let better = match &best {
                None => true,
                Some((s, _, _)) => score > *s,
            };
            if better {
                best = Some((score, cand.clone(), hit.clone()));
            }
        }
    }
    best.map(|(_, c, h)| (c, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(name: &str, artists: &[&str], duration_ms: i64) -> SearchHit {
        SearchHit {
            id: 1,
            name: name.to_string(),
            artists: artists.iter().map(|s| s.to_string()).collect(),
            album: Some("某专辑".to_string()),
            duration_ms,
        }
    }

    fn cand(title: &str, artist: &str) -> Candidate {
        Candidate {
            title: title.to_string(),
            artist: artist.to_string(),
        }
    }

    #[test]
    fn judge_picks_correct_direction() {
        let cands = vec![
            cand("周杰伦 - 晴天", ""),
            cand("晴天", "周杰伦"),
            cand("周杰伦", "晴天"),
        ];
        let hits = vec![hit("晴天", &["周杰伦"], 269_000)];
        let (c, _) = judge(&cands, &hits, 269_000).expect("应命中");
        assert_eq!(c.title, "晴天");
        assert_eq!(c.artist, "周杰伦");
    }

    #[test]
    fn judge_duration_tiebreak_confirms_without_artist() {
        // 候选无艺术家信息，仅靠时长对齐确认
        let cands = vec![cand("Mandy", "")];
        let hits = vec![hit("Mandy", &["Westlife"], 233_000)];
        let (c, _) = judge(&cands, &hits, 232_500).expect("时长容差内应命中");
        assert_eq!(c.title, "Mandy");
        // 时长差超容差且无艺术家 → 不采纳
        assert!(judge(&cands, &hits, 300_000).is_none());
    }

    #[test]
    fn judge_rejects_low_similarity() {
        let cands = vec![cand("晴天", "周杰伦")];
        let hits = vec![hit("七里香", &["周杰伦"], 269_000)];
        assert!(judge(&cands, &hits, 269_000).is_none());
    }

    #[test]
    fn judge_matches_artist_loosely() {
        let cands = vec![cand("Welcome Home", "Coheed & Cambria")];
        // 命中艺术家带标点/大小写差异 → 归一化后仍命中
        let hits = vec![hit("Welcome Home", &["coheed and cambria"], 0)];
        assert!(judge(&cands, &hits, 0).is_some());
    }

    #[test]
    fn normalize_strips_brackets_and_punct() {
        assert_eq!(normalize_title("晴天 (Live)"), "晴天");
        assert_eq!(normalize_title("【高清】Hello, World!"), "helloworld");
    }

    #[test]
    fn similarity_prefers_close_titles() {
        let a = normalize_title("What a Wonderful World");
        assert!(similarity(&a, &normalize_title("What A Wonderful World")) > 0.99);
        assert!(similarity(&a, &normalize_title("晴天")) < 0.1);
    }
}
