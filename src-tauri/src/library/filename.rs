//! 实验室·元数据增强（enhanced-metadata）：本地文件名 / 标题拆分
//!
//! 无损音频常见「艺术家 - 歌曲」或「歌曲 - 艺术家」混写在文件名 / 标题里，
//! 分隔符可能是 `-`、`&`、多空格甚至没有（符号周围还可能带空格）。本模块把
//! 这类命名**先在本地拆成候选**（方向不明时给出两种顺序），由调用方采纳
//! 本地启发式结果，并把候选交后台在线校验（见 `super::verify`）裁决。
//!
//! 设计要点：
//!  - 纯函数、零 IO，各种命名变体可单测覆盖；
//!  - 分隔符分置信等级：带空格的连接号（` - `/` – `/` — `）高置信、必拆；
//!    裸 `-`、`_`、连续多空格、`&`、单空格为低置信，**需要方向信号**
//!    （父目录艺名匹配、标题关键词、前导曲目编号等）才本地采用，否则保持
//!    原样、仅把候选交在线裁决——避免把 "What a Wonderful World" 这类
//!    多词英文标题按第一个空格拦腰拆断；
//!  - 高置信分隔符优先命中，因此 "Green-Day - Holiday" 艺名内部的裸连字符
//!    不参与拆分（裸连字符只在完全没有带空格分隔符时才考虑）；
//!  - 方向启发：`分数 > 0` → 艺术家在前（`A - B`），`< 0` → 歌曲在前，
//!    `= 0` 且高置信 → 按更常见的「艺术家 - 歌曲」默认。

/// 拆分候选：一个方向下的 (标题, 艺术家原文)。
/// `artist` 可能为空字符串（方向不明且无父目录线索时交在线补）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub title: String,
    pub artist: String,
}

/// 解析上下文：来自标签与父目录的方向线索。
pub struct NameCtx<'a> {
    /// 标签艺术家（调用方保证未知时为 "" 或 "未知艺术家"）
    pub tag_artist: &'a str,
    /// 父目录推断的艺人（`infer_album_artist`，未知为 None）
    pub parent_artist: Option<&'a str>,
    /// 父目录推断的专辑
    pub parent_album: Option<&'a str>,
}

/// 拆分结果。
///
/// `local` = 本地立即采用的方向（None = 方向不明且分隔符低置信，保持原样）；
/// `candidates` = 交在线校验的全部候选（原串 + 两种方向），永不为空。
#[derive(Debug)]
pub struct Parsed {
    pub local: Option<Candidate>,
    pub candidates: Vec<Candidate>,
}

/// 分隔符类别（决定本地是否直接采用拆分）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SepClass {
    /// 带空格连接号 / 短横线（` - ` `- ` ` -`）——高置信
    SpacedDash,
    /// En/Em 短横线（`–` `—`，任意位置）——高置信
    EnDash,
    /// 裸连字符（无空格）——低置信
    BareDash,
    /// 下划线 `_` ——低置信
    UnderScore,
    /// 连续多空格（≥2）——低置信
    MultiSpace,
    /// `&`（带不带空格）——低置信
    Amp,
    /// 单个空格——低置信
    SingleSpace,
}

impl SepClass {
    /// 高置信：无论有无方向信号都直接采用本地拆分
    fn is_high(self) -> bool {
        matches!(self, SepClass::SpacedDash | SepClass::EnDash)
    }
}

/// 标题侧关键词（命中说明该侧更像标题）。用较长词根避免 "ft" 误伤。
const TITLE_KEYWORDS: &[&str] = &[
    "live", "remix", "remaster", "acoustic", "instrumental", "karaoke", "featur", "现场",
    "伴奏", "合唱",
];

fn eq_ignore(a: &str, b: &str) -> bool {
    !a.is_empty() && a.trim().eq_ignore_ascii_case(b.trim())
}

fn has_title_keyword(s: &str) -> bool {
    let low = s.to_lowercase();
    TITLE_KEYWORDS.iter().any(|k| low.contains(k))
}

