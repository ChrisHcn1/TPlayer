//! 曲库扫描：遍历目录 + lofty 元数据解析

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;

use super::models::{album_id_of, ScanProgress, ScanStage, Track};

/// 扫描支持的扩展名（索引范围 ≥ 播放范围：dsd/dts/ape 等可入库展示，
/// 播放时若引擎无法解码会以明确错误事件反馈）
pub(crate) const SUPPORTED_EXT: &[&str] = &[
    // 可直接解码播放（rodio/symphonia）
    "mp3", "flac", "wav", "ogg", "oga", "opus", "aac", "m4a",
    // 可索引（lofty 读取标签或回退文件名），播放需后续解码后端
    "wma", "aif", "aiff", "ape", "wv", "tta", "tak", "dsf", "dff", "dts",
];

/// 增量扫描结果
pub struct ScanResult {
    /// 新增 / 文件已变更（mtime 变化）的曲目
    pub upserts: Vec<Track>,
    /// 磁盘上已不存在、需从库中移除的曲目 id
    pub removed_ids: Vec<String>,
    /// 本次实际重新解析的文件数（未变文件直接跳过）
    pub changed_count: usize,
}

/// 判断文件扩展名是否在支持列表内。
///
/// Linux：按原始字节做 ASCII 大小写无关匹配——NTFS 上的非常规文件名
/// （非 UTF-8）经 `to_str()` 会返回 `None` 被漏判，字节匹配保证可入库。
/// 其它平台：保持原有 `to_str()` 语义（Windows 盘符路径，行为不变）。
#[cfg(target_os = "linux")]
pub(crate) fn ext_supported(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Some(ext) = path.extension() else {
        return false;
    };
    let bytes = ext.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    SUPPORTED_EXT.iter().any(|want| {
        bytes.len() == want.len()
            && bytes
                .iter()
                .zip(want.as_bytes())
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
    })
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn ext_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| SUPPORTED_EXT.contains(&ext.to_lowercase().as_str()))
}

/// 系统 / NAS 的回收站、缩略图与快照目录名（按小写比较）。
///
/// 这些目录里的音频是**已删除文件的副本**（群晖 `#recycle`、Windows
/// `$RECYCLE.BIN`、macOS `.Trashes`、Synology 缩略图 `@eaDir` 等）。
/// 若被扫入曲库，会与真实歌曲形成大量重复，且标签常与原件不同（去重键
/// 折不掉），因此一律不纳入扫描。
fn is_junk_dir_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n == "#recycle"
        || n == "#snapshot"
        || n == "@eadir"
        || n == "$recycle.bin"
        || n == "system volume information"
        || n == "lost+found"
        || n == ".trashes"
        || n.starts_with(".trash") // .Trash / .Trash-1000 / .Trashes
}

/// 路径任一段命中垃圾目录名即视为垃圾路径。
fn is_junk_path(path: &Path) -> bool {
    path.components().any(|c| match c {
        std::path::Component::Normal(s) => s.to_str().is_some_and(is_junk_dir_name),
        _ => false,
    })
}

/// 过滤掉落在垃圾目录内的条目（各 `discover_*` 返回前统一调用）。
fn without_junk(mut files: Vec<PathBuf>) -> Vec<PathBuf> {
    files.retain(|p| !is_junk_path(p));
    files
}

/// 递归发现目录下所有音频文件
///
/// Linux：走平台适配版 [`super::scan_linux::discover_files_checked`]——
/// 扩展名按字节匹配（容忍 NTFS 非 UTF-8 文件名），目录错误另行收集
/// （见 `scan_directory` 的 `progress.error` 上报），保证文件完整发现。
/// 其它平台：原栈式遍历（Windows 盘符路径语义，行为不变）。
#[cfg(target_os = "linux")]
pub(crate) fn discover_audio_files(root: &Path) -> Vec<PathBuf> {
    without_junk(super::scan_linux::discover_files_checked(root, SUPPORTED_EXT).files)
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn discover_audio_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if SUPPORTED_EXT.contains(&ext.to_lowercase().as_str()) {
                    files.push(path);
                }
            }
        }
    }
    without_junk(files)
}

/// 递归发现目录下所有 .cue 索引文件（扫描与拖拽导入通用，Release 同样启用）。
///
/// Linux：同样走平台适配版（字节级扩展名匹配 + 错误收集）。
#[cfg(target_os = "linux")]
pub(crate) fn discover_cue_files(root: &Path) -> Vec<PathBuf> {
    without_junk(super::scan_linux::discover_files_checked(root, &["cue"]).files)
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn discover_cue_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("cue") {
                    files.push(path);
                }
            }
        }
    }
    without_junk(files)
}

/// 递归发现目录下所有 .iso 文件（用于 SACD ISO 分轨；Release 也启用，不限 debug）。
///
/// 所有 .iso 都会先收集到 scan_directory 的 `referenced` 中被排除，避免被当作
/// 普通音频 parse_track 误入库；其中能成功解析为 SACD 的才进一步拆出虚拟分轨，
/// 非 SACD 镜像（系统盘 / 游戏 ISO）静默忽略。
#[cfg(target_os = "linux")]
fn discover_sacd_files(root: &Path) -> Vec<PathBuf> {
    without_junk(super::scan_linux::discover_files_checked(root, &["iso"]).files)
}

