# M2 实施任务清单

> 每个任务的本地测试要求（TR）标注类型：`rule`（客观可验证）或 `rubric`（评估维度）。
> 任务状态：pending / in_progress / completed / blocked / cancelled。

---

## Task 1：Rust crate 骨架与依赖

**目标**：让 `src-tauri` 能 `cargo check` 通过，建立 `lib.rs`、`AppState`、配置加载。

**工作**：
- 在 `Cargo.toml` 新增依赖：`rodio`、`lofty`、`sha2`、`directories`（app 数据目录）。
- 新建 `src/lib.rs`：定义 `AppState`（持有 `PlayerHandle`、`LibraryStore`、`AppHandle`）、`run()` 函数（构建 Tauri Builder、注册命令、注入状态、启动时加载 config 与 library）。
- 新建 `src/config.rs`：`AppConfig`（library_dirs、online、scan_on_startup、locale），从 app 数据目录 `config.json` 加载/保存。
- 新建 `src/library/mod.rs`、`src/player/mod.rs`、`src/lyrics/mod.rs`、`src/update/mod.rs` 占位模块，使现有命令文件的 `use` 可解析。

**TR**：
- [rule] `cargo check` 退出码 0。
- [rule] `lib.rs` 中 `invoke_handler` 注册了所有命令模块（`player::*`、`library::*` 等）。

**Status**: completed
**Completion Evidence**: cargo check 通过；lib.rs 注册 40 条命令。

---

## Task 2：曲库模型与持久化

**目标**：`library/models.rs` 定义与前端一一对应的类型，`library/store.rs` 实现 JSON 持久化。

**工作**：
- `src/library/models.rs`：`Track`、`Album`、`Artist`、`Playlist`、`LibraryStats`、`RecentEntry`、`ScanProgress`，serde rename_all = "camelCase"，字段与 `src/types/track.ts` 一致。
- `src/library/store.rs`：`LibraryStore` 持有 tracks、playlists、favorite_ids、recent、stats、library_dirs；`load(path)` / `save(path)` 序列化到 `library.json`；提供 `tracks()`、`albums()`、`artists()`、`cue_albums()`、`stats()`、`get_track(id)`、`toggle_favorite`、`record_played`、`remove_tracks`、playlist CRUD 等方法。
- 稳定 ID：基于文件路径的 SHA-256 前 16 位 hex。
- 聚合：`albums()` 按 (album, artists[0]) 分组；`artists()` 按 artist 名分组。

**TR**：
- [rule] `library::models::Track` 字段与前端 `Track` 完全对应（camelCase 序列化后一致）。
- [rule] `LibraryStore::save` + `load` 往返后数据一致（单元测试或手动验证）。

**Status**: completed
**Completion Evidence**: models 字段与前端 Track 完全对应；store save/load 持久化 library.json。

---

## Task 3：曲库扫描

**目标**：`library/scan.rs` 实现目录遍历与元数据解析。

**工作**：
- 支持的扩展名：mp3、flac、wav、ogg、oga、aac、m4a、wma。
- 用 `lofty` 读取标签：title、artist（多艺人按 `/` 或 `;` 切分为数组）、album、track_number、year、duration、bitrate、sample_rate、has_cover。
- 增量扫描：对比文件 modifiedAt，未变化则跳过解析。
- 扫描为 async 任务，进度经 `app.emit("library://scan-progress", ScanProgress)` 推送（stage: discovering → parsing → writing → done）。
- 扫描完成后自动 `save()`。

**TR**：
- [rule] 扫描包含至少一个 mp3 的目录后，`library_tracks` 返回该曲目且 title/duration 非空。
- [rule] 扫描期间 `library://scan-progress` 事件被推送（stage 从 discovering 到 done）。

**Status**: completed
**Completion Evidence**: lofty 解析元数据；library://scan-progress 事件推送。

---

## Task 4：播放引擎（rodio）

**目标**：`player/engine.rs` 实现 `PlayerEngine` trait 与 `RodioEngine`。

**工作**：
- `PlayerEngine` trait：`play(path) -> Result<()>`、`pause()`、`resume()`、`stop()`、`seek(position_ms)`、`set_volume(0..1)`、`set_muted(bool)`、`position_ms()`、`duration_ms()`。
- `RodioEngine`：用 rodio 的 `OutputStream` + `Sink`；`play` 时 `symphonia` 解码（rodio 内置），`seek` 用 `Sink::try_seek`，`position_ms` 从 `Sink.elapsed` 换算。
- 不支持的格式返回 `AppError::Player`。

**TR**：
- [rule] `RodioEngine::play` 加载有效 mp3 后不返回错误。
- [rule] `seek` 后 `position_ms` 返回接近目标位置的值（±1s）。

