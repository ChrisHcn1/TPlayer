//! 曲库存储：SQLite 持久化（WAL），增量事务写入
//!
//! 历史：M0/M1 使用 library.json 全量读写，收藏/播放记录等任何小改动都会整体
//! 序列化重写；2.0 起迁移到 SQLite（rusqlite bundled），首次启动自动导入旧
//! library.json 并备份为 library.json.bak。

use std::path::PathBuf;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

use super::db;
use super::models::{
    album_id_of, Album, Artist, LibraryStats, Playlist, RecentEntry, Track,
};

/// 旧版 library.json 的完整结构（仅用于首次迁移反序列化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryData {
    pub tracks: Vec<Track>,
    pub playlists: Vec<Playlist>,
    #[serde(rename = "favoriteIds")]
    pub favorite_ids: Vec<String>,
    pub recent: Vec<RecentEntry>,
    pub stats: LegacyStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LegacyStats {
    #[serde(default)]
    pub track_count: u32,
    #[serde(default)]
    pub album_count: u32,
    #[serde(default)]
    pub artist_count: u32,
    #[serde(default)]
    pub total_duration: f64,
    #[serde(rename = "lastScanAt", default)]
    pub last_scan_at: Option<i64>,
}

/// 曲库存储：命令层通过它查询与修改用户数据
pub struct LibraryStore {
    conn: Connection,
    /// 数据库文件路径（崩溃回退的快照 / 运行标记都以它为基准）
    db_path: PathBuf,
}

/// SQL 表达式：`file_path` 的目录部分（统一 '/' 分隔符）
fn sql_dir_expr(path_expr: &str) -> String {
    let norm = format!("replace({path_expr}, '\\', '/')");
    format!("rtrim({norm}, replace({norm}, '/', ''))")
}

/// SQL 表达式：`file_path` 的文件名部分（去掉最后一个扩展名）
fn sql_stem_expr(path_expr: &str) -> String {
    let dir = sql_dir_expr(path_expr);
    let rest = format!("substr({path_expr}, length({dir}) + 1)");
    format!("rtrim({rest}, replace({rest}, '.', ''))")
}

/// SACD 5.1 环绕轨过滤条件（作用于 `tracks` 别名 `t`）。
///
/// ISO 的立体声区与多声道区是同一张碟的两套曲目（标题仅差 `[5.1]` 后缀），
/// 在「全部歌曲 / 统计 / 艺术家」里并排出现会像重复加载；5.1 轨本身仍完整入库，
/// 由 SACD 专辑页的「立体声 / 5.1 环绕」切换访问（`tracks_of_album` 不过滤）。
///
/// `sacd_area` 存的是 serde_json 序列化值（含双引号），故按 `"MultiCh"` 比较。
fn not_sacd_multich() -> &'static str {
    "NOT (t.is_sacd = 1 AND t.sacd_area = '\"MultiCh\"')"
}

/// 「同一首歌」去重条件（作用于 `tracks` 别名 `t`）。
///
/// 用户会把同一首歌放在不同目录（如 `歌手A/专辑/01.flac` 与 `杂录/01.flac`），
/// 全部歌曲里就成了两条。判定「同一首歌」的键 = **主艺人 + 标题 + 专辑 + 格式**：
///  - 格式**不同**（flac / mp3 / ape 各一份）视为不同版本，**都保留**，由用户自己选；
///  - 格式相同则是真正的重复副本，只保留一条。
///
/// 保留哪一条按 (metadata_pending, added_at, id) 升序取最小者——优先已解析完
/// 元数据的、最先入库的，结果稳定可预期； SACD 虚拟分轨不参与（其版本由专辑页的
/// 立体声/5.1 切换表达，不在此处折叠）。
///
/// 依赖索引 `idx_tracks_song_key`（表达式索引，见 db::migrate）以避免大库退化。
fn not_duplicate_song() -> &'static str {
    r#"NOT (t.is_sacd = 0 AND EXISTS (
        SELECT 1 FROM tracks d
         WHERE d.id <> t.id
           AND d.is_sacd = 0
           AND lower(trim(d.title))  = lower(trim(t.title))
           AND lower(trim(d.album))  = lower(trim(t.album))
           AND lower(trim(d.format)) = lower(trim(t.format))
           AND lower(trim(COALESCE(json_extract(d.artists_json, '$[0]'), '')))
               = lower(trim(COALESCE(json_extract(t.artists_json, '$[0]'), '')))
           AND (
                 d.metadata_pending < t.metadata_pending
              OR (d.metadata_pending = t.metadata_pending AND (
                     d.added_at < t.added_at
                  OR (d.added_at = t.added_at AND d.id < t.id)))
           )
    ))"#
}

/// 「CUE 结构大文件」去重条件（作用于 `tracks` 的别名 `t`）。
///
/// CUE 会在「分轨专辑」页展示整碟镜像拆出的分轨；同一张碟的整碟大文件
/// （ape / flac 等）不应再作为独立曲目出现在「全部歌曲 / 专辑 / 艺术家」等
/// 普通列表里，否则用户看到的是同一张碟的两份条目。
///
/// 判定「属于同一张碟」的三种依据（任一命中即视为重复）：
///  1. 路径完全相同（CUE 直接引用的那个镜像文件）；
///  2. 同目录 + 同基名（`Album.flac` / `Album.ape` 这类同碟副本）；
///  3. 同目录 + 同专辑名（镜像文件名与 CUE 源文件不同名的情况，
///     如 `Mozart.-.[…].ape` 与 `Volume 1(CD6).flac`）。
///
/// 分轨本身（`is_cue = 1`）始终保留；比较忽略大小写
/// （Windows / macOS 文件系统不区分大小写）。
fn not_cue_source() -> String {
    let c_dir = sql_dir_expr("c.file_path");
    let t_dir = sql_dir_expr("t.file_path");
    let c_stem = sql_stem_expr("c.file_path");
    let t_stem = sql_stem_expr("t.file_path");
    format!(
        "(
    t.is_cue = 1
    OR NOT EXISTS (
        SELECT 1 FROM tracks c
        WHERE c.is_cue = 1
          AND c.id <> t.id
          AND (
                c.file_path = t.file_path COLLATE NOCASE
             OR ({c_dir} = {t_dir} COLLATE NOCASE AND {c_stem} = {t_stem} COLLATE NOCASE)
             OR ({c_dir} = {t_dir} COLLATE NOCASE
                 AND trim(c.album) <> '' AND trim(t.album) <> ''
                 AND lower(trim(c.album)) = lower(trim(t.album)))
          )
    )
)"
    )
}