#[cfg(not(target_os = "linux"))]
fn discover_sacd_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("iso") {
                    files.push(path);
                }
            }
        }
    }
    without_junk(files)
}

/// 列出**本目录**（不递归）下的 .cue 文件。
///
/// 后台补解析按「镜像所在目录」就近查找 CUE：同一目录的多个镜像共用一份
/// 解析结果（目录级缓存），避免逐文件重复遍历整棵目录树。
pub(crate) fn cue_files_in_dir(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_cue = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("cue"));
        if is_cue && path.is_file() {
            files.push(path);
        }
    }
    files
}

/// 生成稳定 ID（基于文件路径的 SHA-256 前 16 位 hex）
fn track_id_from_path(path: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    let result = hasher.finalize();
    result.iter().take(8).map(|b| format!("{:02x}", b)).collect()
}

/// 解析单个音频文件的元数据
pub(crate) fn parse_track(path: &Path) -> Option<Track> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let file_size = metadata.len();
    let format = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    let id = track_id_from_path(path);

    // 用 lofty 读取标签
    let (
        title,
        artists,
        album,
        track_number,
        year,
        mut duration,
        bitrate,
        sample_rate,
        has_cover,
        replaygain_db,
        replaygain_peak,
    ) = match lofty::read_from_path(path) {
        Ok(file) => {
            let tag = file.primary_tag().cloned();
            let props = file.properties();
            let title = tag
                .as_ref()
                .and_then(|t| t.title().map(|s| s.to_string()))
                .unwrap_or_else(|| {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "未知标题".to_string())
                });
            let artists = tag
                .as_ref()
                .and_then(|t| t.artist().map(|a| split_artists(a.as_ref())))
                .unwrap_or_else(|| vec!["未知艺术家".to_string()]);
            let album = tag
                .as_ref()
                .and_then(|t| t.album().map(|s| s.to_string()))
                .unwrap_or_else(|| "未知专辑".to_string());
            let track_number = tag.as_ref().and_then(|t| t.track());
            let year = tag.as_ref().and_then(|t| t.year());
            let duration = props.duration().as_secs_f64();
            let bitrate = props.audio_bitrate();
            let sample_rate = props.sample_rate();
            let has_cover = tag.as_ref().map(|t| !t.pictures().is_empty()).unwrap_or(false);
            // ReplayGain 内嵌标签：优先取 TRACK 级增益与峰值（形如 "+3.21 dB" / "0.987654"）
            let (replaygain_db, replaygain_peak) = match tag.as_ref() {
                Some(t) => {
                    let gain = t
                        .get_strings(&lofty::tag::ItemKey::ReplayGainTrackGain)
                        .find_map(parse_replaygain_gain_db);
                    let peak = t
                        .get_strings(&lofty::tag::ItemKey::ReplayGainTrackPeak)
                        .find_map(parse_replaygain_peak);
                    (gain, peak)
                }
                None => (None, None),
            };
            (
                title,
                artists,
                album,
                track_number,
                year,
                duration,
                bitrate,
                sample_rate,
                has_cover,
                replaygain_db,
                replaygain_peak,
            )
        }
        Err(_) => (
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "未知标题".to_string()),
            vec!["未知艺术家".to_string()],
            "未知专辑".to_string(),
            None,
            None,
            0.0,
            None,
            None,
            false,
            None,
            None,
        ),
    };

    // 时长兜底：lofty 无法解析的格式（dsf/dff/dts/tta 等）用 ffprobe 探测，
    // 否则 duration=0 会导致播放进度不推进、自动连播失效
    if duration <= 0.0 {
        if let Some(probed) = crate::player::engine::probe_duration(path) {
            duration = probed;
        }
    }

    // 专辑稳定 ID 与 Album 聚合同规则（主艺术家 = artists[0]）
    let first_artist = artists.first().map(String::as_str).unwrap_or("");
    let album_id = Some(album_id_of(&album, first_artist));

    Some(Track {
        id,
        title,
        artists,
        album,
        album_id,
        duration,
        track_number,
        year,
        file_path: path.to_string_lossy().to_string(),
        file_size,
        format,
        bitrate,
        sample_rate,
        has_cover,
        replaygain_db,
        replaygain_peak,
        added_at: chrono::Utc::now().timestamp(),
        modified_at,
        is_cue: false,
        cue_offset: None,
        is_sacd: false,
        sacd_track_index: 0,
        sacd_area: None,
        sacd_dsd_rate: None,
        metadata_pending: false,
        iso_placeholder: false,
    })
}

/// 解析 ReplayGain TRACK_GAIN 标签（形如 "+3.21 dB" / "-2.5 dB" / "0.00 dB"）
fn parse_replaygain_gain_db(raw: &str) -> Option<f32> {
    let trimmed = raw.trim();
    let numeric = trimmed
        .trim_end_matches("dB")
        .trim_end_matches("db")
        .trim_end_matches("DB")
        .trim();
    numeric.parse::<f32>().ok().map(|v| v.clamp(-24.0, 24.0))
}

/// 解析 ReplayGain TRACK_PEAK 标签（形如 "0.987654" 或 "0.98765 dBFS"）
fn parse_replaygain_peak(raw: &str) -> Option<f32> {
    let trimmed = raw.trim();
    let numeric = trimmed
        .trim_end_matches("dBFS")
        .trim_end_matches("dbfs")
        .trim();
    numeric.parse::<f32>().ok().map(|v| v.clamp(0.0, 10.0))
}