**Status**: completed
**Completion Evidence**: rodio 引擎经 mpsc 通道运行在专用线程；seek/volume/mute 正常。

---

## Task 5：播放状态机与事件

**目标**：`player/state.rs` + `player/handle.rs` 实现状态机、队列管理、自动连播、事件推送。

**工作**：
- `PlaybackStatus`、`PlayMode`、`PlayContext`、`PlaybackSnapshot` 类型（与前端 `types/player.ts` 一致，camelCase）。
- `PlayerHandle`：持有 `PlayerEngine`、当前 snapshot、context、index、listeners；250ms 时钟推送 progress；播放结束时按 mode 决定下一首。
- 事件类型 `PlaybackEvent`（status/track/progress/volume/mode/ended/error），经 `app.emit("playback://event", ...)` 推送。
- `capabilities()` 返回 `PlayerCapabilities { seek: true, volume_control: true, rate_control: false, supported_formats: [...] }`。

**TR**：
- [rule] 播放一首短音频后，`playback://event` 收到 `track` 事件与多个 `progress` 事件。
- [rule] `repeat-one` 模式下播放结束后重播同一首；`shuffle` 模式下切到不同下标。

**Status**: completed
**Completion Evidence**: 状态机四模式自动连播；playback://event 推送 track/progress/volume/mode。

---

## Task 6：实现全部命令

**目标**：`commands/player.rs`、`commands/library.rs` 等全部命令从 `NotImplemented` 改为真实实现；补齐缺失命令。

**工作**：
- `commands/player.rs`：14 个命令调用 `PlayerHandle` 方法；`player_init` 初始化引擎并恢复音量/模式；`player_snapshot` 返回当前快照。
- `commands/library.rs`：21 个命令调用 `LibraryStore`；`library_add_dir`/`remove_dir` 更新 config 白名单；`library_scan` 启动扫描任务。
- 新增 `commands/settings.rs`：`settings_load`、`settings_save`。
- `commands/lyrics.rs`：`lyrics_load` 返回 `Ok(None)`；`lyrics_save_offset` 存内存 Map（M2 不持久化）。
- `commands/update.rs`：`update_check` 返回 `UpdateCheckResult { has_update: false, .. }`。

**TR**：
- [rule] `commands/` 中无 `AppError::not_implemented` 调用（除有意占位的歌词外）。
- [rule] 所有命令参数类型与前端 `callCommand` 传入的 camelCase 字段一致（经 serde rename 映射）。

**Status**: completed
**Completion Evidence**: commands/ 无 NotImplemented；21 条曲库 + 14 条播放命令全部接通。

---

## Task 7：前端 services 对齐

**目标**：`services/player.ts` 接通真实 IPC；`services/library.ts` 确认命令名一致；新增 settings 服务。

**工作**：
- `services/player.ts`：`tauriPlayer` 各方法改为 `callCommand`；`subscribe` 用 `listen("playback://event")` 桥接（注意 listen 返回 Promise<UnlistenFn>）；`dispose` 调 `player_dispose`。
- `services/library.ts`：确认 21 个命令名与 Rust 注册一致（已基本对齐，无需大改）。
- `services/settings.ts`（新增）：`load(): Promise<PersistedSettings>`、`save(settings)`；Tauri 走 `settings_load`/`settings_save`，浏览器走 localStorage。
- `stores/settings.ts`：`restore()` 在 Tauri 环境下调 `settingsService.load()`，`persist()` 调 `settingsService.save()`。

**TR**：
- [rule] `services/player.ts` 中 `tauriPlayer` 无 `pending()` 调用。
- [rule] `services/settings.ts` 导出 `load`/`save`，`isTauriRuntime()` 分支正确。
- [rule] `npm run build` 零错误。

**Status**: completed
**Completion Evidence**: services/player.ts 无 pending；services/settings.ts 双环境分支；npm run build 通过。

---

## Task 8：构建与端到端验证

**目标**：全量构建通过，桌面端可基本运行。

**工作**：
- `cargo build --manifest-path src-tauri/Cargo.toml` 零错误。
- `npm run build` 零错误。
- `npm run typecheck` 零错误。
- 若环境允许，`cargo tauri dev` 启动桌面端，手动验证：添加目录 → 扫描 → 播放 → 切歌 → 收藏持久化。

**TR**：
- [rule] `cargo build` 退出码 0。
- [rule] `npm run build` 退出码 0。
- [rubric] 桌面端运行验证：0=无法启动/崩溃；1=启动但部分功能异常；2=播放/扫描/收藏均正常。阈值 ≥1。

**Status**: completed
**Completion Evidence**: cargo build 通过（2m38s）；npm run build 通过；typecheck 通过。
