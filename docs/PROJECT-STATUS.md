# TPlayer —— 项目进度清单

> 本文件是项目的唯一进度台账。**每次继续本项目时，先读本文件，不要全量遍历代码。**
> 最后更新：2026-10-14

---

## 0. 一句话现状

M1 交互骨架已跑通：`npm run build` 零报错，`npm run dev` 在 http://localhost:3100 提供浏览器预览；全部视图经 `services/web/*` mock 引擎（localStorage 持久化）完成真实数据交互——播放推进/切歌/模式/音量、歌词逐行高亮与偏移、队列跳播、收藏/最近/歌单/多选/右键、Ctrl+K 命令浮层、设置六页签、转换器 mock 闭环均已实测。下一步是 M2：Rust 播放引擎与 IPC 端到端打通。

---

## 1. 里程碑总览

| 里程碑 | 目标 | 状态 |
|--------|------|------|
| M0 布局骨架 | 骨架跑通、五区外壳、路由与视图齐备 | ✅ 完成 |
| M1 交互骨架 | 图标集、右键菜单、多选条、浮层、真实数据接入 | ✅ 完成（2026-09-30，浏览器实测） |
| M2 数据层 | Rust 播放引擎统一 Player trait、IPC 打通 | ✅ 完成（2026-09-30，cargo build + npm run build 零错误） |
| M3 功能闭环 | 歌词、转换器、在线模块、SMTC、应用图标 | 🔶 进行中（歌词链/断点续播/播放习惯/ffprobe 兜底/播放条歌词行/系统托盘/应用图标已落地） |
| M4 性能与发布 | SQLite 持久化、虚拟滚动/封面性能、NSIS 打包、应用内自动升级 | ✅ 完成（2026-10-06；**升级 endpoint 当前指向 `ChrisHcn1/TPlayer` 仓库；发布前请确认该仓库即本项目发版仓库且公钥匹配，否则自动更新校验失败**，见 §5 M4 第 6 项） |

---

## 2. 运行与验证方式（重要）

```powershell
cd E:\TPlayerNext
npm run dev        # 开发服务器：http://localhost:3100
npm run build      # vue-tsc --noEmit + vite build（当前零报错、零警告）
npm run typecheck  # 仅类型检查
```

- 依赖已安装（node_modules 51 个包），Node v24.15.0 / npm 11.13.0。
- 技术栈：Tauri 2 + Vue 3.5 + TypeScript 5.9 + Vite 6.4 + Pinia 3 + vue-router 4。
- 浏览器直接访问可在「非 Tauri 环境」下预览界面（窗口控制按钮自动隐藏）。
- `dist/` 为构建验证产物，可随时删除，已在 `.gitignore` 覆盖范围内。

---

## 3. M0 已完成清单

### 3.1 文档基线
- [x] `docs/FEATURES-INVENTORY.md` — 旧版功能清点（8 功能域 / 90+ 功能项 / D-01~D-21 重复冲突 / B-01~B-25 逻辑缺陷）
- [x] `docs/DESIGN.md` — 设计规范定稿（1695 行，§1 信息架构 / §2 布局 / §3 token / §4 组件 / §5 交互 / §6 快捷键）
- [x] `docs/PROJECT-STATUS.md` — 本文件
- [x] `docs/ARCHITECTURE.md`、`docs/ROADMAP.md` — 架构与路线图（2026-10-14 补：分层/命令契约/状态管理/安全边界/在线模块设计约束/平台约束/构建发布；里程碑回顾与 M3 状态、SMTC 决策、发布前待办）

### 3.2 设计资产
- [x] `public/logo.svg`、`public/favicon.svg`、`public/logo-1024.svg`

### 3.3 样式层
- [x] `src/assets/styles/theme.css` — 全量语义 token（暗色 `:root`、`[data-theme=light]` 重映射、density/layout/reduced-motion 覆写、过渡期兼容别名）
- [x] `src/assets/styles/main.css` — reset + 底座 + 滚动条 / 焦点 / 工具类
- [x] 校验：全项目 `var(--x)` 引用与 theme.css 定义差集为空（无未定义变量）

### 3.4 主题单一事实源
- [x] 唯一载体 `<html data-theme>`；唯一持久化键 `tplayer-next.theme`
- [x] 唯一持有方 `src/composables/useTheme.ts`；`src/stores/settings.ts` 已移除 theme 字段与 setTheme

### 3.5 路由与导航
- [x] `src/router/index.ts` — 16 条路由，meta 四维（nav / titleKey / keepAlive / parent）
- [x] `src/i18n/nav.ts` — 导航文案映射，侧栏零硬编码
- [x] `src/components/layout/SideBar.vue` — nav 分组驱动，三组渲染 + 底部固定区 + 折叠态