fn split_artists(raw: &str) -> Vec<String> {
    raw.split(['/', ';', ','])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// 读取文件 mtime（秒）；无法读取时返回 0（退化为总是重新解析）
fn file_mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

///
/// 执行增量扫描并推送进度事件。
///
/// `existing` 为库内 (id, file_path, modified_at) 基线：
///  - mtime 未变的音频文件跳过元数据解析（大曲库二次扫描几乎零成本）；
///  - 新增 / 变更音频文件重新解析进 upserts；
///  - CUE 分轨每轮重新解析（id 稳定，upsert 会保留 added_at）；
///  - 基线中磁盘已消失的曲目（按 id 判定）进 removed_ids。
pub fn scan_directory(
    roots: &[PathBuf],
    existing: &[(String, String, i64)],
    app: &AppHandle,
) -> ScanResult {
    let mut progress = ScanProgress {
        stage: ScanStage::Discovering,
        processed: 0,
        total: 0,
        current_path: None,
        error: None,
    };
    emit(app, &progress);

    // 基线索引：path → (id, mtime)；用于音频文件增量跳过
    let existing_by_path: std::collections::HashMap<String, (String, i64)> = existing
        .iter()
        .map(|(id, path, mtime)| (path.clone(), (id.clone(), *mtime)))
        .collect();

    // 发现阶段：音频文件
    let mut all_audio = Vec::new();
    // Linux：收集无法读取的目录（NTFS 权限等），扫描结束时经 progress.error 上报，
    // 避免「部分目录被静默跳过」导致曲目缺失且无提示（Windows 无此概念，保持原行为）
    #[cfg(target_os = "linux")]
    let mut dir_errors: Vec<(String, String)> = Vec::new();
    for root in roots {
        #[cfg(target_os = "linux")]
        {
            let discovery = super::scan_linux::discover_files_checked(root, SUPPORTED_EXT);
            all_audio.extend(discovery.files);
            dir_errors.extend(discovery.errors);
        }
        #[cfg(not(target_os = "linux"))]
        {
            all_audio.extend(discover_audio_files(root));
        }
    }

    // 虚拟分轨（CUE + SACD）：解析为多条 Track，整盘镜像 / ISO 进 referenced 排除，
    // 避免与分轨重复入库。
    let mut virtual_tracks: Vec<Track> = Vec::new();
    let mut referenced: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    // 「整碟镜像」判定键（目录 + 去扩展名的文件名）：同一张碟的其它副本
    // （如 .ape 与 .flac 同基名）同样不应作为普通曲目入库。
    let mut referenced_discs: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();

    // CUE 分轨（已验证稳定，Release 同样启用，与 SACD 行为一致）
    {
        let mut cue_files = Vec::new();
        for root in roots {
            cue_files.extend(discover_cue_files(root));
        }
        for cue in &cue_files {
            let tracks = super::cue::parse_cue(cue);
            for t in &tracks {
                referenced.insert(norm_path_for_match(std::path::Path::new(&t.file_path)));
                if let Some(key) = super::models::disc_key(&t.file_path) {
                    referenced_discs.insert(key);
                }
            }
            // CUE 解析不出分轨时（编码 / 路径不匹配等），退化为按 CUE 文件自身的
            // 基名排除同目录同名的整碟副本，保证镜像不与分轨重复入库。
            if tracks.is_empty() {
                if let Some(key) = super::models::disc_key(&cue.to_string_lossy()) {
                    referenced_discs.insert(key);
                }
            }
            virtual_tracks.extend(tracks);
        }
    }

    // SACD ISO 分轨（Release 也启用：整张 ISO 拆成虚拟多轨专辑）
    {
        let mut iso_files = Vec::new();
        for root in roots {
            iso_files.extend(discover_sacd_files(root));
        }
        // 所有 .iso 先排除，避免被当作普通音频 parse_track 误入库
        for iso in &iso_files {
            referenced.insert(norm_path_for_match(iso));
        }
        // "只做 ISO"：按 ISO 所在目录收集其音轨标题（归一化），用于剔除同目录下
        // 对同一张 ISO 手动解包产出的散落音频（.dsf/.dff 或标题匹配某条 ISO 音轨），
        // 避免同一首歌以「ISO 虚拟轨 + 散落文件」两种来源重复入库。
        let mut sacd_titles_by_dir: std::collections::HashMap<
            PathBuf,
            std::collections::HashSet<String>,
        > = std::collections::HashMap::new();
        for iso in &iso_files {
            if let Some(mut disc) = super::sacd::SacdDisc::open(iso) {
                // DST 压缩区按数据量推算会把时长压低（仅 ~40%），用 -P 探测真实时长
                disc.probe_durations();
                let tracks = disc.to_tracks();
                if let Some(parent) = iso.parent() {
                    let set = sacd_titles_by_dir
                        .entry(parent.to_path_buf())
                        .or_default();
                    for t in &tracks {
                        let n = normalize_title_for_dedup(&t.title);
                        if !n.is_empty() {
                            set.insert(n);
                        }
                    }
                }
                virtual_tracks.extend(tracks);
            }
            // 非 SACD 镜像（系统盘 / 游戏 ISO）已排除且静默忽略
        }
        retain_sacd_derived(&mut all_audio, &sacd_titles_by_dir);
    }

    // 排除被 CUE / SACD 引用的镜像文件（规范化后比较，避免路径表示差异漏配，
    // 否则整碟大文件会与分轨重复出现在播放列表）；同目录同基名的其它镜像副本
    // （.ape / .flac 等）同样排除。
    if !referenced.is_empty() || !referenced_discs.is_empty() {
        all_audio.retain(|p| {
            if referenced.contains(&norm_path_for_match(p)) {
                return false;
            }
            match super::models::disc_key(&p.to_string_lossy()) {
                Some(key) => !referenced_discs.contains(&key),
                None => true,
            }
        });
    }

    progress.total = (all_audio.len() + virtual_tracks.len()) as u32;
    progress.stage = ScanStage::Parsing;
    emit(app, &progress);

    // 解析阶段（仅新增 / 变更音频文件重新解析）
    let mut upserts = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> =
        std::collections::HashSet::with_capacity(all_audio.len() + virtual_tracks.len());
    for (i, path) in all_audio.iter().enumerate() {
        let path_str = path.to_string_lossy().to_string();
        seen_ids.insert(track_id_from_path(path));
        progress.current_path = Some(path_str.clone());

        let mtime = file_mtime(path);
        let unchanged = existing_by_path
            .get(&path_str)
            .is_some_and(|(_, old_mtime)| *old_mtime == mtime && mtime != 0);
        if !unchanged {
            if let Some(track) = parse_track(path) {
                upserts.push(track);
            }
        }

        progress.processed = (i + 1) as u32;
        // 每 20 个推送一次进度，避免事件洪泛
        if i % 20 == 0 {
            emit(app, &progress);
        }
    }

    // 虚拟分轨（CUE / SACD）：每轮重新解析（id 稳定，upsert 会保留 added_at）
    for t in &virtual_tracks {
        seen_ids.insert(t.id.clone());
    }
    upserts.extend(virtual_tracks);

    // 删除检测：基线中存在、本轮未发现的曲目（按 id 判定，
    // 覆盖「同一镜像路径对应多条分轨」与「镜像从整轨转为分轨」的情况）。
    //
    // 保守策略：仅当文件在磁盘上**确已不存在**时才删除——
    //  - 拖拽导入到曲库目录之外的文件（library_import_files）不属于任何扫描根，
    //    下一轮全盘扫描扫不到，但文件仍在，绝不能连收藏/歌单一起级联清除；
    //  - 目录读取失败（权限 / 网络挂载掉线 / Linux NTFS 未挂载）时该目录下的文件
    //    只是"没被发现"，同样必须保留。
    let mut removed_ids: Vec<String> = existing
        .iter()
        .filter(|(id, path, _)| {
            let p = std::path::Path::new(path);
            // 文件确实消失，或落在回收站/快照等垃圾目录（此前误入库）→ 清出曲库
            !seen_ids.contains(id) && (!p.exists() || is_junk_path(p))
        })
        .map(|(id, _, _)| id.clone())
        .collect();
    removed_ids.sort();

    let changed_count = upserts.len();
    progress.stage = ScanStage::Writing;
    emit(app, &progress);

    // Linux：把目录读取失败（NTFS 挂载权限/未挂载等）聚合进完成事件，
    // 前端扫描进度条据此向用户明示「哪些目录没扫到、为什么」。
    #[cfg(target_os = "linux")]
    {
        progress.error = super::scan_linux::scan_error_summary(&dir_errors);
    }

    progress.stage = ScanStage::Done;
    progress.current_path = None;
    emit(app, &progress);

    ScanResult {
        upserts,
        removed_ids,
        changed_count,
    }
}

/// 构造轻量占位轨道（v6 两阶段懒加载）：仅写入文件名 / 路径 / 大小等轻量信息，
/// 元数据、封面、CUE / SACD 分轨全部留待播放或后台异步补解析。
///
/// `is_iso` 为 true 时生成 SACD ISO 占位（`iso_placeholder=true`）：真实 ISO 尚未
/// 解析成虚拟分轨，占位条目保证网络大目录下 SACD 内容可见、可触发解析。
pub(crate) fn placeholder_track(path: &Path, is_iso: bool) -> Option<Track> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let file_size = metadata.len();
    let format = if is_iso {
        "iso".to_string()
    } else {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default()
    };
    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "未知标题".to_string());

    Some(Track {
        id: track_id_from_path(path),
        title,
        artists: vec!["未知艺术家".to_string()],
        album: "未知专辑".to_string(),
        album_id: Some(super::models::album_id_of("未知专辑", "未知艺术家")),
        duration: 0.0,
        track_number: None,
        year: None,
        file_path: path.to_string_lossy().to_string(),
        file_size,
        format,
        bitrate: None,
        sample_rate: None,
        has_cover: false,
        replaygain_db: None,
        replaygain_peak: None,
        added_at: chrono::Utc::now().timestamp(),
        modified_at,
        is_cue: false,
        cue_offset: None,
        is_sacd: false,
        sacd_track_index: 0,
        sacd_area: None,
        sacd_dsd_rate: None,
        metadata_pending: true,
        iso_placeholder: is_iso,
    })
}