impl LibraryStore {
    /// 打开数据库；首次运行（无 schema_version）时从旧 library.json 迁移
    pub fn open(db_path: PathBuf, legacy_json_path: PathBuf) -> AppResult<Self> {
        let mut conn = db::open(&db_path)?;
        let fresh = db::get_meta(&conn, "schema_version").is_none();
        if fresh {
            migrate_legacy_json(&mut conn, &legacy_json_path)?;
            db::set_meta(&conn, "schema_version", &db::SCHEMA_VERSION.to_string())?;
        }
        // 预留：后续版本在此比对 schema_version 执行增量迁移
        Ok(Self {
            conn,
            db_path,
        })
    }

    /// 把当前库写成「好状态」快照（崩溃回退的还原点）。
    ///
    /// 扫描成功提交后调用；内部 5 分钟节流，频繁扫描不会反复全库拷贝。
    pub fn commit_good_snapshot(&self) {
        db::commit_good(&self.conn, &self.db_path);
    }

    /// 干净退出：清除运行标记，下次启动不做崩溃回退。
    pub fn mark_clean_shutdown(&self) {
        db::mark_clean(&self.db_path);
    }

    // ---- 曲目查询 ----

    /// 是否存在指定文件路径的曲目（大小写不敏感；兼容层文件读取白名单用）。
    /// config.library_dirs 可能被重置而曲库数据仍在（SQLite 持久化），曲库是路径的权威来源。
    pub fn has_file_path(&self, path: &str) -> bool {
        let Ok(mut stmt) = self
            .conn
            .prepare("SELECT 1 FROM tracks WHERE file_path = ?1 COLLATE NOCASE LIMIT 1")
        else {
            return false;
        };
        stmt.query_row([path], |_| Ok(())).is_ok()
    }

    /// 全部曲目（含 SACD 的 5.1 环绕轨）。
    ///
    /// 5.1 轨与立体声区是同一张碟的两套曲目，**不能**在数据层直接裁掉：
    /// 前端「SACD 专辑页」的立体声/5.1 切换需要它们。普通列表（全部歌曲 /
    /// 艺术家 / 统计）的 5.1 去重分别在 SQL 侧（stats/artists/albums）与
    /// 前端 store（tracks 与 allTracks 分离）完成。
    pub fn tracks(&self) -> Vec<Track> {
        // 过滤被 CUE 分轨引用的整碟大文件（同一张碟不重复展示）
        let cue_cond = not_cue_source();
        // 同一首歌（同主艺人+标题+专辑+格式）放在不同目录 → 只保留一条；格式不同则都保留
        let song_cond = not_duplicate_song();
        let sql = format!(
            "SELECT t.* FROM tracks t WHERE {cue_cond} AND {song_cond} ORDER BY t.added_at, t.id"
        );
        let mut stmt = self.conn.prepare(&sql).expect("prepare tracks");
        let rows = stmt
            .query_map([], row_to_track)
            .expect("query tracks");
        rows.flatten().collect()
    }

    pub fn get_track(&self, id: &str) -> Option<Track> {
        self.conn
            .query_row("SELECT * FROM tracks WHERE id = ?1", [id], row_to_track)
            .ok()
    }

    /// 两阶段懒加载：返回所有尚未解析元数据的占位轨道（metadata_pending = 1）。
    /// 供后台补解析（backfill）与「播放时按需解析」流程使用。
    pub fn pending_tracks(&self) -> Vec<Track> {
        let sql = "SELECT * FROM tracks WHERE metadata_pending = 1 ORDER BY added_at, id";
        let mut stmt = self.conn.prepare(sql).expect("prepare pending_tracks");
        let rows = stmt.query_map([], row_to_track).expect("query pending_tracks");
        rows.flatten().collect()
    }

    /// 库内全部 CUE 分轨（后台补解析用）。
    ///
    /// 用途：判断某张整碟镜像是否已经被 CUE 拆成分轨（已入库且 cue 未变更时
    /// 无需重复解析，只把整碟占位移除即可），避免每次扫描都重解析全部 CUE。
    pub fn cue_tracks(&self) -> Vec<Track> {
        let sql = "SELECT * FROM tracks WHERE is_cue = 1 ORDER BY added_at, id";
        let mut stmt = self.conn.prepare(sql).expect("prepare cue_tracks");
        let rows = stmt.query_map([], row_to_track).expect("query cue_tracks");
        rows.flatten().collect()
    }

    /// 更新单曲 ReplayGain 增益（后台分析完成后写回；None 表示无数据则置空）
    pub fn update_replaygain(
        &mut self,
        id: &str,
        gain_db: Option<f32>,
        peak: Option<f32>,
    ) -> AppResult<()> {
        self.conn
            .execute(
                "UPDATE tracks SET replaygain_db = ?1, replaygain_peak = ?2 WHERE id = ?3",
                params![gain_db.map(|v| v as f64), peak.map(|v| v as f64), id],
            )
            .map_err(|e| AppError::Library(format!("写入 ReplayGain 增益失败: {e}")))?;
        Ok(())
    }