### 3.6 五区布局外壳（M0 收口）
- [x] `src/components/layout/TitleBar.vue` — 可拖拽区、Logo + 品牌名、全局搜索、主题切换、设置入口、窗口控制（非 Tauri 环境不渲染）
- [x] `src/components/layout/SideBar.vue` — 曲库组 / 工具组 / 底部组（设置、主题、折叠）
- [x] `src/components/player/PlayerBar.vue` — 播放条骨架
- [x] `src/components/layout/RightPanel.vue` — 歌词 / 队列双页签，关闭时列宽为 0（实例常驻，不丢状态）
- [x] `src/App.vue` — 五区 Grid 装配 + `useLayout` 单例管理折叠与面板开合 + KeepAlive
- [x] 快捷键已接线：Ctrl+B（侧栏）、Ctrl+U / Ctrl+Shift+U（右面板）

### 3.7 视图（14 个，全部一级 + 二级骨架）
- 一级：SongsView、RecentView、FavoritesView、ArtistsView、AlbumsView、CueAlbumsView、PlaylistsView、SearchView、ConverterView、SettingsView
- 二级：ArtistDetailView、AlbumDetailView、CueDetailView、PlaylistDetailView

### 3.8 组件 / 状态 / 服务 / 类型骨架
- 组件：SearchBox、EmptyState、TrackList、TrackRow、ProgressBar
- stores：library / player / lyrics / playlist / settings
- services：player / library / lyrics / online / update / ipc
- types：track / player / lyrics / api
- Rust 侧：`src-tauri/` 目录与 commands / services 骨架、`capabilities/default.json` 权限白名单

### 3.9 编译验证
- [x] `npm install` 完成（51 包，含补装的 vue-router）
- [x] `npm run build` 通过：vue-tsc 零报错，vite build 119 modules 成功（视图分包）

---

## 4. M1 已完成清单（2026-09-30 浏览器实测）

| 序 | 任务 | 落地位置 / 说明 |
|---|------|------|
| 1 | 图标集 | `components/common/icons.ts`（内联 SVG 单事实源，未知图标 alert-triangle 兜底）+ `Icon.vue`；全部布局/列表/播放占位已替换 |
| 2 | 通用反馈件 | `ToastContainer`（info/success/warning/error，右上、行动按钮）、`Modal` + `stores/dialog.ts`（confirm/prompt，danger 焦点策略）、`ContextMenuHost`（视口翻转、键盘导航、独立 fixed 子菜单） |
| 3 | 列表体系 | TrackRow（指示条/复选框/播放中均衡器）、TrackListHeader（三态全选+排序）、TrackList（中文排序、Shift/Ctrl 多选、双击 startIndex 播放、右键批量）、SelectionBar 浮条、ListToolbar、MediaCard、DetailHero |
| 4 | 曲目动作 | `useTrackActions`（播放/收藏/加歌单含新建/移除/转专辑[CUE 分流]/转艺术家/曲库移除危险确认）、`useTrackSelection` |
| 5 | 视图真实数据 | 10 个一级视图 + 4 个二级视图全部接入 library / playlist store；搜索走 `route.query.q`；歌单支持新建/重命名/删除/移除曲目 |
| 6 | Ctrl+K 命令浮层 | `CommandPalette.vue` + `useCommandPalette`：曲目/专辑(含 CUE)/艺术家/歌单/快捷操作分组、键盘导航、无命中跳搜索页 |
| 7 | PlayerBar 全交互 | 封面+标题/艺术家跳转（CUE 分流）、收藏、模式四态菜单、上一首/播放暂停/下一首；ProgressBar 拖拽松手才 seek+时间气泡+aria slider（←/→ ±5s，Shift ±30s）；VolumeControl 静音记忆+悬停展开 |
| 8 | RightPanel 真实内容 | `panel/LyricsPanel.vue`：watch 切歌加载、当前行 40% 居中 smooth scroll、点击行 seek、±0.5s 偏移持久化、无词/失败/空闲四态（无词时"在线匹配"禁用占位）；`panel/QueuePanel.vue`：队列渲染、当前高亮、双击同上下文跳播 |
| 9 | 设置六页签 | SettingsView：播放与曲库（白名单目录：Tauri 走 `services/dialog.ts` 的 plugin-dialog 封装，浏览器降级 prompt；立即扫描；启动扫描开关）/ 外观（useTheme 深/浅色）/ 歌词（优先级说明+清缓存）/ 在线（enable + allowLibraryUpload 双闸）/ 更新（mock 检查）/ 关于（package.json 版本 2.0.0-alpha.0 + 恢复默认） |
| 10 | 转换器 mock 闭环 | ConverterView：拖放/点选加入文件（仅取文件名）、格式/采样率/位深（有损格式禁用位深）/输出目录、本地定时模拟任务队列（进度/串行/完成/清除） |
| 11 | App 全局接线 | 挂载四个浮层宿主；onMounted 恢复设置后并行 library/playlist refresh + player init；切歌→recordPlayed；播放 error→toast；DEFAULT_SHORTCUTS 全量注册（空格/Ctrl+方向/方向/m/f/r/Ctrl+K 等，排除 input/textarea/select/contenteditable/输入法组合） |
| 12 | 编译与实测 | `npm run typecheck` / `npm run build` 零报错；浏览器实测播放推进、切歌、模式、收藏、歌词高亮+偏移、队列跳播、Ctrl+K、设置添加目录、转换 warning、全程无 console error |

