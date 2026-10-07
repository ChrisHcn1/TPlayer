//! 封面解析链（本地优先，与歌词链同构）
//!
//! 优先级：
//!   1. 音频内嵌标签图片（ID3 APIC / FLAC PICTURE / MP4 covr 等，lofty 读取）；
//!   2. 同目录同名边车图片（song.flac → song.jpg / .png / .webp ...）；
//!   3. 在线匹配（网易云专辑封面，受设置 online.enabled 控制，默认关闭；
//!      成功后由命令层写入应用数据目录的封面缓存，下次直接本地命中）；
//!   4. 全部未命中 → None，由前端回落到内置默认封面。
//!
//! 不上传任何曲库信息：在线阶段仅以当前曲目标题/艺术家文本作为搜索关键词。

use std::path::{Path, PathBuf};

use lofty::file::TaggedFileExt;
use serde_json::Value;

use crate::library::models::Track;

/// 同名边车图片支持的扩展名（按常见度排序，命中即停）
const SIDECAR_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp"];

/// 边车扩展名 → MIME
fn mime_from_ext(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => "image/jpeg",
    }
}

/// MIME → 缓存文件扩展名
pub fn ext_for_mime(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        _ => "jpg",
    }
}

/// 标签未声明 MIME 时按文件头魔数兜底判断
pub fn sniff_image_mime(data: &[u8]) -> &'static str {
    if data.len() >= 3 && &data[0..3] == b"\xff\xd8\xff" {
        "image/jpeg"
    } else if data.len() >= 8 && &data[0..8] == b"\x89PNG\r\n\x1a\n" {
        "image/png"
    } else if data.len() >= 6 && (&data[0..6] == b"GIF87a" || &data[0..6] == b"GIF89a") {
        "image/gif"
    } else if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        "image/webp"
    } else if data.len() >= 2 && &data[0..2] == b"BM" {
        "image/bmp"
    } else {
        // 音频标签内嵌封面以 JPEG 居多，作为默认 MIME
        "image/jpeg"
    }
}

