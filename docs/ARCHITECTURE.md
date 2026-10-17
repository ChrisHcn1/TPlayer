# TPlayer 架构文档

> 本文档描述 TPlayer 的整体架构、分层职责、IPC 契约、状态管理、安全边界与平台约束。
> 进度与里程碑台账见 [`PROJECT-STATUS.md`](./PROJECT-STATUS.md)，设计细节（信息架构/布局/token/组件/交互）见 [`DESIGN.md`](./DESIGN.md)，功能清点见 [`FEATURES-INVENTORY.md`](./FEATURES-INVENTORY.md)。
> 最后更新：2026-10-14

## 1. 产品定位与技术栈

**定位**：Windows 优先的本地音乐播放器，隐私优先——在线能力默认关闭、且关闭时绝不外发任何本地曲库信息。

**技术栈**：

| 层 | 选型 |
|---|------|
| 桌面外壳 | Tauri 2（Rust + WebView2） |
| 前端框架 | Vue 3（`<script setup>` + Composition API）+ Vite + TypeScript |
| 状态管理 | Pinia（store 与 Rust `AppState` 领域对象一一对应） |
| 播放后端 | Rust：`rodio` + `symphonia` 解码，`ffmpeg` 进程转码/探测（项目自带 `src-tauri/bin/ffmpeg.exe`） |
| 元数据 | Rust：`lofty` 解析；`ffprobe`（`ffmpeg` 自带）时长兜底 |
| 曲库持久化 | Rust：`rusqlite`（WAL，首次启动从旧 `library.json` 迁移） |
| 升级 | `tauri-plugin-updater`（GitHub Releases，`latest.json`，minisign 签名） |

**双运行模式**：同一前端代码库既可在 Tauri 桌面运行，也可在浏览器以「Web 预览」降级运行——所有服务层（`services/`）均提供 `tauri*` 真实现与 `web*` 降级实现（localStorage / 内存 / mock），通过 `services/index.ts` 按运行时选择。

## 2. 目录结构总览

```
TPlayer Next/
├─ src/                         # 前端（Vue 3 + Vite）
│  ├─ assets/styles/            # theme.css（语义 token 单一事实源）、main.css
│  ├─ components/               # 布局 / 播放 / 通用 / 面板 / 视图组件
│  ├─ composables/             # useTheme / useTrackCover / useCoverTheme / useTrackActions …
│  ├─ router/                   # 16 条路由，meta 四维（nav/titleKey/keepAlive/parent）
│  ├─ services/                 # Tauri/Web 双实现（见 §4）
│  │  └─ web/                   # 浏览器降级实现
│  ├─ stores/                   # Pinia store（与 Rust 领域对象对应）
│  ├─ types/                    # 前后端类型桥（camelCase 命名对齐 Rust serde）
│  └─ views/                    # 一级/二级视图
├─ src-tauri/                   # Rust 后端
│  ├─ src/
│  │  ├─ lib.rs                 # AppState + run() + 命令注册
│  │  ├─ config.rs              # config.json 持久化
│  │  ├─ error.rs               # AppError（含 Converter 变体）
│  │  ├─ security.rs            # ensure_allowed 路径白名单校验
│  │  ├─ converter.rs           # ffmpeg 批量转码（线程/进度/取消）
│  │  ├─ commands/              # player(14)/library(21)/lyrics(2)/update(1)/settings(2)/converter(3)
│  │  ├─ player/                # engine / handle / state / persist
│  │  ├─ library/               # models / store / scan / db / cover / security
│  │  └─ lyrics/                # parser / online
│  ├─ bin/ffmpeg.exe            # 自带 ffmpeg/ffprobe
│  └─ tauri.conf.json
└─ docs/                        # PROJECT-STATUS / DESIGN / FEATURES-INVENTORY / 本文档 / ROADMAP
```

## 3. 分层架构

```
┌─────────────────────────────── 前端 (Vue) ───────────────────────────────┐
│  views → components → composables → stores ─┐                            │
│                                            ↓                            │
│  services/（Tauri 真实现 / Web 降级）──callCommand + listen──┐           │
└───────────────────────────────────────────────────────────│───────────┘
                                                            ↓ Tauri IPC
┌─────────────────────────────── 后端 (Rust) ────────────────────────────┐
│  commands/（#[tauri::command]，AppState 注入）                          │
│     ↓ 调用                                                              │
│  player/(状态机+引擎) · library/(扫描+DB+封面) · lyrics/ · converter/   │
│     ↓ 经 mpsc 通道 / 事件                                               │
│  EngineHandle（专用线程，绕开 Windows COM STA 非 Send 限制）            │
│     ↓ emit 事件                                                         │
│  app.emit("playback://event" 等) ───────────────────→ 前端 listen      │
└────────────────────────────────────────────────────────────────────────┘
```

