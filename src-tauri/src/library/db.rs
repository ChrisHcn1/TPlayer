//! SQLite 连接与 schema（曲库持久化底座）
//!
//! 设计要点：
//! - WAL 日志：读写并发更好、写入是小事务（收藏/播放记录不再全量序列化 JSON）；
//! - 曲目艺术家拆到 track_artists 关联表，艺术家聚合走 JOIN（JSON 列仅为快速还原模型）；
//! - meta 表保存 schema 版本与 last_scan_at 等标量；
//! - 所有 Connection 方法只需要 &self，LibraryStore 由外层 Mutex 串行化即可。

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use rusqlite::{Connection, OpenFlags};

use crate::error::{AppError, AppResult};

/// 当前 schema 版本（变更表结构时递增并写迁移分支）
pub const SCHEMA_VERSION: u32 = 6;

/// 「好状态」备份文件：`library.db` → `library.db.bak`
fn backup_path(db_path: &Path) -> PathBuf {
    db_path.with_extension("db.bak")
}

/// 运行标记文件：`library.db` → `library.db.run`。
/// 启动时写入、干净退出时删除；下次启动若仍存在，说明上次是崩溃 / 强杀 /
/// 卡死未走正常退出流程，此时回退到最后一次好状态。
fn run_marker_path(db_path: &Path) -> PathBuf {
    db_path.with_extension("db.run")
}

/// 备份节流：避免每次扫描都做一次全库快照
fn last_backup_slot() -> &'static Mutex<Option<Instant>> {
    static LAST: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    LAST.get_or_init(|| Mutex::new(None))
}

/// 打开（必要时创建）曲库数据库并完成 PRAGMA / 建表。
///
/// 崩溃回退：打开前先检查上一次是否干净退出——
///  - 运行标记仍在（上次崩溃 / 强杀 / 卡死），或
///  - 库文件已损坏（`PRAGMA quick_check` 非 ok）
/// 满足任一条件且存在 `.bak` 快照时，先把库还原成最后一次好状态再打开，
/// 即「下次加载退回到最后一次正常状态」。无快照时保持原样尽力打开。
pub fn open(db_path: &Path) -> AppResult<Connection> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Library(format!("创建数据目录失败: {}", e)))?;
    }

    recover_from_backup_if_needed(db_path);

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
    migrate(&conn)?;
    migrate_data(&conn)?;

    // 本次运行标记（干净退出时由 mark_clean 清除）
    let _ = std::fs::write(run_marker_path(db_path), b"running");
    Ok(conn)
}

/// 干净退出：清除运行标记，下次启动不做回退。
pub fn mark_clean(db_path: &Path) {
    let _ = std::fs::remove_file(run_marker_path(db_path));
}

/// 把当前库写成「好状态」快照（`VACUUM INTO` 生成一致且紧凑的副本）。
///
/// 节流：5 分钟内不重复备份；调用方在扫描成功提交后调用。
/// 先写临时文件再原子改名，避免强杀时留下半个快照。
pub fn commit_good(conn: &Connection, db_path: &Path) {
    {
        let mut last = last_backup_slot().lock().unwrap();
        if let Some(t) = *last {
            if t.elapsed() < Duration::from_secs(300) {
                return;
            }
        }
        *last = Some(Instant::now());
    }
    write_backup(conn, db_path);
}

