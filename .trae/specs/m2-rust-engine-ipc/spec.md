# M2：Rust 播放引擎与 IPC 打通

## 1. 问题

当前 Rust 后端（`src-tauri/src`）是 M0 骨架：
- `main.rs` 调用 `tplayer_next_lib::run()`，但 `lib.rs` 不存在；
- `commands/player.rs`、`commands/library.rs`、`commands/lyrics.rs`、`commands/update.rs` 引用了不存在的模块（`crate::player::*`、`crate::library::models`、`crate::lyrics::parser`、`crate::update::checker`、`crate::AppState`），Rust 侧完全无法编译；
- 前端 `services/player.ts` 的 `tauriPlayer` 全部返回 `NotImplementedError`，`services/library.ts` 已按命令名调用但 Rust 侧缺少若干命令（`library_cue_albums`、`library_favorite_ids`、`library_set_favorite`、`library_recent`、`library_record_played`、`library_remove_tracks`、`library_add_tracks_to_playlist`、`library_remove_track_from_playlist`）与 `player_dispose`；
- 播放引擎、曲库扫描、元数据解析、持久化均无实现。

浏览器预览（`services/web/*`）已完整驱动 M1 全部 UI，但桌面端没有后端。M2 目标是让 Tauri 桌面端真正跑通：播放真实音频文件、扫描本地曲库目录、持久化用户数据，前端在 Tauri 环境下走真实 IPC。

## 2. 用户与目标

**用户**：在桌面端使用 TPlayer Next 的音乐爱好者。

**目标**：
1. 桌面端可添加本地音乐目录并扫描入库（读取标题/艺术家/专辑/时长/格式/封面等元数据）；
2. 桌面端可真实播放本地音频文件（mp3/flac/wav/ogg/aac/m4a），支持暂停/继续/上一首/下一首/跳转/音量/静音/四种播放模式；
3. 播放状态经 `playback://event` 实时推送到前端，前端 UI 与 M1 浏览器预览行为一致；
4. 收藏、最近播放、歌单、设置持久化到应用数据目录（不再依赖 localStorage）；
5. 前端 services 在 Tauri 环境下走真实命令，浏览器环境仍走 web mock，两套实现行为同构。

## 3. 非目标（明确不做）

- 不实现歌词内嵌/.lrc/在线匹配的真实解析（M3）；`lyrics_load` 在 M2 可返回 `Ok(None)`，前端已有空态处理。
- 不实现在线模块真实通道（M3）。
- 不实现转换器真实后端（M3）；前端 mock 保留。
- 不实现 SMTC / MediaSession（M3）。
- 不实现拖拽导入（M3）。
- 不重写前端 UI 组件（M1 已完成，M2 只改 services 层的 Tauri 实现）。
- 不引入 FFmpeg/ffplay 外部进程（旧版问题），统一用纯 Rust 音频库。

## 4. 功能需求

### 4.1 播放引擎（Rust `player/` 模块）

- **PlayerEngine trait**：抽象播放能力（play/pause/resume/stop/seek/set_volume/set_muted），具体实现为 `RodioEngine`。
- **RodioEngine**：基于 rodio（含 symphonia 解码）播放本地文件；支持 mp3/flac/wav/ogg/aac/m4a；可 seek；可软件音量。
- **状态机**：`idle → loading → playing → paused → stopped → error`；维护当前 `PlaybackSnapshot`、`PlayContext`、当前曲目在队列中的下标。
- **自动连播**：播放结束时根据 `PlayMode`（order / repeat-all / repeat-one / shuffle）决定下一首并自动播放。
- **进度推送**：内部 250ms 时钟，播放期间每 tick 经 `app.emit("playback://event", PlaybackEvent::progress)` 推送位置。
- **模式切换**：`set_mode` 后立即生效，随机模式在切歌时重新决定下标。

### 4.2 曲库（Rust `library/` 模块）

- **模型**：`Track`、`Album`、`Artist`、`Playlist`、`LibraryStats`、`RecentEntry`、`ScanProgress`，字段与前端 `types/track.ts` 一一对应（snake_case ↔ camelCase 由 serde rename 处理）。
- **扫描**：遍历白名单目录，递归发现音频文件（按扩展名），用 lofty 读取元数据（标题/艺术家/专辑/音轨号/年份/时长/格式/比特率/采样率/封面），生成稳定 ID（基于文件路径的哈希），去重入库。扫描进度经 `library://scan-progress` 事件推送。
- **聚合视图**：`albums()` 按专辑名+艺术家聚合；`artists()` 按艺术家聚合；`cue_albums()` 返回 CUE 分轨专辑（M2 可返回空 Vec，前端有处理；若实现可用 `.cue` 文件扫描）。
- **用户数据**：`favorite_ids`、`recent`、`playlists`，均持久化。
- **持久化**：`library.json`（tracks + playlists + favorites + recent + stats）存于 app 数据目录；启动时加载，写操作后保存。

### 4.3 命令层（Rust `commands/`）

实现并注册以下命令，参数命名 snake_case（前端 camelCase 由 Tauri 自动映射）：

**播放**（已有骨架，需实现）：
`player_init`、`player_capabilities`、`player_snapshot`、`player_play`、`player_pause`、`player_resume`、`player_stop`、`player_next`、`player_previous`、`player_seek`、`player_set_volume`、`player_set_muted`、`player_set_mode`；新增 `player_dispose`。