> dev 期优化：`vite.config.ts` 的 `optimizeDeps.include` 预打包 @tauri-apps/api、plugin-dialog、plugin-fs，
> 避免浏览器预览首次进入设置页时 Vite 发现新依赖触发整页 reload。

---

## 5. M2 已完成清单（2026-09-30，cargo build + npm run build 零错误）

| 序 | 任务 | 落地位置 / 说明 |
|---|------|------|
| 1 | Rust crate 骨架 | `lib.rs`（AppState + run() + 40 条命令注册）、`config.rs`（config.json 持久化）、`player/`、`library/`、`lyrics/`、`update/` 模块 |
| 2 | 播放引擎 | `player/engine.rs`：rodio + symphonia 解码；因 `OutputStream` 在 Windows 非 Send（COM STA），引擎运行在专用线程，`EngineHandle` 经 mpsc 通道发命令；支持 mp3/flac/wav/ogg/aac/m4a、seek、软件音量、静音 |
| 3 | 播放状态机 | `player/handle.rs`：idle→loading→playing→paused→stopped→error 状态机；四种模式自动连播；250ms 时钟经 `playback://event` 推送 progress；位置由状态机用 Instant 计时（引擎线程不回传位置） |
| 4 | 曲库模型与持久化 | `library/models.rs`（Track/Album/Artist/Playlist/LibraryStats/RecentEntry/ScanProgress，camelCase 与前端一一对应）；`library/store.rs`（library.json 持久化 + 聚合查询 + 收藏/最近/歌单 CRUD） |
| 5 | 曲库扫描 | `library/scan.rs`：递归发现音频文件、lofty 解析元数据（标题/艺术家/专辑/音轨号/年份/时长/比特率/采样率/封面）、SHA-256 路径哈希生成稳定 ID、`library://scan-progress` 事件推送 |
| 6 | 40 条命令全部实现 | 播放 14 + 曲库 21（含补齐的 cue_albums/favorite_ids/set_favorite/recent/record_played/remove_tracks/add_tracks_to_playlist/remove_track_from_playlist）+ 歌词 2 + 更新 1 + 设置 2 |
| 7 | 前端 services 对齐 | `services/player.ts`：tauriPlayer 全部走 callCommand，subscribe 桥接 `listen('playback://event')`；`services/settings.ts`：Tauri 走 settings_load/save，浏览器回退 localStorage；`stores/settings.ts` 改用 settings 服务 |
| 8 | 安全边界 | `security::ensure_allowed` 白名单校验已就绪；capabilities 与 assetProtocol scope 维持最小权限；`library_add_dir` 仅接受 dialog 选择器返回路径 |

