# TPlayer 旧版源码功能清单与逻辑问题清单（v2 需求基线）

- 分析对象：`E:\TPlayer`（旧版，只读）
- 产出目的：作为 `E:\TPlayerNext`（TPlayer Next）v2 重设计的需求基线，用于逐项勾兑"是否已覆盖旧版全部功能"与"是否已消除旧版逻辑问题"
- 分析方式：全量源码扫描（前端 Vue/TS、后端 Rust、配置文件、语言包）+ 交叉核对（配置声明 vs 代码事实 vs 实际调用链）
- 约束：本清单分析全程对 `E:\TPlayer` **只读**，未修改、未写入、未删除任何文件
- 生成日期：2026-09-26

> 行号约定：`文件:行号` 或 `文件:起始行-结束行`，行号以旧版源码当前状态为准。前端主文件为 `src/App.vue`（总 9752 行，其中非空行 8985 行）。

---

## 0. 文件与代码量基线

### 0.1 前端 `src/`

| 文件 | 行数（非空） | 角色 |
|---|---|---|
| `src/App.vue` | 9752（非空 8985） | **单体主组件**：模板 + 全部业务逻辑 + 全量样式 |
| `src/main.ts` | 9 | 仅 `mount`，无全局初始化 |
| `src/api/music.ts` | 60 | 网易云公开 API 代理封装 |
| `src/components/AboutDialog.vue` | 229 | 关于对话框（读取版本号） |
| `src/components/AudioConverter.vue` | 535 | 独立音频转换器 |
| `src/components/OnlineMatchModal.vue` | 412 | 在线元数据/歌词/封面匹配 |
| `src/components/Settings.vue` | 623（总 680） | 设置面板（props/emit 模式） |
| `src/components/UpdateModal.vue` | 350 | 更新对话框（**未被引用**） |
| `src/components/UpdateProgress.vue` | 405 | 更新进度（**未被引用**） |
| `src/composables/useCue.ts` | 129（总 146） | CUE 专辑/Track 状态与命令封装 |
| `src/services/cacheService.ts` | 186 | 缓存服务（歌词/封面，localStorage 索引 + Rust 缓存命令） |
| `src/services/chartLyricsService.ts` | 49 | ChartLyrics 在线歌词源 |
| `src/services/i18n.ts` | 102 | 多语言初始化 |
| `src/services/localMetadataService.ts` | 72 | 本地元数据读取（**未被任何文件引用**） |
| `src/services/lyricParser.ts` | 364 | LRC/YRC 解析、`parseSmartLrc` |
| `src/services/multiSourceLyricService.ts` | 314 | 多源歌词聚合 |
| `src/services/musicDataService.ts` | 376 | 在线歌曲信息/封面服务 |
| `src/services/onlineMusicService.ts` | 282 | 在线音乐检索与缓存 |
| `src/stores/local.ts` | 214 | 类型定义 + `DEFAULT_SETTINGS` + localforage 持久化 |
| `src/types/tauri.d.ts` | 41 | 后端类型声明 |
| `src/types/vue-virtual-scroller.d.ts` | 13 | 虚拟滚动类型声明 |
| `src/utils/stringSimilarity.ts` | 55（测试 85） | 字符串相似度（匹配算法） |

### 0.2 后端 `src-tauri/src/`

| 文件 | 行数（非空） | 角色 |
|---|---|---|
| `commands.rs` | 1687 | 扫描/播放（rodio）/时长/窗口/环境检查/README |
| `ffmpeg_transcoder.rs` | 2420 | 转码、ffplay 播放、ffprobe 探测、转码缓存 |
| `cue_parser.rs` | 379 | CUE 解析 |
| `http_server.rs` | 565（总 649） | 本地 HTTP 文件服务器（Range 支持） |
| `updater.rs` | 304 | 更新检查/下载/校验/安装 |
| `main.rs` | 285 | 入口、命令注册、托盘、窗口事件 |
| `audio_converter.rs` | 161 | 音频转换（转码输出） |
| `commands_cache.rs` | 117 | 缓存读写命令 |
| `commands_cue.rs` | 92 | CUE 扫描命令 |
| `equalizer.rs` | 23 | 均衡器结构体（**无任何调用方**） |
| `audio_decoder.rs` | 0 | **空占位文件（0 字节）** |
| `ffmpeg_decoder.rs` | 0 | **空占位文件（0 字节）** |
| `special_decoder.rs` | 0 | **空占位文件（0 字节）** |
| `transcoder.rs` | 0 | **空占位文件（0 字节）** |

### 0.3 配置与资源

| 项 | 内容 |
|---|---|
| `package.json` | version **1.0.4** |
| `src-tauri/tauri.conf.json` | version **1.0.6**（与 package.json 不一致）；identifier `D57E920A.TPlayer`；productName `TPlayer` |
| `src-tauri/Cargo.toml` | version 1.0.6 |
| 权限 | `fs` scope `/path: "**/*"`（全盘）；assetProtocol 同样放开 |
| CSP | 允许多个外部域名 |
| `capabilities/default.json` | 仅 `core:default`（与 tauri.conf.json 内联权限**分散在两处**） |
| 文件关联 | `mp3 / flac / wav / aac / ogg / m4a / wma / dsf / dff`（9 种） |
| `public/locales` | 12 语言（zh-CN / zh-TW / en-US / hi-IN / es-ES / ar-SA / fr-FR / bn-BD / ru-RU / pt-PT / ms-MY / de-DE） |
| 打包产物 | NSIS `TPlayer_1.0.4_x64-setup.exe`；MSIX `D57E920A.TPlayer_1.0.6.0_x64.msix` |
| 数据目录 | `%APPDATA%\TPlayer` |
| 仓库根目录杂物 | 20+ 个 md 说明文档、`fix-log.js`、`fix_seek.py`、`temp_log_fix.js`、`test.html`、`app.log`、`tauri_log.txt`、多个调试截图 png |

---

## 1. 全量功能清点（按功能域）

### A. 播放内核与播放控制