/// 该侧是否「以曲目编号开头」（如 `01 Song`——编号后没跟分隔符、
/// 因此没被 [`strip_number_prefix`] 剥掉的情况）。纯数字串不算。
fn is_number_side(s: &str) -> bool {
    let b = s.as_bytes();
    matches!(b.first(), Some(c) if c.is_ascii_digit()) && !s.chars().all(|c| c.is_ascii_digit())
}

/// 剥离前导曲目编号：`01 - Song` / `01. Song` / `01_Song` / `01)Song` → `Song`。
///
/// 编号后必须紧跟（允许空格）`-._)]` 分隔符才剥——避免误伤 `1984`（年份
/// 开头歌曲）与 `24 Karat Magic` 这类数字开头标题；纯编号 / 剥完为空则不剥。
fn strip_number_prefix(s: &str) -> (&str, bool) {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && i < 3 && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || i >= b.len() {
        return (s, false);
    }
    let mut j = i;
    while j < b.len() && b[j] == b' ' {
        j += 1;
    }
    if j >= b.len() || !matches!(b[j], b'-' | b'.' | b'_' | b')' | b']') {
        return (s, false);
    }
    let mut k = j + 1;
    while k < b.len() && b[k] == b' ' {
        k += 1;
    }
    if k >= b.len() {
        return (s, false);
    }
    (s[k..].trim_start(), true)
}

/// 两侧（按 `[start, end)` 切分）trim 后均非空才算有效分隔点。
fn sides_ok(s: &str, start: usize, end: usize) -> bool {
    !s[..start].trim().is_empty() && !s[end..].trim().is_empty()
}

/// 按置信度从高到低找**首个**有效分隔点，返回其字节区间与类别。
///
/// 顺序刻意让位：En/Em 线 → 带空格短横线 → 裸连字符 → `_` → 多空格 →
/// `&` → 单空格。带空格短横线优先于裸连字符，保证 `Green-Day - Holiday`
/// 只在 ` - ` 处拆、不破坏艺名内部的连字符。
fn find_split(s: &str) -> Option<(usize, usize, SepClass)> {
    // 1) En/Em 短横线（多字节字符，任意位置）
    for (i, c) in s.char_indices() {
        if c == '–' || c == '—' {
            let end = i + c.len_utf8();
            if sides_ok(s, i, end) {
                return Some((i, end, SepClass::EnDash));
            }
        }
    }
    // 2) 带空格短横线：`-` 且相邻至少一侧为空格
    let mut idx = 0;
    while let Some(rel) = s[idx..].find('-') {
        let i = idx + rel;
        let prev = s[..i].chars().last();
        let next = s[i + 1..].chars().next();
        if (prev == Some(' ') || next == Some(' ')) && sides_ok(s, i, i + 1) {
            return Some((i, i + 1, SepClass::SpacedDash));
        }
        idx = i + 1;
    }
    // 3) 裸连字符（前后均非空格）
    let mut idx = 0;
    while let Some(rel) = s[idx..].find('-') {
        let i = idx + rel;
        let prev = s[..i].chars().last();
        let next = s[i + 1..].chars().next();
        if prev != Some(' ') && next != Some(' ') && sides_ok(s, i, i + 1) {
            return Some((i, i + 1, SepClass::BareDash));
        }
        idx = i + 1;
    }
    // 4) 下划线
    let mut idx = 0;
    while let Some(rel) = s[idx..].find('_') {
        let i = idx + rel;
        if sides_ok(s, i, i + 1) {
            return Some((i, i + 1, SepClass::UnderScore));
        }
        idx = i + 1;
    }
    // 5) 连续多空格（≥2）：命中后把整段空格作为分隔区间
    let mut idx = 0;
    while let Some(rel) = s[idx..].find("  ") {
        let start = idx + rel;
        let mut end = start;
        while end < s.len() && s.as_bytes()[end] == b' ' {
            end += 1;
        }
        if sides_ok(s, start, end) {
            return Some((start, end, SepClass::MultiSpace));
        }
        idx = end;
    }
    // 6) `&`
    let mut idx = 0;
    while let Some(rel) = s[idx..].find('&') {
        let i = idx + rel;
        if sides_ok(s, i, i + 1) {
            return Some((i, i + 1, SepClass::Amp));
        }
        idx = i + 1;
    }
    // 7) 单个空格（前后均非空格）
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b' ' {
            let prev = if i > 0 { Some(bytes[i - 1]) } else { None };
            let next = bytes.get(i + 1).copied();
            if prev != Some(b' ') && next != Some(b' ') && sides_ok(s, i, i + 1) {
                return Some((i, i + 1, SepClass::SingleSpace));
            }
        }
        i += 1;
    }
    None
}