/// 两阶段懒加载 - 阶段一「前期快速加载」。
///
/// 与 `scan_directory` 的区别：
///  - 逐文件**不做** `parse_track` 元数据解析（lofty / ffprobe 全部跳过）；
///  - **不解析** CUE 分轨与 SACD ISO（sacd_extract），ISO 只生成占位条目；
///  - 不读取封面 / 缩略图；
///  - 只做目录遍历 + 文件名 / 路径 / 大小等轻量信息入库（占位轨道，
///    `metadata_pending=true`），因此网络共享大目录（如 \\172.18.0.21\sound）
///    加载耗时仅取决于目录遍历本身，UI 即时可渲染。
///
/// 阶段二（补解析）见 `commands/library.rs` 的 `library_resolve_track` 与后台
/// `spawn_lazy_backfill`：播放某曲目时或空闲后台线程按需补做元数据 / 封面 /
/// CUE 分轨 / SACD 解析，完成后再 apply_scan 覆盖占位轨道并推送增量事件。
/// 后台补解析按本地 / 网络分流：**本地**做深度解析（元数据 + CUE 分轨 +
/// SACD 虚拟分轨），**网络**只读文件头（虚拟分轨留待播放时按需解析）。
///
/// 增量语义与完整扫描一致：mtime 未变且库内已有完整轨道（非 pending）的文件
/// 不再生成占位，避免把已解析元数据覆盖回占位；磁盘上确已不存在的曲目移除。
pub fn scan_directory_fast(
    roots: &[PathBuf],
    existing: &[(String, String, i64)],
    app: &AppHandle,
) -> ScanResult {
    let mut progress = ScanProgress {
        stage: ScanStage::Discovering,
        processed: 0,
        total: 0,
        current_path: None,
        error: None,
    };
    emit(app, &progress);

    // 基线索引：path → (id, mtime)；用于「已有完整轨道不降级为占位」
    let existing_by_path: std::collections::HashMap<String, (String, i64)> = existing
        .iter()
        .map(|(id, path, mtime)| (path.clone(), (id.clone(), *mtime)))
        .collect();

    // 发现阶段：仅遍历目录，不解析任何元数据
    let mut all_audio = Vec::new();
    let mut iso_files = Vec::new();
    for root in roots {
        #[cfg(target_os = "linux")]
        {
            all_audio.extend(super::scan_linux::discover_files_checked(root, SUPPORTED_EXT).files);
            iso_files.extend(super::scan_linux::discover_files_checked(root, &["iso"]).files);
        }
        #[cfg(not(target_os = "linux"))]
        {
            all_audio.extend(discover_audio_files(root));
            iso_files.extend(discover_sacd_files(root));
        }
    }

    progress.total = (all_audio.len() + iso_files.len()) as u32;
    progress.stage = ScanStage::Parsing;
    emit(app, &progress);

    // 轻量入库阶段：只生成占位轨道，不触碰元数据 / CUE / SACD 解析
    let mut upserts = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> =
        std::collections::HashSet::with_capacity(all_audio.len() + iso_files.len());
    for (i, path) in all_audio.iter().enumerate() {
        let path_str = path.to_string_lossy().to_string();
        seen_ids.insert(track_id_from_path(path));

        let mtime = file_mtime(path);
        // 已有完整轨道（mtime 未变）不降级为占位；仅新增 / 变更文件生成占位
        let keep_existing = existing_by_path
            .get(&path_str)
            .is_some_and(|(_, old_mtime)| *old_mtime == mtime && mtime != 0);
        if !keep_existing {
            if let Some(track) = placeholder_track(path, false) {
                upserts.push(track);
            }
        }

        progress.processed = (i + 1) as u32;
        if i % 50 == 0 {
            emit(app, &progress);
        }
    }

    // SACD ISO：不解析，生成占位条目（同一 ISO 已在库中（占位或分轨）则跳过）
    for (i, iso) in iso_files.iter().enumerate() {
        let iso_str = iso.to_string_lossy().to_string();
        seen_ids.insert(track_id_from_path(iso));
        if !existing_by_path.contains_key(&iso_str) {
            if let Some(track) = placeholder_track(iso, true) {
                upserts.push(track);
            }
        }
        progress.processed = (all_audio.len() + i + 1) as u32;
        if i % 50 == 0 {
            emit(app, &progress);
        }
    }

    // 删除检测：与完整扫描同策略——仅当文件在磁盘上确已不存在时才删除
    // （虚拟分轨与占位的 file_path 指向镜像 / ISO，磁盘仍在故不会误删）
    let mut removed_ids: Vec<String> = existing
        .iter()
        .filter(|(id, path, _)| {
            let p = std::path::Path::new(path);
            // 文件确实消失，或落在回收站/快照等垃圾目录（此前误入库）→ 清出曲库
            !seen_ids.contains(id) && (!p.exists() || is_junk_path(p))
        })
        .map(|(id, _, _)| id.clone())
        .collect();
    removed_ids.sort();

    let changed_count = upserts.len();
    progress.stage = ScanStage::Writing;
    emit(app, &progress);

    progress.stage = ScanStage::Done;
    progress.current_path = None;
    emit(app, &progress);

    ScanResult {
        upserts,
        removed_ids,
        changed_count,
    }
}