    /// 专辑（含 CUE 分轨专辑）下全部曲目，按音轨号升序
    /// 库内所有 SACD ISO 路径（去重；启动期时长校正用）
    pub fn sacd_iso_paths(&self) -> Vec<String> {
        let mut out = Vec::new();
        let Ok(mut stmt) = self
            .conn
            .prepare("SELECT DISTINCT file_path FROM tracks WHERE is_sacd = 1 ORDER BY file_path")
        else {
            return out;
        };
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            out.extend(rows.flatten());
        }
        out
    }

    /// 指定 ISO 在库内的 SACD 分轨 `(id, duration)`（含 5.1 区，启动期时长校正用）
    pub fn sacd_tracks_of_file(&self, file_path: &str) -> Vec<(String, f64)> {
        let mut out = Vec::new();
        let Ok(mut stmt) = self.conn.prepare(
            "SELECT id, duration FROM tracks \
             WHERE file_path = ?1 COLLATE NOCASE AND is_sacd = 1",
        ) else {
            return out;
        };
        if let Ok(rows) = stmt.query_map([file_path], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        }) {
            out.extend(rows.flatten());
        }
        out
    }

    /// 批量更新曲目时长（秒），返回实际更新条数。
    ///
    /// 仅改 duration，不触碰 added_at / 收藏 / 歌单引用，也不重建艺术家关联。
    pub fn update_durations(&mut self, items: &[(String, f64)]) -> AppResult<usize> {
        if items.is_empty() {
            return Ok(0);
        }
        let tx = self
            .conn
            .transaction()
            .map_err(|e| AppError::Library(format!("开启时长更新事务失败: {e}")))?;
        let mut updated = 0usize;
        for (id, duration) in items {
            updated += tx
                .execute(
                    "UPDATE tracks SET duration = ?1 WHERE id = ?2",
                    params![duration, id],
                )
                .map_err(|e| AppError::Library(format!("更新曲目时长失败: {e}")))?;
        }
        tx.commit()
            .map_err(|e| AppError::Library(format!("提交时长更新失败: {e}")))?;
        Ok(updated)
    }

    pub fn tracks_of_album(&self, album_id: &str) -> Vec<Track> {
        let cue_cond = not_cue_source();
        let song_cond = not_duplicate_song();
        let sql = format!(
            "SELECT t.* FROM tracks t
                  WHERE t.album_id = ?1 AND {cue_cond} AND {song_cond}
             ORDER BY COALESCE(t.track_number, 2147483647), t.id"
        );
        let mut stmt = self.conn.prepare(&sql).expect("prepare tracks_of_album");
        stmt.query_map([album_id], row_to_track)
            .expect("query tracks_of_album")
            .flatten()
            .collect()
    }

    /// 艺术家参与（任一阵位）的全部曲目，与前端 artists.includes 口径一致
    pub fn tracks_of_artist(&self, artist: &str) -> Vec<Track> {
        let cue_cond = not_cue_source();
        let song_cond = not_duplicate_song();
        let sql = format!(
            "SELECT t.* FROM tracks t
             JOIN track_artists ta ON ta.track_id = t.id
                  WHERE ta.artist = ?1 AND {cue_cond} AND {song_cond}
             ORDER BY t.album, COALESCE(t.track_number, 2147483647), t.id"
        );
        let mut stmt = self.conn.prepare(&sql).expect("prepare tracks_of_artist");
        stmt.query_map([artist], row_to_track)
            .expect("query tracks_of_artist")
            .flatten()
            .collect()
    }

    ///
    /// 分组封面候选曲目（封面聚合命令用）：
    /// 扫描期确认带内嵌封面的优先，其次音轨号升序，上限 30 首。
    pub fn cover_candidates_of_album(&self, album_id: &str, limit: usize) -> Vec<Track> {
        let cue_cond = not_cue_source();
        let sql = format!(
            "SELECT t.* FROM tracks t
                  WHERE t.album_id = ?1 AND {cue_cond}
             ORDER BY t.has_cover DESC, COALESCE(t.track_number, 2147483647), t.id
             LIMIT ?2"
        );
        let mut stmt = self
            .conn
            .prepare(&sql)
            .expect("prepare cover_candidates_of_album");
        stmt.query_map(params![album_id, limit as i64], row_to_track)
            .expect("query cover_candidates_of_album")
            .flatten()
            .collect()
    }

    pub fn cover_candidates_of_artist(&self, artist: &str, limit: usize) -> Vec<Track> {
        let cue_cond = not_cue_source();
        let sql = format!(
            "SELECT DISTINCT t.* FROM tracks t
             JOIN track_artists ta ON ta.track_id = t.id
                  WHERE ta.artist = ?1 AND {cue_cond}
             ORDER BY t.has_cover DESC, COALESCE(t.track_number, 2147483647), t.id
             LIMIT ?2"
        );
        let mut stmt = self
            .conn
            .prepare(&sql)
            .expect("prepare cover_candidates_of_artist");
        stmt.query_map(params![artist, limit as i64], row_to_track)
            .expect("query cover_candidates_of_artist")
            .flatten()
            .collect()
    }

    /// 增量扫描基线：(id, file_path, modified_at)
    pub fn scan_snapshot(&self) -> Vec<(String, String, i64)> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, file_path, modified_at FROM tracks")
            .expect("prepare scan_snapshot");
        stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .expect("query scan_snapshot")
        .flatten()
        .collect()
    }

    ///
    /// 应用一次增量扫描结果（单事务）：
    ///  - upserts：新增或文件已变更曲目（added_at 保留旧值，modified_at 刷新）；
    ///  - removed_ids：磁盘已不存在的曲目，同步清理收藏/最近播放/歌单引用。
    pub fn apply_scan(&mut self, upserts: &[Track], removed_ids: &[String]) -> AppResult<()> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| AppError::Library(format!("开启扫描事务失败: {e}")))?;

        for track in upserts {
            upsert_track(&tx, track)?;
        }

        for id in removed_ids {
            tx.execute("DELETE FROM tracks WHERE id = ?1", [id])
                .map_err(|e| AppError::Library(format!("删除失效曲目失败: {e}")))?;
            cascade_remove_track(&tx, id)?;
        }

        let now = chrono::Utc::now().timestamp().to_string();
        tx.execute(
            "INSERT INTO meta(key, value) VALUES('last_scan_at', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [&now],
        )
        .map_err(|e| AppError::Library(format!("写入扫描时间失败: {e}")))?;

        tx.commit()
            .map_err(|e| AppError::Library(format!("提交扫描事务失败: {e}")))?;
        Ok(())
    }

    pub fn stats(&self) -> LibraryStats {
        // 统计口径与列表一致：排除被 CUE 分轨引用的整碟大文件、SACD 5.1 轨，
        // 以及跨目录的同一首歌重复副本（格式不同则不算重复，仍计入）
        let cue_cond = not_cue_source();
        let multich_cond = not_sacd_multich();
        let song_cond = not_duplicate_song();
        let sql = format!(
            "SELECT COUNT(*), COALESCE(SUM(duration), 0.0),
                               COUNT(DISTINCT album_id) FROM tracks t
              WHERE {cue_cond} AND {multich_cond} AND {song_cond}"
        );
        let (track_count, total_duration, album_count) = self
            .conn
            .query_row(&sql, [], |row| {
                Ok((
                    row.get::<_, i64>(0)? as u32,
                    row.get::<_, f64>(1)?,
                    row.get::<_, i64>(2)? as u32,
                ))
            })
            .unwrap_or((0, 0.0, 0));
        let artist_count_sql = format!(
            "SELECT COUNT(DISTINCT ta.artist) FROM track_artists ta
             JOIN tracks t ON t.id = ta.track_id
                  WHERE {cue_cond} AND {multich_cond} AND {song_cond}"
        );
        let artist_count = self
            .conn
            .query_row(&artist_count_sql, [], |row| row.get::<_, i64>(0))
            .unwrap_or(0) as u32;
        let last_scan_at = db::get_meta(&self.conn, "last_scan_at")
            .and_then(|value| value.parse::<i64>().ok());

        LibraryStats {
            track_count,
            album_count,
            artist_count,
            total_duration,
            last_scan_at,
        }
    }

    /// 按专辑聚合：每组取音轨号最小者为代表（artists/year）。
    /// `cue_only = true` 时只返回 CUE 分轨专辑，否则只返回普通专辑，
    /// 二者互不重叠（由 tracks.is_cue 区分）。
    /// 专辑聚合模式：normal=普通专辑（排除 CUE / SACD 虚拟专辑），
    /// cue=CUE 分轨专辑，sacd=SACD ISO 分轨专辑（三者互不重叠）。
    fn albums_filtered(&self, mode: &str) -> Vec<Album> {
        // normal 模式额外排除「被 CUE 分轨引用的整碟大文件」所在专辑，
        // 避免同一张碟既出现在分轨专辑页、又以整碟形式出现在普通专辑页。
        let cue_cond = not_cue_source();
        // 同一首歌跨目录重复副本只计一次（格式不同则都计）
        let song_cond = not_duplicate_song();
        let where_clause = match mode {
            "cue" => "t.is_cue = 1".to_string(),
            // SACD 专辑页的曲目数按立体声区统计（与详情页默认版本一致）；
            // 5.1 环绕轨在详情页切换查看，不作为独立曲目重复计数
            "sacd" => format!("t.is_sacd = 1 AND {}", not_sacd_multich()),
            _ => format!("t.is_cue = 0 AND t.is_sacd = 0 AND {cue_cond} AND {song_cond}"),
        };
        let sql = format!(
            r#"
            WITH base AS (
                SELECT t.* FROM tracks t
                WHERE t.album_id IS NOT NULL AND {where_clause}
            ),
            ranked AS (
                SELECT
                    album_id, album, artists_json, year, is_cue, is_sacd,
                    ROW_NUMBER() OVER (
                        PARTITION BY album_id
                        ORDER BY COALESCE(track_number, 2147483647), id
                    ) AS rn,
                    COUNT(*)     OVER (PARTITION BY album_id) AS tc,
                    MAX(has_cover) OVER (PARTITION BY album_id) AS hc
                FROM base
            )
            SELECT album_id, album, artists_json, year, is_cue, is_sacd, tc, hc
            FROM ranked
            WHERE rn = 1
            ORDER BY album
            "#
        );
        let mut stmt = self.conn.prepare(&sql).expect("prepare albums");
        stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let artists_json: String = row.get(2)?;
            let year: Option<i64> = row.get(3)?;
            let is_cue: i64 = row.get(4)?;
            let is_sacd: i64 = row.get(5)?;
            let track_count: i64 = row.get(6)?;
            let has_cover: i64 = row.get(7)?;
            let artists = serde_json::from_str::<Vec<String>>(&artists_json).unwrap_or_default();
            Ok(Album {
                id,
                name,
                artists,
                year: year.map(|y| y as u32),
                track_count: track_count as u32,
                has_cover: has_cover != 0,
                is_cue: is_cue != 0,
                is_sacd: is_sacd != 0,
            })
        })
        .expect("query albums")
        .flatten()
        .collect()
    }

    /// 普通专辑（不含 CUE / SACD 分轨专辑；虚拟专辑由独立入口承载）
    pub fn albums(&self) -> Vec<Album> {
        self.albums_filtered("normal")
    }

    /// CUE 分轨专辑（整盘镜像拆出的虚拟专辑）
    pub fn cue_albums(&self) -> Vec<Album> {
        self.albums_filtered("cue")
    }

    /// SACD ISO 分轨专辑（整张 ISO 拆出的虚拟多轨专辑）
    pub fn sacd_albums(&self) -> Vec<Album> {
        self.albums_filtered("sacd")
    }

    /// 按艺术家聚合（曲目数 / 去重专辑数）
    pub fn artists(&self) -> Vec<Artist> {
        let cue_cond = not_cue_source();
        let multich_cond = not_sacd_multich();
        let song_cond = not_duplicate_song();
        let sql = format!(
            r#"
            SELECT ta.artist,
                   COUNT(DISTINCT ta.track_id) AS track_count,
                   COUNT(DISTINCT t.album_id)  AS album_count
            FROM track_artists ta
            JOIN tracks t ON t.id = ta.track_id
                WHERE {cue_cond} AND {multich_cond} AND {song_cond}
            GROUP BY ta.artist
            ORDER BY ta.artist
        "#
        );
        let mut stmt = self.conn.prepare(&sql).expect("prepare artists");
        stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            let track_count: i64 = row.get(1)?;
            let album_count: i64 = row.get(2)?;
            Ok(Artist {
                id: format!("artist:{}", name),
                name,
                album_count: album_count as u32,
                track_count: track_count as u32,
            })
        })
        .expect("query artists")
        .flatten()
        .collect()
    }

    // ---- 收藏 ----

    pub fn favorite_ids(&self) -> Vec<String> {
        let mut stmt = self
            .conn
            .prepare("SELECT track_id FROM favorites ORDER BY added_at")
            .expect("prepare favorites");
        stmt.query_map([], |row| row.get::<_, String>(0))
            .expect("query favorites")
            .flatten()
            .collect()
    }

    pub fn set_favorite(&mut self, track_id: &str, favorite: bool) -> AppResult<()> {
        if favorite {
            self.conn.execute(
                "INSERT OR IGNORE INTO favorites(track_id, added_at) VALUES(?1, ?2)",
                params![track_id, chrono::Utc::now().timestamp()],
            )
        } else {
            self.conn
                .execute("DELETE FROM favorites WHERE track_id = ?1", [track_id])
        }
        .map_err(|e| AppError::Library(format!("写入收藏失败: {e}")))?;
        Ok(())
    }

    // ---- 最近播放 ----

    pub fn recent(&self) -> Vec<RecentEntry> {
        let mut stmt = self
            .conn
            .prepare("SELECT track_id, played_at FROM recent ORDER BY played_at DESC LIMIT 200")
            .expect("prepare recent");
        stmt.query_map([], |row| {
            Ok(RecentEntry {
                track_id: row.get(0)?,
                played_at: row.get(1)?,
            })
        })
        .expect("query recent")
        .flatten()
        .collect()
    }

    pub fn record_played(&mut self, track_id: &str) -> AppResult<()> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| AppError::Library(format!("开启播放记录事务失败: {e}")))?;
        tx.execute("DELETE FROM recent WHERE track_id = ?1", [track_id])
            .map_err(|e| AppError::Library(format!("更新播放记录失败: {e}")))?;
        tx.execute(
            "INSERT INTO recent(track_id, played_at) VALUES(?1, ?2)",
            params![track_id, chrono::Utc::now().timestamp()],
        )
        .map_err(|e| AppError::Library(format!("写入播放记录失败: {e}")))?;
        // 仅保留最近 200 条
        tx.execute(
            "DELETE FROM recent WHERE track_id NOT IN (
                SELECT track_id FROM recent ORDER BY played_at DESC LIMIT 200
            )",
            [],
        )
        .map_err(|e| AppError::Library(format!("裁剪播放记录失败: {e}")))?;
        tx.commit()
            .map_err(|e| AppError::Library(format!("提交播放记录失败: {e}")))?;
        Ok(())
    }

    // ---- 歌单 ----

    pub fn playlists(&self) -> Vec<Playlist> {
        let mut list: Vec<Playlist> = self
            .conn
            .prepare(
                "SELECT id, name, description, created_at, updated_at, is_system
                 FROM playlists ORDER BY created_at, id",
            )
            .expect("prepare playlists")
            .query_map([], |row| {
                Ok(PlaylistRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    is_system: row.get::<_, i64>(5)? != 0,
                })
            })
            .expect("query playlists")
            .flatten()
            .map(|row| Playlist {
                id: row.id,
                name: row.name,
                description: row.description,
                track_ids: Vec::new(),
                created_at: row.created_at,
                updated_at: row.updated_at,
                is_system: row.is_system,
            })
            .collect();

        for playlist in &mut list {
            playlist.track_ids = self.playlist_track_ids(&playlist.id);
        }
        list
    }

    fn playlist_track_ids(&self, id: &str) -> Vec<String> {
        self.conn
            .prepare("SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position")
            .expect("prepare playlist_tracks")
            .query_map([id], |row| row.get::<_, String>(0))
            .expect("query playlist_tracks")
            .flatten()
            .collect()
    }

    /// 创建歌单。ID 用纳秒时间戳 + 进程内自增序号，避免同一秒内连点/并发创建
    /// 触发主键冲突（旧实现为秒级时间戳 + `.expect`，同秒第二个直接 panic）。
    pub fn create_playlist(&mut self, name: String) -> AppResult<Playlist> {
        let now = chrono::Utc::now().timestamp();
        let id = format!("pl:{}-{}", now, Self::next_playlist_seq());
        self.conn
            .execute(
                "INSERT INTO playlists(id, name, description, created_at, updated_at, is_system)
                 VALUES(?1, ?2, '', ?3, ?3, 0)",
                params![id, name, now],
            )
            .map_err(|e| AppError::Library(format!("创建歌单失败: {e}")))?;
        Ok(Playlist {
            id,
            name,
            description: String::new(),
            track_ids: Vec::new(),
            created_at: now,
            updated_at: now,
            is_system: false,
        })
    }

    /// 进程内自增序号（与时间戳组合保证同秒内唯一）
    fn next_playlist_seq() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        SEQ.fetch_add(1, Ordering::Relaxed)
    }

    pub fn rename_playlist(&mut self, id: &str, name: String) -> AppResult<()> {
        let affected = self
            .conn
            .execute(
                "UPDATE playlists SET name = ?1, updated_at = ?2 WHERE id = ?3",
                params![name, chrono::Utc::now().timestamp(), id],
            )
            .map_err(|e| AppError::Library(format!("重命名歌单失败: {e}")))?;
        if affected == 0 {
            return Err(AppError::Library(format!("歌单不存在: {id}")));
        }
        Ok(())
    }

    pub fn delete_playlist(&mut self, id: &str) -> AppResult<()> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| AppError::Library(format!("开启歌单事务失败: {e}")))?;
        let affected = tx
            .execute("DELETE FROM playlists WHERE id = ?1", [id])
            .map_err(|e| AppError::Library(format!("删除歌单失败: {e}")))?;
        if affected == 0 {
            return Err(AppError::Library(format!("歌单不存在: {id}")));
        }
        tx.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", [id])
            .map_err(|e| AppError::Library(format!("删除歌单曲目失败: {e}")))?;
        tx.commit()
            .map_err(|e| AppError::Library(format!("提交删除歌单失败: {e}")))?;
        Ok(())
    }

    /// 整体替换歌单曲目（按传入顺序重排 position）
    pub fn set_playlist_tracks(&mut self, id: &str, track_ids: Vec<String>) -> AppResult<()> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| AppError::Library(format!("开启歌单事务失败: {e}")))?;
        ensure_playlist_exists(&tx, id)?;
        tx.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", [id])
            .map_err(|e| AppError::Library(format!("清空歌单失败: {e}")))?;
        for (position, track_id) in track_ids.iter().enumerate() {
            tx.execute(
                "INSERT OR REPLACE INTO playlist_tracks(playlist_id, track_id, position)
                 VALUES(?1, ?2, ?3)",
                params![id, track_id, position as i64],
            )
            .map_err(|e| AppError::Library(format!("写入歌单曲目失败: {e}")))?;
        }
        tx.execute(
            "UPDATE playlists SET updated_at = ?1 WHERE id = ?2",
            params![chrono::Utc::now().timestamp(), id],
        )
        .map_err(|e| AppError::Library(format!("更新歌单时间失败: {e}")))?;
        tx.commit()
            .map_err(|e| AppError::Library(format!("提交歌单失败: {e}")))?;
        Ok(())
    }

    pub fn add_tracks_to_playlist(&mut self, id: &str, track_ids: Vec<String>) -> AppResult<()> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| AppError::Library(format!("开启歌单事务失败: {e}")))?;
        ensure_playlist_exists(&tx, id)?;
        let mut next_position = tx
            .query_row(
                "SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_tracks WHERE playlist_id = ?1",
                [id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| AppError::Library(format!("查询歌单位置失败: {e}")))?;
        for track_id in track_ids {
            // 去重：歌单内已存在则跳过（保持原位置）
            let exists: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
                    params![id, track_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            if exists == 0 {
                tx.execute(
                    "INSERT INTO playlist_tracks(playlist_id, track_id, position)
                     VALUES(?1, ?2, ?3)",
                    params![id, track_id, next_position],
                )
                .map_err(|e| AppError::Library(format!("写入歌单曲目失败: {e}")))?;
                next_position += 1;
            }
        }
        tx.execute(
            "UPDATE playlists SET updated_at = ?1 WHERE id = ?2",
            params![chrono::Utc::now().timestamp(), id],
        )
        .map_err(|e| AppError::Library(format!("更新歌单时间失败: {e}")))?;
        tx.commit()
            .map_err(|e| AppError::Library(format!("提交歌单失败: {e}")))?;
        Ok(())
    }

    pub fn remove_track_from_playlist(&mut self, id: &str, track_id: &str) -> AppResult<()> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| AppError::Library(format!("开启歌单事务失败: {e}")))?;
        ensure_playlist_exists(&tx, id)?;
        tx.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
            params![id, track_id],
        )
        .map_err(|e| AppError::Library(format!("删除歌单曲目失败: {e}")))?;
        // 重排 position，保持连续
        let remaining: Vec<String> = tx
            .prepare("SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position")
            .map_err(|e| AppError::Library(format!("查询歌单失败: {e}")))?
            .query_map([id], |row| row.get::<_, String>(0))
            .map_err(|e| AppError::Library(format!("读取歌单失败: {e}")))?
            .flatten()
            .collect();
        for (position, tid) in remaining.iter().enumerate() {
            tx.execute(
                "UPDATE playlist_tracks SET position = ?1 WHERE playlist_id = ?2 AND track_id = ?3",
                params![position as i64, id, tid],
            )
            .map_err(|e| AppError::Library(format!("重排歌单失败: {e}")))?;
        }
        tx.execute(
            "UPDATE playlists SET updated_at = ?1 WHERE id = ?2",
            params![chrono::Utc::now().timestamp(), id],
        )
        .map_err(|e| AppError::Library(format!("更新歌单时间失败: {e}")))?;
        tx.commit()
            .map_err(|e| AppError::Library(format!("提交歌单失败: {e}")))?;
        Ok(())
    }

    // ---- 移除曲目 ----

    pub fn remove_tracks(&mut self, ids: &[String]) -> AppResult<()> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| AppError::Library(format!("开启移除事务失败: {e}")))?;
        for id in ids {
            tx.execute("DELETE FROM tracks WHERE id = ?1", [id])
                .map_err(|e| AppError::Library(format!("删除曲目失败: {e}")))?;
            cascade_remove_track(&tx, id)?;
        }
        tx.commit()
            .map_err(|e| AppError::Library(format!("提交移除失败: {e}")))?;
        Ok(())
    }

    /// 移除指定目录下的全部曲目（含级联清理收藏/最近播放/歌单引用）。
    ///
    /// 用于「用户主动移除曲库目录」：扫描侧的删除检测已改为保守策略
    /// （文件仍存在即保留，防止导入的外部文件与读取失败的目录被误删），
    /// 因此主动移除必须在此显式删除，否则目录下曲目会一直留在库中。
    /// 返回删除的曲目数。
    pub fn remove_tracks_under_dir(&mut self, dir: &str) -> AppResult<usize> {
        // 目录前缀：统一分隔符并补尾分隔符；Windows 大小写不敏感，统一转小写比较
        let norm = |p: &str| p.replace('\\', "/").to_lowercase();
        let prefix = {
            let mut p = norm(dir);
            if !p.ends_with('/') {
                p.push('/');
            }
            p
        };
        let ids: Vec<String> = {
            let mut stmt = self
                .conn
                .prepare("SELECT id, file_path FROM tracks")
                .map_err(|e| AppError::Library(format!("查询目录曲目失败: {e}")))?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|e| AppError::Library(format!("查询目录曲目失败: {e}")))?;
            rows.flatten()
                .filter(|(_, path)| norm(path).starts_with(&prefix))
                .map(|(id, _)| id)
                .collect()
        };
        if ids.is_empty() {
            return Ok(0);
        }
        self.remove_tracks(&ids)?;
        Ok(ids.len())
    }
}