| # | 功能项 | 说明 | 源码位置 |
|---|---|---|---|
| A-01 | 上一首 / 播放暂停 / 下一首 | 底部播放栏三键，含播放模式联动 | `App.vue:499-500`、`App.vue:3951-4064` |
| A-02 | 播放模式切换（顺序/随机/重复） | 三态轮换 + 图标切换 | `App.vue:1178`、`App.vue:1274-1278`、`App.vue:4061-4065` |
| A-03 | 随机模式下一首预选 | randomNextIndex 预确定 | `App.vue:2637`、`App.vue:3951`、`App.vue:4022`、`App.vue:3569`、`App.vue:3693` |
| A-04 | 重复模式列表末尾处理 | 末首后回到首首 | `App.vue:3965`、`App.vue:4042`、`App.vue:5820` |
| A-05 | 播放进度条拖拽 / 跳转（seek） | 前端 seek 与 ffplay seek 两套 | `App.vue:1093`（`seek_ffplay`）、`App.vue:3436`、`App.vue:4507` |
| A-06 | 音量调节 / 静音 | 0-100 滑块、静音记忆 `previousVolume` | `App.vue:548-558`、`App.vue:1180-1181`、`App.vue:4154-4167` |
| A-07 | 交叉淡入淡出（crossfade） | 开关 + 时长 0-3 秒，播放/切换时淡入淡出 | `App.vue:1031-1032`、`App.vue:1917-1945`、`App.vue:3470-3502` |
| A-08 | 自动播放下一首 | 开关控制 | `App.vue:6181-6196`、`Settings.vue:25-31` |
| A-09 | 播放完成检测定时器 | 非 `ended` 事件的兜底完成判定 | `App.vue:1815-1820` |
| A-10 | 播放请求并发保护 | `playSongLock` + `currentPlayId` 序号丢弃过期请求 | `App.vue:1772-1835` |
| A-11 | CUE 音轨区间播放 | 相对/绝对位置换算、结束时间截断 | `App.vue:1977-2049`、`App.vue:2741-2769` |
| A-12 | 后端 rodio 播放内核 | `play_song`（仅 CUE 路径调用） | `commands.rs:763-1430`；调用点 `composables/useCue.ts:106` |
| A-13 | 后端 ffplay 播放内核 | 不支持格式与回退播放 | `App.vue:2089-2170`、`App.vue:3652`；`commands.rs:815-1047` |
| A-14 | 前端 HTMLAudio 播放内核 | asset 协议 / blob URL 播放 | `App.vue:2820-2960`、`App.vue:2911-2963` |
| A-15 | ffplay 状态轮询与 UI 同步 | 定时查 `get_ffplay_status` 同步进度/音量 | `App.vue:2289`、`App.vue:2372`、`App.vue:4283`、`App.vue:4369` |
| A-16 | 播放暂停/恢复/停止（ffplay） | 独立命令 | `App.vue:3799`、`App.vue:3809`、`App.vue:2079`、`App.vue:3632` |
| A-17 | 下一首预转码 | 提前转码下一首 | `App.vue:5750`（`pretranscode_audio`） |
| A-18 | 播放进度持久化 | 恢复上次播放曲目与进度 | `stores/local.ts`（`progress` 键）、`App.vue:5911-5945` |

### B. 格式支持与解码 / 转码链路

| # | 功能项 | 说明 | 源码位置 |
|---|---|---|---|
| B-01 | 音频格式扩展名清单 | 前端支持判定与后端一致/不一致处 | `App.vue:1553`（浏览器分支 `.mp3/.flac/.wav/.ogg/.aac/.m4a`）、`App.vue:2089`（不可直接播放清单 `.dsf/.dff/.dsd/.mqa/.wv/.tta/.ape/.wma`） |
| B-02 | 扫描支持的格式 | Rust 扫描侧扩展名 | `commands.rs:102-210` |
| B-03 | 格式能力判定 | `needs_transcode` / `needs_ffplay_playback` | `commands.rs:636`、`commands.rs:815`；`ffmpeg_transcoder.rs` |
| B-04 | 自动转码不支持的格式 | 设置开关 `enableTranscode`（默认开） | `App.vue:2705`（`get_transcoded_path`）、`Settings.vue:57-68` |
| B-05 | 强制全部转码为 FLAC | 设置开关 `forceTranscode`（默认关） | `App.vue:6196`、`App.vue:895`（`commands.rs`） |
| B-06 | 转码缓存与复用 | 转码产物缓存路径复用 | `ffmpeg_transcoder.rs`（2420 行）、`commands.rs:911-960` |
| B-07 | 时长获取（双源） | lofty 主 + ffprobe 备 | `commands.rs:1432-1560`、`commands.rs:212-274` |
| B-08 | ffprobe/ffmpeg 可执行文件定位 | 环境内查找 | `commands.rs:217`、`commands.rs:1457`、`ffmpeg_transcoder.rs` |
| B-09 | 音频环境检测命令 | `check_audio_environment`（**前端未调用**） | `commands.rs:667-762`；`main.rs:80-118` 已注册 |
| B-10 | 独立音频转换器（UI） | 选择输出目录 + 指定格式转换 | `components/AudioConverter.vue:227`、`:246`；`commands.rs:audio_converter`、`audio_converter.rs:161` |
| B-11 | 本地 HTTP 文件服务 | 支持 Range 的本地流式服务（**前端未调用**） | `http_server.rs`（启动于 `main.rs:69`；`get_file_http_url` 注册 `main.rs:105`） |
| B-12 | 空占位解码器文件 | 4 个 0 字节 rs 文件（音频/FFmpeg/特殊/转码） | `audio_decoder.rs`、`ffmpeg_decoder.rs`、`special_decoder.rs`、`transcoder.rs` |

### C. 均衡器与音效

| # | 功能项 | 说明 | 源码位置 |
|---|---|---|---|
| C-01 | 均衡器面板 UI | 10 段滑块 + 预设下拉 | `App.vue:431-465` |
| C-02 | 预设应用 | flat/rock/pop/jazz/classical/electronic | `App.vue:4727-4743`、`Settings.vue:44-52` |
| C-03 | 均衡器开关 | `equalizerEnabled` 设置项 | `Settings.vue:41-52`、`App.vue:6181` |
| C-04 | 参数持久化 | 预设 + 10 段数值 | `stores/local.ts`（settings）、`App.vue:5913-5914`、`App.vue:6191-6192` |
| C-05 | 后端均衡器 | `Equalizer` 结构体（**无调用方，未生效**） | `equalizer.rs:1-27` |