/// 拖拽导入 / 显式添加：把给定路径（文件或目录）中的音频与虚拟分轨
/// （SACD ISO / CUE）统一解析为待入库曲目。
///
/// 与 `scan_directory` 的虚拟分轨逻辑保持一致：SACD ISO 与 CUE 分轨在 Release
/// 同样启用。被虚拟分轨引用的整盘镜像文件会从普通音频中
/// 排除，避免与分轨重复入库。返回空表示无可识别音频。
pub(crate) fn collect_import_tracks(paths: &[PathBuf]) -> Vec<Track> {
    let mut audio: Vec<PathBuf> = Vec::new();
    let mut iso: Vec<PathBuf> = Vec::new();
    let mut cue: Vec<PathBuf> = Vec::new();

    for raw in paths {
        if raw.is_dir() {
            audio.extend(discover_audio_files(raw));
            iso.extend(discover_sacd_files(raw));
            cue.extend(discover_cue_files(raw));
        } else if raw.is_file() {
            let ext = raw
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());
            match ext.as_deref() {
                Some("iso") => iso.push(raw.clone()),
                Some("cue") => {
                    cue.push(raw.clone());
                }
                _ => {
                    if ext_supported(raw) {
                        audio.push(raw.clone());
                    }
                }
            }
        }
    }

    // 虚拟分轨（SACD ISO / CUE）：解析为多条 Track，
    // 整盘镜像 / CUE 引用的音频文件进 referenced 排除，避免与分轨重复入库。
    let mut virtual_tracks: Vec<Track> = Vec::new();
    let mut referenced: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    // 「整碟镜像」判定键（目录 + 去扩展名的文件名），与 scan_directory 保持一致
    let mut referenced_discs: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();

    let mut sacd_titles_by_dir: std::collections::HashMap<
        PathBuf,
        std::collections::HashSet<String>,
    > = std::collections::HashMap::new();
    for f in &iso {
        referenced.insert(norm_path_for_match(f));
        if let Some(mut disc) = super::sacd::SacdDisc::open(f) {
            disc.probe_durations();
            let tracks = disc.to_tracks();
            if let Some(parent) = f.parent() {
                let set = sacd_titles_by_dir
                    .entry(parent.to_path_buf())
                    .or_default();
                for t in &tracks {
                    let n = normalize_title_for_dedup(&t.title);
                    if !n.is_empty() {
                        set.insert(n);
                    }
                }
            }
            virtual_tracks.extend(tracks);
        }
    }
    // "只做 ISO"：剔除同目录下对 ISO 手动解包产出的散落音频（dsf/dff 或标题匹配某轨）
    retain_sacd_derived(&mut audio, &sacd_titles_by_dir);
    for f in &cue {
        let tracks = super::cue::parse_cue(f);
        for t in &tracks {
            referenced.insert(norm_path_for_match(std::path::Path::new(&t.file_path)));
            if let Some(key) = super::models::disc_key(&t.file_path) {
                referenced_discs.insert(key);
            }
        }
        if tracks.is_empty() {
            if let Some(key) = super::models::disc_key(&f.to_string_lossy()) {
                referenced_discs.insert(key);
            }
        }
        virtual_tracks.extend(tracks);
    }

    if !referenced.is_empty() || !referenced_discs.is_empty() {
        audio.retain(|p| {
            if referenced.contains(&norm_path_for_match(p)) {
                return false;
            }
            match super::models::disc_key(&p.to_string_lossy()) {
                Some(key) => !referenced_discs.contains(&key),
                None => true,
            }
        });
    }

    let mut upserts: Vec<Track> = Vec::with_capacity(audio.len() + virtual_tracks.len());
    for p in &audio {
        if let Some(t) = parse_track(p) {
            upserts.push(t);
        }
    }
    upserts.extend(virtual_tracks);
    upserts
}