/// playlists 表行（组装 Playlist 前的中间结构）
struct PlaylistRow {
    id: String,
    name: String,
    description: String,
    created_at: i64,
    updated_at: i64,
    is_system: bool,
}

/// rusqlite 行 → Track
fn row_to_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
    let artists_json: String = row.get("artists_json")?;
    Ok(Track {
        id: row.get("id")?,
        title: row.get("title")?,
        artists: serde_json::from_str(&artists_json).unwrap_or_default(),
        album: row.get("album")?,
        album_id: row.get("album_id")?,
        duration: row.get("duration")?,
        track_number: row
            .get::<_, Option<i64>>("track_number")?
            .map(|v| v as u32),
        year: row.get::<_, Option<i64>>("year")?.map(|v| v as u32),
        file_path: row.get("file_path")?,
        file_size: row.get::<_, i64>("file_size")? as u64,
        format: row.get("format")?,
        bitrate: row
            .get::<_, Option<i64>>("bitrate")?
            .map(|v| v as u32),
        sample_rate: row
            .get::<_, Option<i64>>("sample_rate")?
            .map(|v| v as u32),
        has_cover: row.get::<_, i64>("has_cover")? != 0,
        replaygain_db: row
            .get::<_, Option<f64>>("replaygain_db")?
            .map(|v| v as f32),
        replaygain_peak: row
            .get::<_, Option<f64>>("replaygain_peak")?
            .map(|v| v as f32),
        added_at: row.get("added_at")?,
        modified_at: row.get("modified_at")?,
        is_cue: row.get::<_, i64>("is_cue").unwrap_or(0) != 0,
        cue_offset: row.get::<_, Option<f64>>("cue_offset")?,
        is_sacd: row.get::<_, i64>("is_sacd").unwrap_or(0) != 0,
        sacd_track_index: row
            .get::<_, Option<i64>>("sacd_track_index")?
            .unwrap_or(0) as u32,
        sacd_area: row
            .get::<_, Option<String>>("sacd_area")?
            .and_then(|s| serde_json::from_str(&s).ok()),
        sacd_dsd_rate: row.get::<_, Option<String>>("sacd_dsd_rate")?,
        metadata_pending: row
            .get::<_, Option<i64>>("metadata_pending")?
            .unwrap_or(0)
            != 0,
        iso_placeholder: row
            .get::<_, Option<i64>>("iso_placeholder")?
            .unwrap_or(0)
            != 0,
    })
}