> 结论（用于 v2）：旧版均衡器**只有配置与 UI，没有作用于音频链路的实现**，属"名义功能"。

### D. 曲库扫描与媒体库

| # | 功能项 | 说明 | 源码位置 |
|---|---|---|---|
| D-01 | 目录选择 + 递归扫描 | `scan_directory` 命令 | `App.vue:1608-1620`、`commands.rs:102-210` |
| D-02 | 浏览器降级扫描 | `webkitdirectory` 文件选择 + blob URL | `App.vue:1478-1600` |
| D-03 | 扫描结果合并与去重 | 与 CUE 曲目池合并、按路径去重、收藏状态回填 | `App.vue:1622-1700` |
| D-04 | 曲目时长/元数据填充 | lofty + ffprobe + 标签封面 | `commands.rs:328-435`、`commands.rs:505` |
| D-05 | 封面读取（扫描期/播放期） | 同名图 + `cover/folder/album/front` 回退 | `App.vue:1858-1945` |
| D-06 | 曲库持久化 | localforage `songs` 键 | `stores/local.ts` |
| D-07 | 艺术家视图 | 按艺术家聚合 | `App.vue:176`、`App.vue:1367` |
| D-08 | 专辑视图 | 按专辑聚合 | `App.vue:250`、`App.vue:1367` |
| D-09 | 全部歌曲视图 | 主列表 | `App.vue:41` |
| D-10 | 虚拟滚动列表 | `vue-virtual-scroller` 类型声明 + 滚动处理 | `App.vue:5880-5883`、`types/vue-virtual-scroller.d.ts` |
| D-11 | 删除歌曲（单曲/批量，仅列表层） | 见缺陷 B-04（不落盘、不清理引用） | `App.vue:5505-5513`、`App.vue:1754-1770` |

### E. 歌单、收藏与批量操作

| # | 功能项 | 说明 | 源码位置 |
|---|---|---|---|
| E-01 | 收藏切换 | 收藏列表 + 状态持久化 | `App.vue:45`、`stores/local.ts`（`favorites`） |
| E-02 | 歌单创建 | prompt 输入名称 | `App.vue:5515-5524`、`App.vue:69-70` |
| E-03 | 歌单列表读取 | localforage | `App.vue:5478`、`App.vue:5943` |
| E-04 | 添加歌曲到歌单 | 右键菜单入口 | `App.vue:5460-5502` |
| E-05 | 多选模式 | 选择模式 + 计数 + 操作条 | `App.vue:120-140`、`App.vue:127` |
| E-06 | 批量播放选中 | 播放选中第一首 | `App.vue:1747-1753` |
| E-07 | 批量加入歌单 | 选中项批量加入 | `App.vue:1738-1746` |
| E-08 | 批量删除选中 | 逐首 `confirm` 循环 | `App.vue:1754-1770` |
| E-09 | 右键菜单 | 播放/加歌单/收藏/编辑/删除 | `App.vue:587-600`、`App.vue:4759-4768`、`App.vue:5848` |

> 待复核：歌单在**侧边栏/主区没有独立视图入口**（`currentFilter` 仅 `all/favorites/artists/albums/cue`，`App.vue:1192`），歌单创建后似乎只能通过"加歌单"弹窗回看。

### F. 歌词（本地 / 多源 / 逐行滚动 / 缓存）

| # | 功能项 | 说明 | 源码位置 |
|---|---|---|---|
| F-01 | 本地歌词优先级链 | 内嵌标签 → 同名 `.lrc` → 在线多源（该实现**两处重复**） | `App.vue:2540-2600`、`App.vue:3020-3060` |
| F-02 | 同名 `.lrc` 读取 | `plugin-fs readTextFile` | `App.vue:2552-2567`、`App.vue:4783-4800` |
| F-03 | LRC / 智能解析 | `parseSmartLrc`（含翻译/时间戳） | `services/lyricParser.ts`（364 行）、`App.vue:5540` |
| F-04 | YRC 逐字歌词支持 | `yrcData` 分支 | `App.vue:5211-5212`、`App.vue:5319-5321` |
| F-05 | 多源在线歌词 | 聚合多源 + 相似度筛选 | `services/multiSourceLyricService.ts:1-314`、`utils/stringSimilarity.ts` |
| F-06 | ChartLyrics 源 | 独立源实现 | `services/chartLyricsService.ts:16-49`、`multiSourceLyricService.ts:2,280` |
| F-07 | 歌词逐行滚动显示 | 按播放位置高亮当前行 | `App.vue:5540-5700`（含 `5692` CUE 偏移处理） |
| F-08 | 歌词显示位置（顶/底） | `lyricsPosition` | `App.vue:783` 区域、`Settings.vue:105-120` |
| F-09 | 歌词显示开关 | `showLyrics`（默认开） | `Settings.vue:98-104` |
| F-10 | 歌词缓存（localforage） | `lyric_cache` 键 | `stores/local.ts` |
| F-11 | 歌词缓存（localStorage） | 多源歌词独立缓存 | `services/multiSourceLyricService.ts:332,348` |
| F-12 | 歌词写入标签 | 编辑标签时保存歌词 | `App.vue:5200-5330` |

### G. CUE 分轨

| # | 功能项 | 说明 | 源码位置 |
|---|---|---|---|
| G-01 | CUE 扫描 | `scan_cue_files` | `composables/useCue.ts:43-59`、`commands_cue.rs:1-92`、`App.vue:1622` |
| G-02 | 单文件 CUE 解析 | `parse_cue_file_command` | `useCue.ts:62-70`、`cue_parser.rs:1-379` |
| G-03 | CUE 专辑视图 | 侧边栏"分轨专辑"入口 + 列表 | `App.vue:57-60`、`App.vue:366-410` |
| G-04 | CUE Track 加入主列表 | 转换为 Song 并入曲库池 | `App.vue:1648-1680` |
| G-05 | 应用内播放 CUE 音轨 | 相对位置换算 + 首尾裁切 | `App.vue:1387-1455`、`App.vue:1977-2049` |
| G-06 | CUE 标签编辑区 | 编辑模态框专属区块 | `App.vue:700`、`App.vue:4801-4810` |
| G-07 | CUE 时间双重来源 | `title` 内嵌 `::` 时间戳 与 `startTime/endTime` 字段并存 | `useCue.ts:14-40`；`App.vue:2741-2769` |
| G-08 | 独立 CUE 播放函数 | `playCueTrack`（**与 G-05 路径重复**） | `useCue.ts:94-117` |
| G-09 | 无实际用途的 CUE 函数 | `selectCueAlbum` / `toggleCueView` / `addCueTracksToPlaylist`（空实现）/ `getCueTrackDuration` | `useCue.ts:73-91`、`useCue.ts:120-133` |