### M3 功能闭环
- [x] 歌词真实来源链（内嵌 > .lrc 边车 > 在线）与在线匹配（2026-10-01：`lyrics/parser.rs` LRC 全量解析含多时间标签/翻译合并/offset 标签；`lyrics/online.rs` 网易云搜索+时长匹配+翻译合并，成功落盘 .lrc 边车；`commands/lyrics.rs` 三级链 + 偏移落盘 lyrics_offsets.json + lyrics_clear_cache；LyricsPanel 空态接在线匹配按钮）
- [x] 断点续播与播放习惯记忆（2026-10-01：`player/persist.rs` playback_state.json；保存时机=播放/暂停/停止/seek/音量/模式/tick 节流 5s/窗口关闭；player_init 恢复为 paused 态定位+返回上下文还原队列，首次 resume/seek 经 needs_engine_load 装载引擎；音量/静音/播放模式一并恢复；2026-10-06 起恢复后若播放列表非空则自动从断点续播）
- [x] ffprobe 时长兜底（2026-10-01：`engine.rs` probe_duration 供扫描期与播放期调用，修复 dsf/dff/dts/tta 等 lofty 解析失败曲目的进度推进与自动连播）
- [x] 实时歌词展示（2026-10-01：DesktopLyricsBar 固定于播放列表上方，当前句高亮+下一句预览，点击正文开合歌词面板；PlayerBar 字幕按钮与歌词条 ⌄ 双入口，开合状态持久化，与右面板歌词页签相互独立）
- [x] 切歌状态同步修复（2026-10-01：handle.rs play() Loading 分支重置 position_ms/duration_ms；stores/player.ts play() 同步重置进度；ended 事件只重置进度不动 status，消除自动连播闪烁）
- [x] 播放事件实时推送修复（2026-10-01：PlaybackEvent serde 补 rename_all="lowercase"，修复 Progress/Status/Track 事件因大小写不匹配被前端丢弃导致进度条不走）
- [x] 无损解码杂音修复（2026-10-01：PcmStreamSource 由逐 2 字节 read 改为 8192 样本内部缓冲批量读管道，消除 rodio 混音线程欠载爆音，dsd/dts/ape/wav 等 ffmpeg 解码曲目适用）
- [x] 系统托盘（2026-10-01：tauri tray-icon + image-png feature；托盘菜单播放/暂停（文案随状态切换）、上一首、下一首、显示主窗口、退出；左键单击显示窗口；tooltip 随切歌显示当前曲目名；窗口关闭改为保存状态并隐藏到托盘，退出走托盘菜单并以 AtomicBool 标志放行 CloseRequested）
- [x] 当前播放曲目列表定位（2026-10-01：TrackList 切歌/启动恢复/KeepAlive 切回时 scrollIntoView block:nearest，行已在视口内不滚动；修掉 watch 先于 sortedTracks 声明的 TDZ 崩溃）
- [x] 后缀名筛选（2026-10-01：filterTracks/matchTrackKeyword 支持 flac/.flac/dsf 等扩展名精确匹配，覆盖视图筛选框与 Ctrl+K，UI 无提示）
- [x] 封面解析链与封面放大层（2026-10-01：`cover.rs` 四级链 内嵌标签 → 同名边车图片（song.flac→song.jpg/png/webp/gif/bmp）→ 在线（网易云专辑封面，受 online.enabled 控制，标题+艺术家搜索+时长匹配，与歌词同构）→ 前端内置默认封面 default-cover.svg；`library_cover` async，魔数嗅探 MIME + 手写 base64 data URL，内存正负缓存 + %APPDATA%/cover-cache/<sha256(trackId)> 磁盘缓存，任一来源成功均落盘；前端 useTrackCover 缓存+并发去重；PlayerBar 显示封面，点击打开 NowPlayingOverlay 大封面叠加三句实时歌词并带播放控制，Esc/遮罩关闭，Ctrl+L 唤起）
- [x] 切歌交叉淡化与沉浸播放视图（2026-10-01：engine.rs 复用单一 OutputStream，Playback{sink,ffmpeg_child} 支持新旧两条链路并存；命令循环改 recv_timeout 20ms 驱动 fade，切歌旧曲 400ms smoothstep 淡出（起点取当前实际增益）+ 新曲 400ms 淡入，seek 后同步快速淡入防爆音；NowPlayingOverlay 重设计为封面铺满全窗（object-fit cover + 暗角渐变 + 主色淡染），底部三句实时歌词/曲目/控制，切歌背景淡入；useCoverTheme canvas 24px 采样 HSL 12 色相桶量化封面主色（h/s 缓存，纯灰度封面回落中性蓝相），直接接管 :root 的 --color-brand/-solid/-hover/-active/-subtle 五件套（文字色按深/浅主题分别派生亮度，solid l=0.5 基准），整套皮肤——侧栏选中态/主按钮/开关/进度滑块/歌词高亮全部随封面变；另注入 --color-cover-accent/-glow/-soft/-wash，TitleBar/SideBar/PlayerBar 背景叠 7% 径向 wash、PlayerBar 封面 glow；切主题按同色相重新派生无需重采样；开关入 useLayout coverAccent，外观设置页可关，关闭/失败 removeProperty 回落主题色）
- [x] 表头"悬浮缝隙"修复（2026-10-01：根因为滚动容器 padding-top 致 Chromium sticky 表头吸顶停在 padding 内侧、曲目行从上方带状区露出；padding 下移到新增 .app-shell__view-pad 普通流层，sticky top:0 真正贴滚动区顶边；SettingsView/ConverterView 去除自带 padding 统一由 view-pad 提供）
- [x] 侧栏折叠把手移位 + 转换器入口归位（2026-10-01：折叠按钮从侧栏底部移到侧栏/内容区分隔线上垂直居中的 26px 圆钮（z-index 10，hover 品牌色）；/converter 路由保留但移出侧栏 nav，设置页新增"工具"页签内嵌 ConverterView，Ctrl+K 命令面板仍可直达）
- [x] 艺术家/专辑封面展示（2026-10-01：MediaCard 新增封面懒加载，IntersectionObserver 300px 视口预取，img object-fit cover 铺满、无封面回落变体图标；AlbumsView/ArtistsView/CueAlbumsView 网格与三个详情页头部全部出封面）
- [x] 专辑/艺术家封面聚合补全（2026-10-06：封面链扩展为 内嵌 → 同名边车 → 目录图(folder/cover/front 等及含 cover/folder/front 的变体) → 在线；新增 library_album_cover/library_artist_cover 命令，组内候选按 hasCover+音轨号排序逐首回退（上限 30），本地全空时仅做一次网易云在线匹配（受 online.enabled 控制）；分组键 album:/artist: 独立内存+磁盘缓存，成员单曲缓存自动镜像到分组键，下次启动零解析零联网，负缓存仅存内存保证重试；前端 useTrackCover 泛化为 track/album/artist 三类 key，网格与详情统一走聚合封面，store 不再维护代表曲目映射）
- [x] 转换器接 Rust 后端（2026-10-11 完成：新增 `converter.rs` 模块 —— ffmpeg 批量转码专用线程（顺序执行、-progress 实时进度、stderr 错误回收、半成品清理、同名 "(n)" 编号绝不覆盖）、取消（任务间跳过 + kill 当前子进程）、`converter://event` 事件（started/progress/completed/failed/finished）；命令 converter_start/cancel/probe_files；AppError::Converter；实测修正 out_time_ms 微秒陷阱改解析 out_time 时间串；前端 types/converter.ts 契约 + services/converter.ts（Tauri 实现 + Web mock 定时器模拟）+ stores/converter.ts（事件驱动任务状态、设置 localStorage 持久化、切页不丢状态）+ ConverterView 真实路径选择（pickAudioFiles/pickDirectory）、取消按钮、失败/取消态展示；四格式命令行参数已用项目 bin/ffmpeg.exe 实测通过；cargo check 与 npm run build 零错误）
- [x] 队列项移除（2026-10-11 完成：`handle.rs` remove_from_queue（下标越界校验、当前曲下标修正、移除当前曲不打断播放、被移除项恰为预加载下一曲时失效标记）、新增 PlaybackEvent::Queue 事件推送最新上下文 + player_remove_from_queue 命令；前端 IPlayer.removeFromQueue 契约 + Tauri/Web 双实现 + store removeFromQueue 动作与 queue 事件同步；QueuePanel 行尾 hover × 移除按钮（绝对定位避免 button 嵌套）；队列跳播已有双击 playFromContext 覆盖，未另设 jumpTo 命令；cargo check 与 npm run build 零错误）
- [x] 在线模块真实通道（2026-10-14 复核：默认关闭、开启也不上报本地曲库、设置开关已就绪；歌词与封面已走 Rust 后端真实联网受 `settings.online.enabled` 控制，通道闭环；`services/online.ts` 统一出口收敛为后续可选优化，见 ROADMAP §3.1）
- [x] 拖拽导入（2026-10-14 完成：Tauri drag-drop 事件拿到真实路径，过滤音频扩展名（文件或目录递归），新增 `library_import_files` 命令复用 `scan::parse_track`/`discover_audio_files` 直接入库（不强制白名单校验，与 dialog 选择器同源可信）；前端 App.vue 注册 `onDragDropEvent` 显示全窗遮罩、松手后 `libraryStore.importDropped` 入库并刷新；`LibraryService.importFiles` 契约 + Tauri/Web 双实现；cargo check 与 npm run build / vue-tsc 零错误）
- [x] 系统媒体控制 SMTC（2026-10-14 实现：Windows-only，内存 `MediaPlayer` 作 SMTC 代理，`SystemMediaTransportControls` 上报元数据/状态，专用 STA 线程消息泵 + channel 隔离所有 WinRT 调用；按钮事件经 `smtc://button` 转发前端 player store 控制（play/pause/next/previous/stop）；元数据/状态随 `playback://event` 同步；`#[cfg(windows)]` 隔离，非 Windows 编译透明；驱动封面（缩略图）为后续增强）
- [x] 应用图标各平台生成（2026-10-01：按 public/logo-1024.svg 几何用 System.Drawing 绘制 1024 PNG（temp/gen-icon.ps1），`npx @tauri-apps/cli icon` 全量生成 32/64/128/ico/icns/Square 系列至 src-tauri/icons）
- [x] `ARCHITECTURE.md` / `ROADMAP.md`（2026-10-14 补：架构分层/命令契约/状态管理/安全边界/在线模块设计约束/平台约束/构建发布；里程碑回顾与 M3 状态、SMTC 决策、发布前待办）