/// 插入或更新单曲（保留 added_at），并重建艺术家关联
fn upsert_track(
    tx: &rusqlite::Transaction<'_>,
    track: &Track,
) -> AppResult<()> {
    let artists_json = serde_json::to_string(&track.artists).unwrap_or_else(|_| "[]".into());
    tx.execute(
        r#"
        INSERT INTO tracks(
            id, title, album, album_id, duration, track_number, year,
            file_path, file_size, format, bitrate, sample_rate,
            has_cover, replaygain_db, replaygain_peak, added_at, modified_at, artists_json,
            is_cue, cue_offset, is_sacd, sacd_track_index, sacd_area, sacd_dsd_rate,
            metadata_pending, iso_placeholder
        ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26)
        ON CONFLICT(id) DO UPDATE SET
            title       = excluded.title,
            album       = excluded.album,
            album_id    = excluded.album_id,
            duration    = excluded.duration,
            track_number= excluded.track_number,
            year        = excluded.year,
            file_path   = excluded.file_path,
            file_size   = excluded.file_size,
            format      = excluded.format,
            bitrate     = excluded.bitrate,
            sample_rate = excluded.sample_rate,
            has_cover   = excluded.has_cover,
            replaygain_db   = excluded.replaygain_db,
            replaygain_peak = excluded.replaygain_peak,
            modified_at = excluded.modified_at,
            artists_json= excluded.artists_json,
            is_cue      = excluded.is_cue,
            cue_offset  = excluded.cue_offset,
            is_sacd     = excluded.is_sacd,
            sacd_track_index = excluded.sacd_track_index,
            sacd_area   = excluded.sacd_area,
            sacd_dsd_rate = excluded.sacd_dsd_rate,
            metadata_pending = excluded.metadata_pending,
            iso_placeholder   = excluded.iso_placeholder
        "#,
        params![
            track.id,
            track.title,
            track.album,
            track.album_id,
            track.duration,
            track.track_number.map(|v| v as i64),
            track.year.map(|v| v as i64),
            track.file_path,
            track.file_size as i64,
            track.format,
            track.bitrate.map(|v| v as i64),
            track.sample_rate.map(|v| v as i64),
            track.has_cover as i64,
            track.replaygain_db.map(|v| v as f64),
            track.replaygain_peak.map(|v| v as f64),
            track.added_at,
            track.modified_at,
            artists_json,
            track.is_cue as i64,
            track.cue_offset,
            track.is_sacd as i64,
            track.sacd_track_index as i64,
            track.sacd_area.as_ref().map(|a| serde_json::to_string(a).unwrap_or_default()),
            track.sacd_dsd_rate,
            track.metadata_pending as i64,
            track.iso_placeholder as i64,
        ],
    )
    .map_err(|e| AppError::Library(format!("写入曲目失败: {e}")))?;

    tx.execute("DELETE FROM track_artists WHERE track_id = ?1", [&track.id])
        .map_err(|e| AppError::Library(format!("清理艺术家关联失败: {e}")))?;
    for (position, artist) in track.artists.iter().enumerate() {
        tx.execute(
            "INSERT OR IGNORE INTO track_artists(track_id, artist, position)
             VALUES(?1, ?2, ?3)",
            params![track.id, artist, position as i64],
        )
        .map_err(|e| AppError::Library(format!("写入艺术家关联失败: {e}")))?;
    }
    Ok(())
}

