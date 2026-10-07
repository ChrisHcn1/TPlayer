# SQLite 持久化 + 性能优化 + 打包发布 + 自动升级 + 关于页 实施计划

## 需求（用户已确认的决策）

1. 持久化：曲库数据（tracks / 收藏 / 最近播放 / 歌单）迁移到 SQLite（rusqlite bundled）；config.json、playback_state.json、lyrics_offsets.json 保持 JSON。首次启动自动从 library.json 导入并备份。
2. 性能优化四项全做：曲目列表虚拟滚动、封面改 asset 文件 URL + LRU、聚合/封面候选查询下推 SQL、扫描增量 upsert。
3. 程序化打包：产出 Windows NSIS 安装包，提供本地构建脚本与 GitHub Actions 发布工作流。
4. 自动升级：GitHub Releases + Tauri 官方 updater（应用内检查/下载/安装）。
5. 关于页：补全项目介绍。

## Repository Research

- 曲库存储现为 [store.rs](file:///e:/TPlayerNext/src-tauri/src/library/store.rs)：`LibraryData` 全量 JSON，任何收藏/播放历史变化都整体序列化重写 library.json；tracks() 等接口全部 `clone()` 整个 Vec；albums()/artists() 每次 O(n) 全量聚合。
- 扫描 [scan.rs](file:///e:/TPlayerNext/src-tauri/src/library/scan.rs)：`ScanResult { tracks }` 全量返回；track id = sha256(绝对路径) 前 16 hex，天然稳定；已记录 modified_at，具备增量基础。命令层 `replace_tracks` 全量替换。
- 封面 [commands/library.rs](file:///e:/TPlayerNext/src-tauri/src/commands/library.rs)：单曲封面返回 base64 data URL；磁盘缓存已在 `%APPDATA%/TPlayer Next/cover-cache/`；专辑/艺术家分组缓存键 `album:`/`artist:`。CSP 已允许 asset / http://asset.localhost。asset scope 现为 `$AUDIO/**`、`$HOME/Music/**`，**不含 cover-cache 目录**。
- 列表 [TrackList.vue](file:///e:/TPlayerNext/src/components/library/TrackList.vue)：普通 `v-for` 全量渲染；`vue-virtual-scroller@2.0.0-beta.8` 已在依赖中。存在当前曲目自动 scrollIntoView、选区、右键菜单、双击播放、sticky 表头等既有行为必须保留。
- 更新：[checker.rs](file:///e:/TPlayerNext/src-tauri/src/update/checker.rs) 永远返回无更新；[SettingsView.vue](file:///e:/TPlayerNext/src/views/SettingsView.vue) checkUpdates 是 setTimeout 假检查；前端 services/update.ts 是 NotImplementedError 占位。
- 打包：tauri.conf.json 已配 nsis、currentUser、18 种文件关联、中文/英文语言；无构建脚本、无 updater 配置、无 icons/installerIcon 问题（icon.ico 存在）。
- 取色 [useCoverTheme.ts](file:///e:/TPlayerNext/src/composables/useCoverTheme.ts)：用 img 绘制 canvas 采样 data URL。**网格/详情改 asset URL 后，取色仍需像素访问**；为避开 asset canvas 跨域污染风险，当前播放曲目封面（播放条/放大层/取色，仅 1 张）保留 data URL 通道不变，仅分组封面走 asset URL。
- AppState 持有 library_path；lib.rs setup 中构造 LibraryStore。
- Tauri 2 updater 要求：endpoints 指向公网 latest.json；安装包用 minisign 私钥签名，公钥写入 tauri.conf.json；构建产物为 `*-setup.nsis.zip` + `.sig`。

## Files and Modules

### 后端（Rust）
- `src-tauri/Cargo.toml`：加 `rusqlite = { version = "0.32", features = ["bundled"] }`、`tauri-plugin-updater = "2"`；release profile 已有 lto/opt-size。
- `src-tauri/src/library/db.rs`（新）：连接、PRAGMA（WAL、foreign_keys、busy_timeout）、schema 初始化、版本/迁移表 `meta(key,value)`。
- `src-tauri/src/library/models.rs`：Track/Playlist 等结构保持 serde 兼容；新增少量行映射辅助。
- `src-tauri/src/library/store.rs`：重写为 SQLite 后端（同名公开 API 尽量保持：tracks/get_track/tracks_of_album/tracks_of_artist/albums/artists/favorites/recent/playlists CRUD/remove_tracks/replace_tracks→upsert_scan）；内部改用参数化 SQL 与事务。
- `src-tauri/src/library/scan.rs`：新增 diff 增量支持（暴露文件枚举与 mtime，或新增 `scan_incremental(existing: &[(id,path,mtime)]) -> ScanChange`）；解析进度事件不变。
- `src-tauri/src/commands/library.rs`：适配新 store；分组封面命令返回 asset URL（新增 `library_album_cover_url` / `library_artist_cover_url` 返回 `tauri::Url`/asset 路径字符串），单曲 `library_cover` 保持 data URL；封面候选查询用 SQL（has_cover DESC, track_number ASC LIMIT 30）。
- `src-tauri/src/lib.rs`：注册 updater 插件；LibraryStore::load 改接 db 路径并执行 JSON 首次迁移；library_path 字段语义改为 db 路径。
- `src-tauri/tauri.conf.json`：assetProtocol.scope 增加 `$APPDATA/cover-cache/**`；plugins.updater（pubkey + endpoints 占位 GitHub releases）。
- `src-tauri/capabilities/default.json`：加 `updater:default` 权限。
- `src-tauri/src/update/`：updater 插件化后 checker 占位代码删除（或保留文档注释指向插件）。
- `.gitignore`：忽略 `*.nsis.zip`、密钥相关文件（私钥永不入库）。

### 前端
- `src/services/library.ts` + `src/services/web/library.ts`：接口 `albumCover/artistCover` 语义变为文件 URL（web mock 仍返回 data URL，无需变）；单曲 cover 不变。
- `src/composables/useTrackCover.ts`：urlCache 改 LRU Map（容量 240），超出淘汰最久未用；其余逻辑不变。
- `src/components/library/TrackList.vue`：接入 `RecycleScroller`（固定行高，:item-size 与现行行高一致；sticky 表头保留在滚动体外）；行组件抽出 TrackRow 已存在，直接复用；保留当前曲目定位（scroller scrollToPosition）、选区/右键/双击/键盘。
- `src/views/SettingsView.vue`：更新页签接真实 updater（检查结果、发布时间/更新说明、下载进度、重启安装）；关于页签补项目介绍。
- `src/services/update.ts`：改为 `@tauri-apps/plugin-updater` + `plugin-process` 封装；web 环境保持 reject/不可用。
- `package.json`：加 `@tauri-apps/plugin-updater`、`@tauri-apps/plugin-process`；加脚本 `build:win`、`tauri:signer:generate`（文档化命令）。
- 启动后静默检查更新一次（App.vue 或 settings store，有更新才 toast，不打扰）。

### 打包 / 发布
- `scripts/build-win.ps1`（新）：校验环境变量 `TAURI_SIGNING_PRIVATE_KEY`（存在则签名出 updater 包，不存在则普通构建并提示）；执行 `npm run tauri build`；输出产物路径与 latest.json 模板说明。
- `.github/workflows/release.yml`（新）：推送 `v*` tag 触发，windows-latest 构建 → 签名 → 发布 GitHub Release（tauri-apps/tauri-action，自动生成 latest.json 与签名 nsis zip）。
- README.md：补充"构建与发布"小节（密钥生成、环境变量、tag 流程、三处版本号同步）。

### 关于页文案要点
- 一句话定位：本地优先的桌面音乐播放器，Tauri 2 + Vue 3 + Rust。
- 核心特性：无损/DSD 等 18 种格式、交叉淡化、封面主色皮肤、CUE、歌单/收藏/最近播放、在线能力默认关闭。
- 隐私说明：曲库信息不上传；封面/歌词在线匹配可关。
- 技术致谢：rodio/symphonia、lofty、FFmpeg（可选边车）、Vue、Tauri。
- 版本、资源目录、GitHub 发布页链接（endpoint 同源地址）。

## Implementation Steps（依赖顺序）

1. **SQLite 基础**：加依赖；db.rs（连接+schema+meta）；cargo check 通过。
2. **store 重写**：建表 tracks（主键 id、file_path 唯一、album_id、modified_at、has_cover、track_number 索引；artists 序列化为 JSON 数组列）、favorites(track_id PK, added_at)、recent(track_id PK, played_at)、playlists(id PK, name, description, created_at, updated_at, is_system)、playlist_tracks(playlist_id, track_id, position)。所有现有公开方法改 SQL 实现；事务包裹多表写。
3. **JSON 迁移**：LibraryStore::open(db_path, legacy_json_path)：若 meta 无 schema_ready 且 library.json 存在 → 建表导入 → json 改名为 library.json.bak → 写 meta；lib.rs 接线。
4. **增量扫描**：store 提供现有 (id, path, mtime) 快照；scan 模块只对新增/变更文件 parse_track，删除文件按 id 清理（含 favorites/recent/playlist 引用，沿用 remove_tracks 口径）；upsert 在单事务提交；扫描进度事件保持。
5. **聚合/封面前选下推**：albums/artists 改 SQL GROUP BY；分组封面候选 SQL 排序 LIMIT 30；命令层适配。
6. **封面 asset URL**：conf scope + 分组命令返回 convertFileSrc 兼容的文件 URL（Rust 侧拼 `http://asset.localhost/` 或由前端 convertFileSrc 编码原始路径——选后者：命令返回缓存文件绝对路径，前端 convertFileSrc；web mock 返回 data URL）；前端 LRU。
7. **虚拟滚动**：TrackList 换 RecycleScroller，逐项回归（播放、定位、选区、菜单、表头吸顶、空态、矮行/高 DPI）。
8. **自动升级**：插件 + 权限 + conf；前端服务与设置页 UI（进度条/安装）；启动静默检查；生成 minisign 密钥（公钥入库、私钥只输出给用户保存路径提示，不写入仓库）。
9. **打包发布**：build-win.ps1、release.yml、README 小节；版本号三处核对 2.0.0。
10. **关于页**：介绍文案与链接。
11. **全量验证**（见下）。

## Dependencies and Considerations

- rusqlite 0.32 + bundled：首次编译多 1–2 分钟（编译 amalgamation），之后走缓存；无需用户装 SQLite。
- asset 协议：前端用 `@tauri-apps/core` 的 `convertFileSrc(absPath)`；scope 用 Tauri 2 变量 `$APPDATA`（实际解析到 `%APPDATA%/com.tplayer.next`？注意 Tauri 2 app_data_dir 实际目录以 identifier 为后缀，scope 变量按框架解析，构建后实测命中）。
- 取色通道不迁移 asset URL，规避 canvas 跨域污染；单曲 data URL 仅当前曲目一张，内存开销固定。
- updater endpoint 与 GitHub 仓库名：计划用占位 `https://github.com/<OWNER>/<REPO>/releases/latest/download/latest.json`，**需用户提供新仓库地址后一行替换**；旧仓库为 ChrisHcn1/TPlayer 但 identifier/产品不同，不建议混用。
- minisign 私钥只存在用户本机/CI Secrets；无代码签名证书时安装包会有 SmartScreen 首次提示（文档说明）。
- web mock（services/web）不经 Rust，store 重写不影响浏览器预览；mock 仍用 localStorage。

## Validation

- `cargo check --manifest-path src-tauri/Cargo.toml`、`npm run typecheck`。
- dev 实测：删除 db 用现有 library.json 启动 → 自动导入、json 变 .bak；曲目/专辑/艺术家/收藏/歌单数据一致；再扫一次走增量（日志/耗时可见，mtime 未变文件不重新解析）。
- 专辑/艺术家网格封面经 asset 加载（Network 面板为 asset.localhost 请求，非 data:）；LRU 淘汰不重复报错；当前播放封面取色皮肤仍生效。
- 曲目列表 1k+ 曲目（可用临时大目录）滚动流畅、DOM 节点数恒定；当前曲目自动定位、双击/右键/选区正常。
- 更新页"检查更新"对当前版本返回无更新（指向测试 endpoint 或本地模拟 200）；安装按钮在 hasUpdate 时才出现。
- `npm run tauri build` 完整 release 构建成功（后台执行，预计 5–15 分钟），产物含 NSIS exe、`*-setup.nsis.zip`、`.sig`；脚本在无密钥环境给出明确提示。
- 浏览器 mock 回归：专辑/艺术家/详情/歌单/收藏页功能不回归。

## Risks

- **虚拟滚动兼容性**（sticky 表头、动态高度、滚动定位）：行高固定时风险低；若出现高度差，给 RecycleScroller 明确 item-size 并保留原非虚拟 DOM 的快照分支不现实，改为充分回归后直接替换；自动定位用 scrollToPosition({index})。
- **增量扫描漏删/误删**：以 path→id 哈希为准做三路 diff（新增/变更/删除），单事务提交；迁移首次导入保留 .bak 可回退。
- **updater 误推**：endpoint 未就绪前先指向占位，检查失败静默处理不打扰用户；正式发布前必须替换真实仓库。
- **release 构建时间长/网络**：tauri-action 在 CI 完成签名发布；本地脚本只要求能出包；若本地构建环境网络受限，CI 路径兜底。
- **范围较大**：按 1→11 顺序提交，每阶段 cargo check/typecheck 绿灯后再进下一阶段；SQLite + store 是后续聚合下推与增量的基础，不并行。