### M4 性能与发布闭环（2026-10-06）
- [x] 曲库持久化迁移 SQLite（rusqlite 0.32 bundled；`library/db.rs` PRAGMA WAL/busy_timeout/foreign_keys；tracks/track_artists/favorites/recent/playlists/playlist_tracks 全 SQL 化，聚合与封面候选查询下推 SQL；首次启动自动导入旧 library.json 并改名 .bak；扫描改增量 mtime diff + 单事务 upsert/删除；顺带修复 album_id 恒为 None 导致真实环境专辑聚合失效）
- [x] 封面性能优化（单曲/分组封面改 asset 协议文件 URL，assetProtocol scope 放开 $APPDATA/cover-cache；useTrackCover 内存缓存改 LRU 240；播放条取色用封面保留 data URL 避免画布跨域污染）
- [x] TrackList 虚拟滚动（vue-virtual-scroller RecycleScroller，行高随密度档 --size-row-h 实时同步，外层滚动容器实时测高，切歌定位改 scrollToItem，KeepAlive/sticky 表头/多选回归保留）
- [x] 应用内自动升级（tauri-plugin-updater + process；minisign 密钥对在 src-tauri/.tauri-signing（已 gitignore），公钥写入 tauri.conf.json；设置-更新页真实检查/发布说明/下载进度/重启安装，启动 8s 静默检查；通道为 GitHub Releases latest.json，**endpoint 当前指向 `ChrisHcn1/TPlayer` 仓库；发布前请确认该仓库即本项目发版仓库且公钥匹配，否则自动更新校验失败**；旧 update 占位模块与命令已删除，capabilities 加 updater:default/process:default）
- [x] 程序化打包（scripts/build-win.ps1 必须带 BOM 的 UTF-8 编码，回落本地密钥；npm run build:win；.github/workflows/release.yml 推 v* tag 触发 tauri-action 自动签名+latest.json+Release 草稿；README 增打包发布章节；版本三处对齐 2.0.0）
- [x] 皮肤色三档分层（同一封面色相派生：皮肤色 brand 亮调（按钮/选中）、--color-cover-progress 饱和实色中调（进度条/拖块）、--color-cover-lyrics 近白（深主题）/近墨（浅主题）高亮 + 强调色辉光（歌词当前行，四处消费方带回落），多维拉开避免同色无法区分）
- [x] 启动自动续播（player_init 恢复断点后播放列表非空即自动从断点位置继续播放，失败静默降级）
- [x] 关于页项目介绍（定位/主要特性 18 格式/隐私说明/技术致谢/版本信息）