fn emit(app: &AppHandle, progress: &ScanProgress) {
    let _ = app.emit("library://scan-progress", progress);
}

/// 规范化路径，用于「CUE / SACD 引用的整碟文件」与「扫描发现的音频文件」之间的比较。
///
/// 直接比较 `PathBuf` 会因 `./` 前缀、`..` 段、符号链接等表示差异而漏配，导致被分轨
/// 引用的整碟大文件没能排除、与分轨重复出现在播放列表。先做词法规范化，能
/// canonicalize 时进一步解析符号链接，保证同一文件的两种表示可匹配。
fn norm_path_for_match(p: &std::path::Path) -> PathBuf {
    let mut lex = PathBuf::new();
    for comp in p.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                lex.pop();
            }
            other => lex.push(other.as_os_str()),
        }
    }
    std::fs::canonicalize(&lex).unwrap_or(lex)
}

/// 归一化标题用于 SACD 去重比对：转小写、去首尾空白，并去掉开头的曲目编号前缀
/// （如 `03 - ` / `03. ` / `03_` / `03 ` → 仅保留歌曲名本身）。
/// 用于把散落文件 `03 - Ultralight.dsf` 与 ISO 音轨 `Ultralight` 对上。
fn normalize_title_for_dedup(title: &str) -> String {
    let t = title.trim().to_lowercase();
    let bytes = t.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return t; // 无前导数字编号，原样返回
    }
    // 去掉数字编号及其后紧跟的一个分隔符
    let rest = t[i..].trim_start();
    let rest = rest
        .strip_prefix('-')
        .or_else(|| rest.strip_prefix('.'))
        .or_else(|| rest.strip_prefix('_'))
        .or_else(|| rest.strip_prefix('—'))
        .or_else(|| rest.strip_prefix(' '))
        .unwrap_or(rest);
    rest.trim().to_string()
}