### H. 元数据与封面匹配

| # | 功能项 | 说明 | 源码位置 |
|---|---|---|---|
| H-01 | 编辑标签模态框 | 多标签页（本地/文件名匹配/在线匹配/歌词/封面） | `App.vue:660-760`、`App.vue:5200-5340` |
| H-02 | 本地标签读取 | `music-metadata` 前端解析 | `App.vue:888`、`App.vue:4977-4978` |
| H-03 | 后端标签读取 | lofty | `commands.rs:328-435`、`commands.rs:505` |
| H-04 | 文件名解析匹配 | 正则解析"艺术家-标题" | `App.vue:1540-1560`；根目录 `SETTINGS_AND_FILENAME_PARSING_FIX.md` |
| H-05 | 在线匹配（手动） | 关键词检索 → 元数据/歌词/封面写入 | `components/OnlineMatchModal.vue:1-412`、`App.vue:5126-5170` |
| H-06 | 自动匹配 | 按曲目自动在线匹配 | `App.vue:5258-5320` |
| H-07 | 在线歌曲信息服务 | 检索 + 封面转 base64 | `services/musicDataService.ts:52-404`、`App.vue:5151`、`App.vue:5300` |
| H-08 | 动态封面 URL | 在线动态封面 | `App.vue:475-476`、`App.vue:952`、`App.vue:5168-5170` |
| H-09 | 封面放大模态框 | 点击封面放大 + 歌词 + 可拖拽 | `App.vue:4913-4958` |
| H-10 | 复制文件路径 | 剪贴板复制 | `App.vue:4958` |
| H-11 | 元数据决策文档 | 旧版已有决策记录（v2 可继承） | 根目录 `METADATA_DECISION.md` |

### I. 搜索与筛选

| # | 功能项 | 说明 | 源码位置 |
|---|---|---|---|
| I-01 | 关键字搜索 | 前端 `filter`（标题/艺术家/专辑） | `App.vue:1192` 附近、`filteredSongs` 计算 |
| I-02 | 视图筛选 | all/favorites/artists/albums/cue | `App.vue:1367-1386` |
| I-03 | 后端全文检索 | **无**（无后端搜索命令） | 对照 `main.rs:80-118` 注册清单 |

### J. 外观、主题与多语言

| # | 功能项 | 说明 | 源码位置 |
|---|---|---|---|
| J-01 | 深浅主题切换 | class 绑定 + 大量 light 覆盖样式 | `App.vue:15`、`App.vue:5915`、`App.vue:6357-8700` |
| J-02 | 主题预加载脚本 | index.html 内联脚本（默认 light，与默认 dark 冲突） | `index.html` |
| J-03 | 主题持久化 | localforage settings | `stores/local.ts` |
| J-04 | 多语言（12 种） | 语言包 + `t()` | `services/i18n.ts:1-102`、`public/locales/*.json`、`Settings.vue:75-90` |
| J-05 | 启动画面 | 加载态遮罩 | `App.vue:15`（`v-if="!isLoading"`）、`App.vue:9612+` |
| J-06 | 应用 Logo / 图标资源 | 根目录多个 Logo png（含多语言截图） | `TplayEN1.png`、`TplayEN2.png`、`TPlayEN3.png`、`TPlayTC.png` 等 |
| J-07 | 样式指南文档 | 按钮样式规范（v2 可继承） | 根目录 `BUTTON_STYLE_GUIDE.md`、`DISPLAY_FIX.md` |
| J-08 | 右键菜单禁用/屏蔽 | `contextmenu` 全局监听 | `App.vue:5848-5865`、`CONTEXTMENU_DISABLE.md` |

### K. 系统集成（托盘 / 窗口 / 快捷键 / 文件关联 / 开发者）

| # | 功能项 | 说明 | 源码位置 |
|---|---|---|---|
| K-01 | 系统托盘 | 菜单：显示/下一首/播放暂停/上一首/退出 | `main.rs:169-281` |
| K-02 | 托盘事件 → 前端 | `tray-next-song` / `tray-previous-song` 事件 | `main.rs:181-187` |
| K-03 | 托盘菜单动态重建 | `update-tray-menu` 监听重建菜单 | `main.rs:235-281` |
| K-04 | 关闭窗口行为 | `CloseRequested` 处理（隐藏/清理） | `main.rs:130-168` |
| K-05 | 自绘标题栏窗口控制 | 最小化 / 最大化 / 关闭 | `App.vue:5631`、`App.vue:5641`、`App.vue:5651-5652` |
| K-06 | 关闭窗口清理 | `cleanup_player_resources` | `commands.rs:1740-1750` |
| K-07 | 全局快捷键 | **仅 F12 开发者工具**（业务快捷键缺失） | `App.vue:6058-6064`；`App.vue:5855-5866`（注释掉的 keydown） |
| K-08 | 文件关联 | 9 种扩展名声明 | `tauri.conf.json` |
| K-09 | README 打开 | `open_readme` 命令（首次运行检测） | `App.vue:6043-6046`、`commands.rs:1666-1735` |
| K-10 | 开发者工具 | `open_devtools` 命令 + F12 | `App.vue:6064`、`main.rs:107` |
| K-11 | 日志系统 | 前端 `logInfo/logError/logDebug` 双写 console | `App.vue:911-930`、`App.vue:1129-1154` |
| K-12 | 首次运行标记 | `tplayer-first-run`（localStorage） | `App.vue:6043-6051` |
| K-13 | 窗口可见性切换 | `toggle_window_visibility`（**前端未调用**） | `commands.rs:1596-1620` |
| K-14 | 系统媒体控制（SMTC/MediaSession） | **未实现** | 全前端未见 `mediaSession` 调用 |

### L. 在线服务

