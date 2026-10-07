//! 在线歌词抓取（默认关闭，受设置 online.enabled 控制）
//!
//! 数据流：搜索（标题+艺术家）→ 按时长选最佳匹配 → 取 LRC 歌词 →
//! 合并翻译行为同时间戳行（本应用 LRC 解析器约定的双语格式）→
//! 返回合并后的 LRC 文本，由调用方写为 .lrc 边车（下次本地命中，不再请求网络）。
//!
//! 不上传任何曲库信息：仅以当前曲目的标题/艺术家文本作为搜索关键词。

use serde_json::Value;

/// 网易云音乐公开接口（provider = netease）
const SEARCH_URL: &str = "https://music.163.com/api/search/get";
const LYRIC_URL: &str = "https://music.163.com/api/song/lyric";
/// 搜索关键词与请求头
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) TPlayerNext/2.0";
/// 时长匹配容差（毫秒）：搜索结果与本地时长差在此范围内优先选用
const DURATION_TOLERANCE_MS: i64 = 3000;

fn http_client() -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .ok()
}

/// 搜索候选歌曲：返回 (歌曲 ID, 时长毫秒)
async fn search_song(
    client: &reqwest::Client,
    title: &str,
    artist: &str,
) -> Option<Vec<(u64, i64)>> {
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
        let id = song.get("id")?.as_u64()?;
        let duration = song.get("duration").and_then(|d| d.as_i64()).unwrap_or(0);
        out.push((id, duration));
    }
    Some(out)
}

/// 取歌词：返回 (原文 LRC, 翻译 LRC)
async fn fetch_lyric_text(client: &reqwest::Client, song_id: u64) -> Option<(String, String)> {
    let resp = client
        .get(LYRIC_URL)
        .query(&[
            ("id", song_id.to_string()),
            ("lv", "1".to_string()),
            ("kv", "1".to_string()),
            ("tv", "-1".to_string()),
        ])
        .header("Referer", "https://music.163.com")
        .send()
        .await
        .ok()?;
    let json: Value = resp.json().await.ok()?;
    let lrc = json
        .get("lrc")
        .and_then(|v| v.get("lyric"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if lrc.trim().is_empty() {
        return None;
    }
    let tlyric = json
        .get("tlyric")
        .and_then(|v| v.get("lyric"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Some((lrc, tlyric))
}

/// 提取 LRC 文本中的 (时间标签毫秒, 文本) 行（宽松解析，供翻译合并使用）
fn extract_timed(content: &str) -> Vec<(u64, String)> {
    let re = match regex::Regex::new(r"\[\s*(\d+)\s*:\s*(\d+(?:[.,]\d+)?)\s*\]") {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for line in content.lines() {
        let matches: Vec<_> = re.captures_iter(line).collect();
        if matches.is_empty() {
            continue;
        }
        let text = re.replace_all(line, "").trim().to_string();
        if text.is_empty() {
            continue;
        }
        if let Some(cap) = matches.first() {
            let m = cap[1].trim().parse::<u64>().unwrap_or(0);
            let s = cap[2].replace(',', ".").trim().parse::<f64>().unwrap_or(0.0);
            out.push((((m as f64 * 60.0 + s) * 1000.0) as u64, text));
        }
    }
    out
}

/// 合并原文与翻译：翻译按相同时间戳插在原文行之后
/// （parser.rs 的解析约定：相邻同时间行的第二行作为翻译）
fn merge_translation(lrc: &str, tlyric: &str) -> String {
    let translations = extract_timed(tlyric);
    if translations.is_empty() {
        return lrc.to_string();
    }
    let tmap: std::collections::HashMap<u64, String> = translations.into_iter().collect();

    let mut out = String::new();
    for line in lrc.lines() {
        out.push_str(line);
        out.push('\n');
        // 找本行第一个时间标签，查翻译
        for (t, text) in extract_timed(line) {
            if let Some(translation) = tmap.get(&t) {
                out.push_str(&format!(
                    "{}{}\n",
                    super::parser::format_lrc_time(t),
                    translation
                ));
            }
            let _ = text;
        }
    }
    out
}

/// 在线匹配歌词：成功返回合并后的 LRC 文本；无结果/网络失败返回 None
pub async fn fetch_lyrics(title: &str, artists: &[String], duration_ms: u64) -> Option<String> {
    let client = http_client()?;
    let artist = artists.first().cloned().unwrap_or_default();
    let candidates = search_song(&client, title, &artist).await?;
    if candidates.is_empty() {
        return None;
    }

    // 时长匹配优先（容差内取最近），否则用首个搜索结果
    let picked = if duration_ms > 0 {
        candidates
            .iter()
            .filter(|(_, d)| (*d - duration_ms as i64).abs() <= DURATION_TOLERANCE_MS)
            .min_by_key(|(_, d)| (*d - duration_ms as i64).abs())
            .or(candidates.first())
    } else {
        candidates.first()
    };
    let (song_id, _) = *picked?;

    let (lrc, tlyric) = fetch_lyric_text(&client, song_id).await?;
    Some(merge_translation(&lrc, &tlyric))
}