/// "只做 ISO" 去重：从待入库的散落音频中剔除「位于 SACD ISO 同目录、且实为对该 ISO
/// 手动解包产物」的文件，避免同一首歌以 ISO 虚拟轨 + 散落文件两种来源重复入库。
///
/// 判定满足其一即剔除（仅作用于与某 SACD ISO 同目录的文件）：
///  1. 扩展名为 `dsf` / `dff`（SACD 原生抽取格式，几乎只可能来自 SACD 解包）；
///  2. 文件名归一化后与该目录某条 ISO 音轨标题一致（兼容 .wav 等其它解包格式）。
///
/// 因要求「同目录」且（对非 dsf/dff）「标题真正匹配」，不会误伤与 ISO 同根目录却
/// 无关的其它专辑文件（例如与 Sandy Lam ISO 同处的 Westlife/Mozart 的 mp3/flac/wav）。
fn retain_sacd_derived(
    audio: &mut Vec<PathBuf>,
    iso_titles_by_dir: &std::collections::HashMap<PathBuf, std::collections::HashSet<String>>,
) {
    if iso_titles_by_dir.is_empty() {
        return;
    }
    audio.retain(|p| {
        let Some(parent) = p.parent() else {
            return true;
        };
        let Some(titles) = iso_titles_by_dir.get(parent) else {
            return true; // 不在任何 SACD ISO 目录，保留
        };
        // 信号 1：SACD 原生抽取格式
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());
        if matches!(ext.as_deref(), Some("dsf") | Some("dff")) {
            return false;
        }
        // 信号 2：标题与某条 ISO 音轨匹配
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            let n = normalize_title_for_dedup(stem);
            if !n.is_empty() && titles.contains(&n) {
                return false;
            }
        }
        true
    });
}

/// 收集 SACD 时长校正的输入：每个 ISO 及其在库内的现有 `(id, duration)`。
///
/// 由调用方在**持有曲库锁**时调用（只做查询，很快）；探测与对比必须在锁外进行。
pub fn sacd_duration_plan(
    library: &super::store::LibraryStore,
) -> Vec<(String, Vec<(String, f64)>)> {
    library
        .sacd_iso_paths()
        .into_iter()
        .map(|iso| {
            let existing = library.sacd_tracks_of_file(&iso);
            (iso, existing)
        })
        .filter(|(_, existing)| !existing.is_empty())
        .collect()
}

