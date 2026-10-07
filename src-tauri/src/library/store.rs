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
use super::models::{album_id_of, Album, Artist, LibraryStats, Playlist, RecentEntry, Track};

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
        Ok(Self { conn })
    }

    // ---- 曲目查询 ----

    pub fn tracks(&self) -> Vec<Track> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM tracks ORDER BY added_at, id")
            .expect("prepare tracks");
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

    /// 专辑（含 CUE 分轨专辑）下全部曲目，按音轨号升序
    pub fn tracks_of_album(&self, album_id: &str) -> Vec<Track> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT * FROM tracks WHERE album_id = ?1
                 ORDER BY COALESCE(track_number, 2147483647), id",
            )
            .expect("prepare tracks_of_album");
        stmt.query_map([album_id], row_to_track)
            .expect("query tracks_of_album")
            .flatten()
            .collect()
    }

    /// 艺术家参与（任一阵位）的全部曲目，与前端 artists.includes 口径一致
    pub fn tracks_of_artist(&self, artist: &str) -> Vec<Track> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT t.* FROM tracks t
                 JOIN track_artists ta ON ta.track_id = t.id
                 WHERE ta.artist = ?1
                 ORDER BY t.album, COALESCE(t.track_number, 2147483647), t.id",
            )
            .expect("prepare tracks_of_artist");
        stmt.query_map([artist], row_to_track)
            .expect("query tracks_of_artist")
            .flatten()
            .collect()
    }

    ///
    /// 分组封面候选曲目（封面聚合命令用）：
    /// 扫描期确认带内嵌封面的优先，其次音轨号升序，上限 30 首。
    pub fn cover_candidates_of_album(&self, album_id: &str, limit: usize) -> Vec<Track> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT * FROM tracks WHERE album_id = ?1
                 ORDER BY has_cover DESC, COALESCE(track_number, 2147483647), id
                 LIMIT ?2",
            )
            .expect("prepare cover_candidates_of_album");
        stmt.query_map(params![album_id, limit as i64], row_to_track)
            .expect("query cover_candidates_of_album")
            .flatten()
            .collect()
    }

    pub fn cover_candidates_of_artist(&self, artist: &str, limit: usize) -> Vec<Track> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT DISTINCT t.* FROM tracks t
                 JOIN track_artists ta ON ta.track_id = t.id
                 WHERE ta.artist = ?1
                 ORDER BY t.has_cover DESC, COALESCE(t.track_number, 2147483647), t.id
                 LIMIT ?2",
            )
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
        let (track_count, total_duration, album_count) = self
            .conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(duration), 0.0),
                        COUNT(DISTINCT album_id) FROM tracks",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)? as u32,
                        row.get::<_, f64>(1)?,
                        row.get::<_, i64>(2)? as u32,
                    ))
                },
            )
            .unwrap_or((0, 0.0, 0));
        let artist_count = self
            .conn
            .query_row("SELECT COUNT(DISTINCT artist) FROM track_artists", [], |row| {
                row.get::<_, i64>(0)
            })
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

    /// 按专辑聚合：每组取音轨号最小者为代表（artists/year）
    pub fn albums(&self) -> Vec<Album> {
        let sql = r#"
            WITH ranked AS (
                SELECT
                    album_id, album, artists_json, year,
                    ROW_NUMBER() OVER (
                        PARTITION BY album_id
                        ORDER BY COALESCE(track_number, 2147483647), id
                    ) AS rn,
                    COUNT(*)     OVER (PARTITION BY album_id) AS tc,
                    MAX(has_cover) OVER (PARTITION BY album_id) AS hc
                FROM tracks
                WHERE album_id IS NOT NULL
            )
            SELECT album_id, album, artists_json, year, tc, hc
            FROM ranked WHERE rn = 1
            ORDER BY album
        "#;
        let mut stmt = self.conn.prepare(sql).expect("prepare albums");
        stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let artists_json: String = row.get(2)?;
            let year: Option<i64> = row.get(3)?;
            let track_count: i64 = row.get(4)?;
            let has_cover: i64 = row.get(5)?;
            let artists = serde_json::from_str::<Vec<String>>(&artists_json).unwrap_or_default();
            Ok(Album {
                id,
                name,
                artists,
                year: year.map(|y| y as u32),
                track_count: track_count as u32,
                has_cover: has_cover != 0,
                is_cue: false,
            })
        })
        .expect("query albums")
        .flatten()
        .collect()
    }

    /// CUE 分轨专辑（当前无 CUE 解析，返回空）
    pub fn cue_albums(&self) -> Vec<Album> {
        Vec::new()
    }

    /// 按艺术家聚合（曲目数 / 去重专辑数）
    pub fn artists(&self) -> Vec<Artist> {
        let sql = r#"
            SELECT ta.artist,
                   COUNT(DISTINCT ta.track_id) AS track_count,
                   COUNT(DISTINCT t.album_id)  AS album_count
            FROM track_artists ta
            JOIN tracks t ON t.id = ta.track_id
            GROUP BY ta.artist
            ORDER BY ta.artist
        "#;
        let mut stmt = self.conn.prepare(sql).expect("prepare artists");
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

    pub fn create_playlist(&mut self, name: String) -> Playlist {
        let now = chrono::Utc::now().timestamp();
        let id = format!("pl:{}", now);
        self.conn
            .execute(
                "INSERT INTO playlists(id, name, description, created_at, updated_at, is_system)
                 VALUES(?1, ?2, '', ?3, ?3, 0)",
                params![id, name, now],
            )
            .expect("insert playlist");
        Playlist {
            id,
            name,
            description: String::new(),
            track_ids: Vec::new(),
            created_at: now,
            updated_at: now,
            is_system: false,
        }
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
        added_at: row.get("added_at")?,
        modified_at: row.get("modified_at")?,
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
            has_cover, added_at, modified_at, artists_json
        ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)
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
            modified_at = excluded.modified_at,
            artists_json= excluded.artists_json
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
            track.added_at,
            track.modified_at,
            artists_json,
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
    if let Err(e) = std::fs::rename(legacy_json_path, &backup) {
        eprintln!("[library] 备份旧 library.json 失败: {e}");
    }
    Ok(())
}