| # | 功能项 | 说明 | 源码位置 |
|---|---|---|---|
| L-01 | 网易云公开 API 代理 | 搜索/歌曲信息/歌词/封面 | `api/music.ts:1-60` |
| L-02 | 在线音乐服务层 | 检索 + localStorage/Rust 双层缓存 | `services/onlineMusicService.ts:1-282` |
| L-03 | 歌词缓存（在线） | `getLyric/saveLyric` | `onlineMusicService.ts:113-124`、`cacheService.ts` |
| L-04 | 封面缓存（在线） | `getCover/saveCover` | `onlineMusicService.ts:241-258`、`cacheService.ts` |
| L-05 | 在线匹配弹窗 | 候选选择与写入 | `components/OnlineMatchModal.vue:1-412` |
| L-06 | 缓存清理 | `clear_cache` | `App.vue`（cacheService）、`cacheService.ts:140,203` |
| L-07 | 网络信息外发约束 | 无任何开关/白名单控制 | 对照 `tauri.conf.json` CSP 与 `api/music.ts` |

### M. 自动更新

| # | 功能项 | 说明 | 源码位置 |
|---|---|---|---|
| M-01 | 更新检查（手动） | `check_update_manual`（**前端未调用**） | `updater.rs`；注册 `main.rs:112` |
| M-02 | 更新检查（自动） | `check_update_auto`（**前端未调用**） | `updater.rs`；注册 `main.rs:113` |
| M-03 | 下载更新 | `download_update_command`（**未调用**） | `updater.rs`；注册 `main.rs:114` |
| M-04 | 校验更新 | `verify_update_command`（**未调用**） | `updater.rs`；注册 `main.rs:115` |
| M-05 | 安装更新 | `install_update_command`（**未调用**） | `updater.rs`；注册 `main.rs:116` |
| M-06 | 版本号读取 | `get_current_version`（被调用） | `main.rs:117`、`AboutDialog.vue:76`、`Settings.vue:465` |
| M-07 | 更新设置项 | autoUpdate / updateInterval / backgroundUpdate | `Settings.vue:157-256`（props 默认值） |
| M-08 | 更新 UI 组件 | `UpdateModal.vue`、`UpdateProgress.vue`（**均未被挂载**） | 全文件 350 + 405 行 |
| M-09 | 设置面板更新触发 | `emit('checkUpdate')`（**App.vue 未监听**） | `Settings.vue:222-234` |

> 结论（用于 v2）：旧版**更新链路整体断裂**——后端命令齐备但前端从不调用，更新 UI 组件从未挂载，设置面板的触发事件无人接收。v2 必须重新设计更新闭环（或明确不做更新）。

### N. 持久化与缓存

| # | 存储位置 | 键/用途 | 源码位置 |
|---|---|---|---|
| N-01 | localforage | `songs` / `playlists` / `favorites` / `progress` / `settings` / `lyric_cache` | `stores/local.ts:1-214` |
| N-02 | localStorage | `tplayer-first-run` | `App.vue:6043-6051` |
| N-03 | localStorage | 缓存索引 `INDEX_KEY` | `cacheService.ts:30,51` |
| N-04 | localStorage | 多源歌词缓存 `cacheKey` | `multiSourceLyricService.ts:332,348` |
| N-05 | Rust 端缓存目录 | 歌词/封面二进制缓存 | `commands_cache.rs:1-117`、`cacheService.ts:64-140` |
| N-06 | 数据目录 | `%APPDATA%\TPlayer` | `tauri.conf.json` |

---

## 2. 交互与配置清点

### 2.1 设置项全集（键 / 默认值 / 落盘位置）

| 设置项 | 键名 | 默认值 | 来源（默认值定义处） | 持久化 |
|---|---|---|---|---|
| 主题 | `theme` | `dark` | `stores/local.ts` DEFAULT_SETTINGS；`Settings.vue:266-269`；**`index.html` 预加载为 light（冲突）** | localforage `settings` |
| 语言 | `language` | `zh-CN` | `Settings.vue:270-273` | localforage `settings` |
| 音量 | `volume` | `80` | `App.vue:1181` | localforage `settings` |
| 播放模式 | `playbackMode` | `order` | `App.vue:1178` | localforage `settings` |
| 均衡器开关 | `equalizerEnabled` | `false` | `Settings.vue:281-284` | localforage `settings` |
| 均衡器预设 | `equalizerPreset` / `currentPreset` | `flat` | `App.vue:1198` | localforage `settings` |
| 均衡器频段 | `equalizerBands` | `[0×10]` | `App.vue:1199` | localforage `settings` |
| 交叉淡入淡出开关 | `crossfadeEnabled` | `false` | `App.vue:1031` | localforage `settings` |
| 淡入淡出时长 | `crossfadeDuration` | `1`（0-3 秒） | `App.vue:1032` | localforage `settings` |
| 自动播放下一首 | `autoPlayNext` | `true` | `Settings.vue:246-249` | localforage `settings` |
| 显示歌词 | `showLyrics` | `true` | `Settings.vue:274-277` | localforage `settings` |
| 歌词位置 | `lyricsPosition` | `bottom` | `Settings.vue:278-280` | localforage `settings` |
| 音乐目录 | `musicDirectory` | `''` | `Settings.vue:290-293` | localforage `settings` |
| 自动转码 | `enableTranscode` | `true` | `Settings.vue:285-288` | localforage `settings` |
| 强制转码 FLAC | `forceTranscode` | `false` | `Settings.vue:286-289` | localforage `settings` |
| 自动检查更新 | `autoUpdate` | `true` | `Settings.vue`（props 默认 true） | **仅 UI，无消费者** |
| 更新检查间隔 | `updateInterval` | `24`（24h/168h） | `Settings.vue`（props 默认 24） | **仅 UI，无消费者** |
| 后台更新 | `backgroundUpdate` | `false` | `Settings.vue`（props 默认 false） | **仅 UI，无消费者** |

### 2.2 配置同步与持久化机制

- 统一 watch 落盘：`App.vue:6181-6196`（14 个设置项一次 watch → localforage）。
- 启动恢复：`App.vue:5911-5945`（含 playlists、progress）。
- 歌曲库/收藏/进度独立 watch 落盘：`App.vue:6101-6310` 区间。
- 设置面板为 props + emit 双向绑定模式（`Settings.vue:157-256` 定义 props/emits，`App.vue:783-784` 等 `v-model:` 绑定），并非共享 store。

### 2.3 非设置类交互清单