/// 立即写一次「好状态」快照（无节流；测试与需要强制刷新的场景使用）。
pub(crate) fn write_backup(conn: &Connection, db_path: &Path) {
    let bak = backup_path(db_path);
    let tmp = db_path.with_extension("db.bak.tmp");
    let _ = std::fs::remove_file(&tmp);
    let ok = conn
        .execute("VACUUM INTO ?1", [tmp.to_string_lossy().to_string()])
        .is_ok();
    if !ok {
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    if std::fs::rename(&tmp, &bak).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// 需要回退时从 `.bak` 还原（删除 db 与其 WAL/SHM 后原子替换）。
fn recover_from_backup_if_needed(db_path: &Path) {
    if !db_path.exists() {
        return;
    }
    let bak = backup_path(db_path);
    if !bak.exists() {
        // 没有好状态可回退；清掉过期运行标记，后续按新库/损坏库尽力打开
        let _ = std::fs::remove_file(run_marker_path(db_path));
        return;
    }

    let unclean_exit = run_marker_path(db_path).exists();
    let corrupt = !db_is_healthy(db_path);
    if !unclean_exit && !corrupt {
        return;
    }

    let tmp = db_path.with_extension("db.restore");
    let _ = std::fs::remove_file(&tmp);
    if std::fs::copy(&bak, &tmp).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    // 主库与 WAL/SHM 必须一起清掉，否则陈旧 WAL 会污染还原后的库
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let stale = PathBuf::from(format!("{}{}", db_path.display(), suffix));
        let _ = std::fs::remove_file(&stale);
    }
    if std::fs::rename(&tmp, db_path).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    let _ = std::fs::remove_file(run_marker_path(db_path));
    eprintln!("[library] 上次未干净退出或库已损坏，已从备份恢复曲库（回退到最后一次好状态）");
}

/// 只读打开做一次 `quick_check`，判断库文件是否完好
fn db_is_healthy(db_path: &Path) -> bool {
    match Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => conn
            .query_row("PRAGMA quick_check(1)", [], |r| r.get::<_, String>(0))
            .map(|s| s.eq_ignore_ascii_case("ok"))
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// 一次性数据迁移（区别于结构迁移 migrate）。
///
/// v4 -> v5：CUE 分轨专辑的 `album_id` 归一为 `album:<专辑名>:`（仅按专辑名聚合）。
/// 修复旧版把每轨演唱者写进 album_id，导致合辑（如 6CD 群星）每轨不同歌手被拆成
/// "每轨一专辑"的问题；新扫描由 cue.rs 直接生成同形 album_id。
///
/// 幂等/安全：
///  - 新库（无 schema_version）跳过，交由 store::open 迁移旧 JSON 后统一写版本，避免
///    抢在 migrate_legacy_json 前把版本写成最新而跳过旧数据导入；
///  - 版本已 >= SCHEMA_VERSION 直接跳过（不会重复执行）。
fn migrate_data(conn: &Connection) -> AppResult<()> {
    let stored = get_meta(conn, "schema_version");
    match stored.as_deref().and_then(|v| v.parse::<u32>().ok()) {
        None => return Ok(()),                     // 新库：由 store::open 统一处理
        Some(v) if v >= SCHEMA_VERSION => return Ok(()), // 已最新
        Some(_) => {}
    }
    conn.execute(
        "UPDATE tracks
         SET album_id = 'album:' || album || ':'
         WHERE is_cue = 1 AND album_id IS NOT NULL",
        [],
    )
    .map_err(|e| AppError::Library(format!("迁移 CUE 专辑聚合（v5）失败: {e}")))?;
    set_meta(conn, "schema_version", &SCHEMA_VERSION.to_string())?;
    Ok(())
}

/// 幂等迁移：老库已建表但缺新列时补充（新库由 SCHEMA_SQL 直接带列，跳过）
pub(crate) fn migrate(conn: &Connection) -> AppResult<()> {
    let has_column = |name: &str| -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('tracks') WHERE name = ?1",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false)
    };
    // schema v3：去掉 file_path 唯一约束（CUE 分轨共享镜像路径），并新增
    // is_cue / cue_offset 列。SQLite 不支持直接 DROP UNIQUE，故整体重建 tracks 表。
    if !has_column("is_cue") {
        conn.execute_batch(
            "CREATE TABLE tracks_new (
                id           TEXT PRIMARY KEY,
                title        TEXT NOT NULL,
                album        TEXT NOT NULL,
                album_id     TEXT,
                duration     REAL NOT NULL,
                track_number INTEGER,
                year         INTEGER,
                file_path    TEXT NOT NULL,
                file_size    INTEGER NOT NULL,
                format       TEXT NOT NULL,
                bitrate      INTEGER,
                sample_rate  INTEGER,
                has_cover    INTEGER NOT NULL DEFAULT 0,
                added_at     INTEGER NOT NULL,
                modified_at  INTEGER NOT NULL,
                artists_json TEXT NOT NULL DEFAULT '[]',
                is_cue       INTEGER NOT NULL DEFAULT 0,
                cue_offset   REAL,
                replaygain_db   REAL,
                replaygain_peak REAL
            );
            INSERT INTO tracks_new (
                id, title, album, album_id, duration, track_number, year,
                file_path, file_size, format, bitrate, sample_rate,
                has_cover, added_at, modified_at, artists_json
            )
            SELECT id, title, album, album_id, duration, track_number, year,
                   file_path, file_size, format, bitrate, sample_rate,
                   has_cover, added_at, modified_at, artists_json
            FROM tracks;
            DROP TABLE tracks;
            CREATE INDEX idx_tracks_album ON tracks_new(album_id);
            ALTER TABLE tracks_new RENAME TO tracks;",
        )
        .map_err(|e| AppError::Library(format!("迁移 schema v3 失败: {e}")))?;
    }
    if !has_column("replaygain_db") {
        conn.execute_batch("ALTER TABLE tracks ADD COLUMN replaygain_db REAL")
            .map_err(|e| AppError::Library(format!("迁移 schema v2 失败（replaygain_db）: {e}")))?;
    }
    if !has_column("replaygain_peak") {
        conn.execute_batch("ALTER TABLE tracks ADD COLUMN replaygain_peak REAL")
            .map_err(|e| AppError::Library(format!("迁移 schema v2 失败（replaygain_peak）: {e}")))?;
    }
    // —— SACD ISO 分轨列（与 CUE 同构）——
    if !has_column("is_sacd") {
        conn.execute_batch("ALTER TABLE tracks ADD COLUMN is_sacd INTEGER NOT NULL DEFAULT 0")
            .map_err(|e| AppError::Library(format!("迁移 schema v4 失败（is_sacd）: {e}")))?;
    }
    if !has_column("sacd_track_index") {
        conn.execute_batch("ALTER TABLE tracks ADD COLUMN sacd_track_index INTEGER NOT NULL DEFAULT 0")
            .map_err(|e| AppError::Library(format!("迁移 schema v4 失败（sacd_track_index）: {e}")))?;
    }
    if !has_column("sacd_area") {
        conn.execute_batch("ALTER TABLE tracks ADD COLUMN sacd_area TEXT")
            .map_err(|e| AppError::Library(format!("迁移 schema v4 失败（sacd_area）: {e}")))?;
    }
    if !has_column("sacd_dsd_rate") {
        conn.execute_batch("ALTER TABLE tracks ADD COLUMN sacd_dsd_rate TEXT")
            .map_err(|e| AppError::Library(format!("迁移 schema v4 失败（sacd_dsd_rate）: {e}")))?;
    }
    // —— v6：两阶段懒加载标记（轻量占位轨道 = 尚未解析元数据）——
    if !has_column("metadata_pending") {
        conn.execute_batch(
            "ALTER TABLE tracks ADD COLUMN metadata_pending INTEGER NOT NULL DEFAULT 0",
        )
        .map_err(|e| AppError::Library(format!("迁移 schema v6 失败（metadata_pending）: {e}")))?;
    }
    if !has_column("iso_placeholder") {
        conn.execute_batch("ALTER TABLE tracks ADD COLUMN iso_placeholder INTEGER NOT NULL DEFAULT 0")
            .map_err(|e| AppError::Library(format!("迁移 schema v6 失败（iso_placeholder）: {e}")))?;
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_tracks_metadata_pending ON tracks(metadata_pending)",
    )
    .map_err(|e| AppError::Library(format!("迁移 schema v6 失败（索引 metadata_pending）: {e}")))?;
    // 同一首歌跨目录去重（见 store::not_duplicate_song）的表达式索引：
    // 相关性子查询按这三个表达式等值匹配，没有它大库会退化成逐行全表扫描。
    // 索引定义必须与查询里的表达式逐字一致，SQLite 才会真正用它。
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_tracks_song_key
         ON tracks(lower(trim(title)), lower(trim(album)), lower(trim(format)))",
    )
    .map_err(|e| AppError::Library(format!("创建 song_key 索引失败: {e}")))?;
    Ok(())
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
    file_path    TEXT NOT NULL,
    file_size    INTEGER NOT NULL,
    format       TEXT NOT NULL,
    bitrate      INTEGER,
    sample_rate  INTEGER,
    has_cover    INTEGER NOT NULL DEFAULT 0,
    added_at     INTEGER NOT NULL,
    modified_at  INTEGER NOT NULL,
    -- 艺术家数组 JSON，仅用于快速还原 Track 模型；聚合查询走 track_artists
    artists_json TEXT NOT NULL DEFAULT '[]',
    -- CUE 分轨：标识该曲是否属于整盘镜像中的某一轨
    is_cue       INTEGER NOT NULL DEFAULT 0,
    -- CUE 分轨在镜像文件中的起始偏移（秒）；NULL 表示非分轨
    cue_offset   REAL,
    -- SACD ISO 分轨字段（与 CUE 同构的虚拟分轨）
    is_sacd            INTEGER NOT NULL DEFAULT 0,
    sacd_track_index   INTEGER NOT NULL DEFAULT 0,
    sacd_area          TEXT,
    sacd_dsd_rate      TEXT,
    -- v6 两阶段懒加载：1 = 轻量占位（仅文件名/大小/路径），元数据待后台解析
    metadata_pending   INTEGER NOT NULL DEFAULT 0,
    -- v6 SACD ISO 占位（真实 ISO 文件尚未解析成虚拟分轨，先占位让用户可播/可触发解析）
    iso_placeholder    INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album_id);
