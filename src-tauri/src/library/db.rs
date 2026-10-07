//! SQLite 连接与 schema（曲库持久化底座）
//!
//! 设计要点：
//! - WAL 日志：读写并发更好、写入是小事务（收藏/播放记录不再全量序列化 JSON）；
//! - 曲目艺术家拆到 track_artists 关联表，艺术家聚合走 JOIN（JSON 列仅为快速还原模型）；
//! - meta 表保存 schema 版本与 last_scan_at 等标量；
//! - 所有 Connection 方法只需要 &self，LibraryStore 由外层 Mutex 串行化即可。

use std::path::Path;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

/// 当前 schema 版本（变更表结构时递增并写迁移分支）
pub const SCHEMA_VERSION: u32 = 1;

/// 打开（必要时创建）曲库数据库并完成 PRAGMA / 建表
pub fn open(db_path: &Path) -> AppResult<Connection> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Library(format!("创建数据目录失败: {}", e)))?;
    }

    let conn = Connection::open(db_path)
        .map_err(|e| AppError::Library(format!("打开曲库数据库失败: {}", e)))?;

    // 忙等待最多 5s（WAL 下写锁偶发竞争）
    conn.pragma_update(None, "busy_timeout", 5000)
        .map_err(|e| AppError::Library(format!("设置 busy_timeout 失败: {}", e)))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| AppError::Library(format!("启用 WAL 失败: {}", e)))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| AppError::Library(format!("启用外键失败: {}", e)))?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| AppError::Library(format!("设置 synchronous 失败: {}", e)))?;

    init_schema(&conn)?;
    Ok(conn)
}

/// 建表（IF NOT EXISTS，幂等）
pub fn init_schema(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(SCHEMA_SQL)
        .map_err(|e| AppError::Library(format!("初始化曲库表结构失败: {}", e)))?;
    Ok(())
}

const SCHEMA_SQL: &str = r#"
-- 标量元数据（schema_version / last_scan_at ...）
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- 单曲（id 为文件路径哈希，天然稳定；file_path 唯一供增量扫描比对）
CREATE TABLE IF NOT EXISTS tracks (
    id           TEXT PRIMARY KEY,
    title        TEXT NOT NULL,
    album        TEXT NOT NULL,
    album_id     TEXT,
    duration     REAL NOT NULL,
    track_number INTEGER,
    year         INTEGER,
    file_path    TEXT NOT NULL UNIQUE,
    file_size    INTEGER NOT NULL,
    format       TEXT NOT NULL,
    bitrate      INTEGER,
    sample_rate  INTEGER,
    has_cover    INTEGER NOT NULL DEFAULT 0,
    added_at     INTEGER NOT NULL,
    modified_at  INTEGER NOT NULL,
    -- 艺术家数组 JSON，仅用于快速还原 Track 模型；聚合查询走 track_artists
    artists_json TEXT NOT NULL DEFAULT '[]'
);
CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album_id);

-- 曲目 ↔ 艺术家 关联（含主艺术家，position=0）
CREATE TABLE IF NOT EXISTS track_artists (
    track_id TEXT NOT NULL,
    artist   TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY (track_id, artist, position)
);
CREATE INDEX IF NOT EXISTS idx_track_artists_artist ON track_artists(artist);

-- 收藏
CREATE TABLE IF NOT EXISTS favorites (
    track_id TEXT PRIMARY KEY,
    added_at INTEGER NOT NULL
);

-- 最近播放（played_at 降序维护，上限 200 条由写入侧裁剪）
CREATE TABLE IF NOT EXISTS recent (
    track_id  TEXT PRIMARY KEY,
    played_at INTEGER NOT NULL
);

-- 歌单
CREATE TABLE IF NOT EXISTS playlists (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    is_system   INTEGER NOT NULL DEFAULT 0
);

-- 歌单曲目（position 即顺序）
CREATE TABLE IF NOT EXISTS playlist_tracks (
    playlist_id TEXT NOT NULL,
    track_id    TEXT NOT NULL,
    position    INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, position)
);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_playlist ON playlist_tracks(playlist_id);
"#;

/// 读 meta 标量
pub fn get_meta(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
        row.get::<_, String>(0)
    })
    .ok()
}

/// 写 meta 标量（upsert）
pub fn set_meta(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO meta(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )
    .map_err(|e| AppError::Library(format!("写入 meta({key}) 失败: {e}")))?;
    Ok(())
}