| 交互 | 入口 | 源码位置 |
|---|---|---|
| 右键菜单 | 歌曲行 / CUE 行 / 专辑行 | `App.vue:214`、`:289`、`:340`、`:587-600` |
| 多选模式 | 工具栏进入，逐行勾选 | `App.vue:120-140` |
| 封面模态框拖拽 | mousemove/mouseup 全局监听 | `App.vue:4913-4935`、`:4924-4925` |
| 列表滚动 | 虚拟滚动 | `App.vue:5880-5883` |
| F12 开发者工具 | 全局 keydown | `App.vue:6058-6064` |
| 首次运行打开 README | 启动检测 | `App.vue:6043-6046` |
| 文件拖入导入 | **未实现**（仅封面模态框拖拽） | 全前端无 `dragover/drop` 处理 |
| 启动参数（双击文件打开） | **未见解析实现** | 全前端无 `argv/launch` 处理（文件关联已声明 9 种格式） |

---

## 3. 重复与冲突清单（逐条含证据与影响）

| 编号 | 类型 | 现象 | 证据（文件:行号） | 影响 |
|---|---|---|---|---|
| D-01 | 重复实现 | 歌词加载优先级链完整复制两份（内嵌 → `.lrc` → 在线） | `App.vue:2540-2600` 与 `App.vue:3020-3060` | 双份逻辑需两处同步修改；易出现行为不一致 |
| D-02 | 重复实现 | 播放内核三套并存：HTMLAudio、rodio(`play_song`)、ffplay | `App.vue:2820-2960`、`useCue.ts:106`、`App.vue:2152/3652` | 状态/进度/音量三套来源；进度回传与 UI 同步口径不一 |
| D-03 | 职责重叠 | 元数据解析三处：Rust lofty、Rust ffprobe、前端 music-metadata | `commands.rs:329/505/1437`、`commands.rs:213/1457`、`App.vue:888,4977`、`services/localMetadataService.ts:17-80` | 同一文件可能得到不同结果；`localMetadataService` 整体是死代码 |
| D-04 | 重复实现 | 时长获取两条链路（lofty 主 + ffprobe 备）且前端另调 `get_audio_duration` | `commands.rs:1432-1560`、`App.vue:2684` | 播放前需额外 IPC 往返，进度条初始长度可能抖动 |
| D-05 | 重复实现 | 歌词缓存三套：localforage `lyric_cache`、localStorage（多源）、Rust 缓存命令 | `stores/local.ts`、`multiSourceLyricService.ts:332,348`、`cacheService.ts:64-140` | 缓存不一致、清理入口不统一（清理只覆盖 Rust 侧） |
| D-06 | 重复实现 | CUE 播放两条路径：`playCueTrackInApp`（App.vue）与 `useCue.playCueTrack` | `App.vue:1387-1455`、`useCue.ts:94-117` | 后者走 rodio 且不更新主列表状态，易产生"界面与声音不同步" |
| D-07 | 职责重叠 | CUE 时间双重表达：`title` 内嵌 `::` 与 `startTime/endTime` 字段 | `useCue.ts:14-40`、`App.vue:2741-2769` | 两套来源冲突时以谁为准不明，是历史 bug 高发点 |
| D-08 | 配置分散 | 权限配置分散在 `tauri.conf.json`（内联 fs/assetProtocol）与 `capabilities/default.json`（仅 core:default） | `tauri.conf.json`、`capabilities/default.json` | 权限边界不集中，审计困难；易误以为已收窄 |
| D-09 | 逻辑互相覆盖 | 默认主题三处不一致：index.html 预加载 light、DEFAULT_SETTINGS dark、CSS 默认深色 | `index.html`、`stores/local.ts`、`App.vue:15` | 首屏闪白/闪黑；首帧与恢复值不一致 |
| D-10 | 版本事实不一致 | package.json 1.0.4 vs tauri.conf.json / Cargo.toml 1.0.6；NSIS 包名 1.0.4，MSIX 1.0.6 | `package.json`、`tauri.conf.json`、`Cargo.toml` | 版本追溯混乱，更新比对与发布流程不可信 |
| D-11 | 死代码/未接线 | 更新链路三段脱节：设置触发无监听、更新组件未挂载、5 个 updater 命令未调用 | `Settings.vue:222-234`、`UpdateModal.vue`、`UpdateProgress.vue`、`main.rs:112-116` | 更新功能对用户完全不可用，却仍有设置项与 UI 交付 |
| D-12 | 死代码 | 均衡器前后端均未接入音频链路 | `equalizer.rs:1-27`、`App.vue:431-465,4727-4743` | 用户拉动滑块无实际效果（名义功能） |
| D-13 | 死代码 | 本地 HTTP 服务始终启动但前端从不调用 | `main.rs:69`、`http_server.rs`、`main.rs:105`（`get_file_http_url` 未调用） | 常驻本地端口 + 攻击面扩大，收益为零 |
| D-14 | 死代码 | 4 个 0 字节 rs 占位文件 | `audio_decoder.rs`、`ffmpeg_decoder.rs`、`special_decoder.rs`、`transcoder.rs` | 误导后续维护者判断"已有多解码器架构" |
| D-15 | 死代码 | 未被引用的服务与函数 | `services/localMetadataService.ts`、`useCue.ts:73-91,120-133` | 增加阅读成本，接口契约含糊 |
| D-16 | 已注册未使用命令 | `check_audio_environment`、`toggle_window_visibility`、`close_window`、`set_ffplay_volume`、`check_http_server_status`、5 个 updater 命令 | `commands.rs:667`、`:1590-1620`、`main.rs:80-118` | IPC 暴露面过大（含更新/文件服务），无调用却可被调用 |
| D-17 | 权限过宽 | `fs` scope `**/*` 与 assetProtocol 全盘放开；CSP 允许外部域名 | `tauri.conf.json` | 任一前端注入即可读写全盘；与"本地音乐播放器"最小权限原则冲突 |
| D-18 | 单体结构 | `App.vue` 9752 行承载模板 + 业务 + 样式 | `App.vue` | 无法并行开发/测试；改动风险与回归成本极高 |
| D-19 | 浏览器降级分支混杂 | 桌面/浏览器两套扫描与播放逻辑交织在同一函数 | `App.vue:1478-1600`（浏览器扫描）与 `App.vue:1608-1700`（桌面扫描） | 分支状态互相污染，桌面路径易被浏览器分支改动误伤 |
| D-20 | 日志重复 | `logInfo/logError` 与 `console.log/console.error` 成对重复输出（含调试残留注释） | `App.vue:911-930`、`App.vue:2100-2170` 等 | 日志噪音大、定位困难；生产无日志级别控制 |
| D-21 | 工程卫生 | 仓库根目录堆积 20+ md、3 个临时脚本、`app.log`/`tauri_log.txt`、调试截图 | 仓库根目录 | 发布包/仓库内容不洁，易误提交 |