/// 删除曲目时级联清理用户数据（收藏 / 最近播放 / 歌单引用）
fn cascade_remove_track(tx: &rusqlite::Transaction<'_>, id: &str) -> AppResult<()> {
    tx.execute("DELETE FROM track_artists WHERE track_id = ?1", [id])
        .map_err(|e| AppError::Library(format!("清理艺术家关联失败: {e}")))?;
    tx.execute("DELETE FROM favorites WHERE track_id = ?1", [id])
        .map_err(|e| AppError::Library(format!("清理收藏失败: {e}")))?;
    tx.execute("DELETE FROM recent WHERE track_id = ?1", [id])
        .map_err(|e| AppError::Library(format!("清理最近播放失败: {e}")))?;
    tx.execute("DELETE FROM playlist_tracks WHERE track_id = ?1", [id])
        .map_err(|e| AppError::Library(format!("清理歌单引用失败: {e}")))?;
    Ok(())
}

fn ensure_playlist_exists(
    tx: &rusqlite::Transaction<'_>,
    id: &str,
) -> AppResult<()> {
    let exists: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM playlists WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|e| AppError::Library(format!("查询歌单失败: {e}")))?;
    if exists == 0 {
        return Err(AppError::Library(format!("歌单不存在: {id}")));
    }
    Ok(())
}