/// 方向打分：`> 0` 左侧是艺术家（`A - B`），`< 0` 右侧是艺术家。
///
/// 信号（可叠加）：父目录 / 标艺名匹配 ±2、父专辑名匹配 ±1（该侧是专辑
/// 语义、更接近标题侧）、标题关键词 ±1、曲目编号侧 ±1。
fn direction_score(a: &str, b: &str, ctx: &NameCtx) -> i32 {
    let mut s = 0;
    if let Some(pa) = ctx.parent_artist {
        if eq_ignore(a, pa) {
            s += 2;
        }
        if eq_ignore(b, pa) {
            s -= 2;
        }
    }
    if !ctx.tag_artist.trim().is_empty() {
        if eq_ignore(a, ctx.tag_artist) {
            s += 2;
        }
        if eq_ignore(b, ctx.tag_artist) {
            s -= 2;
        }
    }
    if let Some(pal) = ctx.parent_album {
        if eq_ignore(a, pal) {
            s -= 1;
        }
        if eq_ignore(b, pal) {
            s += 1;
        }
    }
    if has_title_keyword(b) {
        s += 1;
    }
    if has_title_keyword(a) {
        s -= 1;
    }
    if is_number_side(a) {
        s -= 1;
    }
    if is_number_side(b) {
        s += 1;
    }
    s
}