**关键约束**：播放引擎（rodio `OutputStream`）在 Windows 上**非 `Send`**（COM STA 线程模型），因此引擎运行在专用线程，`EngineHandle` 通过 `mpsc` 通道异步收发命令，绝不在主线程直接持有 `OutputStream`。

## 4. 服务层双实现模式

每个外部能力在 `services/` 下都有 `Tauri 实现` + `Web 降级实现` 两份，由 `services/index.ts` 在运行时择一导出：

- **Tauri 实现**：经 `callCommand(name, args)` 调 Rust 命令，经 `listen(event)` 桥接 Rust `app.emit` 事件。
- **Web 降级**：localStorage/内存/mock 定时器，保证浏览器预览不崩、类型契约一致。

例：`services/library.ts` 暴露 `LibraryService` 契约，`tauriLibrary` 调 `library_scan` 等命令，`webLibrary` 走 `localStorage`；`services/converter.ts` 的 `webConverter` 用定时器模拟进度事件。

## 5. IPC 契约（命令 + 事件）

命令分组（命名 `snake_case`，参数 `camelCase` 经 serde rename）：

| 域 | 数量 | 关键命令 |
|---|---|---|
| player | 14 | `player_play/pause/resume/next/previous/seek/set_volume/set_mode/play_from_queue/remove_from_queue…` |
| library | 21 | `library_scan/library_list_tracks/library_cover/library_album_cover/library_artist_cover/library_import_files/library_add_dir/library_remove_dir/playlist CRUD/favorite/recent…` |
| lyrics | 2 | `lyrics_match_online`、`lyrics_clear_cache` |
| update | 1 | `update_check` |
| settings | 2 | `settings_load`、`settings_save` |
| converter | 3 | `converter_start`、`converter_cancel`、`converter_probe_files` |

**事件**（Rust `app.emit` → 前端 `listen`）：

| 事件 | 载荷 | 消费方 |
|---|---|---|
| `playback://event` | `PlaybackEvent`（Progress/Status/Track/Queue/Error，serde `rename_all="lowercase"`） | `stores/player` |
| `library://scan-progress` | `ScanProgress` | `stores/library`（扫描进度） |
| `library://changed` | 空 | 曲库变更通知 |
| `converter://event` | `ConverterEvent`（started/progress/completed/failed/finished） | `stores/converter` |
| `tauri://drag-drop` | 拖放路径（JS 侧经 `onDragDropEvent`） | `App.vue` 拖拽导入 |

> 注意：`PlaybackEvent` 的 serde `rename_all="lowercase"` 是关键——早期因大小写不匹配导致前端丢弃 Progress/Status/Track 事件、进度条不走，已修复。

## 6. 状态管理

**Rust `AppState`**（单例，经 `State` 注入命令）：聚合 `library`、`player`、配置、封面缓存、统计等。播放状态机在 `player/handle.rs`：`idle→loading→playing→paused→stopped→error`，四种播放模式自动连播；位置由状态机用 `Instant` 计时（引擎线程不回传位置），250ms 时钟经 `playback://event` 推 progress。

**Pinia stores**（与 `AppState` 领域对象对应）：

| store | 职责 |
|---|---|
| `player` | 播放状态/队列/进度/模式，订阅 `playback://event` |
| `library` | 曲库聚合（tracks/albums/artists/cueAlbums/favorites/recent）、扫描进度、拖拽导入 `importDropped` |
| `lyrics` | 歌词解析/偏移/在线匹配 |
| `playlist` | 歌单 CRUD |
| `settings` | 设置加载/保存，持有 `online` 配置 |
| `converter` | 转码任务（事件驱动、localStorage 持久化、切页不丢） |
| `toast` / `dialog` / `contextMenu` | 全局反馈宿主 |

## 7. 曲库与持久化

