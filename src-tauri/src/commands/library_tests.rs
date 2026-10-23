//! 后台补解析（本地深度 / 网络浅层）的行为约束。
//!
//! 覆盖三个关键不变量：
//!  1. 本地整碟镜像被 CUE 引用时，占位被**分轨取代**（removed 含占位 id，
//!     upserts 为分轨），且分轨 id 与占位 id 不同；
//!  2. 普通单曲覆盖占位时**不得**把同一 id 放进 removed
//!     （`apply_scan` 先 upsert 后 remove，同 id 会把自己删掉）；
//!  3. 分轨已在库且 cue 未变更时，不再重复解析，只移除占位。

use super::*;
use std::io::Write;

/// 构造一个最小可解析的 WAV（44 字节头 + 空数据块），供 lofty 读取。
fn write_min_wav(path: &Path) {
    let channels: u16 = 2;
    let bits: u16 = 16;
    let sample_rate: u32 = 44_100;
    let byte_rate = sample_rate * channels as u32 * bits as u32 / 8;
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&36u32.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&(channels * bits / 8).to_le_bytes());
    bytes.extend_from_slice(&bits.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&0u32.to_le_bytes());
    let mut f = std::fs::File::create(path).expect("create wav");
    f.write_all(&bytes).expect("write wav");
}

/// 写一个引用 `Album.flac` 的两轨 CUE。
fn write_cue(dir: &Path) {
    let cue = "FILE \"Album.flac\" WAVE\n\
               TRACK 01 AUDIO\n  TITLE \"First\"\n  INDEX 01 00:00:00\n\
               TRACK 02 AUDIO\n  TITLE \"Second\"\n  INDEX 01 00:00:30\n";
    let mut f = std::fs::File::create(dir.join("Album.cue")).expect("create cue");
    f.write_all(cue.as_bytes()).expect("write cue");
}

/// 占位轨道（模拟快速扫描产出的轻量条目）。
fn placeholder(id: &str, path: &Path) -> Track {
    Track {
        id: id.to_string(),
        title: "未知标题".into(),
        artists: vec!["未知艺术家".into()],
        album: "未知专辑".into(),
        album_id: Some("album:未知专辑:未知艺术家".into()),
        duration: 0.0,
        track_number: None,
        year: None,
        file_path: path.to_string_lossy().to_string(),
        file_size: 0,
        format: "flac".into(),
        bitrate: None,
        sample_rate: None,
        has_cover: false,
        replaygain_db: None,
        replaygain_peak: None,
        added_at: 0,
        modified_at: 0,
        is_cue: false,
        cue_offset: None,
        is_sacd: false,
        sacd_track_index: 0,
        sacd_area: None,
        sacd_dsd_rate: None,
        metadata_pending: true,
        iso_placeholder: false,
    }
}

/// 唯一临时目录（按测试名区分，避免并行测试互相干扰）。
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tplayer_resolve_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn deep_resolve_replaces_mirror_placeholder_with_cue_tracks() {
    let dir = temp_dir("cue_mirror");
    let mirror = dir.join("Album.flac");
    std::fs::write(&mirror, b"not-a-real-flac").expect("write mirror");
    write_cue(&dir);

    let ph = placeholder("ph-mirror", &mirror);
    let mut ctx = ResolveContext::new(Vec::new());
    let outcome = resolve_placeholder(&ph, ResolveDepth::Deep, &mut ctx);

    // 占位被分轨取代
    assert_eq!(outcome.removed, vec!["ph-mirror".to_string()]);
    assert_eq!(outcome.upserts.len(), 2, "应产出 2 条 CUE 分轨");
    assert!(outcome.upserts.iter().all(|t| t.is_cue));
    assert_ne!(outcome.upserts[0].id, "ph-mirror", "分轨 id 与占位 id 不同");
    assert_eq!(outcome.upserts[0].title, "First");
    assert_eq!(outcome.upserts[1].title, "Second");
    // 分轨 id 不在移除清单里（否则 apply_scan 会把刚写入的分轨删掉）
    assert!(!outcome.removed.contains(&outcome.upserts[0].id));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deep_resolve_keeps_single_track_with_same_id_and_no_removal() {
    let dir = temp_dir("single");
    let song = dir.join("song.wav");
    write_min_wav(&song);

    let ph = placeholder("ph-song", &song);
    let mut ctx = ResolveContext::new(Vec::new());
    let outcome = resolve_placeholder(&ph, ResolveDepth::Deep, &mut ctx);

    // 普通单曲：同 id 覆盖占位，且**不能**把该 id 放进 removed
    assert_eq!(outcome.upserts.len(), 1);
    assert_eq!(outcome.upserts[0].id, "ph-song");
    assert!(
        outcome.removed.is_empty(),
        "同 id 覆盖时不得删除自身，实际 removed = {:?}",
        outcome.removed
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deep_resolve_reuses_already_known_cue_tracks() {
    let dir = temp_dir("known");
    let mirror = dir.join("Album.flac");
    std::fs::write(&mirror, b"not-a-real-flac").expect("write mirror");
    write_cue(&dir);

    // 第一轮：解析出分轨
    let ph = placeholder("ph-mirror", &mirror);
    let mut ctx = ResolveContext::new(Vec::new());
    let first = resolve_placeholder(&ph, ResolveDepth::Deep, &mut ctx);
    assert_eq!(first.upserts.len(), 2);

    // 第二轮：分轨已在库（且 cue 未变更）→ 不重复解析，只移除占位
    let mut ctx2 = ResolveContext::new(first.upserts.clone());
    let second = resolve_placeholder(&ph, ResolveDepth::Deep, &mut ctx2);
    assert!(second.upserts.is_empty(), "已入库的分轨不应重复写入");
    assert_eq!(second.removed, vec!["ph-mirror".to_string()]);
    assert_eq!(second.existing.len(), 2, "应复用已入库的分轨");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shallow_resolve_skips_virtual_tracks_for_network() {
    let dir = temp_dir("shallow");
    let mirror = dir.join("Album.wav");
    write_min_wav(&mirror);
    // 同目录存在引用该镜像的 CUE
    let cue = "FILE \"Album.wav\" WAVE\n\
               TRACK 01 AUDIO\n  TITLE \"First\"\n  INDEX 01 00:00:00\n";
    let mut f = std::fs::File::create(dir.join("Album.cue")).expect("create cue");
    f.write_all(cue.as_bytes()).expect("write cue");

    let ph = placeholder("ph-mirror", &mirror);
    let mut ctx = ResolveContext::new(Vec::new());
    let outcome = resolve_placeholder(&ph, ResolveDepth::Shallow, &mut ctx);

    // 网络共享：只读文件头 → 只有普通单曲元数据，不产出 CUE 分轨
    assert_eq!(outcome.upserts.len(), 1, "只应产出普通单曲元数据");
    assert!(!outcome.upserts[0].is_cue, "网络浅解析不产出 CUE 分轨");
    assert!(outcome.removed.is_empty(), "占位保留（播放时再解析）");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn disc_key_matches_cue_reference_across_extensions() {
    // 同一张碟的多个镜像副本（.flac / .ape）应归到同一个整碟键
    let key_flac = disc_key("D:/music/Album.flac");
    let key_ape = disc_key("D:/music/Album.ape");
    assert_eq!(key_flac, key_ape);
    assert!(key_flac.is_some());
}