---

### 性能优化专项：切歌/下一首间隙（淡入淡出减少切换间隙）（2026-10-10 新增）

> 目标：将「点击下一首 / 自动连播」到新曲可闻的间隙压缩到 ~400ms 内并平滑过渡。
> 根因（已定位）：引擎 `play()` 同步阻塞完成解码初始化后，新曲才从静默开始 400ms 淡入；全程无预解码/预加载下一曲；`FADE_MS` 硬编码 400ms 不可调；ffmpeg 后端切歌还需新起子进程（seek 为 `-ss` 重启）；状态机在曲目真正结束后才取 next_id 再 play。

- [x] P0 预解码/预加载下一曲（核心）：引擎维护 `next` 链路（File::open + Decoder / ffmpeg 子进程提前驻留），切歌时直接 swap 接管为新 current、旧曲转 fading_out，消除「打开+解码初始化」可听间隙；注意句柄/子进程生命周期与多路驻留上限
- [x] P1 交叉淡化时长参数化：`FADE_MS` 常量改为设置项（开关 + 0~3s 滑块，对齐旧版 crossfadeEnabled/crossfadeDuration 默认 1s），持久化进 config.json / settings store，engine 命令下发（2026-10-10 后端落地；2026-10-11 确认 SettingsView 播放页签 UI 已接好，闭环完成）
- [ ] P1 解码初始化异步化：`play()` 内的 File::open / Decoder::new / spawn_ffmpeg 移出引擎主循环（或独立预解码线程），避免阻塞 fade 驱动与命令处理；ffmpeg 后端 seek 的 `-ss` 重启同步改异步
- [x] P2 自动连播预加载（2026-10-10：handle.rs tick 循环在剩余 < 3s 且 next 空时经 maybe_preload_next 取下一曲 id 调 engine.preload 预解码到 next 链路，推送 NextPreloaded 事件；on_track_ended 优先 swap_to_next 无缝接管，失败/曲目变更回退普通 play；主动 play/stop 使预加载失效）
- [x] P2 前端衔接（2026-10-10：PlayerBar 传输区「下一首准备中」徽标，由 NextPreloaded 事件驱动 nextReady 状态，切歌/停止自动复位；仅用 theme.css token，无写死色值）

---

### 功能增强专项（2026-10-10 新增）

> 目标：延续核心体验打磨，按「尖刀功能优先」原则分两档推进——近期先落地三个高价值尖刀功能，其余建议功能列入后期更新项目计划。

#### 近期实施项
- [x] 收听报告 + 分享卡片：按曲目/歌手/时段/榜单维度生成收听统计报告，一键渲染可分享卡片（导出图片/复制文本）（2026-10-10 完成：本地统计 store + 收听报告视图 + SVG 分享卡片 PNG 导出）
- [x] 音量归一化 ReplayGain：解析 ReplayGain 标签（或按参考响度计算），播放时按增益自动归一化，避免切歌音量跳变（2026-10-10 完成：入库解析内嵌 TRACK_GAIN/TRACK_PEAK 标签 + db v2 持久化 + 无标签曲目首次播放 ffmpeg ebur128 响度分析 + 引擎按增益归一化音量（-14 LUFS 目标、峰值防削波）+ 设置页开关，cargo build --release 与 npm run build 零错误）
- [x] 目录自动监控：监听曲库目录变更（新增/删除/改名），增量同步进曲库，无需手动重新扫描（2026-10-11 完成：Rust notify 实时监听（600ms 去抖批量）+ 按磁盘实况增量入库/更新/移除 + Tauri 事件推送前端自动刷新曲库 + 设置页开关与监控状态展示，cargo build --release 与 npm run build 零错误）
- [x] 睡眠定时器（2026-10-11 完成：`handle.rs` SleepTimer（epoch 到期 + wait_track_end 模式到期后播完当前曲才停、delay=0 立即进入曲终等待），tick 到期立即暂停并推送 sleepTimer 事件，on_track_ended 曲终停播分支；新增 player_set_sleep_timer / player_cancel_sleep_timer / player_sleep_timer_status 命令；IPlayer 契约 + Web mock（setTimeout 暂停/曲终停止）+ player store sleepTimer 状态与动作；PlayerBar 右侧新增月亮按钮，菜单 15/30/45/60/90 分钟 / 播完当前曲目后停止 / 取消定时，激活态高亮 + title 倒计时；顺带修复 NextPreloaded 事件 serde tag 全小写导致前端 'nextPreloaded' 分支不命中的隐患（改显式 rename）；cargo check 与 npm run build 零错误）