///
/// 首次启动迁移：旧 library.json 存在时全量导入空数据库，成功后把 JSON
/// 改名为 library.json.bak（解析失败按空库处理，沿用旧版 unwrap_or_default 语义）。
fn migrate_legacy_json(conn: &mut Connection, legacy_json_path: &std::path::Path) -> AppResult<()> {
    let Ok(content) = std::fs::read_to_string(legacy_json_path) else {
        return Ok(());
    };
    let data: LibraryData = serde_json::from_str(&content).unwrap_or_else(|_| LibraryData {
        tracks: Vec::new(),
        playlists: Vec::new(),
        favorite_ids: Vec::new(),
        recent: Vec::new(),
        stats: LegacyStats::default(),
    });

    let tx = conn
        .transaction()
        .map_err(|e| AppError::Library(format!("开启迁移事务失败: {e}")))?;
    for track in &data.tracks {
        // 兼容更早期 album_id 为空的数据：按当前规则补算
        let mut track = track.clone();
        if track.album_id.is_none() {
            track.album_id = Some(album_id_of(
                &track.album,
                track.artists.first().map(String::as_str).unwrap_or(""),
            ));
        }
        upsert_track(&tx, &track)?;
    }
    let now = chrono::Utc::now().timestamp();
    for (index, id) in data.favorite_ids.iter().enumerate() {
        tx.execute(
            "INSERT OR IGNORE INTO favorites(track_id, added_at) VALUES(?1, ?2)",
            params![id, now - index as i64],
        )
        .map_err(|e| AppError::Library(format!("迁移收藏失败: {e}")))?;
    }
    for entry in &data.recent {
        tx.execute(
            "INSERT OR IGNORE INTO recent(track_id, played_at) VALUES(?1, ?2)",
            params![entry.track_id, entry.played_at],
        )
        .map_err(|e| AppError::Library(format!("迁移最近播放失败: {e}")))?;
    }
    for playlist in &data.playlists {
        tx.execute(
            "INSERT OR REPLACE INTO playlists(id, name, description, created_at, updated_at, is_system)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                playlist.id,
                playlist.name,
                playlist.description,
                playlist.created_at,
                playlist.updated_at,
                playlist.is_system as i64
            ],
        )
        .map_err(|e| AppError::Library(format!("迁移歌单失败: {e}")))?;
        for (position, track_id) in playlist.track_ids.iter().enumerate() {
            tx.execute(
                "INSERT OR REPLACE INTO playlist_tracks(playlist_id, track_id, position)
                 VALUES(?1, ?2, ?3)",
                params![playlist.id, track_id, position as i64],
            )
            .map_err(|e| AppError::Library(format!("迁移歌单曲目失败: {e}")))?;
        }
    }
    if let Some(scan_at) = data.stats.last_scan_at {
        tx.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES('last_scan_at', ?1)",
            [scan_at.to_string()],
        )
        .map_err(|e| AppError::Library(format!("迁移扫描时间失败: {e}")))?;
    }
    tx.commit()
        .map_err(|e| AppError::Library(format!("提交迁移事务失败: {e}")))?;

    // 导入成功后备份旧文件（覆盖旧备份）
    let backup = legacy_json_path.with_extension("json.bak");
    let _ = std::fs::remove_file(&backup);
    // 备份失败不阻断启动：旧 library.json 保留在原地，后续仍可手动导入
    let _ = std::fs::rename(legacy_json_path, &backup);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 内存库（建表 + 结构迁移，与真实库一致）
    fn test_store() -> LibraryStore {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        db::init_schema(&conn).expect("init schema");
        db::migrate(&conn).expect("migrate schema");
        LibraryStore {
            conn,
            db_path: std::env::temp_dir().join("tplayer-test-inmemory.db"),
        }
    }

    /// 插入一条曲目（仅测试所需字段）
    fn insert_track(store: &LibraryStore, id: &str, path: &str, album: &str, is_cue: bool) {
        store
            .conn
            .execute(
                "INSERT INTO tracks(id, title, album, duration, file_path, file_size, format,
                                    added_at, modified_at, artists_json, is_cue)
                 VALUES(?1, ?2, ?3, 0.0, ?4, 0, 'flac', 0, 0, '[]', ?5)",
                params![id, id, album, path, is_cue as i64],
            )
            .expect("insert track");
    }

    fn ids(store: &LibraryStore) -> Vec<String> {
        let mut stmt = store.conn.prepare("SELECT id FROM tracks ORDER BY id").unwrap();
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap();
        rows.flatten().collect()
    }

    /// 「全部歌曲」查询应隐藏整碟镜像
    fn visible_ids(store: &LibraryStore) -> Vec<String> {
        store.tracks().into_iter().map(|t| t.id).collect()
    }

    /// 插入一条带完整键（标题/艺人/专辑/格式）的曲目
    fn insert_song(
        store: &LibraryStore,
        id: &str,
        path: &str,
        title: &str,
        artist: &str,
        album: &str,
        format: &str,
        added_at: i64,
    ) {
        let artists_json = serde_json::to_string(&vec![artist.to_string()]).unwrap();
        store
            .conn
            .execute(
                "INSERT INTO tracks(id, title, album, duration, file_path, file_size, format,
                                    added_at, modified_at, artists_json, is_cue)
                 VALUES(?1, ?2, ?3, 0.0, ?4, 0, ?5, ?6, 0, ?7, 0)",
                params![id, title, album, path, format, added_at, artists_json],
            )
            .expect("insert song");
    }

    /// 同一首歌（同主艺人+标题+专辑+格式）放在不同目录 → 只保留一条
    #[test]
    fn dedupes_same_song_across_directories() {
        let store = test_store();
        insert_song(
            &store,
            "a",
            r"D:\A\Album\01.flac",
            "Bohemian Rhapsody",
            "Queen",
            "A Night at the Opera",
            "flac",
            100,
        );
        insert_song(
            &store,
            "b",
            r"D:\杂录\Queen - Bohemian Rhapsody.flac",
            "Bohemian Rhapsody",
            "Queen",
            "A Night at the Opera",
            "flac",
            200,
        );
        // 大小写/空白差异也算同一首
        insert_song(
            &store,
            "c",
            r"D:\B\queen - bohemian rhapsody .flac",
            " bohemian rhapsody ",
            "QUEEN",
            "a night at the opera",
            "FLAC",
            300,
        );
        assert_eq!(visible_ids(&store), vec!["a".to_string()], "应只保留最先入库的那条");
    }

    /// 格式不同（flac / mp3 各一份）→ 视为不同版本，都保留
    #[test]
    fn keeps_different_formats_of_same_song() {
        let store = test_store();
        insert_song(&store, "flac", r"D:\A\01.flac", "Song", "A", "Al", "flac", 100);
        insert_song(&store, "mp3", r"D:\A\01.mp3", "Song", "A", "Al", "mp3", 200);
        insert_song(&store, "ape", r"D:\B\01.ape", "Song", "A", "Al", "ape", 300);
        assert_eq!(
            visible_ids(&store),
            vec!["flac".to_string(), "mp3".to_string(), "ape".to_string()],
            "格式不同应全部保留（顺序按 added_at）"
        );
    }

    /// 标题相同但专辑 / 艺人不同 → 不是同一首歌，都保留
    #[test]
    fn keeps_same_title_in_different_albums() {
        let store = test_store();
        insert_song(&store, "x", r"D:\A\01.flac", "Yesterday", "The Beatles", "Help!", "flac", 100);
        insert_song(
            &store,
            "y",
            r"D:\B\Yesterday.flac",
            "Yesterday",
            "The Beatles",
            "Yesterday and Today",
            "flac",
            200,
        );
        insert_song(
            &store,
            "z",
            r"D:\C\Yesterday.flac",
            "Yesterday",
            "Else",
            "Help!",
            "flac",
            300,
        );
        assert_eq!(visible_ids(&store).len(), 3);
    }

    /// SACD 虚拟分轨不参与同一首歌去重（版本由专辑页区域切换表达）
    #[test]
    fn sacd_tracks_are_not_deduped() {
        let store = test_store();
        insert_song(&store, "p", r"D:\A\01.flac", "Song", "A", "Al", "flac", 100);
        store
            .conn
            .execute(
                "INSERT INTO tracks(id, title, album, duration, file_path, file_size, format,
                                    added_at, modified_at, artists_json, is_cue, is_sacd)
                 VALUES('s', 'Song', 'Al', 0.0, 'D:\\A\\disc.iso', 0, 'iso-dsd', 200, 0, '[\"A\"]', 0, 1)",
                [],
            )
            .unwrap();
        let mut visible = visible_ids(&store);
        visible.sort();
        assert_eq!(visible, vec!["p".to_string(), "s".to_string()]);
    }

    /// 同目录同基名的整碟副本（.ape / .flac）不应出现在普通列表
    #[test]
    fn tracks_query_hides_same_stem_mirror() {
        let store = test_store();
        insert_track(&store, "cue1", r"G:\music\Album.flac", "Album", true);
        insert_track(&store, "mirror", r"G:\music\Album.ape", "Album", false);
        insert_track(&store, "other", r"G:\music\Other.mp3", "Other", false);

        assert_eq!(visible_ids(&store), vec!["cue1".to_string(), "other".to_string()]);
    }

    /// 镜像文件名与 CUE 源文件不同名、但同目录同专辑名 → 同样隐藏
    #[test]
    fn tracks_query_hides_same_album_mirror() {
        let store = test_store();
        insert_track(&store, "cue1", r"G:\music\Volume 1(CD6).flac", "Volume 1(CD6)", true);
        insert_track(
            &store,
            "mirror",
            r"G:\music\Mozart.-.[Brilliant].ape",
            "Volume 1(CD6)",
            false,
        );

        assert_eq!(visible_ids(&store), vec!["cue1".to_string()]);
    }

    /// 同名专辑但不同目录 → 视为不同专辑，保留
    #[test]
    fn tracks_query_keeps_same_album_other_dir() {
        let store = test_store();
        insert_track(&store, "cue1", r"G:\music\Album.flac", "Album", true);
        insert_track(&store, "other", r"G:\other\Album.ape", "Album", false);

        assert_eq!(visible_ids(&store).len(), 2);
    }

    /// CUE 分轨本身必须保留（不能把自己当成镜像隐藏）
    #[test]
    fn tracks_query_keeps_cue_tracks() {
        let store = test_store();
        insert_track(&store, "cue1", r"G:\music\Album.flac", "Album", true);
        insert_track(&store, "cue2", r"G:\music\Album.flac", "Album", true);

        assert_eq!(visible_ids(&store).len(), 2);
    }

    /// 统计口径与列表一致：镜像条目不计入曲目数
    #[test]
    fn stats_excludes_cue_mirror() {
        let store = test_store();
        insert_track(&store, "cue1", r"G:\music\Album.flac", "Album", true);
        insert_track(&store, "mirror", r"G:\music\Album.ape", "Album", false);

        assert_eq!(store.stats().track_count, 1);
    }
}