- **扫描**（`library/scan.rs`）：递归发现音频、`lofty` 解析元数据、`SHA-256(路径)` 生成稳定 ID、`mtime` 增量 diff 单事务 upsert。
- **持久化**（`library/db.rs`，rusqlite WAL）：`tracks / track_artists / favorites / recent / playlists / playlist_tracks`；首次启动自动导入旧 `library.json`（改名 `.bak`）。
- **封面**（`library/cover.rs`，四级链）：内嵌标签 → 同名边车图片 → 目录图（`folder/cover/front` 等） → 在线（网易云，受 `online.enabled` 控制） → 内置默认 `default-cover.svg`。`library_cover` async，魔数嗅探 MIME + 手写 base64 data URL，内存正负缓存 + `%APPDATA%/cover-cache/<sha256(trackId)>` 磁盘缓存。专辑/艺术家封面聚合（`library_album_cover`/`library_artist_cover`）组内候选按 `hasCover+音轨号` 排序逐首回退，分组键独立缓存。
- **拖拽导入**（`library_import_files`）：Tauri 拖放事件拿真实路径，过滤音频扩展名（文件或目录递归），复用 `scan::parse_track` 直接入库（绕过白名单目录扫描，与 dialog 选择器同源可信）。

## 8. 在线模块设计约束（重要）

**设计意图（M0 定调）**：在线能力是「整个前端唯一的联网出口」，便于审计与一键关闭；默认关闭、默认不外发本地曲库信息。

**当前实现状态（2026-10-14）**：

- ✅ **在线歌词 / 封面已真实化**：走 Rust 后端（`lyrics/online.rs` 网易云搜索+时长匹配+翻译合并；`cover.rs` 在线匹配），受 `settings.online.enabled` 控制；前端经各自服务（`services/lyrics`、`services/cover` / `useTrackCover`）调用，关闭时拒绝联网。
- ⚠️ **`services/online.ts` 是 M0 设计的「统一在线出口」抽象（`onlineService`）**：其 `search` / `fetchLyrics` / `fetchCover` 目前仍是 `NotImplementedError` 骨架，**未被任何 UI 或 store 消费**。歌词/封面落地时直接经各自服务 + Rust 命令，未收敛到该统一出口。

**隐私硬约束**（任何在线代码不得放宽）：

1. 默认关闭：`enabled=false` 时任何方法必须先拒绝，不得「先发请求再判断开关」。
2. 默认不外发：`allowLibraryUpload=false` 时，请求体禁止包含本地路径/文件名/曲库统计/音频指纹。
3. 在线能力与本地曲库逻辑彻底解耦。

> 后续可选优化：将歌词/封面的在线调用收敛到 `onlineService` 单一受控出口，兑现「唯一联网出口」的审计目标（见 `ROADMAP.md`）。

## 9. 安全边界

- **路径白名单**：`security::ensure_allowed` 对所有接收路径参数的命令做校验；凡接收路径的命令均经此校验，仅 dialog 选择器返回的可信路径（如 `library_add_dir`）豁免。
- **capabilities**：`capabilities/default.json` 维持最小权限（core/event/window/dialog/fs/updater/process 按需）。
- **asset 协议**：scope 放开 `$APPDATA/cover-cache`，仅用于封面文件 URL，仍受前述校验约束。
- **拖拽导入**属用户主动操作，路径来自 Tauri 拖放事件，与 dialog 选择器同源可信，不强制白名单校验。

## 10. 平台约束与降级

- **Windows 优先**：自带 `ffmpeg.exe`（转码/探测），播放引擎受 COM STA 限制运行于专用线程。
- **Web 预览降级**：服务层 `web*` 实现保证浏览器可运行预览（无真实文件解析/转码/系统托盘），封面与曲库走内存或 localStorage。
- **系统媒体控制（SMTC）**：Windows-only（见 `smtc.rs`，`#[cfg(windows)]` 隔离，非 Windows 编译透明）。创建内存 `MediaPlayer` 作 SMTC 代理，经其 `SystemMediaTransportControls` 上报元数据/播放状态；所有 WinRT 调用集中在专用 STA 线程（消息泵 + channel），避免跨 apartment 错误；按钮事件经 `app.emit("smtc://button")` 转发前端 `stores/player` 处理（复用播放控制逻辑）。驱动封面（缩略图）为后续增强。
- **应用图标**：`public/logo-1024.svg` → `npx @tauri-apps/cli icon` 全量生成至 `src-tauri/icons`。

## 11. 构建与发布

- 开发：`npm run dev`（前端）+ `npm run tauri:dev`（带 Rust 热重载）。
- 校验：`cargo check`（Rust）、`npm run build`（vue-tsc + vite build，零错误前置条件）。
- 打包：`scripts/build-win.ps1`（必须带 BOM 的 UTF-8）+ `npm run build:win`；`.github/workflows/release.yml` 推 `v*` tag 触发 `tauri-action` 自动签名 + `latest.json` + Release 草稿。
- 升级通道：`tauri.conf.json` 内 `updater` endpoint 的 `<OWNER>/<REPO>` 待发布前替换为实际仓库。