---

## 4. 逻辑缺陷清单

### 4.1 已知问题逐条确认现状

| 编号 | 已知问题 | 现状确认 | 证据 |
|---|---|---|---|
| B-01 | index.html 主题预加载死代码 / 默认主题注释与逻辑不一致 | **确认**：内联脚本默认 `light`，注释称默认深色，而 `DEFAULT_SETTINGS.theme = 'dark'` 且 CSS 基色为深色 | `index.html`、`stores/local.ts`、`App.vue:15,5915` |
| B-02 | 版本号不一致 | **确认**：1.0.4（package.json）vs 1.0.6（tauri.conf.json / Cargo.toml），安装包命名随之分裂 | 同上 |
| B-03 | 4 个空占位 rs 文件 | **确认**：4 个文件 0 字节，且 `main.rs` 未 `mod` 声明（对照 `main.rs:12-18`） | `audio_decoder.rs` 等 4 个文件 |
| B-04 | App.vue 8985 行（非空）单体 | **确认**：总 9752 行，模板 + 逻辑 + 样式全在一个 SFC | `App.vue` |
| B-05 | 权限过宽 | **确认**：fs/assetProtocol `**/*`，CSP 含外部域名，capabilities 另存一份 | `tauri.conf.json`、`capabilities/default.json` |
| B-06 | 更新功能断裂 | **确认**：见 D-11（触发无监听 + 组件未挂载 + 命令未调用） | `Settings.vue:222-234`、`main.rs:112-116` |

### 4.2 新增发现（本次分析补充）

| 编号 | 缺陷 | 说明与影响 | 证据 |
|---|---|---|---|
| B-07 | 删除歌曲不落盘、且不清理引用 | `deleteSong` 仅 `songs.splice()`（内存 + 随 watch 写入曲库），但确认框文案为"此操作不可撤销"；不清理 `favorites` 与歌单中的引用，产生幽灵条目 | `App.vue:5505-5513`、`App.vue:1754-1770` |
| B-08 | 批量删除为 N 次串行确认 | 循环 `deleteSong` → 每首弹一次 `confirm`，选中 50 首即 50 次弹窗 | `App.vue:1754-1770` |
| B-09 | 本地 HTTP 服务鉴权弱、可任意路径读取 | `/file/` 前缀仅做 token 等值校验，随后对 URL 解码结果直接 `File::open`，**无路径白名单**；响应带 `Access-Control-Allow-Origin: *`；服务在启动时无条件开启 | `http_server.rs:226-233,242-309,470-505`、`main.rs:69` |
| B-10 | CUE 音轨时长计算依赖浮点解析字符串 | `getCueTrackDuration` 用 `parseFloat("mm:ss")`… 该类字符串无法被 `parseFloat` 正确解析（返回分钟数），导致时长为 0/错误 | `useCue.ts:120-133`、`useCue.ts:14-24` |
| B-11 | 交叉淡入淡出与音量状态三处并存 | `volume.value` / `audioElement.volume` / `ffplayVolume`，且淡出期间临时改音频元素音量，恢复路径依赖注释人工保证 | `App.vue:1071,1180-1181,1912-1945,2967-2977,3470-3502` |
| B-12 | 播放完成判定存在双机制 | `ended` 事件 + 定时器兜底 + `isPlaybackFinished` 标志，切换曲目时需多处重置 | `App.vue:1815-1825`、`App.vue:2963` |
| B-13 | 前端"文件存在性检查"依赖前端 fs 插件 | 播放前对每条路径做存在性检查，性能差且与后端重复 | `App.vue:2666` |
| B-14 | 音量/进度在 ffplay 轮询中被回写 | 轮询用后端 status 覆盖前端 `volume`/进度，用户拖动滑块时可能被回写抖动 | `App.vue:2289-2313`、`App.vue:4283-4307`、`App.vue:4369-4389` |
| B-15 | 无系统媒体控制（SMTC / MediaSession） | 无媒体键响应、无系统播放信息面板，与"桌面音乐播放器"期望不符 | 全前端无相关实现（对照 K-14） |
| B-16 | 无全局业务快捷键 | 仅有 F12 开发者工具；播放/切歌快捷键缺失（注释里曾尝试） | `App.vue:6058-6064`、`App.vue:5855-5866` |
| B-17 | 无文件拖入导入 | 已声明 9 种文件关联与桌面播放器定位，但缺少拖拽添加文件/文件夹能力 | 全前端无 `dragover/drop` |
| B-18 | 启动参数（关联打开）未见处理 | 文件关联声明 9 种格式，前端未检索到 `argv/launch` 解析逻辑，推测双击文件不会加入播放 | 全前端无相关调用（对照 `tauri.conf.json`） |
| B-19 | 搜索仅前端过滤、无后端检索 | 大曲库（万级）时全量前端过滤会阻塞 UI；无分页/无索引 | `App.vue:1192` 附近、`main.rs:80-118` |
| B-20 | 无音量归一化 / ReplayGain / 无输出设备选择 | 高于同类的"音乐播放器"功能缺位（v2 可选） | 全前端与 Rust 命令清单均无 |
| B-21 | 语言包与文案同步风险 | 12 语言包需人工同步；`settings.*` 部分文案在组件内硬编码中文（如"平坦/摇滚/自动转码不支持的格式"） | `Settings.vue:44-52,57-68` |
| B-22 | 首屏加载策略单一 | `v-if="!isLoading"` 整页阻塞，无骨架/渐进渲染 | `App.vue:15` |
| B-23 | 调试残留 | 源码中保留 `console.log` 同步日志、被注释代码块（如"toggleRepeat 函数已移除"） | `App.vue:4148`、`App.vue:5855-5866`、`App.vue:2100-2170` |
| B-24 | 歌词/封面在列表层可能被整体写入曲库 | 播放时动态读取封面并挂到 song 对象上，随 song 一起被持久化，导致曲库体积膨胀 | `App.vue:1858-1945`、`stores/local.ts`（songs 落盘） |
| B-25 | `playSongLock` 锁定被"取消式"处理 | 新请求直接置空旧锁，旧请求可能已释放后端资源（停止 ffplay/卸载 audio），存在竞态 | `App.vue:1778-1800`、`App.vue:2079`、`App.vue:3632` |