/// 按 `sacd_extract -P` 的探测结果计算需要校正的分轨时长 `(id, 新时长)`。
///
/// DST 压缩区的存储数据量只有未压缩 DSD 的 ~25%，旧版本按「区间字节数 / 每帧字节数」
/// 推算的时长会压到约 40%（Energy 每轨 5:52 显示成 2:22）。这类错值入库后不会自行
/// 消失——扫描只对新增/变更文件重解析，SACD 虚拟分轨虽每轮重建，但用户不重扫就一直是
/// 旧值。启动期用它就地校正，让升级即生效。
///
/// 探测失败（工具缺失/解析失败）的 ISO 不产生任何修改，仍保留原值。
pub fn sacd_duration_fixes(plan: &[(String, Vec<(String, f64)>)]) -> Vec<(String, f64)> {
    let mut fixes: Vec<(String, f64)> = Vec::new();
    for (iso, existing) in plan {
        let Some(mut disc) = super::sacd::SacdDisc::open(std::path::Path::new(iso)) else {
            continue;
        };
        disc.probe_durations();
        if !disc.has_probed_durations() {
            continue;
        }
        for track in disc.to_tracks() {
            let Some((_, old)) = existing.iter().find(|(id, _)| id == &track.id) else {
                continue;
            };
            // 0.5s 容差：避免探测的小数抖动造成无意义写入
            if (old - track.duration).abs() > 0.5 {
                fixes.push((track.id, track.duration));
            }
        }
    }
    fixes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;

    /// 端到端：库里存着旧版本按数据量推算的时长（DST 区约 40%）时，
    /// 启动期校正必须把 duration 就地改成探测值（真实 ISO，需 SACD_TEST_ISO）
    #[test]
    fn sacd_duration_repair_fixes_stale_rows() {
        let Ok(iso) = std::env::var("SACD_TEST_ISO") else {
            return;
        };
        if crate::player::engine::resolve_bin_tool("sacd_extract").is_none() {
            return;
        }
        let Some(mut disc) = super::super::sacd::SacdDisc::open(std::path::Path::new(&iso)) else {
            return;
        };
        disc.probe_durations();
        if !disc.has_probed_durations() {
            return;
        }
        let correct = disc.to_tracks();

        // 模拟旧库：DST 压缩区按字节推算 → 时长约为真实值的 40%
        let mut stale = correct.clone();
        for t in &mut stale {
            t.duration *= 0.4;
        }

        let dir = std::env::temp_dir().join(format!("tplayer-sacd-dur-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut lib = crate::library::store::LibraryStore::open(
            dir.join("library.db"),
            dir.join("legacy.json"),
        )
        .expect("打开临时曲库");
        lib.apply_scan(&stale, &[]).expect("写入旧时长");

        let plan = sacd_duration_plan(&lib);
        assert_eq!(plan.len(), 1, "应收集到该 ISO");
        let fixes = sacd_duration_fixes(&plan);
        assert_eq!(fixes.len(), stale.len(), "每轨时长都应判为需要校正");
        let updated = lib.update_durations(&fixes).expect("写回校正时长");
        assert_eq!(updated, fixes.len());

        let after = lib.sacd_tracks_of_file(&iso);
        for t in &correct {
            let (_, dur) = after
                .iter()
                .find(|(id, _)| id == &t.id)
                .unwrap_or_else(|| panic!("缺少曲目 {}", t.title));
            assert!(
                (dur - t.duration).abs() < 0.6,
                "{}: 校正后 {dur:.2}s，应为 {:.2}s",
                t.title,
                t.duration
            );
        }
        // 5.1 轨同样被校正（其帧数来自立体声区，旧值同样偏短）
        let multich: Vec<_> = after
            .iter()
            .filter(|(_, d)| (d - correct[0].duration).abs() < 0.6)
            .collect();
        assert!(multich.len() >= 2, "5.1 轨也应校正");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalize_strips_track_number_prefix() {
        assert_eq!(normalize_title_for_dedup("03 - Ultralight"), "ultralight");
        assert_eq!(normalize_title_for_dedup("03. Ultralight"), "ultralight");
        assert_eq!(normalize_title_for_dedup("03_Ultralight"), "ultralight");
        assert_eq!(normalize_title_for_dedup("03 Ultralight"), "ultralight");
        assert_eq!(normalize_title_for_dedup("  Ultralight "), "ultralight");
        // 无前导编号：仅小写/去空白
        assert_eq!(normalize_title_for_dedup("The Whistler"), "the whistler");
        // 纯编号或空：归一为空（调用方据此跳过匹配）
        assert_eq!(normalize_title_for_dedup("03"), "");
    }

    #[test]
    fn sacd_derived_dedup_scopes_to_iso_dir_and_matches_title() {
        let iso_dir = PathBuf::from(r"G:\AITEMP\Fourplay - Energy (SACD-2.0-5.1)");
        let root = PathBuf::from(r"G:\AITEMP");

        let mut titles: HashMap<PathBuf, HashSet<String>> = HashMap::new();
        titles
            .entry(iso_dir.clone())
            .or_default()
            .insert("ultralight".to_string());
        titles
            .entry(iso_dir.clone())
            .or_default()
            .insert("the whistler".to_string());

        let mut audio = vec![
            // 同目录、dsf 抽取产物 → 剔除（格式信号）
            iso_dir.join("03 - Ultralight.dsf"),
            // 同目录、标题匹配 ISO 音轨（非 dsf，如 wav 抽取）→ 剔除（标题信号）
            iso_dir.join("04 - The Whistler.wav"),
            // 同目录、标题不匹配 → 保留
            iso_dir.join("Bonus Track.flac"),
            // 根目录、与 ISO 无关的其它专辑文件 → 保留（不被 Sandy Lam 等同根 ISO 误伤）
            root.join("Westlife-Mandy.mp3"),
            root.join("一生一遇.wav"),
        ];
        retain_sacd_derived(&mut audio, &titles);

        let names: Vec<String> = audio
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["Bonus Track.flac", "Westlife-Mandy.mp3", "一生一遇.wav"]);
    }

    #[test]
    fn sacd_derived_dedup_noop_without_iso() {
        let mut audio = vec![PathBuf::from(r"G:\AITEMP\Westlife-Mandy.mp3")];
        retain_sacd_derived(&mut audio, &HashMap::new());
        assert_eq!(audio.len(), 1);
    }

    #[test]
    fn junk_directories_are_excluded() {
        // 普通目录不受影响
        assert!(!is_junk_path(Path::new(r"E:\KwDownload\song\小苹果.mp3")));
        assert!(!is_junk_path(Path::new(r"G:\AITEMP\Westlife-Mandy.mp3")));
        // 名字里含 trash/recycle 但并非目录段的文件名不被误伤
        assert!(!is_junk_path(Path::new(r"E:\KwDownload\Trash Metal - Song.mp3")));
        // 各类系统 / NAS 垃圾目录
        for p in [
            r"Y:\#recycle\Music\小苹果 - 筷子兄弟.mp3",
            r"Y:\#snapshot\Music\a.flac",
            r"D:\@eaDir\a.mp3",
            r"E:\$RECYCLE.BIN\a.mp3",
            r"C:\System Volume Information\a.wav",
            r"/mnt/.Trash-1000/a.flac",
            r"/home/u/.Trashes/a.mp3",
            r"/lost+found/a.iso",
        ] {
            assert!(is_junk_path(Path::new(p)), "{p} 应被判为垃圾路径");
        }
    }

    #[test]
    fn without_junk_retains_normal_paths() {
        let files = vec![
            PathBuf::from(r"E:\KwDownload\song\a.mp3"),
            PathBuf::from(r"Y:\#recycle\Music\b.mp3"),
            PathBuf::from(r"G:\AITEMP\c.flac"),
            PathBuf::from(r"E:\$RECYCLE.BIN\d.wav"),
        ];
        let kept = without_junk(files);
        assert_eq!(
            kept.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>(),
            vec![
                r"E:\KwDownload\song\a.mp3".to_string(),
                r"G:\AITEMP\c.flac".to_string()
            ]
        );
    }
}