#### 后期更新项目计划（未实施）
- [x] 睡眠定时器（→ 已提前完成，见上）
- [ ] 播放统计权重
- [ ] 桌面歌词透明模式
- [ ] 迷你模式
- [ ] 频谱可视化
- [ ] 元数据编辑器
- [ ] 重复曲目检测
- [ ] 智能歌单
- [ ] 歌单导入导出 m3u/pls
- [ ] 均衡器补齐
- [ ] 变速播放
- [ ] 播放历史导出

---

### 音频格式兼容补丁模块（2026-10-15 新增，独立模块，零侵入）

- 目的：不改任何播放核心代码（tauriPlayer / webPlayer / WebPlayer / Rust 引擎），兜底 wma/aiff/ape/dts/dsf 等原生不支持、或后缀错配 / 冷门编码 / 轻微头损坏导致的「无法播放」。
- 接入：`services/player.ts` 的 `playerService` 改为 `createCompatPlayer(basePlayer)`；`tauri.conf.json` CSP connect-src 加 `asset:`（兜底经 asset 协议读字节）。
- 结构（`src/services/compat/`，纯前端 9 文件）：
  - `index.ts` `CompatPlayer` 包裹 `IPlayer`——仅包一层 play：播放前用 `plugin-fs` 的 `open+read(64字节)` 精确读头做魔数预判（~1-3ms，不依赖后缀名、零 Rust 改动）；原生可播(mp3/flac/ogg)或 WAV 容器先试原生（失败再兜底），冷门格式(wma/aiff/ape/dts/dsf/amr 等)直接走兜底省去必败的原生探测；不持久化标记、每首实时判定（解码产物仍按 trackId:size:mtime 缓存复用）；`fallback-player.ts` 用隐藏 `<audio>` 播标准 WAV 并桥接进度事件。
  - `magic.ts` 魔数识别真实格式（不依赖后缀名，含头损坏容错）：WAV/MP3/FLAC/AAC/AMR/OGG/WMA/AIFF/DSF/DFF/DTS/APE/WV/TTA/TAK/MIDI。
  - `decoders.ts` 轻量 JS 解码器（L1，KB 级）：WAV 容错解析 + PCM 直提(8/16/24/32-bit) + G.711(μ/A-law) + IMA/MS ADPCM。
  - `worker.ts` 解码调度（所有解码跑在 Web Worker，主线程零阻塞）+ `ffmpeg.ts` L2 兜底（FFmpeg.wasm 懒加载，默认关，变量名动态 import 规避构建依赖）。
  - `cache.ts` 转码缓存（内存 + IndexedDB，按 `trackId:size:modifiedAt` 复用）；`config.ts` 远程开关 / 灰度 / 回滚；`telemetry.ts` 埋点（触发率 / 成功率 / 耗时）。
- 性能：首屏零兼容资源加载；正常文件完全不受影响。风险隔离：所有异常捕获，兜底失败静默降级。
- L2 启用前置：`npm i @ffmpeg/ffmpeg @ffmpeg/util` 并将 core 置于 `public/ffmpeg/`；远程开关 `ffmpegWasmEnabled` 控制。

## 6. 已知问题与决策备忘

| 编号 | 问题 | 状态 |
|------|------|------|
| BUG-001 | 主题双写双源 | ✅ 已修复（useTheme 唯一持有） |
| BUG-002 | `src/styles/` 路径错误 | ✅ 已修复（实际 `src/assets/styles/`） |
| BUG-003 | 旧版 index.html 主题死代码 | ✅ 已修复 |
| BUG-004 | Logo 绝对路径冲突 | ✅ 已修复（base `./`） |
| BUG-005 | 侧栏折叠按钮假入口 | ✅ 已修复（useLayout 接管，Ctrl+B） |
| BUG-006 | 图标集未接入 | ✅ 已修复（icons.ts 内联 SVG 集 + Icon.vue，M1 全量替换占位） |
| BUG-007 | node_modules 未安装 | ✅ 已解决（已安装 51 包） |
| BUG-008 | 标题栏品牌名 | ✅ 已统一为「TPlayer」 |
| BUG-009 | 依赖安装越界 | ⚠️ 记录：vue-router 缺失 + node_modules 缺失导致编译阻塞，file-agent 已代为 `npm install`（含补装 vue-router），用户此前约束为「本阶段不做重量级安装」，需知悉 |
| BUG-010 | `dist/` 构建产物 | ⚠️ 复核用产物，可删除；已确认在 `.gitignore` 内 |
| BUG-011 | 启动即假死（窗口"未响应"） | ✅ 已修复（2026-10-11。根因：目录监控 setup 中 `state.watch.reconfigure(app, &state.config.lock().unwrap().library_dirs, state.config.lock().unwrap().watch.enabled)` —— 同一表达式两次 lock 同一把 config Mutex，第 1 个临时 MutexGuard 存活到语句结束导致同线程自死锁（std Mutex 不可重入），主线程卡死在 setup 内、事件循环永不启动。修复：先在独立作用域取出 (dirs, watch_enabled) 再调用。经验教训：同一调用表达式中不要对同一 Mutex 多次 lock。诊断记录：Tauri 2 中非 async 命令默认跑在主线程、窗口先于 setup 创建（假死但窗口可见）、GUI 子系统下 eprintln 不可见需文件插桩 |