/// 从文件名 / 标题拆出候选。返回 None 表示无可拆内容（无分隔符且无编号可剥）。
///
/// 仅做**本地启发式**：是否写库、写哪个方向由调用方按 `local` 决定；
/// `candidates` 全量交在线校验裁决（见 `super::verify::judge`）。
pub fn parse_stem(stem: &str, ctx: &NameCtx) -> Option<Parsed> {
    let stem = stem.trim();
    if stem.is_empty() {
        return None;
    }
    let (stripped, had_num) = strip_number_prefix(stem);
    let base = stripped.trim();
    if base.is_empty() {
        return None;
    }

    let Some((s, e, class)) = find_split(base) else {
        // 无分隔符：仅在确实剥掉了曲目编号时才有可写结果
        if !had_num {
            return None;
        }
        let only = Candidate {
            title: base.to_string(),
            artist: String::new(),
        };
        return Some(Parsed {
            local: Some(only.clone()),
            candidates: vec![only],
        });
    };

    let a = base[..s].trim();
    let b = base[e..].trim();
    if a.is_empty() || b.is_empty() {
        // 分隔点贴边（理论上 sides_ok 已挡）：退化为「仅剥编号」
        if !had_num {
            return None;
        }
        let only = Candidate {
            title: base.to_string(),
            artist: String::new(),
        };
        return Some(Parsed {
            local: Some(only.clone()),
            candidates: vec![only],
        });
    }

    let score = direction_score(a, b, ctx);
    // 低置信分隔符要求至少一个方向信号才本地拆，否则保持原样交在线裁决
    let apply = class.is_high() || score != 0;
    let left_is_artist = score >= 0; // 分数为 0 时默认「艺术家 - 歌曲」

    let local = if apply {
        let (title, artist) = if left_is_artist {
            (b, a)
        } else {
            (a, b)
        };
        Some(Candidate {
            title: title.to_string(),
            artist: artist.to_string(),
        })
    } else if had_num {
        // 未拆但剥掉了编号：只写干净标题，艺术家维持未知
        Some(Candidate {
            title: base.to_string(),
            artist: String::new(),
        })
    } else {
        None
    };

    // 在线候选：原串（保留父目录艺名参与比对）+ 两种方向
    let mut candidates = Vec::with_capacity(3);
    let orig = Candidate {
        title: base.to_string(),
        artist: ctx.parent_artist.unwrap_or_default().to_string(),
    };
    candidates.push(orig);
    let ab = Candidate {
        title: a.to_string(),
        artist: b.to_string(),
    };
    let ba = Candidate {
        title: b.to_string(),
        artist: a.to_string(),
    };
    if ab == ba {
        candidates.push(ab);
    } else {
        candidates.push(ab);
        candidates.push(ba);
    }

    Some(Parsed { local, candidates })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(parent_artist: Option<&'a str>, parent_album: Option<&'a str>) -> NameCtx<'a> {
        NameCtx {
            tag_artist: "",
            parent_artist,
            parent_album,
        }
    }

    /// 候选里是否含某方向（标题, 艺术家）
    fn has_dir(p: &Parsed, title: &str, artist: &str) -> bool {
        p.candidates.iter().any(|c| c.title == title && c.artist == artist)
    }

    #[test]
    fn strip_number_variants() {
        assert_eq!(strip_number_prefix("01 - Song"), ("Song", true));
        assert_eq!(strip_number_prefix("01. Song"), ("Song", true));
        assert_eq!(strip_number_prefix("01_Song"), ("Song", true));
        assert_eq!(strip_number_prefix("01-Song"), ("Song", true));
        assert_eq!(strip_number_prefix("01) Song"), ("Song", true));
        // 年份开头 / 纯编号 / 无编号不剥
        assert_eq!(strip_number_prefix("1984 - Song"), ("1984 - Song", false));
        assert_eq!(strip_number_prefix("24 Karat Magic"), ("24 Karat Magic", false));
        assert_eq!(strip_number_prefix("01"), ("01", false));
        assert_eq!(strip_number_prefix("晴天"), ("晴天", false));
    }

    #[test]
    fn high_confidence_split_always_applies() {
        // 带空格短横线：无任何方向信号也本地拆，默认「艺术家 - 歌曲」
        let p = parse_stem("周杰伦 - 晴天", &ctx(None, None)).unwrap();
        let local = p.local.clone().expect("高置信必拆");
        assert_eq!(local.title, "晴天");
        assert_eq!(local.artist, "周杰伦");
        // 两种方向都在在线候选里
        assert!(has_dir(&p, "晴天", "周杰伦"));
        assert!(has_dir(&p, "周杰伦", "晴天"));
    }

    #[test]
    fn parent_dir_decides_direction() {
        // 同一个目录下两种命名都收敛到 歌曲=晴天 / 艺术家=周杰伦
        let c = ctx(Some("周杰伦"), None);
        let p1 = parse_stem("周杰伦 - 晴天", &c).unwrap();
        assert_eq!(p1.local.unwrap().title, "晴天");
        let p2 = parse_stem("晴天 - 周杰伦", &c).unwrap();
        let l2 = p2.local.unwrap();
        assert_eq!(l2.title, "晴天");
        assert_eq!(l2.artist, "周杰伦");
    }

    #[test]
    fn number_prefix_marks_title_side() {
        // 编号没被剥（编号后是空格+字母），但作为「该侧是标题」的方向信号
        let p = parse_stem("01 Song - Westlife", &ctx(Some("Westlife"), None)).unwrap();
        let l = p.local.unwrap();
        assert_eq!(l.title, "01 Song");
        assert_eq!(l.artist, "Westlife");
        // 前导编号在标题侧被剥掉
        let p2 = parse_stem("01 - Song", &ctx(None, None)).unwrap();
        let l2 = p2.local.unwrap();
        assert_eq!(l2.title, "Song");
        assert!(l2.artist.is_empty(), "无方向信号时艺术家留空交在线");
    }

    #[test]
    fn bare_dash_applied_with_signal_only() {
        // 有父目录艺名信号：裸连字符也本地拆
        let p = parse_stem("Westlife-Mandy", &ctx(Some("Westlife"), None)).unwrap();
        let l = p.local.unwrap();
        assert_eq!(l.title, "Mandy");
        assert_eq!(l.artist, "Westlife");
        // 无信号：保持原样，但两种方向仍交在线裁决
        let p2 = parse_stem("Artist-Song", &ctx(None, None)).unwrap();
        assert!(p2.local.is_none(), "低置信无信号不本地拆");
        assert!(has_dir(&p2, "Song", "Artist"));
        assert!(has_dir(&p2, "Artist", "Song"));
    }

    #[test]
    fn green_day_kept_when_spaced_sep_present() {
        // 艺名内部的裸连字符不参与拆分：带空格短横线优先命中
        let p = parse_stem("Green-Day - Holiday", &ctx(None, None)).unwrap();
        let l = p.local.unwrap();
        assert_eq!(l.artist, "Green-Day");
        assert_eq!(l.title, "Holiday");
    }

    #[test]
    fn single_space_never_mangles_multiword_title() {
        // 无方向信号的多词英文标题保持原样（不按第一个空格腰斩）
        let p = parse_stem("What a Wonderful World", &ctx(None, None)).unwrap();
        assert!(p.local.is_none());
        // 但候选已备好（首空格拆分），在线可裁决
        assert!(has_dir(&p, "a Wonderful World", "What"));
        // 有父目录信号则本地拆
        let p2 = parse_stem("Westlife Mandy", &ctx(Some("Westlife"), None)).unwrap();
        let l2 = p2.local.unwrap();
        assert_eq!(l2.title, "Mandy");
        assert_eq!(l2.artist, "Westlife");
    }

    #[test]
    fn amp_and_underscore_with_signal() {
        let c = ctx(Some("Simon"), None);
        let p = parse_stem("Simon & Garfunkel", &c).unwrap();
        let l = p.local.unwrap();
        assert_eq!(l.artist, "Simon");
        assert_eq!(l.title, "Garfunkel");
        let p2 = parse_stem("Artist_Song", &ctx(Some("Artist"), None)).unwrap();
        let l2 = p2.local.unwrap();
        assert_eq!(l2.artist, "Artist");
        assert_eq!(l2.title, "Song");
    }

    #[test]
    fn multi_space_with_signal() {
        let p = parse_stem("周杰伦  晴天", &ctx(Some("周杰伦"), None)).unwrap();
        let l = p.local.unwrap();
        assert_eq!(l.artist, "周杰伦");
        assert_eq!(l.title, "晴天");
    }

    #[test]
    fn first_split_point_wins() {
        let p = parse_stem("A - B - C", &ctx(None, None)).unwrap();
        let l = p.local.unwrap();
        assert_eq!(l.artist, "A");
        assert_eq!(l.title, "B - C");
    }

    #[test]
    fn no_split_returns_none() {
        assert!(parse_stem("晴天", &ctx(None, None)).is_none());
        assert!(parse_stem("01", &ctx(None, None)).is_none());
        assert!(parse_stem("", &ctx(None, None)).is_none());
        // 无任何分隔符的单段标题 → 无可拆内容
        assert!(parse_stem("HighwayToHell", &ctx(None, None)).is_none());
    }

    #[test]
    fn number_only_split_applies_strip() {
        // 「编号 - 歌名」：编号在找分隔点之前就被剥掉 → 标题干净、艺术家留空交在线
        let p = parse_stem("01 - Song", &ctx(Some("某艺人"), None)).unwrap();
        let l = p.local.unwrap();
        assert_eq!(l.title, "Song");
        assert!(l.artist.is_empty(), "编号侧不充当艺术家，留空交在线补全");
    }
}