**曲库**（已有 11 个 + 需补 8 个）：
已有：`library_scan`、`library_cancel_scan`、`library_tracks`、`library_albums`、`library_artists`、`library_stats`、`library_add_dir`、`library_remove_dir`、`library_list_playlists`、`library_create_playlist`、`library_rename_playlist`、`library_delete_playlist`、`library_set_playlist_tracks`。
新增：`library_cue_albums`、`library_favorite_ids`、`library_set_favorite`、`library_recent`、`library_record_played`、`library_remove_tracks`、`library_add_tracks_to_playlist`、`library_remove_track_from_playlist`。

**歌词**（M2 返回 None 即可）：`lyrics_load`、`lyrics_save_offset`。
**更新**（M2 可返回"无更新"占位）：`update_check`。

### 4.4 前端 services 对齐

- `services/player.ts`：`tauriPlayer` 各方法改为 `callCommand`；`subscribe` 改为 `listen("playback://event")` 桥接；`dispose` 调 `player_dispose`。
- `services/library.ts`：命令名已对齐，确认 Rust 侧补齐缺失命令即可。
- `services/lyrics.ts`：Tauri 实现已用 `callCommand`，无需改。
- `services/settings.ts`（新增或改造）：在 Tauri 环境下通过 `settings_load`/`settings_save` 命令读写 `config.json`，浏览器环境保留 localStorage。

### 4.5 安全与权限

- 所有来自前端的路径参数经 `security::ensure_allowed` 校验（白名单 = `library_dirs`）。
- `library_add_dir` 接收的路径直接来自 `tauri-plugin-dialog` 的目录选择器结果，可信；入库后成为白名单根。
- asset protocol scope 维持 `$AUDIO/**` 与 `$HOME/Music/**`，不扩大。
- capabilities 维持最小权限集，不新增 `**/*`。

## 5. 非功能需求

- **构建**：`cargo check` 与 `cargo build` 零错误；前端 `npm run build` 零错误。
- **性能**：扫描 1000 首曲目在 10 秒内完成（元数据解析为 I/O 密集，可接受）。
- **稳定性**：播放引擎异常（文件损坏/格式不支持）落入 `error` 状态并推送 `error` 事件，不崩溃。
- **兼容性**：前端在浏览器与 Tauri 双环境下行为同构（同一 IPlayer / LibraryService 契约）。

## 6. 约束、依赖与假设

- **约束**：遵循 PROJECT-STATUS §7 的 7 条硬约束（不碰旧版、不做减法、token 语义化、改原文件、useTheme 唯一、IPC 经 services 层、build 零错误）。
- **依赖**：Rust 侧新增 `rodio`、`lofty`、`sha2`（ID 生成）、`serde`（已有）、`tauri`（已有）。不引入 FFmpeg。
- **假设**：
  - 用户音乐目录位于 `$AUDIO` 或 `$HOME/Music` 下（capabilities 限制）；若用户通过 dialog 选择其他目录，需在 capabilities 中放行——M2 暂假设用户选择的目录在上述范围内，或后续通过 `$RESOURCE`/动态 scope 处理。
  - Windows 平台为首要目标，音频输出走 rodio 的 cpal 后端。
- **开放问题**：
  - CUE 分轨专辑是否在 M2 实现？→ M2 返回空 Vec，前端已处理空态；真实 CUE 解析放 M3。
  - 设置持久化是否必须在 M2 落地？→ 是（M2 清单第 4 项），但优先级低于播放与曲库。

## 7. 验收标准（Acceptance Criteria）

### AC-1（rule）：Rust 后端编译通过
- `cargo check --manifest-path src-tauri/Cargo.toml` 退出码 0，无 error。

### AC-2（rule）：前端构建通过
- `npm run build` 退出码 0，vue-tsc 与 vite build 均零错误。

### AC-3（rule）：命令契约完整
- `src-tauri/src/lib.rs` 的 `invoke_handler` 注册了本文 §4.3 列出的全部命令（播放 14 + 曲库 21 + 歌词 2 + 更新 1 + 设置 2 = 40 条）。

### AC-4（rule）：前端 Tauri 实现走真实 IPC
- `services/player.ts` 的 `tauriPlayer` 无 `pending()` 调用，全部方法经 `callCommand` 或 `listen`。
- `services/library.ts` 的 `tauriLibrary` 命令名与 Rust 注册名一一对应。

### AC-5（rule）：播放引擎可播放真实文件
- `RodioEngine` 能加载并播放 mp3/flac/wav 文件；`player_play` 后 `player_snapshot` 返回 `status: playing` 且 `trackId` 正确。

### AC-6（rule）：曲库扫描与元数据
- `library_scan` 能遍历白名单目录，用 lofty 解析元数据，`library_tracks` 返回非空 Vec，Track 字段（title/artists/album/duration/format）非空。

### AC-7（rule）：持久化
- 收藏/最近播放/歌单写入 `library.json`，重启后 `library_favorite_ids` / `library_recent` / `library_list_playlists` 返回与写入一致的数据。

### AC-8（rule）：事件推送
- 播放期间 `playback://event` 以 progress 事件推送 `positionMs`；切歌推送 track 事件；`library_scan` 期间 `library://scan-progress` 推送进度。

### AC-9（rule）：安全边界
- 非白名单路径调用 `library_*` 命令时返回 `PATH_NOT_ALLOWED` 错误，不读取文件。

### AC-10（rubric）：双环境同构
- 维度：前端在 Tauri 与浏览器环境下的 IPlayer/LibraryService 行为一致性。
- 锚点：0=两套实现方法签名或返回类型不一致；1=签名一致但部分行为有差异；2=方法签名、返回结构、事件语义完全一致。
- 通过阈值：≥1。