const B64_TABLE: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// 标准 Base64 编码（封面为内嵌/下载的二进制，数据量小，避免再引入一个依赖）
pub fn base64_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let triple = ((chunk[0] as u32) << 16)
            | ((*chunk.get(1).unwrap_or(&0) as u32) << 8)
            | (*chunk.get(2).unwrap_or(&0) as u32);
        out.push(B64_TABLE[((triple >> 18) & 63) as usize] as char);
        out.push(B64_TABLE[((triple >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64_TABLE[((triple >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64_TABLE[(triple & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// 二进制封面 → 可直接用于 `<img src>` 的 data URL
pub fn to_data_url(mime: &str, data: &[u8]) -> String {
    format!("data:{};base64,{}", mime, base64_encode(data))
}

/// 1. 内嵌标签封面：返回 (MIME, 字节)
fn embedded_cover(path: &Path) -> Option<(&'static str, Vec<u8>)> {
    let tagged = lofty::read_from_path(path).ok()?;
    // 主标签优先，其次遍历其余标签（部分格式封面只写在非主标签）
    let picture = tagged
        .primary_tag()
        .and_then(|tag| tag.pictures().first())
        .or_else(|| tagged.tags().iter().find_map(|tag| tag.pictures().first()))?;
    let data = picture.data();
    if data.is_empty() {
        return None;
    }
    // 统一按文件头嗅探：标签声明的 MIME 偶有缺失或错误
    Some((sniff_image_mime(data), data.to_vec()))
}

/// 2. 同名边车图片：song.flac → song.jpg/.png/.webp/...
fn sidecar_cover(track: &Track) -> Option<(&'static str, Vec<u8>)> {
    let audio = PathBuf::from(&track.file_path);
    let stem = audio.file_stem()?;
    let dir = audio.parent()?;
    for ext in SIDECAR_EXTS {
        let candidate = dir.join(stem).with_extension(ext);
        if let Ok(data) = std::fs::read(&candidate) {
            if !data.is_empty() {
                return Some((mime_from_ext(ext), data));
            }
        }
    }
    None
}

/// 目录级整专辑封面的常见文件名（大量抓轨资源以 folder.jpg/cover.jpg 存封面，
/// 曲目本身不带内嵌图）。按常见度排序，命中即停。
const DIR_COVER_STEMS: &[&str] = &[
    "folder", "cover", "front", "album", "albumart", "albumartsmall", "folderlarge",
];

/// 2b. 目录级封面：曲目所在目录下的 folder.jpg / cover.png / ...
/// Windows 文件系统不区分大小写，跨平台时遇到 Folder.JPG 也兼容。
fn directory_cover(track: &Track) -> Option<(&'static str, Vec<u8>)> {
    let dir = PathBuf::from(&track.file_path).parent()?.to_path_buf();

    // 先按约定名精确匹配（各扩展名轮一遍）
    for stem in DIR_COVER_STEMS {
        for ext in SIDECAR_EXTS {
            let candidate = dir.join(format!("{}.{}", stem, ext));
            if let Ok(data) = std::fs::read(&candidate) {
                if !data.is_empty() {
                    return Some((mime_from_ext(ext), data));
                }
            }
        }
    }

    // 兜底：目录下文件名（小写）含 cover/folder/front 的首张图片，
    // 解决 Cover.jpg、cd-cover.jpeg、folder-front.png 等变体。
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let is_image = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| SIDECAR_EXTS.contains(&ext.to_ascii_lowercase().as_str()))
                .unwrap_or(false);
            let looks_like_cover = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| {
                    let lower = s.to_ascii_lowercase();
                    lower.contains("cover") || lower.contains("folder") || lower.contains("front")
                })
                .unwrap_or(false);
            is_image && looks_like_cover
        })
        .collect();
    entries.sort();
    for candidate in entries {
        if let Ok(data) = std::fs::read(&candidate) {
            if !data.is_empty() {
                let mime = candidate
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(mime_from_ext)
                    .unwrap_or("image/jpeg");
                return Some((mime, data));
            }
        }
    }
    None
}

/// 单曲目本地封面链（不联网）：内嵌 → 同名边车 → 目录级封面
fn local_cover_for_track(track: &Track) -> Option<(&'static str, Vec<u8>)> {
    let path = PathBuf::from(&track.file_path);
    embedded_cover(&path)
        .or_else(|| sidecar_cover(track))
        .or_else(|| directory_cover(track))
}

/// 分组（专辑/艺术家）封面解析：
///   1. 候选曲目已由命令层按「带内嵌封面优先、音轨号升序」排好；
///   2. 逐首走本地链（最多 MAX_LOCAL_CANDIDATES 首），任一首命中即共用；
///   3. 全部本地无图且允许在线时，仅用代表曲目做一次在线匹配（同专辑封面一致，
///      无需逐首联网）。
pub async fn resolve_group_cover(
    tracks: &[Track],
    online_enabled: bool,
) -> Option<(&'static str, Vec<u8>)> {
    /// 大拼盘专辑也只尝试前若干首，避免几百首曲目逐文件解析
    const MAX_LOCAL_CANDIDATES: usize = 30;

    for track in tracks.iter().take(MAX_LOCAL_CANDIDATES) {
        if let Some(found) = local_cover_for_track(track) {
            return Some(found);
        }
    }

    if online_enabled {
        if let Some(representative) = tracks.first() {
            if let Some(found) = online_cover(representative).await {
                return Some(found);
            }
        }
    }
    None
}

// ---- 在线匹配（网易云） -------------------------------------------------

const SEARCH_URL: &str = "https://music.163.com/api/search/get";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) TPlayerNext/2.0";
/// 时长匹配容差（毫秒）：与歌词在线匹配保持一致
const DURATION_TOLERANCE_MS: i64 = 3000;

fn http_client() -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .ok()
}

/// 搜索候选歌曲：返回 (专辑封面 URL, 时长毫秒)；按时长接近度排序
async fn search_covers(
    client: &reqwest::Client,
    title: &str,
    artist: &str,
) -> Option<Vec<(String, i64)>> {
    let keyword = if artist.is_empty() || artist == "未知艺术家" {
        title.to_string()
    } else {
        format!("{} {}", title, artist)
    };
    let resp = client
        .get(SEARCH_URL)
        .query(&[("s", keyword.as_str()), ("type", "1"), ("limit", "10")])
        .header("Referer", "https://music.163.com")
        .send()
        .await
        .ok()?;
    let json: Value = resp.json().await.ok()?;
    let songs = json.get("result")?.get("songs")?.as_array()?;
    let mut out = Vec::new();
    for song in songs {
        let pic = song
            .get("album")
            .and_then(|album| album.get("picUrl"))
            .and_then(|url| url.as_str())
            .unwrap_or("");
        if pic.is_empty() {
            continue;
        }
        let duration = song.get("duration").and_then(|d| d.as_i64()).unwrap_or(0);
        out.push((pic.to_string(), duration));
    }
    Some(out)
}

/// 下载封面图片字节
async fn download(client: &reqwest::Client, url: &str) -> Option<Vec<u8>> {
    // 网易云部分结果为 http 协议，统一升级为 https
    let url = url.replace("http://", "https://");
    let resp = client
        .get(&url)
        .header("Referer", "https://music.163.com")
        .send()
        .await
        .ok()?;
    let bytes = resp.bytes().await.ok()?;
    if bytes.is_empty() {
        None
    } else {
        Some(bytes.to_vec())
    }
}

/// 3. 在线封面：搜索 → 时长匹配 → 下载，成功返回 (MIME, 字节)
async fn online_cover(track: &Track) -> Option<(&'static str, Vec<u8>)> {
    let client = http_client()?;
    let artist = track.artists.first().cloned().unwrap_or_default();
    let candidates = search_covers(&client, &track.title, &artist).await?;
    if candidates.is_empty() {
        return None;
    }
    let duration_ms = (track.duration * 1000.0) as i64;
    let picked = if duration_ms > 0 {
        candidates
            .iter()
            .filter(|(_, d)| (*d - duration_ms).abs() <= DURATION_TOLERANCE_MS)
            .min_by_key(|(_, d)| (*d - duration_ms).abs())
            .or(candidates.first())
    } else {
        candidates.first()
    }?;
    let data = download(&client, &picked.0).await?;
    Some((sniff_image_mime(&data), data))
}

///
/// 完整封面解析链（不含磁盘缓存，缓存由命令层管理）。
/// 顺序：内嵌 → 同名边车 → 目录级封面 → 在线（online_enabled 时）。
/// 成功返回 (MIME, 字节)，全链未命中返回 None。
pub async fn resolve_cover(
    track: &Track,
    online_enabled: bool,
) -> Option<(&'static str, Vec<u8>)> {
    if let Some(found) = local_cover_for_track(track) {
        return Some(found);
    }
    if online_enabled {
        if let Some(found) = online_cover(track).await {
            return Some(found);
        }
    }
    None
}