---

## 5. 功能归类建议（v2 功能域与模块划分依据）

### 5.1 功能域收敛（建议 8 域）

| 功能域 | 覆盖内容（对应本清单条目） | 建议 v2 模块 |
|---|---|---|
| ① 播放内核 | A-01~A-18、B-01~B-08 | `player/engine`（**单一内核**：统一 Player 接口，替换 HTMLAudio + rodio + ffplay 三套） |
| ② 媒体库与曲库 | D-01~D-11、I-01~I-03 | `library/scan`、`library/index`（含后端检索与分页） |
| ③ 歌单与收藏 | E-01~E-09 | `library/playlist`（含批量操作，走统一确认流） |
| ④ 歌词 | F-01~F-12 | `lyrics/*`（统一缓存与优先级链，仅一份实现） |
| ⑤ 元数据与封面 | H-01~H-11、B-07~B-08 | `metadata/*`（单一解析链路：Rust 侧统一，前端不重复解析） |
| ⑥ 外观与体验 | J-01~J-08、K-05、B-21~B-22 | `views`/`components`/`theme`（含 i18n、主题单点定义） |
| ⑦ 系统集成 | K-01~K-14、文件关联、启动参数 | `system/tray`、`system/shortcuts`、`system/file-association`（含 SMTC、拖入导入） |
| ⑧ 在线服务 | L-01~L-07、M-01~M-09 | `online/*`（独立可开关、默认不向外部发送本地库信息）+ `update/*`（重建更新闭环） |

> 另设"内部基础设施域"（非用户可见）：持久化（N-01~N-06，统一到单一声明式存储层）、日志（K-11，分级 + 生产关闭）、安全（权限白名单）。

### 5.2 与 v2 架构硬约束的对应关系

- 播放内核统一：对应 ① 域，消除 D-02/D-06/D-11/D-16 中的三套内核与重复播放路径。
- 元数据单链路：对应 ⑤ 域，消除 D-03 与 `localMetadataService` 死代码。
- 歌词单链路 + 单缓存：对应 ④ 域，消除 D-01/D-05。
- 权限收窄：对应 ⑦/内部安全域，消除 D-17/B-09/B-18（白名单 + 移除无用本地 HTTP 服务 + 明确关联打开实现）。
- 分层与行数上限（后端单文件 ≤800 行、前端按 views/components/composables/stores/services 分层）：直接对冲 D-18/D-19。

### 5.3 UI 信息架构建议（供 v2 界面重设计参考）

- 左侧导航：全部歌曲 / 收藏 / 艺术家 / 专辑 / 分轨专辑（CUE）/ **歌单（旧版缺失入口，需补）** / 转换器。
- 主区：曲库（搜索 + 排序 + 虚拟滚动）、批量操作条、空状态引导；**新增**拖入导入区域。
- 底部播放栏：封面 + 标题/艺术家 + 进度 + 播放控制 + 音量 + 播放模式 + 歌词区（顶/底可切换）。
- 全局：主题单点定义（首屏与恢复值一致，消除闪白/闪黑）、设置面板（播放 / 界面 / 歌词 / 在线 / 更新 / 关于）、托盘、快捷键、系统媒体控制。
- 模态框：编辑标签（含 CUE 区）、在线匹配、封面放大、转换器、更新。

---

## 6. 未验证 / 待复核项（诚实标注）

以下内容本次未逐行读完或未取得完整证据，标注为待复核，供 v2 设计时按需补查：

1. `ffmpeg_transcoder.rs`（2420 行）内部细节：ffmpeg/ffplay 可执行文件的查找与下载策略、转码缓存目录与清理策略、缓存命中判定。
2. `cue_parser.rs`（379 行）解析规则细节（编码探测、多文件 CUE、REM 字段语义）。
3. `OnlineMatchModal.vue` 与 `musicDataService.ts` 的候选排序/相似度阈值细节。
4. 12 个语言包的键完整性与缺失键回退行为（是否全部覆盖 `t()` 调用）。
5. `commands_cache.rs` 缓存文件的实际落盘位置与容量上限。
6. 文件关联双击打开时的真实行为（前端未见参数解析，需在 v2 明确重做）。
7. `api/music.ts` 依赖的第三方公开 API 的稳定性与合规性（隐私政策文件 `PRIVACY_POLICY.md` 已存在，需复核一致性）。

---

## 7. 给 v2 的最小勾兑清单（可直接作为验收项）

1. 播放内核唯一化：一个 Player 接口覆盖全部格式，进度/音量/播放模式单一状态源。
2. 元数据唯一化：单一解析链路（Rust 侧），前端不再解析音频文件。
3. 歌词：单一优先级链 + 单一缓存，支持逐行滚动与顶/底位置。
4. CUE：单一时间来源（禁用 `title` 内嵌 `::`），修复时长解析缺陷，时长计算改为数值秒。
5. 删除语义明确：删除 = 从曲库移除（并清理收藏/歌单引用），或明确提供"删除文件"（高风险二次确认），文案与实际一致。
6. 主题：单点定义，首屏与恢复值一致。
7. 版本号单一来源（package.json / tauri.conf.json / Cargo.toml 同步，打包命名一致）。
8. 更新闭环：要么完整实现（检查-下载-校验-安装-UI），要么彻底移除（含设置项与未挂载组件）。
9. 均衡器：要么接入音频链路真正生效，要么先不做（不保留"假开关"）。
10. 安全：权限白名单化，移除无用本地 HTTP 服务与未使用的 IPC 命令，CSP 收敛。
11. 补齐桌面播放器应有能力：全局快捷键、系统媒体控制、拖入导入、双击文件打开。
12. 代码组织：拆分单体，单文件 ≤800 行，消除重复实现与死代码（含 4 个空占位 rs）。