-- CUE 去重查询（NOT EXISTS 子查询按 file_path / is_cue 匹配）走索引
CREATE INDEX IF NOT EXISTS idx_tracks_file_path ON tracks(file_path);
CREATE INDEX IF NOT EXISTS idx_tracks_is_cue ON tracks(is_cue);
-- 注：idx_tracks_metadata_pending 由 migrate() 在 ALTER 补列之后创建，
-- 避免旧库缺列时此处建索引导致 init_schema 失败（见 v6 迁移块）

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

#[cfg(test)]
mod crash_recovery_tests {
    use super::*;

    /// 好状态快照 + 运行标记仍在（上次未干净退出）→ 下次打开前回退到快照
    #[test]
    fn restores_backup_after_unclean_exit() {
        let dir = std::env::temp_dir().join(format!("tplayer-db-rec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("library.db");

        {
            let conn = open(&db).unwrap();
            conn.execute("INSERT INTO meta(key, value) VALUES ('state', 'good')", [])
                .unwrap();
            write_backup(&conn, &db);
        }
        // 模拟崩溃：运行标记仍在，且库被写坏（追加垃圾字节）
        let mut bytes = std::fs::read(&db).unwrap();
        bytes.extend_from_slice(b"corrupted-tail");
        std::fs::write(&db, &bytes).unwrap();
        assert!(run_marker_path(&db).exists(), "应保留运行标记表示未干净退出");

        let conn = open(&db).unwrap();
        assert_eq!(
            get_meta(&conn, "state").as_deref(),
            Some("good"),
            "应从快照还原曲库"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 干净退出后标记被清除 → 即使有快照也不回退（保留最新数据）
    #[test]
    fn no_restore_after_clean_shutdown() {
        let dir = std::env::temp_dir().join(format!("tplayer-db-clean-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("library.db");

        {
            let conn = open(&db).unwrap();
            conn.execute("INSERT INTO meta(key, value) VALUES ('state', 'good')", [])
                .unwrap();
            write_backup(&conn, &db);
            mark_clean(&db);
        }
        {
            let conn = open(&db).unwrap();
            conn.execute("INSERT INTO meta(key, value) VALUES ('added', 'new')", [])
                .unwrap();
            mark_clean(&db);
        }
        let conn = open(&db).unwrap();
        assert_eq!(get_meta(&conn, "state").as_deref(), Some("good"));
        assert_eq!(
            get_meta(&conn, "added").as_deref(),
            Some("new"),
            "干净退出后不应回退，快照之后新增的数据应保留"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 库损坏（无运行标记，例如断电损坏）也能回退
    #[test]
    fn restores_backup_when_db_corrupt() {
        let dir = std::env::temp_dir().join(format!("tplayer-db-corrupt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("library.db");

        {
            let conn = open(&db).unwrap();
            conn.execute("INSERT INTO meta(key, value) VALUES ('state', 'good')", [])
                .unwrap();
            write_backup(&conn, &db);
            mark_clean(&db); // 上次是干净退出的
        }
        std::fs::write(&db, b"not a sqlite database at all").unwrap();

        let conn = open(&db).unwrap();
        assert_eq!(
            get_meta(&conn, "state").as_deref(),
            Some("good"),
            "库损坏时应回退到快照"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