---

## 7. 关键约定（不可违背）

1. `E:\TPlayerNext` 与旧版 `E:\TPlayer` 完全隔离，**严禁读写或改动旧版任何文件**。
2. 旧版功能必须完整覆盖，**不做功能减法**；重梳逻辑消除重复与冲突。
3. 样式一律引用 `theme.css` 语义 token，禁止写死色值与像素。
4. 改已有文件直接原文件修改，**严禁另存带时间戳的重命名副本**。
5. 主题单一事实源：`useTheme` 唯一持有，`settings store` 不得再出现 theme 字段。
6. 中间产物写 `temp/`，最终产出写 `output/`。

---

## 8. 文件索引（按需查阅，勿全量遍历）

```
E:\TPlayerNext\
├── docs\        DESIGN.md · FEATURES-INVENTORY.md · PROJECT-STATUS.md
├── public\      logo.svg · favicon.svg · logo-1024.svg
├── src\
│   ├── App.vue                     五区布局壳 + 全局接线（浮层宿主/初始化/快捷键/切歌联动）
│   ├── constants.ts                存储键常量
│   ├── assets\styles\              theme.css · main.css
│   ├── components\layout\          TitleBar.vue · SideBar.vue · RightPanel.vue
│   ├── components\player\          PlayerBar.vue · ProgressBar.vue · VolumeControl.vue
│   ├── components\panel\           LyricsPanel.vue · QueuePanel.vue
│   ├── components\library\         TrackList · TrackRow · TrackListHeader · SelectionBar
│   │                               · ListToolbar · MediaCard · DetailHero
│   ├── components\common\          Icon · icons.ts · SearchBox · EmptyState · BaseButton
│   │                               · Modal · ToastContainer · DialogHost · ContextMenuHost
│   │                               · CommandPalette
│   ├── composables\                useTheme · useLayout · usePlayer · useKeyboardShortcuts
│   │                               · useContextMenu · useCommandPalette
│   │                               · useTrackActions · useTrackSelection
│   ├── router\index.ts             16 条路由
│   ├── i18n\nav.ts                 导航文案映射
│   ├── views\                      14 个视图组件（M1 全部接入真实数据）
│   ├── stores\                     library · player · lyrics · playlist · settings
│   │                               · toast · dialog
│   ├── services\                   player · library · lyrics · online · update · ipc · dialog · settings · converter
│   │                               └── web\      dataset · library · player · lyrics（浏览器 mock 引擎）
│   ├── stores\                     converter
│   └── types\                      track · player · lyrics · api · converter
└── src-tauri\
    ├── Cargo.toml                  rodio · lofty · sha2 · rand · chrono 依赖
    ├── tauri.conf.json             bundle.targets=["nsis"] · assetProtocol scope 限定音乐目录
    ├── capabilities\default.json   最小权限集（窗口/dialog/fs 只读，限定 $AUDIO 与 $HOME/Music）
    ├── icons\                      32x32/128x128/256.png · icon.ico · icon.icns（M2 占位）
    └── src\
        ├── main.rs                 入口：调用 tplayer_next_lib::run()
        ├── lib.rs                  AppState + run() + 40 条命令注册 + 进度时钟
        ├── config.rs               AppConfig（library_dirs/online/scanOnStartup/locale）→ config.json
        ├── error.rs                AppError / AppResult（code/message/detail 序列化）
        ├── security.rs             ensure_allowed 路径白名单校验
        ├── commands\               player(14) · library(21) · lyrics(2) · update(1) · settings(2) · converter(3)
        ├── converter.rs            ffmpeg 批量转码（专用线程、-progress 实时进度、取消 kill 子进程、同名 "(n)" 编号不覆盖）
        ├── player\
        │   ├── engine.rs           rodio 引擎 + EngineHandle（mpsc 通道，解决 OutputStream 非 Send）
        │   ├── handle.rs           状态机 + 自动连播 + playback://event 推送
        │   └── state.rs            PlaybackSnapshot/PlayMode/PlayContext/PlaybackEvent
        ├── library\
        │   ├── models.rs           Track/Album/Artist/Playlist/LibraryStats/RecentEntry/ScanProgress
        │   ├── store.rs            LibraryStore：library.json 持久化 + 聚合 + 收藏/最近/歌单 CRUD
        │   └── scan.rs             lofty 元数据扫描 + library://scan-progress 事件
        ├── lyrics\parser.rs        M2 占位（load 返回 None，offset 存内存）
        └── update\checker.rs       M2 占位（始终返回无更新）
```
