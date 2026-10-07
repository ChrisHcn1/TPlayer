# TPlayer v2 界面与布局重设计规范（DESIGN）

- 文档版本：v1.0 · 2026-09-26
- 适用对象：`E:\TPlayerNext`（TPlayer Next）v2 前端全部界面实现（Vue 3 + TypeScript + Tauri v2）
- 需求基线：`E:\TPlayerNext\docs\FEATURES-INVENTORY.md`（旧版功能域 A~N、重复冲突 D-01~D-21、缺陷 B-01~B-25）
- 文档性质：**界面与布局的单点事实源（Single Source of Truth）**。凡尺寸、颜色、间距、圆角、动效、交互行为，一律以本文档为准；实现与本文冲突时改实现，不改本文。本文档变更须升版本号并在文末登记。
- 阅读顺序建议：§1 信息架构 → §2 布局 → §3 设计规范（token 全集）→ §4 组件 → §5 交互 → §6 无障碍 → §7 自适应 → §8 差异表（验收勾兑）。

---

## 0. 设计目标与原则

### 0.1 本次重设计要解决的问题（全部来自基线清单）

| 问题类别 | 基线证据 | 设计侧对策 |
|---|---|---|
| 单体界面无法维护 | D-18 / B-04（`App.vue` 9752 行，模板+逻辑+样式全在一个文件） | 界面按 视图 / 布局 / 组件 / 面板 四层拆分，每层只依赖 §3 token，不写裸色值与裸尺寸 |
| 主题三处定义不一致导致闪白/闪黑 | D-09 / B-01 | 主题 token 单点定义（§3.1），首屏与持久化值同源；`html` 根属性由单一入口写入 |
| 视觉无规范，样式互相覆盖 | J-01、B-23 | 全量 CSS 变量命名体系 + 组件状态矩阵，禁止组件内自定义颜色 |
| 交互能力缺位（快捷键/拖入/媒体键/骨架屏） | B-15~B-18、B-22、K-07 | §5 交互规则 + §6 快捷键表，把"应有能力"写成规范条目而非可选项 |
| 破坏性操作语义混乱 | B-07 / B-08（删除不落盘、N 次串行弹窗） | §5.5 删除语义与统一确认流；文案与实际行为强一致 |
| 大曲库卡顿 | B-19（搜索纯前端过滤） | §5.7 搜索契约：后端检索 + 分页 + 虚拟滚动，界面侧只做流式渲染 |

### 0.2 设计原则（五条，冲突时的裁决顺序自上而下）

1. **功能不减法**：旧版任一可见功能（A-01~A-18、B-10、C-01~C-04、D-01~D-11、E-01~E-09、F-01~F-12、G-03~G-07、H-01~H-10、I-01~I-02、J-01~J-08、K-01~K-13、L-01~L-06）在 v2 界面中必须有落点；暂不实现的（C-05、L-07、M 域按 §8 结论处理）须在界面中不留"假入口"。
2. **单一事实源**：一份状态只有一个 UI 表达（如音量只由播放条一处控制、主题只由 token 一处定义），消除 D-02/D-06/D-11/B-11 那类"多套状态并存"。
3. **可预测**：同一手势在同一类对象上结果一致（单击=选中并播放、双击=就地展开、右键=动作菜单）；同一视觉元素在同一状态下的观感一致。
4. **渐进呈现**：默认只呈现主路径（播放、检索、浏览），高级能力（批量、标签编辑、在线匹配、转换）通过右键、右面板、模态框进入，不占用一级界面。
5. **键盘优先**：所有核心链路（检索→播放→切歌→调音量→切视图）不依赖鼠标；焦点可见、可达、可恢复（§6）。

### 0.3 与全局目标的映射（本 Agent 只对界面与布局负责）

| 全局目标 | 界面侧贡献 | 不越权部分 |
|---|---|---|
| 完整覆盖旧版功能 | §1 导航 + §8 差异表逐项勾兑 8 功能域 | 功能实现本身（kernel/后端） |
| 消除重复与冲突 | §3 token 单点、§4 组件唯一实现、§5 状态唯一来源 | 代码层去重（服务/命令层） |
| 更简洁清晰 | §2 三区骨架（导航/主区/播放条）+ 一个可选右面板，砍掉旧版控件堆叠 | 工程目录组织 |
| 更流畅 | §5.2 骨架屏与虚拟滚动约定、§3.6 动效时长上限 | 播放内核性能 |
| 更安全稳定 | §5.5 破坏性操作确认流、§8 对 D-17/B-09 的界面侧前置提示 | 权限白名单等后端配置 |

---

## 1. 信息架构（IA）

### 1.1 主导航项清单与功能域对应

导航共 **10 项**，分 3 组；左侧栏自上而下：曲库组 → 工具组 → 底部固定组。

| # | 导航项 | 路由 | 归属功能域 | 旧版对应 | 导航图标 | 说明 |
|---|---|---|---|---|---|---|
| 1 | 全部歌曲 | `/songs` | ② 媒体库 | D-09 | `music-note` | 默认落地视图；承载搜索、排序、虚拟滚动、批量操作、拖入导入 |
| 2 | 最近播放 | `/recent` | ② 媒体库 | A-18（progress 持久化，旧版无视图） | `clock` | **新增入口**：把旧版只用于"恢复上次播放"的数据变成可浏览视图 |
| 3 | 收藏 | `/favorites` | ③ 歌单与收藏 | E-01 | `heart` | 与歌曲行心形按钮同源状态 |
| 4 | 艺术家 | `/artists` → `/artists/:id` | ② 媒体库 | D-07 | `user` | 二级详情复用专辑网格 + 曲目列表 |
| 5 | 专辑 | `/albums` → `/albums/:id` | ② 媒体库 | D-08 | `disc` | 网格 → 详情的标准两跳 |
| 6 | 分轨专辑 | `/cue` → `/cue/:id` | ② 媒体库（G 域） | G-03 | `layers` | 旧版"CUE 专辑"入口保留；详情页按音轨区间展示 |
| 7 | 歌单 | `/playlists` → `/playlists/:id` | ③ 歌单与收藏 | E-02~E-04（**旧版无视图入口**，基线 5.3 明确要求补齐） | `list-music` | 补齐缺口：创建/重命名/删除/排序/移除曲目全在此视图 |
| 8 | 转换器 | `/converter` | ② 媒体库（B-10） | B-10 | `swap` | 工具组；可从曲目右键"转换…"带入待转文件 |
| 9 | 设置 | `/settings`（可 `/settings/:tab`） | ⑥ 外观与体验 + ⑧ 在线服务 | J、L、M | `settings` | 底部固定组；6 个 tab（见 §1.4） |
| 10 | 关于 | `/settings/about` | ⑥ 外观与体验 | H-10（AboutDialog）+ M-06 | — | 从设置进入，不单独占侧栏项；显示版本号单一来源 |

**不占导航项、以常驻区域承载的功能域**：

| 功能域 | 承载方式 | 依据 |
|---|---|---|
| ① 播放内核 | 底部常驻播放条（§2.5） | A-01~A-11 |
| ④ 歌词 | 右侧面板「歌词」页签 + 播放条歌词区（顶/底可切） | F-07~F-09 |
| ⑤ 元数据与封面 | 曲目右键 + 「编辑标签」模态框 + 「在线匹配」模态框 + 封面放大模态框 | H-01~H-09 |
| ⑦ 系统集成 | 自绘标题栏窗口控制 + 托盘 + 系统媒体控制 + 全局快捷键，无独立视图 | K-01~K-13 |
| ⑧ 在线服务 | 设置 > 在线（开关与缓存清理）；更新在 设置 > 更新 | L-01~L-07、M-01~M-09 |

### 1.2 层级结构（一级视图 / 二级详情 / 三级覆盖层）

```
一级（左侧导航直达，主内容区整体切换，导航项高亮）
├─ 全部歌曲 / 最近播放 / 收藏        —— 曲目列表型视图
├─ 艺术家 → 艺术家列表（网格）
├─ 专辑   → 专辑网格
├─ 分轨专辑 → CUE 专辑网格
├─ 歌单   → 歌单网格
├─ 转换器、设置
│
二级（在同一导航项内下钻，主区内容替换，标题栏出现面包屑与返回）
├─ /artists/:id  艺术家详情：头部信息 + 专辑网格 + 全部曲目
├─ /albums/:id   专辑详情：封面 + 元信息 + 曲目表
├─ /cue/:id      CUE 专辑详情：音轨区间表（起止时间列）
└─ /playlists/:id 歌单详情：封面 + 简介 + 曲目表（支持拖拽排序）
│
三级（覆盖层，不改变左侧导航高亮，Esc 关闭）
├─ 「编辑标签」模态框（本地 / 文件名匹配 / 在线匹配 / 歌词 / 封面，5 页签）
├─ 「在线匹配」模态框（由编辑标签内或曲目右键进入）
├─ 「封面放大」模态框（大封面 + 歌词 + 复制文件路径）
├─ 全局搜索浮层（Ctrl+K）
├─ 确认对话框（删除 / 清空 / 覆盖等，统一形态见 §5.5）
└─ Toast（右上角浮层，不阻断操作）
```

**层级判定规则**：需要"看完再回来"→ 二级；需要"处理完就消失"→ 三级模态；只做通知 → Toast。

### 1.3 跨域入口归属（明确唯一归属，避免旧版"D-11 触发无监听"式断裂）

| 跨域入口 | 归属域 | 入口位置 | 目标 | 约束 |
|---|---|---|---|---|
| 全局搜索（`Ctrl+K`） | ② 媒体库 | 标题栏居中搜索框（常驻） | `/search?q=` 结果页，按 曲目 / 专辑 / 艺术家 / 歌单 分组 | 走后端检索（B-19），结果分组顺序固定，每组默认 5 条 + "查看全部" |
| 视图内筛选 | ② 媒体库 | 各列表视图工具条搜索框 | 当前视图内即时过滤 | 仅过滤已加载页；不做跨域召回 |
| 设置 | ⑥ 外观与体验 | 侧栏底部 | `/settings` | 6 tab：播放 / 界面 / 歌词 / 在线 / 更新 / 关于；每个设置项必须有消费者（针对 D-11） |
| 播放队列 | ① 播放内核 | 右面板「队列」页签 | 常驻面板 | 拖拽排序、跳播、清空（清空走确认） |
| 歌词 | ④ 歌词 | 右面板「歌词」页签 + 播放条按钮 | 常驻面板 | 顶/底位置切换（F-08）落在播放条按钮与设置内 |
| 编辑标签 / 在线匹配 / 封面放大 | ⑤ 元数据 | 曲目行右键、专辑详情按钮、播放条封面点击 | 三级模态 | 写入前须显示"将写入哪些字段"摘要 |
| 转换器 | ② 媒体库 | 侧栏工具组 + 曲目右键 | `/converter` | 右键进入时预填待转文件 |
| 更新 | ⑧ 在线服务 | 设置 > 更新 | 三级模态（检查/下载/校验/安装进度） | 若 v2 决定不做更新，须同时移除设置项与入口（见 §8 第 26 条） |
| 主题切换 | ⑥ 外观与体验 | 标题栏右侧快捷按钮 + 设置 > 界面 | 立即生效 | 只改 `<html data-theme>`，不触发重挂载 |

### 1.4 路由清单（可直接落地为 `router` 配置）

| path | name | 视图组件（建议路径） | meta（`nav` 分组 / `titleKey` / `keepAlive`） |
|---|---|---|---|
| `/` | — | 重定向至 `/songs` | — |
| `/songs` | `songs` | `views/SongsView.vue` | `nav=library`、`titleKey=nav.songs`、`keepAlive=true` |
| `/recent` | `recent` | `views/RecentView.vue` | `nav=library`、`keepAlive=true` |
| `/favorites` | `favorites` | `views/FavoritesView.vue` | `nav=library`、`keepAlive=true` |
| `/artists` | `artists` | `views/ArtistsView.vue` | `nav=library`、`keepAlive=true` |
| `/artists/:id` | `artist-detail` | `views/ArtistDetailView.vue` | `nav=library`、`parent=artists`、`keepAlive=false` |
| `/albums` | `albums` | `views/AlbumsView.vue` | `nav=library`、`keepAlive=true` |
| `/albums/:id` | `album-detail` | `views/AlbumDetailView.vue` | `nav=library`、`parent=albums` |
| `/cue` | `cue` | `views/CueAlbumsView.vue` | `nav=library`、`keepAlive=true` |
| `/cue/:id` | `cue-detail` | `views/CueDetailView.vue` | `nav=library`、`parent=cue` |
| `/playlists` | `playlists` | `views/PlaylistsView.vue` | `nav=library`、`keepAlive=true` |
| `/playlists/:id` | `playlist-detail` | `views/PlaylistDetailView.vue` | `nav=library`、`parent=playlists` |
| `/search` | `search` | `views/SearchView.vue` | `nav=library`、`query.q`、`keepAlive=false` |
| `/converter` | `converter` | `views/ConverterView.vue` | `nav=tools` |
| `/settings` | `settings` | `views/SettingsView.vue` | `nav=bottom`、默认 tab=playback |
| `/settings/:tab` | `settings-tab` | 同上 | `tab∈{playback,appearance,lyrics,online,update,about}` |

> 侧栏导航由 `meta.nav` + `meta.titleKey` 驱动生成（对齐现有 `SideBar.vue` 读 `router.meta` 的骨架做法），新增视图无需改侧栏代码。二级路由的 `parent` 用于父导航项高亮与返回目标。

### 1.5 旧版 IA 缺口（必须在 v2 界面中闭合）

| 缺口 | 旧版事实（基线证据） | v2 落点 |
|---|---|---|
| 歌单无独立入口 | 基线 5.3 / E-02 待复核：`currentFilter` 仅 `all/favorites/artists/albums/cue`，创建歌单后只能从"加歌单"弹窗回看 | 侧栏第 7 项 + `/playlists` 与 `/playlists/:id` |
| 收藏无专属视图？ | E-01 有收藏状态，视图筛选含 `favorites` | 保留为一级导航第 3 项（图标 `heart`） |
| "最近播放"不可见 | A-18 只做恢复上次播放 | 一级导航第 2 项，读进度与播放历史 |
| CUE 音轨时长错误 | B-10（`parseFloat("mm:ss")` 误算） | CUE 详情表显式列出 `开始 / 结束 / 时长` 三列，时长由数值秒格式化，杜绝字符串解析 |
| 均衡器为假开关 | C-05 / D-12（`equalizer.rs` 无调用方） | **v2 界面默认不出现均衡器入口**；若内核未接入，则设置与面板中均不得出现该开关（禁止"假开关"） |
| 更新链路断裂 | D-11 / B-06（触发无监听、组件未挂载、命令未调用） | 设置 > 更新 一个入口，其状态机与后端命令一一对应；不做则整条链路移除 |
| 无全局快捷键 | K-07 / B-16（仅 F12） | §6.1 快捷键表 |
| 无拖入导入 | B-17（无 `dragover/drop`） | §5.6 全窗口 DropZone |
| 无系统媒体控制 | B-15 / K-14 | §5.1 与系统 SMTC/MediaSession 的界面回执约定 |

---

## 2. 整体布局方案

### 2.1 窗口骨架（三段网格 + 一个可选面板）

整体为 **5 个区域**：标题栏（TitleBar）、左侧导航（SideBar）、主内容区（Content）、右侧面板（RightPanel，可开合）、底部常驻播放条（PlayerBar）。使用 CSS Grid 实现，右面板关闭时其列宽为 `0`（而非 `display:none`，以便保留宽度记忆与动画）。

```
grid-template-columns: var(--size-sidebar-w) minmax(0, 1fr) var(--size-panel-w);
grid-template-rows:    var(--size-titlebar-h) minmax(0, 1fr) var(--size-playerbar-h);
grid-template-areas:
  "titlebar titlebar titlebar"
  "sidebar  content  panel"
  "player   player   player";
```

```
┌─ TitleBar ──────--size-titlebar-h: 36px──────────────────────────────────────────────────┐
│  可拖拽区（drag region）                                                                  │
├─ SideBar ─240px──┬─ Content ──minmax(0,1fr)──────────────────┬─ RightPanel ─320px─────────┤
│                  │                                            │  (默认关闭，宽度记忆)      │
│  导航分组         │  视图头部（标题 / 工具条）                  │  页签：歌词 | 队列         │
│  底部固定组       │  视图主体（列表 / 网格 / 表单）             │  内容区（虚拟滚动）        │
│  折叠按钮在底部   │  多选操作条（浮动）                        │                            │
│                  │                                            │                            │
├──────────────────┴────────────────────────────────────────────┴────────────────────────────┤
│ PlayerBar ──--size-playerbar-h: 72px─────────────────────────────────────────────────────│
│  [封面] 标题/艺术家   ⏮ ▶ ⏭ 模式   进度条  时间码   音量   歌词/队列开关                   │
└───────────────────────────────────────────────────────────────────────────────────────────┘
```

**硬性规则**

1. 只有 **Content 区**会滚动；TitleBar / SideBar / PlayerBar / RightPanel 各自内部滚动，整窗永不整体滚动（`html, body { overflow: hidden }`）。
2. SideBar 与 RightPanel 是"边框分区"（`border` 而非 `shadow`），PlayerBar 是"抬升区"（`--shadow-elevation-2` + 顶部 1px 边框），保证视觉层级稳定。
3. 任何区域的背景色只用 `--color-bg-*` 语义变量；右侧面板与播放条用 `--color-bg-elevated`，侧栏用 `--color-bg-sidebar`，内容区用 `--color-bg-surface`。

### 2.2 区域规格表

| 区域 | 尺寸 token | 默认值 | 最小 | 最大 | 伸缩规则 | 折叠策略 |
|---|---|---|---|---|---|---|
| 标题栏 | `--size-titlebar-h` | 36px | 36px | 36px | 固定 | 迷你/全屏模式下隐藏或替换为迷你栏 |
| 左侧导航 | `--size-sidebar-w` | 240px | 200px | 280px | 可拖拽（右侧 4px 热区） | 折叠为 64px（仅图标 + Tooltip），快捷键 `Ctrl+B` |
| 主内容区 | — | `1fr` | 640px | ∞ | 自适应剩余宽度 | 不可折叠 |
| 右面板 | `--size-panel-w` | 320px | 280px | 420px | 可拖拽（左侧 4px 热区） | 关闭（列宽 0）/ 打开（默认 320px）；`Ctrl+U` 切歌词、`Ctrl+Shift+U` 切队列 |
| 播放条 | `--size-playerbar-h` | 72px | 72px | 88px（两行） | 固定 | 全屏歌词模式保留；迷你模式改为 120px 窄条 |
| 内容区内边距 | `--space-content-x` / `-y` | 24px / 20px | 16px | 32px | 随断点（§2.8） | — |
| 列表行高 | `--size-row-h` | 48px | 40px（紧凑） | 56px（宽松） | 由密度设置 `density ∈ {compact, regular, comfortable}` 控制 | — |
| 列表表头 | `--size-header-h` | 32px | 32px | 32px | 固定，滚动时 sticky | — |
| 多选操作条 | `--size-selectionbar-h` | 56px | 56px | 56px | 浮动于 Content 底部 +16px | 仅选择模式出现 |

### 2.3 布局模式与折叠策略

| 模式 | 触发 | 表现 | 状态记忆 |
|---|---|---|---|
| **标准模式** | 默认（窗口 ≥ 1280px） | 侧栏 240px 展开；右面板内嵌（可关） | `layout.sidebarWidth` / `layout.panelOpen` / `layout.panelTab` / `layout.panelWidth` |
| **紧凑模式** | 窗口 1024–1279px 或用户手动折叠侧栏 | 侧栏 64px（icon-only，hover 出 Tooltip 与二级气泡）；右面板改为**浮层**（宽 360px + 半透明遮罩，Esc 关闭，不挤压主区）；播放条改两行 88px | 同上；自动折叠状态**不覆盖**用户手动偏好（手动锁定后不再自动切换） |
| **全屏歌词模式** | `Ctrl+L` 或播放条音乐符图标 | 隐藏 SideBar/RightPanel，Content 区替换为全屏歌词（居中，行高 1.9）；背景为封面高斯模糊 + 遮罩；按 Esc 或再次 `Ctrl+L` 退出 | 不记忆 |
| **迷你模式** | 标题栏"迷你"按钮 | 窗口 380×120，无边框置顶小条：封面 64 + 标题 + 播放/上一首/下一首 + 进度条；双击返回标准 | `layout.miniSize` |

**折叠三原则**
1. 侧栏折叠按钮固定在侧栏底部（不在标题栏），与"设置"同级，位置不随窗口变化。
2. 面板开合有 180ms 位移 + 淡入动效；`prefers-reduced-motion` 下改为瞬时切换（§3.6）。
3. 所有折叠状态在下次启动恢复；恢复失败一律回落到标准模式（不得出现"启动即 0 宽侧栏"）。

### 2.4 主视图布局草图（ASCII 线框图）

> 说明：下述线框为**规范图**，实现时列宽/间距取 §2.2 与 §3 的 token；图中文字为示例内容，不代表文案要求（文案走 i18n 键）。

#### 图 2-4-1 曲库视图（列表型，`/songs` `/favorites` `/recent`，右面板打开）

```
┌─ TitleBar ──────────────────────────────────────────────────────────────────────────────────────┐
│ ▣ TPlayer       ⌕ 搜索歌曲 / 专辑 / 艺术家 / 歌单                 ⌘K     ☾ 主题   ⚙   ─  □  ✕     │
├─ SideBar 240 ─────────┬─ Content ─────────────────────────────────────┬─ RightPanel 320 ─────────┤
│ 曲库                   │ 全部歌曲                              1,024 首  │  歌词  │  队列        ⊗     │
│  ♪ 全部歌曲     1,024  │ ┌───────────────────────────────────────────┐ │ ┌──────────────────────┐ │
│  ◷ 最近播放        36  │ │ ⌕ 筛选本视图      [排序 ▾]  [密度 ▾]  ⋮    │ │ │        封面          │ │
│  ♥ 收藏           128  │ ├───────────────────────────────────────────┤ │ │      280×280         │ │
│  ◉ 艺术家          42  │ │ ☐   标题         艺术家    专辑     时长 ⋮ │ │ └──────────────────────┘ │
│  ◍ 专辑            38  │ ├───────────────────────────────────────────┤ │   曲目名称（16/600）      │
│  ≡ 分轨专辑         6  │ │ ☐ ♪ 曲目一       艺术家A   专辑X   03:45   │ │   艺术家 · 专辑（13 次级）│
│  ☰ 歌单             5  │ │ ☐ ▶ 曲目二       艺术家B   专辑Y   04:12   │ │ ══════════════════════   │
│                        │ │ ☐ ♪ 曲目三       艺术家C   专辑Z   05:01   │ │   ♪ 当前高亮歌词行        │
│ 工具                   │ │ ☐ ♪ 曲目四       …                         │ │     歌词行               │
│  ⇄ 转换器              │ │ ☐ ♪ 曲目五       …（虚拟滚动区）           │ │     歌词行               │
│                        │ │ …                                          │ │     歌词行               │
│ ────────────────────   │ └───────────────────────────────────────────┘ │     歌词行               │
│  ⚙ 设置   ☾ 浅色  ⇤ 折叠│                                                │                          │
├────────────────────────┴───────────────────────────────────────────────┴──────────────────────────┤
│ PlayerBar: [封面 48] 曲目名称 / 艺术家    ⏮  ▶  ⏭   ⟳ 顺序   ─────●─────  01:23 / 03:45   🔈 ──●  ☰  ⊕ │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

要点：表头 sticky；行内 `⋮` 与右键等价（§5.4）；播放中行左侧 3px `--color-brand` 指示条 + 标题 500 字重；批量选择时表头左侧出现全选复选框。

#### 图 2-4-2 曲目列表列定义与裁剪优先级

| 列 | 宽度 | 内容 | 裁剪优先级（数字越小越晚裁） |
|---|---|---|---|
| 复选框 | 40px | 选择框（悬停或选择模式显示） | 1 |
| 序号 / 播放态 | 40px | 序号；播放中变 `▶` / 播放态均衡动画 | 1 |
| 标题 | `minmax(200px, 2fr)` | 标题 + （可选）心形、音质角标 | 1（永不裁剪） |
| 艺术家 | `minmax(120px, 1fr)` | 艺术家 | 4 |
| 专辑 | `minmax(140px, 1.2fr)` | 专辑名 | 3 |
| 时长 | 72px | `mm:ss`，等宽数字 | 5（最先裁剪） |
| 操作 | 48px | `⋮` 更多 | 2 |

裁剪顺序：时长 → 专辑 → 艺术家（< 1120px 时依次隐藏，且这些字段仍可在曲目详情/右键"属性"中查看）。

#### 图 2-4-3 专辑/艺术家/歌单/分轨专辑网格（`/albums` `/artists` `/playlists` `/cue`）

```
│ 专辑                                    [排序 ▾]   [⊞ 网格]  [☰ 列表]   38 张                  │
│ ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐                   │
│ │            │ │            │ │            │ │            │ │            │  卡片 168×224     │
│ │   封面     │ │   封面     │ │   封面     │ │   封面     │ │   封面     │  封面 168×168     │
│ │  168×168   │ │            │ │            │ │            │ │            │  圆角 md(8)       │
│ └────────────┘ └────────────┘ └────────────┘ └────────────┘ └────────────┘                   │
│  专辑名称       专辑名称       专辑名称       专辑名称       专辑名称      ← 14/500，两行截断  │
│  艺术家 · 12 首 艺术家 · 8 首  艺术家 · 15 首 艺术家 · 6 首  艺术家 · 9 首  ← 12/次级          │
│                                                                                              │
│ 网格：repeat(auto-fill, minmax(var(--size-card-min), 1fr))  gap: --space-5 (20px)            │
```

- 艺术家网格使用**圆形**头像（`--radius-full`）；专辑/歌单/CUE 使用圆角矩形（`--radius-md`）。
- 卡片悬停：封面 1.02 倍缩放 + 中央浮出 44px 圆形播放按钮（`--color-brand`，白图标）；键盘 `Tab` 聚焦后同样出现，`Enter` 播放，`Space` 选中。

#### 图 2-4-4 详情页统一模板（`/albums/:id` `/artists/:id` `/playlists/:id`）

```
│ ← 返回                                                                              [⋮ 更多]  │
│ ┌──────────┐   专辑名称（--font-xl / 600）                                                   │
│ │          │   艺术家 · 2024 · 12 首 · 48:21                                                │
│ │  封面    │   [ ▶ 播放全部 ]  [ ⊕ 加入队列 ]  [ ♥ 收藏 ]  [ ⇄ 转换… ]                       │
│ │ 200×200  │                                                                                │
│ └──────────┘                                                                                │
│ ══════════════════════════════════════════════════════════════════════════════════════════  │
│ #   标题            艺术家      专辑        时长    ⋮                                        │
│ 1   ♪ 曲目一        艺术家A     专辑X       03:45                                          │
│ 2   ♪ 曲目二        艺术家B     专辑Y       04:12                                          │
```

- 头部为 "封面 + 信息 + 主操作" 两栏；封面 200px（歌单/CUE 同）。
- 歌单详情：曲目表首列前增加拖拽手柄 `⠿`（§5.6 拖拽排序），表尾允许直接拖入文件追加。
- CUE 详情：时间列扩展为 `开始 / 结束 / 时长` 三列（对齐 B-10 修复要求，时长一律由数值秒格式化）。
- 艺术家详情：头部为圆形头像（160px）+ 简介，下方先"专辑网格"，再"全部曲目"（分区标题 + 分区内滚动）。

#### 图 2-4-5 设置视图（`/settings/:tab`）

```
│ 设置                                                                                          │
│ ┌── Tab 列表 180px ──┬──────────────────────────────────────────────────────────────────┐     │
│ │ 播放               │  播放                                                                   │     │
│ │ 界面               │  ┌────────────────────────────────────────────────────────────────┐  │     │
│ │ 歌词               │  │ 自动播放下一首                          [ ●——— ] 开              │  │     │
│ │ 在线               │  │ 交叉淡入淡出                            [ ———○ ] 关              │  │     │
│ │ 更新               │  │   淡入淡出时长                          1.0s  [ ──●── ]        │  │     │
│ │ 关于               │  │ 播放模式                                [ 顺序 ▾ ]              │  │     │
│ │                    │  │ 音乐目录                                D:\Music      [ 选择… ] │  │     │
│ │                    │  └────────────────────────────────────────────────────────────────┘  │     │
│ │                    │  （表单区 max-width 880px，行高 56px，行间分割线 --color-border-subtle）│     │
```

- 每个分组用**卡片**（`--color-bg-elevated` + `--radius-lg` + `--shadow-elevation-1`）承载，标题 16/600。
- 设置项行结构：`标签（14/400） + 说明（12/次级，可选） | 控件（右对齐，宽 220px）`。
- 每个开关旁必须能看到"当前状态"文字（开/关/数值），不使用纯图标状态（无障碍 §6.4）。
- tab 切换不改 URL 之外的任何状态；直接访问 `/settings/online` 必须直达该 tab。

#### 图 2-4-6 转换器（`/converter`）

```
│ 音频转换                                                                                      │
│ ┌─ 待转文件卡片 ────────────────────────────────────────────────────────────────────────┐    │
│ │  拖入文件到此处，或 [ 选择文件 ] [ 选择文件夹 ]                     已选 3 个文件        │    │
│ │  ───────────────────────────────────────────────────────────────────────────────────  │    │
│ │  ♪ 曲目一.mp3      MP3 · 8.2 MB · 03:45                                     ✕          │    │
│ └───────────────────────────────────────────────────────────────────────────────────────┘    │
│ ┌─ 输出设置 ────────────────────────────────────────────────────────────────────────────┐    │
│ │  输出格式 [ FLAC ▾ ]   采样率 [ 保持原样 ▾ ]   位深 [ 保持原样 ▾ ]                     │    │
│ │  输出目录 [ D:\Music\Converted            ] [ 选择… ]   [ ⇄ 开始转换 ]                  │    │
│ └───────────────────────────────────────────────────────────────────────────────────────┘    │
│ ┌─ 任务队列 ────────────────────────────────────────────────────────────────────────────┐    │
│ │  文件            状态          进度          输出                                        │    │
│ │  曲目一.mp3      转换中        ▓▓▓▓░░░ 62%   D:\Music\Converted\曲目一.flac             │    │
│ │  曲目二.flac     已完成        100%          D:\Music\Converted\曲目二.flac             │    │
│ │  曲目三.ape      排队中        —             —                                          │    │
│ └───────────────────────────────────────────────────────────────────────────────────────┘    │
```

#### 图 2-4-7 全局搜索（`Ctrl+K` 浮层 + `/search` 结果页）

浮层（浮于内容之上，居中，宽 640px，顶部对齐 `--space-12`）：

```
        ┌─ 全局搜索 ─────────────────────────────────────────────────┐
        │ ⌕ 请输入关键词…                                             │
        ├────────────────────────────────────────────────────────────┤
        │ 曲目                                                        │
        │   ♪ 曲目一           艺术家A · 专辑X                        │
        │   ♪ 曲目二           艺术家B · 专辑Y                        │
        │ 专辑                                                        │
        │   ▣ 专辑X            艺术家A · 12 首                        │
        │ 艺术家                                                      │
        │   ◉ 艺术家A           3 张专辑                              │
        ├────────────────────────────────────────────────────────────┤
        │ ↑↓ 选择   ⏎ 打开   Tab 切换分组   Esc 关闭                  │
        └────────────────────────────────────────────────────────────┘
```

浮层与结果页共用同一套分组组件；浮层内 `⏎` 直接播放（曲目）或跳转（专辑/艺术家/歌单）。

#### 图 2-4-8 空状态 / 加载态 / 无结果态

```
空状态（曲库为空）                         加载态（骨架屏）              无结果态
┌──────────────────────────────┐    ┌──────────────────────────┐   ┌──────────────────────┐
│                              │    │ ▢ ▬▬▬▬▬▬▬▬▬▬▬▬▬▬        │   │      ⌕               │
│           ⌕  ♪              │    │ ▢ ▬▬▬▬▬▬▬▬▬▬▬           │   │  未找到相关曲目      │
│                              │    │ ▢ ▬▬▬▬▬▬▬▬▬▬▬▬▬          │   │  试试更短的关键词     │
│  还没有音乐                  │    │ ▢ ▬▬▬▬▬▬▬▬▬              │   │  [ 清除筛选 ]        │
│  添加音乐文件夹，或把文件     │    │ ▢ ▬▬▬▬▬▬▬▬▬▬▬▬▬▬        │   │                      │
│  拖到这里                    │    └──────────────────────────┘   └──────────────────────┘
│  [ 选择文件夹 ]  [ 导入文件 ]│    骨架行高 = --size-row-h，     （图标 48px --color-text-tertiary）
│  ┈┈ 也可以将文件拖入窗口 ┈┈  │    呼吸动效 1.2s（reduced-motion 下静止）
└──────────────────────────────┘
```

统一空状态结构：`图标(48px) + 标题(--font-md/600) + 说明(--font-sm/次要) + 主按钮 + 次按钮(可选) + 拖入提示(可选)`；容器垂直居中，最大宽 360px。

### 2.5 播放条布局（两种形态）

**A. 单行三段式（≥1120px，高 72px）**

```
│ [封面 48]  曲目名称            ⏮   ▶   ⏭    ⟳ 顺序    ─────●─────   01:23/03:45    🔈 ──●   ☰  ⊕ │
│  └─左区 1fr(≥200)          └─中区 固定 320(居中) ──┘   └─ 进度 1fr ─┘  └─ 时间 96 ─┘  └右区 auto┘ │
```

- 左区：封面（48×48，`--radius-sm`，可点击放大）+ 标题（14/500，单行省略）+ 艺术家（12/次级）+ 心形按钮（播放条内仅收藏）。
- 中区：上一首 / 播放暂停（40×40 主按钮）/ 下一首 + 播放模式（顺序 / 随机 / 单曲循环 / 列表循环），模式按钮长按或右键展开选择菜单（不再"三态轮换"，消除 A-02 的不可见状态）。
- 进度条：轨道高 4px，悬停/聚焦时 6px；已播部分 `--color-brand`；缓冲部分 `--color-brand-subtle`；拖拽时显示浮动时间气泡（跟随指针，`--color-bg-elevated` + `--shadow-elevation-3`）。
- 右区：音量（图标 + 100px 滑块，可展开；右键静音）、歌词按钮、队列按钮。
- 时间码使用等宽数字（`font-variant-numeric: tabular-nums`），宽度固定 96px，避免跳动（旧版 B-14 抖动问题的界面侧约束）。

**B. 两行式（1024–1119px，高 88px）**

```
│ [封面 48] 曲目名称 / 艺术家                                       ⏮  ▶  ⏭  ⟳   🔈  ☰  ⊕  │
│  ───────────────────────●─────────────────────────  01:23 / 03:45                          │
```

### 2.6 右面板（歌词 / 队列）

```
│  歌词  │  队列                                        ⊗   │   ← 页签高 40px，选中页签底部 2px --color-brand
│ ┌──────────────────────────────────────────────────────┐ │
│ │                  封面缩略图（240）                    │ │   ← 点击封面区可进入全屏歌词模式
│ └──────────────────────────────────────────────────────┘ │
│   曲目名称（16/600，两行截断）                             │
│   艺术家 · 专辑（12/次级）                                 │
│ ═══════════════════════════════════════════════════════  │
│      歌词行（距当前行 ≥3 行时 13/次要）                    │
│      歌词行                                              │
│   ♪  当前高亮行（16/600，--color-brand，整行居中）         │
│      歌词行                                              │
│      歌词行                                              │
│  [ 逐字 ]  [ 翻译 ]  [ 底/顶 ]      ← 底部工具条（可选开关）│
```

- 歌词滚动使用 `scroll-behavior: smooth`（reduced-motion 下改为直接定位），当前行始终位于面板垂直 **40%** 位置。
- 无歌词态：显示"未找到歌词" + `[ 在线匹配 ]` 按钮（`F-01` 优先级链失败后的唯一手动出口）。
- 队列页签：当前曲目行 `--color-bg-selected` + 左侧 3px 指示条；支持拖拽排序；表尾 `[ 清空队列 ]`（走确认对话框）。
- 歌词位置"顶/底"：`bottom`（面板内）与 `top`（播放条上方横条，高 56px）两态；顶态时队列页签不可用（互斥提示）。

### 2.7 多选操作条（SelectionBar）

```
                                    ┌──────────────────────────────────────────────┐
                                    │  已选 12 首   [ ▶ 播放 ]  [ ⊕ 加入歌单 ]  [ ⇄ 转换 ]  [ 🗑 删除 ]  [ ✕ 取消 ] │
                                    └──────────────────────────────────────────────┘
        ↑ 浮动于 Content 区底部 +16px，宽 = Content 宽 - 48px，高 56，--radius-lg，--shadow-elevation-3
```

- 进入方式：行内复选框 / `Ctrl+A` / 右键"多选" / 行首悬停出现复选框后 Shift 连选（§5.5）。
- 删除按钮走 §5.5 统一确认流（**一次**确认，替代旧版 B-08 的 N 次串行弹窗）。

### 2.8 断点与自适应网格

| 档位 | 窗口宽度 | 侧栏 | 右面板 | 内容内边距 | 网格最小卡宽 | 播放条 | 列裁剪 |
|---|---|---|---|---|---|---|---|
| Wide | ≥ 1600px | 240px（可拖至 280） | 内嵌 320–420px | 32 / 24 | 176px | 单行 72 | 全列 |
| Regular | 1280–1599px | 240px | 内嵌 320px | 24 / 20 | 168px | 单行 72 | 全列 |
| Compact | 1120–1279px | 64px（自动） | 浮层 360px | 20 / 16 | 160px | 单行 72 | 隐藏「时长」 |
| Narrow | 1024–1119px | 64px | 浮层 360px | 16 / 16 | 148px | 两行 88 | 隐藏「时长」「专辑」 |

- **最小窗口尺寸：1024 × 640**（逻辑像素，Tauri `minWidth/minHeight`）。低于此值布局不保证可用，禁止发布更小下限。
- 断点判定一律用 **ResizeObserver 监听根容器宽度**（而非 `window.innerWidth`），保证"两栏布局/多显示器 DPI 变化"下行为一致。
- 文本放大（系统缩放至 200%）时：允许侧栏自动升为 64px、播放条升为两行，**功能入口不得消失**（§6.5）。
- Content 区在超宽屏（≥ 2200px）下的列表型视图设 `max-width: 1440px` 居中，避免行列拉得过长；网格型视图不受此限制。

---

## 3. 设计规范（Design Tokens）

### 3.0 落地方式（强制）

1. 全部 token 定义在**唯一文件** `src/styles/theme.css` 中；`main.ts` 只引入一次；组件内 **禁止** 出现十六进制色值、`rgb()/rgba()` 字面量、魔法尺寸（除 1px 描边、`0`、`100%`、`50%`）。
2. 主题切换只改 `<html data-theme="dark|light">`，**不改** class 名、不重挂载组件、不重新请求数据（消除 D-09/B-01 的"三处默认值不一致"）。
3. 首屏防闪：`index.html` 内联脚本（唯一入口）按 `localStorage.layout.theme` → 缺省 `dark` 写入 `data-theme`，并在同一个 `<html>` 上写 `color-scheme`；CSS 的 `:root` 即 dark 值，保证"无 JS 也得到正确首帧"。
4. 命名规则：`--color-{语义}-{变体}`（颜色）、`--font-{属性}-{档}`、`--space-{n}`（4px 基数）、`--radius-{档}`、`--shadow-elevation-{n}`、`--duration-{档}` / `--ease-{类型}`、`--size-{区域}`、`--z-{层}`。
5. 语义优先：组件只允许引用语义 token（如 `--color-bg-elevated`），**禁止**引用原始调色板变量（如 `--palette-gray-900`），以保证换肤可整体重映射。
6. 密度切换通过 `<html data-density="compact|regular|comfortable">` 覆写 `--size-row-h` 等少数尺寸 token，组件不得自行判断密度。

### 3.1 色彩体系

#### 3.1.1 分层模型

| 层 | token 前缀 | 用途 | 禁止用途 |
|---|---|---|---|
| 基底 | `--color-bg-base` | 窗口最底（可见于圆角外沿） | 不用于卡片 |
| 表面 | `--color-bg-surface` | 主内容区 | 不用于浮层 |
| 抬升 | `--color-bg-elevated` | 播放条、面板、卡片、模态框、下拉菜单 | 不用于整页背景 |
| 侧栏 | `--color-bg-sidebar` | 左侧导航 | — |
| 遮罩 | `--color-overlay` / `-strong` | 模态框遮罩 / 全屏歌词遮罩 | — |
| 交互态 | `--color-bg-hover` / `-active` / `-selected` | 悬停 / 按下 / 选中 | 不用作静态底色 |
| 描边 | `--color-border-subtle` / `-border` / `-border-strong` | 内部分割线 / 常规边框 / 强调边框（输入框聚焦前） | — |
| 文本 | `--color-text-primary` / `-secondary` / `-tertiary` / `-disabled` / `-inverse` | 主/次/辅助/禁用/反色 | 正文不得用 tertiary |
| 品牌 | `--color-brand`（文字/图标）、`--color-brand-solid`（填充）、`--color-on-brand`、`--color-brand-subtle` | 主操作、选中、进度、链接 | 不用于大面积背景 |
| 状态 | `--color-success|warning|danger|info` + `-bg` | 提示、错误、危险按钮 | 不用作装饰 |
| 焦点 | `--color-focus-ring` | 焦点环 | — |

#### 3.1.2 对比度要求（WCAG 2.1，计算值）

| 配对 | 暗色比值 | 亮色比值 | 结论 |
|---|---|---|---|
| `text-primary` on `/surface` | ≈ 14.5:1 | ≈ 16.5:1 | AA/AAA 正文 |
| `text-secondary` on `/surface` | ≈ 6.6:1 | ≈ 6.1:1 | AA 正文可用 |
| `text-tertiary` on `/surface` | ≈ 3.6:1 | ≈ 3.4:1 | **仅** 12px 以上辅助文字/图标，不得承载必要信息 |
| `text-disabled` on `/surface` | ≈ 2.4:1 | ≈ 2.3:1 | 仅禁用态（禁用态必须有非颜色提示，§6.3） |
| `on-brand` on `brand-solid` | ≈ 4.8:1 | ≈ 4.6:1 | AA 正文（品牌实心按钮白字达标） |
| `brand`（文字） on `/surface` | ≈ 6.3:1 | ≈ 5.8:1 | AA 链接/强调 |
| 进度条已播段 vs 轨道 | ≥ 3:1 | ≥ 3:1 | 非文本 UI 达标 |
| 焦点环 vs 相邻色 | ≥ 3:1 | ≥ 3:1 | 非文本 UI 达标 |

#### 3.1.3 完整变量定义（可直接复制）

```css
/* ==========================================================================
   theme.css —— TPlayer v2 design tokens（唯一定义处）
   ========================================================================== */

:root {
  /* ---- 原始调色板（仅供本文件内引用，组件禁止直接使用） ---- */
  --palette-neutral-950: #0f1115;
  --palette-neutral-900: #121419;
  --palette-neutral-850: #16181d;
  --palette-neutral-800: #1e2128;
  --palette-neutral-700: #2a2e37;
  --palette-neutral-600: #3a3f4b;
  --palette-neutral-400: #6b7280;
  --palette-neutral-300: #9aa0aa;
  --palette-neutral-100: #e8eaed;
  --palette-white:        #ffffff;

  /* ---- 颜色：暗色主题（默认，:root 即暗色） ---- */
  --color-bg-base:        var(--palette-neutral-950);
  --color-bg-surface:     var(--palette-neutral-850);
  --color-bg-elevated:    var(--palette-neutral-800);
  --color-bg-sidebar:     var(--palette-neutral-900);
  --color-bg-input:       oklch(from var(--palette-neutral-950) l c h / 0.55);
  --color-bg-hover:       rgba(255, 255, 255, 0.06);
  --color-bg-active:      rgba(255, 255, 255, 0.10);
  --color-bg-selected:    rgba(122, 149, 255, 0.16);
  --color-bg-scrim:       rgba(0, 0, 0, 0.45);
  --color-overlay:        rgba(15, 17, 21, 0.72);
  --color-overlay-strong: rgba(15, 17, 21, 0.88);

  --color-border-subtle:  rgba(255, 255, 255, 0.06);
  --color-border:         var(--palette-neutral-700);
  --color-border-strong:  var(--palette-neutral-600);

  --color-text-primary:   var(--palette-neutral-100);
  --color-text-secondary: var(--palette-neutral-300);
  --color-text-tertiary:  var(--palette-neutral-400);
  --color-text-disabled:  #4b515c;
  --color-text-inverse:   var(--palette-neutral-950);

  --color-brand:          #7a95ff;   /* 文字 / 图标 / 描边 */
  --color-brand-solid:    #4a69e0;   /* 填充背景（配 on-brand 白字） */
  --color-brand-hover:    #5876e8;
  --color-brand-active:   #3e5acb;
  --color-brand-subtle:   rgba(122, 149, 255, 0.16);
  --color-on-brand:       var(--palette-white);

  --color-success:        #3dbe8b;
  --color-success-bg:     rgba(61, 190, 139, 0.14);
  --color-warning:        #e0a93b;
  --color-warning-bg:     rgba(224, 169, 59, 0.14);
  --color-danger:         #f2555a;
  --color-danger-bg:      rgba(242, 85, 90, 0.14);
  --color-info:           #4b9fe1;
  --color-info-bg:        rgba(75, 159, 225, 0.14);

  --color-focus-ring:     #7a95ff;
  --color-scrollbar-thumb:        rgba(255, 255, 255, 0.16);
  --color-scrollbar-thumb-hover:  rgba(255, 255, 255, 0.28);

  --color-cover-placeholder: linear-gradient(135deg, #232830 0%, #191c22 100%);
  --color-blur-backdrop:  rgba(22, 24, 29, 0.72);

  color-scheme: dark;

  /* ---- 字体 ---- */
  --font-family-base: -apple-system, BlinkMacSystemFont, "Segoe UI", "Microsoft YaHei",
                      "PingFang SC", "Noto Sans SC", Roboto, "Helvetica Neue", Arial, sans-serif;
  --font-family-mono: "JetBrains Mono", "Cascadia Mono", Consolas, "Courier New", monospace;
  --font-weight-regular: 400;
  --font-weight-medium:  500;
  --font-weight-semibold: 600;
  --font-weight-bold:    700;

  --font-size-2xs: 11px;  --line-height-2xs: 16px;   /* 角标、极小辅助 */
  --font-size-xs:  12px;  --line-height-xs:  18px;   /* 时间码、次级说明 */
  --font-size-sm:  13px;  --line-height-sm:  20px;   /* 列表次行、表单说明 */
  --font-size-base:14px;  --line-height-base:22px;   /* 正文、列表主行、按钮 */
  --font-size-md:  16px;  --line-height-md:  24px;   /* 区块标题、面板主标题 */
  --font-size-lg:  20px;  --line-height-lg:  28px;   /* 详情页标题 */
  --font-size-xl:  24px;  --line-height-xl:  32px;   /* 视图标题 */
  --font-size-2xl: 30px;  --line-height-2xl: 38px;   /* 空状态/欢迎标题（少用） */
  --font-letter-spacing-tight: -0.01em;              /* ≥20px 标题 */

  /* ---- 间距（4px 基数） ---- */
  --space-0:  0;
  --space-1:  4px;
  --space-2:  8px;
  --space-3:  12px;
  --space-4:  16px;
  --space-5:  20px;
  --space-6:  24px;
  --space-8:  32px;
  --space-10: 40px;
  --space-12: 48px;
  --space-16: 64px;

  /* ---- 圆角 ---- */
  --radius-xs:   4px;
  --radius-sm:   6px;
  --radius-md:   8px;
  --radius-lg:   12px;
  --radius-xl:   16px;
  --radius-full: 999px;

  /* ---- 阴影层级（暗色：以描边 + 柔和投影共同表达层级） ---- */
  --shadow-elevation-0: none;
  --shadow-elevation-1: 0 1px 2px rgba(0, 0, 0, 0.24);
  --shadow-elevation-2: 0 -1px 0 rgba(255, 255, 255, 0.04), 0 2px 8px rgba(0, 0, 0, 0.28);
  --shadow-elevation-3: 0 8px 24px rgba(0, 0, 0, 0.36);
  --shadow-elevation-4: 0 16px 48px rgba(0, 0, 0, 0.48);
  --shadow-focus: 0 0 0 2px var(--color-bg-surface), 0 0 0 4px var(--color-focus-ring);

  /* ---- 动效 ---- */
  --duration-instant: 80ms;
  --duration-fast:    120ms;
  --duration-base:    180ms;
  --duration-slow:    240ms;
  --duration-slower:  320ms;
  --ease-standard:   cubic-bezier(0.2, 0, 0, 1);
  --ease-decelerate: cubic-bezier(0, 0, 0.2, 1);
  --ease-accelerate: cubic-bezier(0.4, 0, 1, 1);
  --ease-emphasized: cubic-bezier(0.2, 0, 0, 1);
  --transition-colors: color var(--duration-fast) var(--ease-standard),
                       background-color var(--duration-fast) var(--ease-standard),
                       border-color var(--duration-fast) var(--ease-standard);
  --transition-transform: transform var(--duration-base) var(--ease-emphasized);
  --transition-opacity: opacity var(--duration-fast) var(--ease-standard);

  /* ---- 尺寸 ---- */
  --size-titlebar-h:     36px;
  --size-sidebar-w:      240px;
  --size-sidebar-w-min:  200px;
  --size-sidebar-w-max:  280px;
  --size-sidebar-collapsed: 64px;
  --size-panel-w:        320px;
  --size-panel-w-min:    280px;
  --size-panel-w-max:    420px;
  --size-playerbar-h:    72px;
  --size-row-h:          48px;
  --size-header-h:       32px;
  --size-toolbar-h:      48px;
  --size-selectionbar-h: 56px;
  --size-card-min:       168px;
  --size-cover-sm:       40px;
  --size-cover-md:       48px;
  --size-cover-lg:       200px;
  --size-icon-sm:        16px;
  --size-icon-md:        20px;
  --size-icon-lg:        24px;
  --size-control-h:      32px;   /* 输入框/按钮常规高 */
  --size-control-h-lg:   40px;   /* 主按钮/搜索框 */
  --size-scrollbar:      10px;

  /* ---- 层叠 ---- */
  --z-base: 0;
  --z-sticky: 10;
  --z-panel: 20;
  --z-playerbar: 30;
  --z-titlebar: 40;
  --z-dropdown: 100;
  --z-tooltip: 110;
  --z-modal-backdrop: 200;
  --z-modal: 210;
  --z-toast: 300;
}

/* ==========================================================================
   亮色主题：仅重映射语义 token，不新增组件级规则
   ========================================================================== */
[data-theme="light"] {
  --color-bg-base:        #f0f1f4;
  --color-bg-surface:     #ffffff;
  --color-bg-elevated:    #ffffff;
  --color-bg-sidebar:     #eaecf0;
  --color-bg-input:       #ffffff;
  --color-bg-hover:       rgba(15, 17, 21, 0.05);
  --color-bg-active:      rgba(15, 17, 21, 0.09);
  --color-bg-selected:    rgba(47, 111, 235, 0.10);
  --color-bg-scrim:       rgba(15, 17, 21, 0.28);
  --color-overlay:        rgba(15, 17, 21, 0.36);
  --color-overlay-strong: rgba(15, 17, 21, 0.56);

  --color-border-subtle:  rgba(15, 17, 21, 0.06);
  --color-border:         #d8dbe0;
  --color-border-strong:  #bfc4cc;

  --color-text-primary:   #1b1f26;
  --color-text-secondary: #5b6270;
  --color-text-tertiary:  #868d99;
  --color-text-disabled:  #a8aeb8;
  --color-text-inverse:   #ffffff;

  --color-brand:          #1f5bd8;
  --color-brand-solid:    #2f6feb;
  --color-brand-hover:    #2560d8;
  --color-brand-active:   #1b4fbf;
  --color-brand-subtle:   rgba(47, 111, 235, 0.10);
  --color-on-brand:       #ffffff;

  --color-success:        #12734e;  --color-success-bg: rgba(31, 157, 107, 0.12);
  --color-warning:        #8a5a00;  --color-warning-bg: rgba(183, 122, 15, 0.12);
  --color-danger:         #c22b33;  --color-danger-bg:  rgba(217, 54, 62, 0.10);
  --color-info:           #14639e;  --color-info-bg:    rgba(31, 123, 196, 0.10);

  --color-focus-ring:     #1f5bd8;
  --color-scrollbar-thumb:        rgba(15, 17, 21, 0.18);
  --color-scrollbar-thumb-hover:  rgba(15, 17, 21, 0.32);

  --color-cover-placeholder: linear-gradient(135deg, #e6e8ec 0%, #d9dce2 100%);
  --color-blur-backdrop:  rgba(255, 255, 255, 0.72);

  --shadow-elevation-1: 0 1px 2px rgba(15, 17, 21, 0.06);
  --shadow-elevation-2: 0 -1px 0 rgba(15, 17, 21, 0.06), 0 2px 8px rgba(15, 17, 21, 0.08);
  --shadow-elevation-3: 0 8px 24px rgba(15, 17, 21, 0.12);
  --shadow-elevation-4: 0 16px 48px rgba(15, 17, 21, 0.18);

  color-scheme: light;
}

/* ---- 密度档 ---- */
[data-density="compact"]     { --size-row-h: 40px; --size-toolbar-h: 44px; --size-cover-md: 40px; }
[data-density="regular"]     { --size-row-h: 48px; --size-toolbar-h: 48px; --size-cover-md: 48px; }
[data-density="comfortable"] { --size-row-h: 56px; --size-toolbar-h: 52px; --size-cover-md: 56px; }

/* ---- 断点覆写（由根容器 class/resize 观察写入，与 §2.8 一致） ---- */
[data-layout="compact"], [data-layout="narrow"] {
  --size-sidebar-w: var(--size-sidebar-collapsed);
  --size-playerbar-h: 88px;
  --size-card-min: 148px;
}
[data-layout="wide"] { --size-sidebar-w: 264px; --size-card-min: 176px; }

/* ---- 减弱动效 ---- */
@media (prefers-reduced-motion: reduce) {
  :root {
    --duration-instant: 1ms; --duration-fast: 1ms; --duration-base: 1ms;
    --duration-slow: 1ms;    --duration-slower: 1ms;
    --transition-transform: none; --transition-opacity: none;
  }
}
```

> 注：`--color-bg-input` 中若目标运行时不支持 `oklch(from ...)`，实现时降级为 `rgba(15,17,21,0.55)`（暗）/ `#ffffff`（亮），不得改用其他写法（仅此一处允许降级）。

### 3.2 字体与字号阶梯

```css
body {
  font-family: var(--font-family-base);
  font-size: var(--font-size-base);
  line-height: var(--line-height-base);
  font-weight: var(--font-weight-regular);
  color: var(--color-text-primary);
  -webkit-font-smoothing: antialiased;
  text-rendering: optimizeLegibility;
}
```

| 角色 | 字号/行高 | 字重 | 颜色 | 其他 |
|---|---|---|---|---|
| 视图标题（`全部歌曲`） | `--font-size-xl` / 32 | 600 | primary | `letter-spacing: -0.01em` |
| 详情页标题（专辑名） | `--font-size-lg` / 28 | 600 | primary | 两行截断 |
| 区块标题（`专辑`、设置分组） | `--font-size-md` / 24 | 600 | primary | 与上方间距 `--space-6` |
| 列表主行（曲目名） | `--font-size-base` / 22 | 400（播放中 500） | primary | 单行省略 |
| 列表次行 / 副标题 | `--font-size-sm` / 20 | 400 | secondary | — |
| 面板标题（当前曲目） | `--font-size-md` / 24 | 600 | primary | 两行截断 |
| 歌词当前行 | `--font-size-md` / 24 | 600 | `--color-brand` | 居中 |
| 歌词普通行 | `--font-size-sm` / 20 | 400 | secondary | 距当前行 ≥3 行时降至 tertiary |
| 按钮文字 | `--font-size-base` / 22 | 500 | 依变体 | — |
| 表单标签 | `--font-size-base` / 22 | 400 | primary | 说明用 `--font-size-xs` + secondary |
| 输入框文本 | `--font-size-base` / 22 | 400 | primary | placeholder 用 tertiary |
| 时间码 / 数值 | `--font-size-xs` / 18 | 400 | secondary | `font-variant-numeric: tabular-nums`，固定宽度 |
| 角标 / 计数（`128`、`FLAC`） | `--font-size-2xs` / 16 | 500 | tertiary 或品牌 | 底色 `--color-brand-subtle`，`--radius-xs` |
| 提示 / Toast | `--font-size-base` / 22 | 400 | primary | — |
| 快捷键提示（`⌘K`） | `--font-size-2xs` / 16 | 500 | tertiary | `--font-family-mono` + 1px 边框 |

规则：**最小可读字号 11px**；正文（承载必要信息）不得小于 12px；同一视图内字重不超过 3 种。

### 3.3 间距基数

- 基数 **4px**，所有间距必须取自 `--space-*`。
- 语义映射（推荐用法，避免随意取值）：

| 语义 | 取值 | 场景 |
|---|---|---|
| 图标与文字 | `--space-2` | 按钮内、行内图标 |
| 行内元素之间 | `--space-3` | 播放条控制按钮组 |
| 卡片内部 | `--space-4` | 卡片 padding、列表行左右 padding |
| 卡片/网格间隙 | `--space-5` | 网格 gap |
| 卡片与卡片之间（纵向） | `--space-6` | 表单卡片纵向间距 |
| 区块之间 | `--space-8` | 页头与内容之间 |
| 页面内边距 | `--space-content-x/y`（24/20，见 §2.2） | 内容区 |

- 禁止使用奇数间距（1/3/5px 例外：1px 描边、3px 选中指示条、5px 图标描边容差）。

### 3.4 圆角

| token | 值 | 用途 |
|---|---|---|
| `--radius-xs` | 4px | 角标、复选框、快捷键提示 |
| `--radius-sm` | 6px | 小按钮、输入框、封面小图 |
| `--radius-md` | 8px | 卡片、列表行悬停态、封面中图、下拉菜单 |
| `--radius-lg` | 12px | 面板卡片、模态框、Toast、多选操作条 |
| `--radius-xl` | 16px | 大卡片（专辑网格 card）、空状态容器 |
| `--radius-full` | 999px | 圆形播放按钮、开关滑块、艺术家头像、进度条拇指 |

### 3.5 阴影层级（Elevation）

| token | 用途 | 暗色表达 | 亮色表达 |
|---|---|---|---|
| `--shadow-elevation-0` | 内容区、列表行、侧栏 | 无（靠描边分区） | 同 |
| `--shadow-elevation-1` | 设置卡片、网格卡片静态 | 1px 微投影 | 1px 微投影 |
| `--shadow-elevation-2` | 播放条、sticky 表头 | 向上投影 + 顶部 1px 高光 | 向上投影 |
| `--shadow-elevation-3` | 下拉菜单、右键菜单、多选操作条、时间气泡 | 8/24 | 8/24 |
| `--shadow-elevation-4` | 模态框、全局搜索浮层 | 16/48 | 16/48 |
| `--shadow-focus` | 键盘焦点环（仅 `:focus-visible`） | 双环，外环 `--color-focus-ring` | 同 |

层级一致性规则：**层级越高，投影越大且背景越"抬升"**；同一屏内不超过 3 个不同层级同时出现。

### 3.6 动效

| 场景 | 时长 | 缓动 | 属性 |
|---|---|---|---|
| 颜色/边框/悬停 | `--duration-fast` (120ms) | `--ease-standard` | color / background-color / border-color |
| 卡片封面缩放、图标位移 | `--duration-base` (180ms) | `--ease-emphasized` | transform |
| 面板开合、抽屉式滑入 | `--duration-base` (180ms) | `--ease-decelerate` | width / transform / opacity |
| 下拉菜单、右键菜单出现 | `--duration-fast` (120ms) | `--ease-decelerate` | opacity + translateY(4px→0) |
| 模态框出现 | `--duration-base` (180ms) | `--ease-emphasized` | opacity + scale(0.98→1) |
| Toast 进出 | `--duration-slow` (240ms) | `--ease-decelerate` | opacity + translateX(8px→0) |
| 进度条拖拽/悬停加粗 | `--duration-instant` (80ms) | `--ease-standard` | height / width |
| 骨架屏呼吸 | 1.2s 循环 | `ease-in-out` | opacity 0.4↔0.7 |
| 视图切换（路由） | `--duration-base` (180ms) | `--ease-standard` | opacity（仅内容区，不做大面积位移） |

约束：
1. 单次交互动效 ≤ **320ms**；超过 200ms 的动效必须可在设置 > 界面中关闭（`reducedMotion` 手动开关）。
2. `prefers-reduced-motion: reduce` 或用户关闭动效时：所有时长降为 1ms、`transform` 动效取消、骨架屏静止、歌词滚动改为瞬时定位。
3. **禁止**动效阻塞交互：动画期间元素必须立即可点击（不用 `pointer-events: none` 挡位）。
4. 不使用弹跳（bounce）与超过 1.05 的缩放，避免"廉价感"与可访问性问题。

### 3.7 图标规范

| 项 | 规范 |
|---|---|
| 网格 | 24×24，内容区 20×20 安全边距 2px |
| 描边 | 1.5px，`stroke-linecap: round`，`stroke-linejoin: round`，`fill: none` |
| 颜色 | `currentColor`（永不在 SVG 内写死颜色） |
| 尺寸档 | `--size-icon-sm` 16（行内、次级按钮）/ `md` 20（常规按钮、导航）/ `lg` 24（工具条、空状态用 32 例外）/ 48（空状态） |
| 状态变体 | 播放/收藏/静音等需要"填充态"的图标必须有 filled 变体（如 `heart` ↔ `heart-filled`），填充态用 `--color-brand` |
| 命名 | `icon-{语义}.svg`，语义名与 i18n key 同源，便于替换 |
| 对齐 | 图标与文字基线对齐；图标按钮尺寸不小于 32×32（触达区），间距 `--space-2` |
| 禁止 | 不使用 emoji 作为功能图标；不使用图片格式图标（须为内联/雪碧 SVG） |

**必需图标清单（按区域）**

| 区域 | 图标 |
|---|---|
| 侧栏导航 | `music-note`、`clock`、`heart`、`heart-filled`、`user`、`disc`、`layers`、`list-music`、`swap`、`settings`、`chevron-left`（折叠） |
| 标题栏/窗口 | `logo`、`search`、`sun`、`moon`、`minus`、`square`、`close`、`pin`（置顶/迷你） |
| 传输控制 | `previous`、`play`、`pause`、`next`、`repeat`、`repeat-one`、`shuffle`、`list-order` |
| 播放条右区 | `volume-high`、`volume-low`、`volume-mute`、`lyrics`、`queue` |
| 列表/表格 | `more-vertical`、`sort`、`filter`、`density`、`grid`、`list`、`checkbox`、`checkbox-checked`、`drag-handle`、`chevron-right`、`back` |
| 动作 | `plus`、`trash`、`edit`、`copy`、`external-link`、`download`、`refresh`、`check`、`alert-triangle`、`info`、`x-circle` |
| 状态/空态 | `music-off`（空曲库）、`search-empty`、`cloud-offline`（在线不可用）、`loader`（spinner） |

### 3.8 层叠顺序（z-index）

| 层 | token | 说明 |
|---|---|---|
| 常规内容 | `--z-base` | 列表、网格 |
| sticky 表头 / 工具条 | `--z-sticky` | 列表内滚动时吸附 |
| 右面板 | `--z-panel` | 紧凑模式下浮层时同值 |
| 播放条 | `--z-playerbar` | 永远在内容之上 |
| 标题栏 | `--z-titlebar` | 与播放条同级语义，取更高值 |
| 下拉菜单 / 右键菜单 | `--z-dropdown` | 由触发元素计算出现位置 |
| Tooltip | `--z-tooltip` | 高于菜单 |
| 模态遮罩 / 模态框 | `--z-modal-backdrop` / `--z-modal` | 模态内不再出现菜单（如需则用内联选择器） |
| Toast | `--z-toast` | 最高，任何情况可见 |

### 3.9 文本、滚动与状态通用规则

1. **截断**：单行用 `overflow: hidden; text-overflow: ellipsis; white-space: nowrap;`；两行用 `-webkit-line-clamp: 2`。可选中的长文本（如文件路径）用 `--font-family-mono` + 横向滚动，禁止省略关键尾部。
2. **滚动条**：宽 `--size-scrollbar`(10px)，轨道透明，滑块 `--color-scrollbar-thumb`、悬停 `--color-scrollbar-thumb-hover`、`--radius-full`；滚动条区域不遮挡内容（`scrollbar-gutter: stable`）。
3. **焦点环**：仅键盘触发（`:focus-visible`）显示 `--shadow-focus`；鼠标点击不显示焦点环。
4. **禁用态**：不透明度 0.45 + `cursor: not-allowed`，并且必须能通过 Tooltip/说明文字得知禁用原因（不得只用"变灰"表达，§6.3）。
5. **加载态**：控件进入加载态时保留宽度（防布局跳动），图标替换为 `loader`（16px，旋转 0.9s linear 循环）；加载中控件禁用交互。
6. **选中态**：`--color-bg-selected` + 左侧 3px `--color-brand` 指示条（仅列表行；网格卡片用 2px 描边 + 右上角对勾）。
7. **空/错/载三态必须成对实现**：任何"取数据"的视图都要给出 loading / empty / error 三个分支的界面（§2.4 图 2-4-8），禁止只做成功态（针对 B-22）。

---

## 4. 组件规格

### 4.0 通用约定

- **状态词表（全组件统一，不得自造）**：`default`（默认）、`hover`（悬停）、`pressed`（按下）、`focus-visible`（键盘焦点）、`selected`（选中）、`on / off`（可切换件的激活/未激活）、`disabled`（禁用）、`loading`（加载）、`error`（错误）。
- **可达性（对比度 / 命中区）**：任何可点击元素的**命中区不得小于 32×32px**；图标按钮视觉尺寸可 20px，但外层 padding 补足。
- 每个组件必须在 `docs` 注释中标明它引用的 token 范围；**禁止**在组件内使用未在 §3 定义的变量。
- 组件状态切换必须走 `--transition-colors`，不得逐处手写 transition。

### 4.1 Button（按钮）

**结构**：`<button class="btn btn--{variant} btn--{size}">` + 可选前置图标（20px）+ 文本（`--font-size-base/500`）。

| 变体 | 背景 | 文本 | 边框 | 用途 |
|---|---|---|---|---|
| `primary` | `--color-brand-solid` | `--color-on-brand` | 无 | 主操作：播放全部、开始转换、确认 |
| `secondary` | `--color-bg-elevated` | `--color-text-primary` | 1px `--color-border` | 次操作：加入队列、选择文件夹 |
| `ghost` | 透明 | `--color-text-secondary` | 无 | 工具条按钮、面板内操作 |
| `danger` | `--color-danger-bg`（静态）/ `--color-danger`（确认态） | `--color-danger` → `--color-on-brand` | 1px `--color-danger` | 删除类操作 |
| `link` | 透明 | `--color-brand` | 无 | 内联跳转（如"查看全部"） |

| 尺寸 | 高度 | 水平内边距 | 圆角 | 字号 |
|---|---|---|---|---|
| `sm` | 28px | `--space-3` | `--radius-sm` | `--font-size-sm` |
| `md`（默认） | `--size-control-h` 32px | `--space-4` | `--radius-sm` | `--font-size-base` |
| `lg`（主 CTA） | `--size-control-h-lg` 40px | `--space-5` | `--radius-md` | `--font-size-base` |
| `icon` | 32×32（lg 40×40） | — | `--radius-sm` | 图标 20px |

**状态矩阵**

| 状态 | primary | ghost / secondary | danger |
|---|---|---|---|
| default | 见上表 | 见上表 | 见上表 |
| hover | `--color-brand-hover` | 背景 `--color-bg-hover` | 背景 `--color-danger-bg`，文字 `--color-danger` |
| pressed | `--color-brand-active` + `scale(0.98)` | 背景 `--color-bg-active` | 同上 + `scale(0.98)` |
| focus-visible | `--shadow-focus`，外环不裁切 | 同 | 同 |
| disabled | 不透明度 0.45，`cursor: not-allowed`，Tooltip 说明原因 | 同 | 同 |
| loading | 图标位替换 `loader` 旋转 0.9s，宽度不变，禁用点击 | 同 | 同 |

### 4.2 SideBar / NavItem（侧栏导航项）

**结构**：`图标(20) + 文本(--font-size-base) + 右侧计数(Badge，可选)`；高度 36px，左右内边距 `--space-3`，圆角 `--radius-sm`，项间距 `--space-1`。

| 状态 | 背景 | 文本/图标 | 附加 |
|---|---|---|---|
| default | 透明 | `--color-text-secondary` | — |
| hover | `--color-bg-hover` | `--color-text-primary` | 折叠态显示 Tooltip（图标右侧，`--duration-fast` 延迟 400ms） |
| selected（当前路由） | `--color-bg-selected` | `--color-brand` | 左侧 3px `--color-brand` 指示条 |
| pressed | `--color-bg-active` | 同 hover | — |
| focus-visible | 透明 | 不变 | `--shadow-focus` |
| disabled | 透明 | `--color-text-disabled` | Tooltip 说明（如"曲库未添加目录"） |

- 分组标题（`曲库` / `工具`）为 11px/500/tertiary，非可点击元素；分组之间 `--space-2` 间隔 + 1px `--color-border-subtle` 分割。
- 侧栏底部固定：`设置`、主题切换、`折叠`按钮；窗口高度不足（< 560px）时该组自动收起为图标行。

### 4.3 TitleBar / WindowControls（标题栏）

| 元素 | 规格 |
|---|---|
| 高度 | `--size-titlebar-h` 36px；背景 `--color-bg-sidebar`；底部 1px `--color-border-subtle` |
| 拖拽区 | 标题栏除控件外全部为 drag region（`data-tauri-drag-region`）；双击切换最大化 |
| Logo 位 | 24×24 Logo + 应用名（`--font-size-sm/500`）；折叠态只留 Logo |
| 全局搜索入口 | 居中，宽 320px（Compact 档 200px），高 28px，`--radius-sm`，背景 `--color-bg-input`，右侧显示 `Ctrl K`；点击或 `Ctrl+K` 打开浮层（§2.4 图 2-4-7） |
| 快捷按钮 | 主题切换（`sun`/`moon`，切换即生效）、设置 |
| 窗口控件 | 最小化 / 最大化还原 / 关闭，各 46×36；关闭按钮 hover 背景 `--color-danger` + 白图标；不使用系统默认标题栏 |
| 状态指示 | 曲库扫描中：`loader` + "正在扫描 N/M"；在线不可用：`cloud-offline`（Tooltip 说明） |

### 4.4 SearchBox（搜索/筛选输入框）

**结构**：左侧 `search` 图标（16，tertiary）+ 输入区 + 右侧清除按钮（有内容时出现）+ 可选快捷键提示。

| 属性 | 规格 |
|---|---|
| 高度 | 工具条内 32px；标题栏内 28px；`lg` 变体 40px |
| 背景 | `--color-bg-input`；边框 1px `--color-border` |
| 圆角 | `--radius-sm`（工具条）/ `--radius-md`（lg） |
| 状态 | hover：边框 `--color-border-strong`；focus-visible：边框 `--color-brand` + `--shadow-focus`；disabled：不透明度 0.45；loading：右侧 `loader`（后端检索中） |
| 行为 | 输入 200ms 防抖后触发；`Esc` 先清空再退出（两段行为）；`/` 与 `Ctrl+K`（全局）聚焦；空结果显示无结果态（§2.4 图 2-4-8） |
| 无结果 | 输入框内不弹层，改由列表区展示"未找到相关曲目 + 清除筛选" |

### 4.5 TrackRow（曲目行）

**结构（从左到右）**：选择框(40) → 序号/播放态(40) → 标题区(`minmax(200px,2fr)`，含播放态图标、标题、可选角标) → 艺术家 → 专辑 → 时长(72) → 更多(48)。

| 状态 | 行背景 | 左侧指示条 | 前景 | 其他 |
|---|---|---|---|---|
| default | 透明 | 无 | 标题 primary / 次级 secondary | 行高 `--size-row-h`；斑马纹禁用 |
| hover | `--color-bg-hover` | 无 | 序号位置显示 `play` 图标（可点击，32×32 命中区） | 选择框淡入 |
| selected（多选勾选） | `--color-bg-selected` | 无 | 标题 500 | 复选框为勾选态 |
| playing（当前曲目） | `--color-bg-selected` | 3px `--color-brand` | 标题 500 + `--color-brand`；图标变播放态均衡动画（3 条 2px 竖条，1.2s 循环，reduced-motion 下静止为 `play` 图标） | 时长位置替换为"正在播放"？否——时长保留，播放态只在序号位表达 |
| focus-visible | 同 hover | 无 | 不变 | `--shadow-focus` 内缩（`inset` 版，避免裁切） |
| disabled（文件缺失） | 透明 | 无 | 全部 `--color-text-disabled`；标题后加 `alert-triangle` 16px | Tooltip 显示"文件不存在"（对齐 B-13：缺失检测由后端返回，不在前端逐文件探测） |
| loading（封面/时长未就绪） | 透明 | 无 | 时长位置显示 40×12 骨架条 | 标题/艺术家已可用时正常显示（渐进填充） |

补充：CUE 音轨行在时长左侧增加 `音轨号` 与（详情视图中）`起始/结束` 列；长标题省略时 `title` 属性 + 可访问名称给出全文。

### 4.6 TrackListHeader（列表头）

| 元素 | 规格 |
|---|---|
| 高度 | `--size-header-h` 32px；sticky 于列表顶部（`--z-sticky`），背景 `--color-bg-surface`，底部 1px `--color-border` |
| 全选 | 复选框，三态：未选 / 半选 / 全选（半选用横线图标，不用颜色区分） |
| 排序列 | 点击切换升/降序，图标 `chevron-up/down`（12px，仅当前排序列可见）；当前排序列文字 primary + 500，其余 secondary |
| 列宽 | 与 §2.4 表 2-4-2 一致；禁止用户列宽拖拽（避免与虚拟滚动冲突），改由列裁剪策略自适应 |
| 交互 | 表头行也响应右键（提供"显示/隐藏列"菜单：时长、专辑、艺术家） |

### 4.7 ListToolbar（视图工具条）

高度 `--size-toolbar-h`；左侧 `筛选框 + 视图切换（☰/⊞）`；右侧 `排序下拉 + 密度下拉 + ⋮ 更多`。

| 状态 | 表现 |
|---|---|
| 默认 | 背景透明，与列表同面 |
| 滚动中 | 保持 sticky（与表头一起，`--z-sticky`） |
| 有筛选条件 | 筛选框显示清除按钮，右侧出现 `已筛选 (N)` Chip，点击清除 |
| 无数据 | 工具条仍在（可恢复操作），仅列表区切换为空状态 |

### 4.8 SelectionBar（多选操作条）

| 项 | 规格 |
|---|---|
| 位置 | 浮动于内容区底部 `--space-4` 处，宽 = 内容宽 − `--space-12`，高 56px |
| 外观 | 背景 `--color-bg-elevated`，1px `--color-border`，`--radius-lg`，`--shadow-elevation-3` |
| 内容 | `已选 N 首`（`--font-size-base/600`）+ 按钮组：播放 / 加入歌单 / 转换 / 收藏（可选）/ 删除（danger）/ 取消（ghost） |
| 出现 | 200ms 上滑 + 淡入（`--ease-decelerate`） |
| 边界 | N=0 时自动退出选择模式；选择模式中按 `Esc` 退出并清空 |
| 禁用 | 无选中项时按钮 disabled + Tooltip；跨类型选择（如同时选曲目与歌单）不允许，选择模式内只允许同类对象 |

### 4.9 PlayerBar 组件族

#### 4.9.1 NowPlaying（左区）

| 项 | 规格 |
|---|---|
| 封面 | 48×48，`--radius-sm`，缺失时 `--color-cover-placeholder` 渐变 + `music-note` 图标；点击 → 封面放大模态框（H-09） |
| 标题 | `--font-size-base/500`，单行省略；播放中可点击进入所在专辑（二级路由） |
| 艺术家 | `--font-size-xs`，secondary，可点击进入艺术家详情 |
| 收藏按钮 | 24px 图标按钮，`heart` ↔ `heart-filled`（`--color-brand`）；hover 显示 |
| 宽度 | `1fr`，最小 200px；Compact 档隐藏艺术家行 |

#### 4.9.2 TransportControls（中区）

- 按钮顺序：`previous`(32) → `play/pause`(40 圆形，`--color-brand-solid` + 白图标) → `next`(32) → `playmode`(32) → `volume`(图标 20，右区)。

| 按钮 | hover | pressed | disabled | 备注 |
|---|---|---|---|---|
| previous / next | 图标色 primary，背景 `--color-bg-hover` | `scale(0.94)` | 队列仅 1 首时 next 可用（单曲循环语义），无曲目时全禁用 | 长按 next/previous 触发快进/快退（1.5 倍速 seek，松手停止）可选 |
| play / pause | 背景 `--color-brand-hover` | `--color-brand-active` + `scale(0.96)` | 无曲目时禁用（图标置灰 + Tooltip"请先添加音乐"） | 加载中显示 `loader` 且不可重复点击（防 A-10 式重入） |
| playmode | 同 ghost | 同 ghost | — | 单击循环四态（顺序/列表循环/单曲循环/随机），图标 + Tooltip 同步更新；右键/长按弹出选择菜单（状态可见性优于旧版三态轮换） |

#### 4.9.3 ProgressBar（进度条）

| 项 | 规格 |
|---|---|
| 轨道 | 高 4px，`--color-border`，`--radius-full` |
| 已播 | `--color-brand-solid` |
| 缓冲 | `--color-brand-subtle`（后端支持时） |
| 悬停/聚焦 | 轨道加高至 6px（`--duration-instant`），拇指出现 12px 圆形（`--color-brand-solid` + 2px `--color-bg-elevated` 环） |
| 拖拽 | 拖动中：`is-dragging` 态，仅本地更新视觉位置，**不**实时 seek；松手时一次性提交 seek（避免 B-14 式回写抖动）；拖动中显示跟随指针的时间气泡（`--shadow-elevation-3`） |
| 时间码 | 右区固定 96px，`01:23 / 03:45`，tabular-nums；未加载时显示 `--:--` 且禁用拖拽（对齐 B-04/A-16 的时长来源统一） |
| 状态 | disabled（无曲目/直播流无时长）、loading（时长未就绪 → 骨架轨道）、error（seek 失败 → 轨道闪 `--color-danger` 一次 + Toast） |
| 无障碍 | `role="slider"`，`aria-valuemin/max/now/valuetext`；`←/→` ±5s、`Shift+←/→` ±30s、`Home/End` 到首尾 |

#### 4.9.4 VolumeControl（音量）

| 项 | 规格 |
|---|---|
| 形态 | 图标按钮（20）+ 悬停展开 100px 滑块（默认收起的横条，展开 120ms） |
| 图标态 | `volume-mute`（=0）/ `volume-low`（<50）/ `volume-high`（≥50）；图标单击 = 静音切换 |
| 行为 | 静音前记忆 `previousVolume`；从静音恢复时回到记忆值（对齐 A-06，但只保留一个状态源） |
| 数值 | 拖拽中在图标上方显示百分比气泡（`--font-size-2xs`） |
| 状态 | disabled（无音频输出设备时，Tooltip 说明）；键盘：↑/↓ ±5 |
| 右键 | 静音 / 音量 100% / 打开系统音量（系统集成域） |

### 4.10 RightPanel（右面板）

| 项 | 规格 |
|---|---|
| 页签 | 高 40px，`歌词` / `队列`；选中项底部 2px `--color-brand`，文字 primary/500；未选中 secondary |
| 关闭按钮 | 右上 `close` 20px，hover 背景 `--color-bg-hover` |
| 内边距 | `--space-5`，面板内可滚区域自带 `scrollbar-gutter: stable` |
| 宽度 | `--size-panel-w`，左侧 4px 热区可拖拽（200ms 内不重复触发） |
| 状态 | 打开（内嵌）/ 浮层（Compact 档，带 `--color-overlay` 遮罩，Esc 关闭）/ 关闭 |
| 内容页签 | 歌词：封面缩略 + 曲目信息 + 歌词滚动区 + 底部工具条；队列：当前队列列表（可拖拽排序、可跳播） |

### 4.11 LyricsView / LyricsBar（歌词）

| 项 | 规格 |
|---|---|
| 行高 | 普通行 40px；当前行 48px（含 8px 下内边距） |
| 字号 | 当前行 `--font-size-md/600` + `--color-brand`；相邻 3 行内 `--font-size-sm` secondary；更远 `--font-size-sm` tertiary（`opacity: 0.7` 过渡） |
| 居位 | 当前行固定于面板垂直 40% 处 |
| 交互 | 滚动：自动滚动 + 用户手动滚动后 3s 内暂停自动跟随，之后恢复（恢复到当前行）；单击某行 = seek 到该行时间（有 `Esc` 撤销？否，仅 Toast 提示"已跳转到 xx:xx"）；右键 = 复制该行（H-10 类能力）；双击 = 复制该行歌词 |
| 工具条 | `逐字`（有 YRC 时可用，否则 disabled + Tooltip）、`翻译`（有翻译行时可用）、`顶/底` 位置切换（F-08） |
| 状态 | loading（骨架 3 行）、empty（"未找到歌词"+`[在线匹配]`按钮，对齐 F-01 失败出口）、error（在线源失败 → 提示"在线歌词源不可用，可稍后重试"并保留本地歌词） |
| 纯音乐 | 显示 `♪` 与"纯音乐，请欣赏"（不进入在线匹配重试） |

### 4.12 QueueList（队列）

| 项 | 规格 |
|---|---|
| 行结构 | 拖拽手柄(24) + 序号(28) + 标题 + 艺术家 + 时长 + 移除(32, hover 出现) |
| 状态 | default / hover（`--color-bg-hover` + 手柄显现）/ playing（同 TrackRow 播放态）/ dragging（`opacity: .6` + 2px `--color-brand-solid` 落点线）/ disabled |
| 交互 | 拖拽排序（§5.6）、双击跳播、`✕` 移除、`Delete` 移除选中项、表尾 `[ 清空队列 ]`（确认对话框） |
| 空状态 | "队列为空，去曲库添加一些音乐" + `[ 浏览曲库 ]` |

### 4.13 Card（网格卡片：专辑 / 艺术家 / 歌单 / CUE）

| 项 | 规格 |
|---|---|
| 尺寸 | 宽 `minmax(--size-card-min, 1fr)`；封面 1:1；卡片总高 = 封面 + 文本区 48px |
| 圆角 | 封面 `--radius-md`（艺术家用 `--radius-full`）；卡片容器 `--radius-xl`（仅 hover 背景使用） |
| 文本 | 名称 `--font-size-base/500` 两行截断（artist 单行居中）；副标题 `--font-size-xs` secondary 单行 |
| hover | 卡片背景 `--color-bg-hover`；封面 `scale(1.02)`（`--transition-transform`）；中央浮出播放按钮（44px 圆形 `--color-brand-solid`，`--shadow-elevation-2`） |
| selected | 封面外 2px `--color-brand` 描边 + 右上 16px 对勾（不用纯色填充区分） |
| loading | 封面 `--color-cover-placeholder` 渐变 + 闪动；文本区骨架条 |
| 无封面 | 封面区显示 `disc`/`user`/`list-music` 图标（64px，tertiary） |

### 4.14 EmptyState / ErrorState / Skeleton

| 组件 | 规格 |
|---|---|
| EmptyState | 图标 48px（tertiary）→ 标题 `--font-size-md/600` → 说明 `--font-size-sm` secondary（≤2 行，宽 ≤360px）→ 主按钮(`primary`) + 次按钮(`secondary`，可选) → 拖入提示（虚线框 1px `--color-border-strong`，`--radius-lg`，文案"也可以把文件拖到这里"） |
| ErrorState | 图标 `alert-triangle` 48px（`--color-danger`）+ 错误摘要 + `错误码/技术详情`（可折叠，`--font-family-mono`）+ `[ 重试 ]` 主按钮 + `[ 复制详情 ]` ghost 按钮 |
| Skeleton | 曲目行骨架：`▢(40) + ▬▬▬▬▬▬▬▬ (标题) + ▬▬▬▬ (艺术家) + ▬▬ (时长)`；颜色 `--color-bg-hover`，呼吸动效 1.2s（reduced-motion 静止）；列表首屏固定渲染 8 行；加载完成后**不做**位置跳动（骨架行高 = `--size-row-h`） |

### 4.15 Modal / Dialog

| 变体 | 宽 | 用途 | 结构 |
|---|---|---|---|
| `dialog-sm` | 400px | 确认、输入（新建歌单名） | 标题 + 正文 + 底部按钮右对齐 |
| `dialog-md` | 560px | 一般表单 | 同，正文可滚（最大高 60vh） |
| `dialog-lg` | 760px | 编辑标签（5 页签） | 标题 + 页签栏 + 内容 + 说明区 + 底部按钮 |
| `dialog-xl` | 960px | 在线匹配候选列表 | 左候选列表 + 右详情/试听 |

| 状态/规则 | 规格 |
|---|---|
| 遮罩 | `--color-overlay`，点击遮罩 = 取消（破坏性确认框除外：点击遮罩不取消） |
| 出现 | 淡入 + `scale(0.98→1)`，180ms `--ease-emphasized`；遮罩淡入 120ms |
| 焦点 | 打开时焦点落在首个可交互元素（破坏性确认框落在**取消**按钮上）；`Tab` 在框内循环；`Esc` 关闭（同遮罩规则） |
| 关闭后 | 焦点返回触发元素 |
| 滚动 | 正文区滚动，标题与底部按钮固定 |
| 禁用 | 底部主按钮在必填项未满足时 disabled + Tooltip 指出缺失项 |
| 危险确认 | 标题左侧 `alert-triangle`（`--color-danger`）；主按钮变 `danger`；正文必须写明**具体对象数量与后果**（如"将从曲库移除 12 首曲目（不删除磁盘文件）"） |

### 4.16 Toast

| 项 | 规格 |
|---|---|
| 位置 | 右上角，距标题栏 `--space-4`，距右边缘 `--space-5`；多条纵向堆叠 8px |
| 宽 | 320–400px；背景 `--color-bg-elevated`；`--radius-lg`；`--shadow-elevation-3`；左侧 3px 状态色条 |
| 变体 | `info`（`info` 图标）/ `success`（`check`）/ `warning`（`alert-triangle`）/ `error`（`x-circle` + 停留时间更长 + 可展开详情） |
| 结构 | 图标 + 标题（14/500）+ 说明（13/secondary，≤2 行）+ 可选操作按钮（如"撤销"）+ 关闭按钮 |
| 时长 | info/success 4s；warning 6s；error 8s；带"撤销"操作的 Toast **10s**；悬停暂停计时 |
| 队列 | 同时最多 3 条；超出时最新入队、最旧出队；同类错误在 3s 内合并为"N 个错误" |
| 无障碍 | 容器 `role="status" aria-live="polite"`；error 用 `role="alert" aria-live="assertive"` |

### 4.17 DropdownMenu / ContextMenu（共用同一实现）

| 项 | 规格 |
|---|---|
| 容器 | 背景 `--color-bg-elevated`，1px `--color-border`，`--radius-md`，`--shadow-elevation-3`，内边距 `--space-1` |
| 项 | 高 32px，左图标 20 + 文本 14 + 右侧快捷键提示（mono 11 tertiary）；水平内边距 `--space-3` |
| 项状态 | hover `--color-bg-hover`；pressed `--color-bg-active`；disabled `--color-text-disabled` + Tooltip 说明原因；danger 项文本 `--color-danger` |
| 分隔 | 1px `--color-border-subtle` + 上下 `--space-1` |
| 子菜单 | 右侧 `chevron-right`，展开延迟 120ms，与父项左对齐 |
| 定位 | 自动翻转（下方空间不足时向上/向左），距触发元素 4px，不超出窗口 8px 边距 |
| 行为 | Esc 关闭、点击外部关闭、`↑↓` 选择、`Home/End`、`⏎` 执行、`Tab` 关闭并移动焦点；关闭后焦点回触发元素 |
| 多选语义 | 由 ContextMenu 触发的批量操作，作用于"当前选中集合"（若无选中则作用于被右键的单个对象），菜单标题显式写出"将作用于 12 首" |

### 4.18 Switch（开关）

| 项 | 规格 |
|---|---|
| 尺寸 | 轨道 36×20，`--radius-full`；滑块 16px 圆，`--radius-full`，2px 内边距 |
| on | 轨道 `--color-brand-solid`，滑块 `#fff` 靠右 |
| off | 轨道 `--color-border-strong`，滑块 secondary 靠左 |
| 状态 | hover：轨道亮度略提（用 `--color-brand-hover` / `--color-bg-hover` 叠加）；focus-visible：`--shadow-focus`；disabled：不透明度 0.45 + Tooltip；loading：滑块位置暂时锁定的同时右侧显示 `loader` |
| 无障碍 | `role="switch"` + `aria-checked`；`Space`/`⏎` 切换；旁边常有文字状态（`开`/`关`），不允许只靠颜色（§6.3） |
| 立即生效 | 设置项开关即改即存（无需"保存"按钮）；改动失败时回滚并 Toast error（对齐"设置必须有消费者"的要求） |

### 4.19 Slider（通用滑块：音量 / 进度 / 转换参数）

| 项 | 规格 |
|---|---|
| 轨道 | 4px（可 `--size` 变体），`--color-border`；已选段 `--color-brand-solid`；`--radius-full` |
| 拇指 | 12px 圆，`--color-brand-solid`，2px `--color-bg-elevated` 环；hover 放大至 14px |
| 状态 | default / hover（轨道加粗至 6px）/ active（拇指 14px + 时间气泡）/ focus-visible（光圈）/ disabled（不透明度 0.45，禁用拖拽）/ readonly（仅展示，允许 hover 显示数值） |
| 键盘 | `←→` ±步长、`Shift+←→` ×5、`Home/End` 到首尾；`aria-valuetext` 给出人类可读值（如 `01:23`、`80%`） |
| 数值气泡 | `--color-bg-elevated` + `--shadow-elevation-3` + `--font-size-2xs`，跟随指针，超出窗口边界时自动翻转到另一侧 |

### 4.20 Tabs（右面板页签 / 设置页签 / 模态页签）

| 变体 | 规格 |
|---|---|
| `line`（右面板） | 高 40px，选中底部 2px `--color-brand`；hover 文本 primary |
| `pill`（模态框内） | 高 32px，`--radius-sm`，选中 `--color-bg-selected` + 文本 `--color-brand` |
| `side`（设置） | 竖向 180px 宽，项高 36px，选中 `--color-bg-selected` + 左 3px 指示条 |
| 键盘 | `←→`/`↑↓` 切换（roving tabindex，整组只 1 个可 Tab 进入），`Home/End` 首尾；`aria-selected` + `role="tab"`/`role="tabpanel"` |

### 4.21 表单控件（TextField / Select / Checkbox / Radio / 文件选择行）

| 控件 | 规格（高 32px，`--radius-sm`，1px `--color-border`，背景 `--color-bg-input`） |
|---|---|
| TextField | 内边距 `--space-3`；placeholder tertiary；hover 边框 `-strong`；focus 边框 `--color-brand` + 焦点环；error 边框 `--color-danger` + 下方 12px 错误文案（`--color-danger`）；disabled 0.45；只读（如路径展示）用 `--color-bg-hover` 底 + mono 字体 + 右侧"复制"按钮 |
| Select | 同 TextField，右侧 `chevron-down`；展开用 DropdownMenu 规格；支持键盘首字母跳转；空选项显示"—"；禁用项 disabled + 原因 Tooltip |
| Checkbox | 16×16，`--radius-xs`；未选：1px `--color-border-strong`；已选：`--color-brand-solid` + 白对勾；半选：横线；focus-visible 焦点环；disabled 0.45 |
| Radio | 与 Checkbox 同尺寸同状态，形状圆形；组内 `role="radiogroup"`，`↑↓` 切换 |
| 文件/目录选择行 | 只读路径框 + `[ 选择… ]` secondary 按钮；路径超长时中部省略（保留盘符与文件名尾部）；选择后 Toast success 提示"已添加 N 个文件" |

### 4.22 Badge / Chip / Tag

| 组件 | 规格 | 用途 |
|---|---|---|
| Badge（计数） | 11/500，`--radius-xs`，内边距 2×6；默认 `--color-bg-hover` + secondary；选中态 `--color-brand-subtle` + `--color-brand` | 侧栏计数、导航项 |
| Chip（筛选/可删标记） | 高 24px，`--radius-full`，1px 边框；含文本 + `✕`；hover 背景 `--color-bg-hover` | `已筛选 (N)`、当前排序 |
| Tag（属性标签） | 高 18px，`--radius-xs`，`--font-size-2xs` | `FLAC`、`44.1kHz`、`CUE`、`无损` |
| 状态色 | 音质/无损用 info 系；警告（文件缺失）用 warning 系；均需附带文本，不用纯色点 | — |

### 4.23 Tooltip

| 项 | 规格 |
|---|---|
| 触发 | 悬停 400ms 或键盘聚焦（聚焦立即显示） |
| 外观 | 背景 `--color-bg-elevated`，1px `--color-border`，`--radius-sm`，`--shadow-elevation-3`，`--font-size-xs`，最大宽 240px，内边距 `--space-2`/`--space-3` |
| 定位 | 默认上方 8px；空间不足自动换向；不遮挡触发元素 |
| 内容 | 单一短句（≤2 行）；含快捷键时右对齐 mono 提示（如 `播放/暂停   Space`） |
| 无障碍 | 触发元素带 `aria-describedby`；Tooltip 本身 `role="tooltip"` |
| 禁止 | 不用 Tooltip 承载必须阅读的信息（如错误详情应进 Toast/对话框） |

### 4.24 AsyncProgress（转换/扫描/更新任务进度）

| 项 | 规格 |
|---|---|
| 形态 | 行内 `进度条 + 百分比 + 状态文本`；批量时为"总进度 + 当前文件"两行 |
| 状态 | queued（`—` + tertiary）/ running（进度条 `--color-brand-solid` + 百分比 tabular-nums）/ done（`check` success）/ failed（`x-circle` danger + `[ 重试 ]`）/ canceled（secondary） |
| 行为 | 长任务（>2s）必须在播放条上方或视图内显示可关闭的进度卡；支持"最小化到侧栏指示器"；任务完成 Toast 汇总（成功 N / 失败 M） |
| 取消 | `[ 取消 ]` secondary 按钮，执行中的单项允许取消（转换器），取消后保留已产出文件并说明 |

---

## 5. 交互与反馈规则

### 5.1 鼠标与键盘的等价约定（统一口径）

| 对象 | 单击 | 双击 | 右键 | 悬停浮现 | 键盘等价 |
|---|---|---|---|---|---|
| 曲目行 | 选中（不播放） | 立即播放 | 曲目动作菜单 | 行首 `play` 按钮、复选框 | `↑↓` 移动选中，`⏎` 播放，`Space` 播放/暂停 |
| 专辑 / 歌单 / CUE 卡片 | 进入详情 | 播放全部 | 卡片动作菜单 | 中央 44px 播放按钮 | `⏎` 进入详情，`Space` 选中，`Ctrl+⏎` 播放 |
| 艺术家卡片 | 进入详情 | 播放该艺术家全部曲目 | 卡片动作菜单 | 中央播放按钮 | 同上 |
| 队列行 | 选中 | 跳播该曲 | 队列动作菜单 | 移除按钮、拖拽手柄 | `⏎` 跳播，`Delete` 移除，`Alt+↑↓` 排序 |
| 歌词行 | 跳转到该行时间 | 复制该行 | 复制 / 复制全部 | — | `⏎` 跳转 |
| 侧栏导航项 | 切换视图 | — | 无 | Tooltip（折叠态） | `Tab` 聚焦，`⏎` 进入 |

**红线**：任何"仅在悬停时可见"的操作，必须同时满足 —— ①可通过右键菜单触达；②键盘聚焦该行后可见；③在触摸/无悬停环境下常驻可见（用 `@media (hover: none)` 常显）。

### 5.2 播放操作反馈（播放类操作的即时回执）

| 触发 | 视觉反馈 | 时间窗 | 失败处理 |
|---|---|---|---|
| 点击播放/双击曲目 | 目标行立即进入 `playing` 态（指示条 + 均衡动效）；播放条左区标题即时更新；播放按钮进入 loading 态 | ≤ 100ms 内出视觉响应 | 解码失败 → 该行 `alert-triangle` + Toast error（含"跳过此曲"按钮），自动尝试下一首 |
| 暂停/继续 | 播放按钮图标切换 + 均衡动效暂停（不消失） | 立即 | — |
| 切歌（上/下一首） | 播放条标题淡出→淡入（120ms），封面同步 | ≤ 150ms | 队列耗尽 → Toast info "已到列表末尾" |
| 切换播放模式 | 图标更换 + Tooltip 立即显示新模式名 + Toast info（1.5s 轻提示，仅首次切换时） | 立即 | — |
| 拖动进度 | 时间气泡跟随，不实时 seek | 松手提交 | seek 失败 → 进度条闪 danger 一次 + Toast error |
| 调整音量 | 图标态切换 + 气泡百分比 | 立即 | 无音频设备 → 控件 disabled |
| 收藏/取消收藏 | 心形填充动画（scale 0.8→1.1→1，180ms）+ 无 Toast（除非从详情页操作 → success Toast） | 立即 | 写入失败 → 图标回滚 + Toast error |
| 加入队列 | Toast success "已加入队列（共 N 首）"，带"查看队列"按钮 | 立即 | 失败 → Toast error |
| 从系统媒体键/托盘控制 | 与界内操作完全一致的表现 | — | — |

### 5.3 加载与骨架屏

| 场景 | 表现 | 阈值规则 |
|---|---|---|
| 首屏加载 | 应用外壳（侧栏/标题栏/播放条）**立即渲染**，仅内容区显示骨架 | 无阈值 |
| 列表/网格数据 | 骨架行 = `--size-row-h`（列表，固定 8 行）/ 骨架卡（网格，按当前列数） | 请求 > 200ms 才显示骨架（<200ms 直接出内容，避免闪烁） |
| 封面加载 | `--color-cover-placeholder` 渐变 + 中央 `music-note`；加载完成 120ms 淡入 | 无阈值 |
| 分页 / 无限滚动 | 列表底部哨兵行：`loader` + "加载更多…"；到末尾显示"已全部加载（N 首）" | 触底 200px 预取 |
| 扫描/导入 | 标题栏状态指示 + 列表实时追加（追加行 120ms 淡入）；不整表刷新、不清空已有数据 | 无阈值 |
| 按钮级操作（保存设置、执行在线匹配） | 按钮进入 loading，禁用重复点击，宽度不变 | 立即 |
| 进度类任务 | 见 §4.24；长任务允许后台继续，界面显示"转到后台" | > 2s 必须给出可关闭的进度卡 |

**禁止**：①全屏遮罩式 loading（阻断整个界面）；②加载中清空已有列表（先清空再填充造成"闪空"）；③多个 spinner 同时出现在同一视图。

### 5.4 错误提示分级

| 级别 | 判定 | 呈现 | 示例 |
|---|---|---|---|
| L1 轻微（可自愈/可忽略） | 单次网络歌词失败、封面下载失败 | 就地降级 + 不提示（仅日志） | 在线封面超时 |
| L2 局部错误 | 单个文件解码失败、单次设置写入失败 | 行内标记（`alert-triangle`）+ Tooltip 原因 + 必要时 Toast warn | 某曲目文件缺失 |
| L3 操作失败 | 批量操作整体失败、目录选择失败、转换失败 | Toast error（8s，可展开详情）+ 提供重试/复制详情 | 输出目录无写权限 |
| L4 阻断错误 | 曲库数据库不可用/损坏、应用启动关键资源缺失 | 全屏 ErrorState（含错误码、技术详情、重试、打开日志目录、"安全模式启动"） | 数据库迁移失败 |

规则：错误文案必须写 **"发生了什么 + 影响范围 + 可执行的下一步"**，禁止只显示 `Error: xxx`；技术详情必须可折叠、可复制（对接 B 系列逻辑缺陷的可诊断性要求）。

### 5.5 右键菜单（按对象的动作清单）

| 对象 | 菜单项（顺序固定） |
|---|---|
| 曲目行 | 播放 / 下一首播放 / 添加到队列 / 添加到歌单 ▸ / 收藏（切换）/ 转换… / 在资源管理器中显示 / 复制曲目信息 / 属性 / ─ / 多选 |
| 专辑卡片 | 播放 / 下一首播放 / 添加到队列 / 添加到歌单 ▸ / 收藏 / 在资源管理器中显示 / 属性 |
| 艺术家卡片 | 播放全部 / 添加到队列 / 在资源管理器中显示 / 属性 |
| 歌单卡片 | 播放 / 重命名 / 编辑（曲目） / 导出… / ─ / 删除歌单（danger） |
| 队列行 | 跳播 / 下一首播放 / 从队列移除 / 在资源管理器中显示 |
| 歌词行 | 复制该行 / 复制全部歌词 / 在线匹配… |
| 列表空白区 | 全选 / 导入文件… / 导入文件夹… / ─ / 排序 ▸ / 显示列 ▸ / 密度 ▸ |
| 侧栏项 | 复选框式"显示/隐藏"（仅"曲库"分组下可隐藏的项） |

**规则**：①有选中集合时，菜单标题栏显示"将作用于 N 个对象"；②破坏性项置底、danger 样式、与普通项之间有分割线；③菜单在 `Esc`/点击外部后关闭并把焦点还给触发元素；④同一对象的主操作（如"播放"）必须同时存在于菜单与悬停按钮。

### 5.6 批量选择

| 项 | 约定 |
|---|---|
| 进入选择模式 | 点击任一复选框；`Ctrl+A`（列表已聚焦）；右键"多选"；`Shift+单击` 连选（自动进入） |
| 区间选择 | `Shift+单击` 选中锚点至目标；`Ctrl+单击` 增减单项；`Ctrl+A` 全选；`Ctrl+Shift+A` 取消全选 |
| 退出 | `Esc`（清空并退出）/ 点击 `取消` 按钮 / 数据刷新后（清空） |
| 选择模式中的交互变更 | 单击行 = 切换勾选（不再改变播放）；双击仍可播放（不打断选择） |
| 操作条 | 见 §4.8；操作后**保留**选择以便连续操作（删除类除外，删除后退出） |
| 跨页/跨视图 | 切换视图时清空选择，并 Toast info "已切换视图，选择已清空"（禁止隐式跨视图保留选择集合） |

### 5.7 破坏性操作与统一确认流（对齐 D-04/D-07/B-08）

| 操作 | 确认方式 | 文案要点 |
|---|---|---|
| 从曲库移除曲目（单个/批量） | Modal `dialog-sm` confirm（**一次**，不逐条弹窗） | 明确写出"将从曲库移除 **N** 首曲目（文件仍保留在磁盘）"；副选项 `[ 同时删除磁盘文件 ]`（默认**不勾选**，勾选后升级为 🔴 语义：按钮文案变"删除文件并移除"） |
| 删除歌单 | Modal `dialog-sm` | "删除歌单 **X**（含 N 首曲目）？曲目不会从曲库移除。" |
| 清空队列 | 确认框 | "清空队列（N 首）？当前播放将继续。" |
| 删除磁盘文件 | 确认框 + 明确文件数 + 二次输入文件名（可选开关） | 高危险操作：主按钮 danger，且必须在标题与正文同时出现"永久删除"字样 |
| 清除曲库数据/重建索引 | 确认框 | 说明耗时与影响（播放列表是否丢失） |
| 可撤销性 | 所有 L2/L3 级破坏操作完成后，Toast 带 `[ 撤销 ]`（10s 内有效）；不可撤销的操作必须在确认框写明"此操作无法撤销" |

**红线**：破坏性操作不得以"整理/清理/释放空间"等中性词汇表述；一次确认只覆盖一次操作，禁止"一次点头、连续删除"。

### 5.8 拖拽（Drag & Drop）

#### 5.8.1 外部文件拖入（对齐 B-17）

| 阶段 | 表现 |
|---|---|
| 拖入窗口 | 全窗口覆盖 `DropZone` 遮罩：`--color-overlay` + 中央虚线框（2px dashed `--color-brand`，`--radius-lg`）+ 文案"松开以导入 N 个文件" + 下方小字"支持 mp3 / flac / wav / ogg / m4a / ape / cue + 图片封面" |
| 拖入列表内特定位置 | DropZone 收窄为"插入线"（2px `--color-brand-solid`）显示将插入的位置（歌单/队列允许；曲库列表不允许插入排序 → 显示为普通导入） |
| 拖入歌单卡片 | 卡片高亮（2px `--color-brand` 描边）+ 文案"添加到歌单 X" |
| 松开 | 立即 Toast info "正在导入 N 个文件…" → 完成后 Toast success 汇总（成功 N / 跳过 M / 失败 K，可展开） |
| 不支持的类型 | 拖入阶段即显示禁止光标，遮罩文案变"不支持的文件类型"（`--color-danger`） |
| 内部拖拽（队列/歌单排序） | 被拖项 `opacity: .6` + 落点插入线；`Esc` 取消拖拽并复原；拖拽中不触发滚动（`autoScroll` 仅靠近边缘 40px 内启动） |

**安全约束**：仅接受**导入/复制**语义的拖入，永不以"拖入"作为删除或覆盖触发；同名文件导入时弹确认框（覆盖 / 重命名 / 跳过，默认"重命名"，对齐 A-08）。

### 5.9 搜索交互

| 类型 | 位置 | 行为 |
|---|---|---|
| 全局搜索 | 标题栏入口 → `Ctrl+K` 浮层 | 范围：曲目/专辑/艺术家/歌单（不含设置项）；输入 200ms 防抖；结果按分组，默认高亮第一项；`↑↓` 移动、`Tab` 切分组、`⏎` 播放或跳转、`Esc` 关闭；`⏎` 前不改变当前视图/播放 |
| 视图内筛选 | 各列表工具条 | 仅过滤当前视图数据；`Esc` 清空；与全局搜索互不影响（互不写同一状态，消除 B-20 类状态串扰） |
| 设置搜索 | 设置页顶部 | 匹配设置项标题与关键词，命中项高亮并滚动定位 |
| 空关键词 | — | 全局搜索显示"最近搜索（最近 5 条，可清除）"；视图内筛选显示全部 |

### 5.10 状态记忆与恢复

| 状态 | 存储位置 | 失效回落 |
|---|---|---|
| 主题（dark/light）、密度、动效开关 | `localStorage` (`layout.*`) | dark / regular / 开 |
| 侧栏宽度、折叠、面板宽度/页签/开关 | `localStorage` | 240 / 展开 / 320 / `歌词` / 关闭 |
| 上次视图与滚动位置 | `localStorage` (`session.lastRoute`) | 首页 = 全部歌曲 |
| 每视图排序方式与列显示 | `localStorage` (`view.{route}.*`) | 默认：标题升序 / 全列 |
| 播放位置（播放位置记忆） | 后端（曲库元数据） | 从头播放 |
| 窗口尺寸/位置、最大化 | 后端/Tauri store | 1024×640 居中 |
| 读取失败/字段非法 | — | 必须以默认值启动，**禁止**启动崩溃或 0 宽侧栏（针对 B-12 类"记忆状态污染"） |

### 5.11 系统集成的界面回执（对齐 B-15/B-16/K-14）

| 集成 | 界面表现 |
|---|---|
| 系统媒体控制（SMTC / MediaSession） | 与播放条状态双向同步；系统侧切歌/暂停时，界面标题、图标、进度、歌词滚动同步（≤300ms），无需用户手动刷新 |
| 全局快捷键 | 触发时显示 1.2s 的轻量 Toast（"已暂停"/"已切换到下一首"），并保持窗口不激活（不抢焦点） |
| 托盘菜单 | 与播放条一致的播放/暂停、上/下一首、显示主窗口、退出；托盘图标随播放态变化（播放/暂停轮廓） |
| 关联文件双击打开 | 应用前台激活 + 立即播放该文件 + Toast info 显示文件名；队列中新文件替换默认临时队列（不破坏用户自定义队列，另开"临时播放"队列并在界面标注） |
| 关闭窗口 | 默认"最小化到托盘并继续播放"（首次触发时 Toast 说明 + "退出应用"/"记住我的选择"）；`Shift+点击关闭` 直接退出；退出前若有进行中的转换任务 → 确认框 |
| 在线服务不可用 | 顶部（内容区）出现窄条提示（高 32px，`--color-info-bg`）"在线服务不可用，本地功能不受影响"，可关闭且本次会话不再出现（不得反复弹窗，针对 A-15） |

---

## 6. 无障碍与键盘

### 6.1 快捷键总表

> 快捷键分两组：**窗口内快捷键**（应用聚焦时生效）与**全局快捷键**（应用失焦也生效，需系统注册，可在设置中修改）。所有快捷键必须与菜单/Tooltip 中的提示一致（单一来源常量表，禁止多处硬编码）。

#### 6.1.1 播放控制

| 快捷键 | 动作 |
|---|---|
| `Space` | 播放 / 暂停 |
| `Ctrl+→` / `Ctrl+←` | 下一首 / 上一首 |
| `→` / `←` | 快进 5s / 快退 5s |
| `Shift+→` / `Shift+←` | 快进 30s / 快退 30s |
| `↑` / `↓` | 音量 +5% / −5% |
| `M` | 静音切换 |
| `F` | 收藏 / 取消收藏当前曲目 |
| `R` | 切换播放模式（顺序 → 列表循环 → 单曲循环 → 随机） |
| `Ctrl+L` | 全屏歌词模式开 / 关 |
| `Q` | 队列面板开 / 关 |

#### 6.1.2 视图与导航

| 快捷键 | 动作 |
|---|---|
| `Ctrl+K` | 打开全局搜索浮层 |
| `Ctrl+1` … `Ctrl+9` | 跳转侧栏第 1–9 项（当前可用项顺序） |
| `Alt+←` / `Alt+→` | 历史后退 / 前进 |
| `Ctrl+B` | 折叠 / 展开侧栏 |
| `Ctrl+U` | 歌词面板开 / 关 |
| `Ctrl+Shift+U` | 队列面板开 / 关 |
| `Ctrl+,` | 打开设置 |
| `Ctrl+Shift+L` | 切换明 / 暗主题 |
| `Esc` | 关闭浮层/菜单 → 退出全屏歌词 → 清空搜索/退出选择模式（按层级逐层回退，一次只做一件事） |
| `F5` | 重新扫描曲库 |

#### 6.1.3 列表与选择

| 快捷键 | 动作 |
|---|---|
| `↑` / `↓` | 移动选中行（列表聚焦时优先于全局音量） |
| `Shift+↑` / `Shift+↓` | 扩展选择 |
| `Home` / `End` | 跳到首 / 尾 |
| `PageUp` / `PageDown` | 上 / 下翻页（约为可视行数） |
| `Ctrl+A` | 全选 |
| `Ctrl+Shift+A` | 取消全选 |
| `⏎` | 播放选中项（卡片为进入详情） |
| `Ctrl+⏎` | 加入队列 |
| `Alt+⏎` / `Shift+F10` | 打开属性 / 上下文菜单 |
| `Delete` | 从曲库移除选中项（走 §5.7 确认流） |
| `F2` | 重命名（歌单等可重命名对象） |
| `Alt+↑` / `Alt+↓` | 队列内上 / 下移排序（焦点在队列时） |
| 连续输入字母 | 首字母定位跳转（300ms 内连续输入视为组合串） |
| `Tab` / `Shift+Tab` | 焦点前移 / 后移（列表内使用 roving tabindex，整表只占 1 个 Tab 位） |

#### 6.1.4 窗口与系统

| 快捷键 | 动作 |
|---|---|
| `F11` | 全屏开 / 关 |
| `Ctrl+W` | 关闭窗口（按 §5.11 的关闭策略） |
| `Ctrl+T` | 切换窗口置顶 |
| `Ctrl+Shift+M` | 最小化到托盘 |
| `Ctrl+Shift+P` | 打开"首选项/设置"中的快捷键页 |

#### 6.1.5 全局快捷键（默认值，可在设置中修改/禁用）

| 快捷键 | 动作 |
|---|---|
| 媒体键（播放/暂停、下一首、上一首、停止） | 对应播放控制 |
| `Ctrl+Alt+Space` | 播放 / 暂停 |
| `Ctrl+Alt+→` / `Ctrl+Alt+←` | 下一首 / 上一首 |
| `Ctrl+Alt+M` | 静音切换 |
| `Ctrl+Alt+F` | 收藏当前曲目 |
| `Ctrl+Alt+Shift+T` | 显示 / 隐藏主窗口 |

**快捷键设置页要求**：以表格列出"动作 / 当前组合 / 状态"；支持点击录制（录制中显示"请按下组合键…"，`Esc` 取消）；注册失败（被系统或他应用占用）时状态列显示 `冲突`（`--color-warning` + 图标）并**阻止保存**，提供"清除"与"恢复默认"；重复组合立即在同页高亮冲突的两行。

### 6.2 焦点管理

| 场景 | 规则 |
|---|---|
| 路由/视图切换 | 焦点移到新视图标题（`tabindex="-1"` + `:focus` 不可见描边），随后 `Tab` 进入视图内首个控件 |
| 模态框打开 | 焦点圈定在框内（`Tab` 循环，`Shift+Tab` 反向）；破坏性确认框初始焦点在**取消**按钮 |
| 模态框关闭 | 焦点返回打开它的触发元素（若元素已不存在 → 返回同位置的合理锚点，如列表容器） |
| 菜单/Tooltip 关闭 | 焦点返回触发元素 |
| 面板（歌词/队列）打开 | 若由快捷键打开 → 焦点进入面板首个可交互控件；若由鼠标点击 → 焦点保持原位 |
| 列表数据刷新 | 尽量保持原选中项与滚动位置；选中项消失则聚焦到同索引位置，并 `aria-live` 播报"列表已更新" |
| 加载完成 | 不主动抢焦点（禁止 spinner 消失后把焦点吸走） |
| 全屏歌词模式 | 焦点圈定在歌词容器内（`Esc` 退出），提供"歌词可聚焦行"的 roving tabindex |

### 6.3 颜色与状态的可访问性红线

1. **不得以颜色作为唯一信息载体**：选中 = 背景色 + 左侧指示条/对勾；错误 = 颜色 + 图标 + 文本；禁用 = 透明度 + Tooltip 说明原因；播放态 = 指示条 + 均衡动效 + 标题字重。
2. 对比度下限：正文 ≥ 4.5:1；≥ 18.66px 或 ≥ 14px/600 的文本 ≥ 3:1（本设计一律按 4.5:1 执行，除 `text-tertiary` 的辅助文字，见 §3.1.2）；图标/边框/进度条等**非文本 UI ≥ 3:1**；焦点环与相邻色 ≥ 3:1。
3. 明暗两套主题使用**同一组 token 名**，切换后所有配对必须重新满足上述比值（不得只测暗色）。
4. 不依赖"仅悬停"或"仅动画"传达状态（对齐 §5.1 红线）。

### 6.4 屏幕阅读器（SR）标签策略

| 区域/元素 | 语义与标签 |
|---|---|
| 页面地标 | `<header>`（标题栏）、`<nav aria-label="主导航">`（侧栏）、`<main>`（内容区）、`<aside aria-label="歌词与队列">`（右面板）、`<footer>`（播放条） |
| 曲目列表 | `role="grid"`；行 `role="row"` + `aria-selected`；列头 `role="columnheader"` + `aria-sort`（ascending/descending/none）；表格 `aria-rowcount` 提供总数（虚拟滚动下必须显式提供，否则 SR 读到错误行号） |
| 曲目行可访问名 | 组合为"曲目名，艺术家，专辑，时长，第 N 首，[正在播放]"；播放态通过 `aria-current="true"` 表达 |
| 侧栏项 | `aria-current="page"`（当前视图）+ `aria-label="全部歌曲，1024 首"`（计数并入名称） |
| 播放条 | `<footer role="region" aria-label="播放控制">`；标题区 `aria-live="polite"`（切歌播报"正在播放：X — Y"，节流 1s） |
| 播放/暂停按钮 | `aria-label` 随状态变化（"播放"/"暂停"）；`aria-pressed` 不用于此（语义为动作） |
| 进度条 / 音量 | `role="slider"`，`aria-valuemin/max/now/valuetext`（`01:23 / 03:45`、`80%`） |
| 播放模式按钮 | `aria-label="播放模式：单曲循环"`（每次切换更新） |
| 图标按钮 | 无可见文本时**必须**有 `aria-label`；纯装饰图标 `aria-hidden="true"` |
| 开关 | `role="switch"` + `aria-checked`，可见文本状态"开/关"合并进名称 |
| Toast / 状态播报 | `role="status" aria-live="polite"`（成功/信息）；`role="alert" aria-live="assertive"`（错误）；批量操作完成统一播报"已导入 12 个文件，成功 10，失败 2" |
| 歌词 | 容器 `role="log" aria-live="off"`（避免逐行轰炸）；当前行 `aria-current="true"`；提供设置项"朗读歌词"（默认关） |
| 加载态 | 骨架容器 `aria-busy="true"` + 容器级 `aria-label="正在加载曲目列表"`；加载结束后移除 `aria-busy` 并播报一次 |
| 语言 | `<html lang>` 随 i18n 实时更新；混排（如英文曲名）由 SR 自行处理，不逐元素覆盖 `lang` |

### 6.5 键盘可达性验收清单（DoD）

1. 不接鼠标可完成：播放/暂停、切歌、调音量、搜索并播放、进入各主导航、批量选择与批量加入歌单、查看与编辑设置、取消转换任务。
2. 每个视图：`Tab` 顺序与视觉顺序一致（侧栏 → 内容工具条 → 列表/网格 → 面板）；无"焦点黑洞"。
3. 所有焦点位置在 200% 缩放下**仍可见**（无被 sticky 元素遮挡）。
4. 所有对话框有明确标题（`aria-labelledby`），所有图标按钮有标签。
5. 动效关闭后功能不缺失（`prefers-reduced-motion` 与手动开关两条路径都验证）。

---

## 7. 自适应策略与最小窗口尺寸

### 7.1 尺寸下限与各模式边界

| 项 | 值 | 说明 |
|---|---|---|
| 最小窗口 | **1024 × 640**（逻辑 px） | Tauri `minWidth/minHeight`；低于此值不保证布局 |
| 默认启动尺寸 | 1280 × 800，居中 | 首次启动；之后恢复上次尺寸/位置 |
| 迷你模式 | 380 × 120 | 无边框、可置顶，双击恢复 |
| 最大化 / 全屏 | 任何尺寸 | 侧栏与面板策略不变，仅内容区变宽 |
| 超宽屏内容上限 | 列表型视图 `max-width: 1440px` 居中 | 网格型视图不限 |

### 7.2 断点行为细则（配合 §2.8 表）

| 断点 | 触发后的**行为变更**（不只是变量变化） |
|---|---|
| Wide ≥ 1600 | 侧栏 264px、卡片最小宽 176px；内容内边距 32/24；右面板最大可拖至 420px |
| Regular 1280–1599 | 默认形态（侧栏 240 / 面板 320 / 内边距 24/20） |
| Compact 1120–1279 | 侧栏自动折叠为 64（若用户手动固定为展开则不折叠）；右面板转浮层（宽 360 + 遮罩）；列表隐藏「时长」列；网格最小卡宽 160 |
| Narrow 1024–1119 | 播放条转两行 88px；列表隐藏「时长」「专辑」；内容内边距 16；工具条按钮仅保留图标（文字进入 Tooltip）；网格最小卡宽 148 |

**切换稳定性要求**：断点切换使用 ResizeObserver 在**根容器宽度**上判定（阈值 1599/1279/1119，含 ±8px 迟滞，避免临界抖动）；跨断点时**不重建组件树**（只切 `data-layout`），不丢失滚动位置、选择集合与播放状态。

### 7.3 系统缩放与 DPI

1. 全部尺寸单位使用 **px 逻辑像素**，由系统缩放统一放大；禁止按物理像素写判断逻辑，禁止硬编码 `devicePixelRatio` 分支。
2. 1px 描边在非整数缩放下（125%/150%）使用 `border-width: 1px` 交由渲染层处理，必要时用 `box-shadow: 0 0 0 1px` 代替（避免消失或加粗）。
3. 跨显示器拖动时（DPI 变化）由 Tauri 重新计算窗口逻辑尺寸；前端**监听 resize 而非常量缓存**，并重新走 §7.2 判定。
4. 图标必须使用矢量 SVG（不缩放模糊），封面图片按显示尺寸的 2 倍分辨率请求。

### 7.4 文本放大（无障碍字号）

- 系统字号/缩放放大至 **200%** 时：侧栏允许自动折叠、播放条允许转两行；**功能入口不得消失**（不得因放大而 `display:none` 掉工具条或音量控件）。
- 容器高度使用 `min-height` 而非固定 `height`，文本区允许撑高；行高与字号成对使用 `--line-height-*`。
- 长文本（长歌名/长路径）：单行省略 + `title`/`aria-label` 提供全文；路径类不省略尾部（§3.9）。
- 禁止用 `text-overflow` 隐藏**唯一**操作入口（如"更多"按钮不得因空间不足而消失，只允许移入 "⋮"）。

### 7.5 环境适配

| 环境 | 处理 |
|---|---|
| 无悬停设备（触摸/平板模式） | `@media (hover: none)`：悬停才显示的操作常显；命中区 ≥ 44×44px；拖拽排序提供"上移/下移"菜单替代 |
| 键盘导航用户 | 见 §6；`Esc` 逐层回退 |
| 高对比度模式（系统） | 保留 token 体系，不引入额外 `forced-colors` 覆盖；确保边框/焦点环在 `forced-colors: active` 下仍可见（使用 `border` 而非仅 `background` 表达层级） |
| 低性能设备 | 封面懒加载 + 列表虚拟滚动（≥ 200 行强制启用）；拖动/滚动动画降到 `--duration-instant`；波形/频谱等装饰性动效默认关闭 |
| 网络不可用 | 在线服务降级（§5.11），不阻塞本地播放与曲库 |

### 7.6 界面性能约束（对应"更流畅"目标）

1. 长列表（曲目 ≥ 200 行、队列任意长度）必须虚拟滚动；滚动帧率目标 ≥ 55fps，滚动中不做同步计算与全量重排。
2. 列表更新使用 key 复用，禁止整表重建；批量操作只局部更新受影响行（对接 B-22 类"整表刷新"问题）。
3. 搜索/筛选输入 200ms 防抖；滚动/拖拽事件用 `requestAnimationFrame` 节流。
4. 封面与歌词异步加载，渲染优先级低于列表交互；封面加载失败不阻塞行渲染。
5. 首屏可交互时间（Time to Interactive）目标 < 800ms（不含曲库扫描），侧栏/播放条在任何数据就绪前必须可操作。

---

## 8. 与旧版界面的逐项差异表

> 依据 `FEATURES-INVENTORY.md`（旧版 E:\TPlayer 只读分析结论）。"旧版做法"为分析所得现状，"v2 做法"为本设计规范。旧版编号沿用清单编号（A/B/D/F/H/K 系列）便于追溯。

### 8.1 信息架构与导航

| 编号 | 旧版做法 | v2 做法 | 收益 |
|---|---|---|---|
| IA-1 | 左侧导航缺歌单入口，歌单只能从其他路径进入 | 侧栏「曲库」组固定包含：全部歌曲 / 最近播放 / 收藏 / 艺术家 / 专辑 / 分轨专辑 / 歌单，共 7 项，顺序与显示/隐藏可配置 | 功能入口完备、可预期，符合用户心智 |
| IA-2 | 设置分散在多处（设置窗口 + 局部开关），无统一入口 | 统一 `/settings/:tab` 六页签（播放/界面/歌词/在线/更新/关于），侧栏底部与 `Ctrl+,` 双入口 | 消除"设置找不到/找不全" |
| IA-3 | 搜索仅针对当前视图，无全局搜索 | 标题栏常驻全局搜索（`Ctrl+K` 浮层）+ 视图内筛选，两者状态完全分离 | 目标直达；消除状态串扰（B-20） |
| IA-4 | 转换器为独立弹窗，与曲库割裂 | 一级导航「工具 > 转换器」独立视图；曲目右键/多选可"转换…"并把曲目带入（预填充） | 流程连贯、可批量 |
| IA-5 | 无历史导航语义 | 视图层级固定：一级视图 → 详情视图（→ 二级详情）；`Alt+←/→` 与面包屑式"← 返回" | 深层跳转不迷路（D 类冲突减少） |

### 8.2 布局与视觉

| 编号 | 旧版做法 | v2 做法 | 收益 |
|---|---|---|---|
| UI-1 | `App.vue` 8985 行单体，布局与业务混写 | 布局拆为 5 个独立区域组件（TitleBar/SideBar/Content/RightPanel/PlayerBar）+ CSS Grid 骨架，区域只关心自身 | 可维护、可单测、改动不外溢 |
| UI-2 | 主题三处默认值不一致，启动闪白/闪黑（D-09/B-01） | 单文件 token + `:root` 即暗色 + `index.html` 内联脚本写 `data-theme`，首帧即正确 | 消除启动闪白，主题切换零重挂载 |
| UI-3 | 无右侧面板，歌词只能挤在内容区或独立窗口 | 可开合右面板（歌词/队列页签）+ 顶态歌词条；全屏歌词模式（`Ctrl+L`）独立成态 | 听歌/浏览不互相打断 |
| UI-4 | 播放条为单行固定布局，窄窗口下控件溢出被裁 | 单行 72px / 两行 88px 双形态，按宽度断点切换；进度条与时间码宽度固定 | 最小尺寸下功能不丢失 |
| UI-5 | 无密度设置，行高写死 | 三档密度（紧凑 40 / 常规 48 / 宽松 56）+ 网格密度，`data-density` 单一开关 | 大曲库可扫视效率提升 |
| UI-6 | 图标来源不统一（部分 emoji/位图） | 统一图标规范（24 网格 / 1.5 描边 / currentColor）+ 必需图标清单 | 视觉一致、可换主题、可高 DPI |
| UI-7 | 选中/播放/悬停以颜色区分，暗色下难以分辨 | 颜色 + 指示条 + 图标 + 字重多通道表达；含对比度下限表 | 暗色可辨识、色盲友好 |

### 8.3 播放与队列

| 编号 | 旧版做法 | v2 做法 | 收益 |
|---|---|---|---|
| PL-1 | 播放模式按钮三态轮换，当前状态不可见（A-02） | 图标 + Tooltip + 首次切换 Toast 三重可见；右键展开模式菜单 | 状态可见，减少误操作 |
| PL-2 | 音量静音前后状态多处存储，恢复值不一致（A-06） | 单一状态源 + `previousVolume` 记忆；静音恢复回到记忆值 | 消除逻辑冲突 |
| PL-3 | 拖动进度时实时 seek 并回写，进度条抖动（B-14） | 拖动中仅本地视觉更新，松手一次性提交 seek；时间码 tabular-nums 定宽 | 消除抖动与回写竞态 |
| PL-4 | 播放/切歌无即时反馈，需等后端事件 | 目标行 ≤100ms 进入播放态，播放按钮 loading 态防重入 | 手感"跟手"，避免重复点击触发多次播放 |
| PL-5 | 无队列视图，队列不可见不可编辑 | 右面板「队列」页签：可跳播、拖拽排序、移除、清空（确认） | 队列可控 |
| PL-6 | 时长来源不统一，未加载时显示错乱（B-04/A-16） | 时长单一来源（元数据服务），未就绪显示 `--:--` 且禁用拖拽 | 消除显示不一致 |
| PL-7 | 均衡器为假开关（无实际效果） | 本设计不提供伪功能入口：未接入 DSP 前**不出现**均衡器界面；如接入，则必须提供可视化响应曲线与"已启用"状态回执 | 消除"假开关"信任损伤 |

### 8.4 曲库与列表

| 编号 | 旧版做法 | v2 做法 | 收益 |
|---|---|---|---|
| LB-1 | 无拖入导入（B-17），只能走"选择文件夹" | 全窗口 DropZone（三态：可导入/插入/不支持）+ 同名冲突确认（默认重命名，A-08） | 导入路径短、防误覆盖 |
| LB-2 | 无批量选择或批量操作为逐条弹窗（B-08） | 选择模式 + 多选操作条；破坏性操作**一次**确认，文案含数量与后果 | 批量效率提升，不再"点 N 次确认" |
| LB-3 | 删除语义含糊（移除曲库 vs 删除文件混淆） | 明确二分：默认"从曲库移除（保留文件）"，删除磁盘文件为独立选项并升级确认等级 | 数据安全，语义清晰 |
| LB-4 | 文件缺失需前端逐文件探测（B-13） | 缺失状态由后端随元数据返回；行内 `alert-triangle` + Tooltip，不阻塞列表 | 大规模曲库不卡顿 |
| LB-5 | 无右键菜单或菜单项不完整 | 每类对象有固定右键菜单（§5.5），主操作与悬停按钮同源 | 操作路径短且一致 |
| LB-6 | 列表无排序/列控制 | 表头点击排序（`aria-sort`）+ 工具条排序下拉；列显示可配置，窄窗口自动裁剪 | 大库可勘探 |
| LB-7 | 列表无性能策略，数千行直接渲染 | 虚拟滚动（≥200 行强制）、key 复用、局部更新 | 滚动流畅（对应"更流畅"目标） |
| LB-8 | 歌单曲目不可排序 | 歌单/队列支持拖拽排序 + 键盘 `Alt+↑↓` | 歌单可编排 |

### 8.5 歌词、封面与元数据

| 编号 | 旧版做法 | v2 做法 | 收益 |
|---|---|---|---|
| LY-1 | 在线歌词优先级链失败后无出口（F-01） | 失败态明确展示"未找到歌词 + [在线匹配]"；纯音乐直接显示 `♪` 不做无效重试 | 减少无效请求与困惑 |
| LY-2 | 歌词位置固定，无"顶/底"切换（F-08） | 面板内与播放条上方两态可切；含逐字/翻译开关的状态可见性（不可用时禁用 + 说明） | 满足不同使用姿势 |
| LY-3 | 歌词不可交互、不可复制（H-10） | 单击跳转、双击/右键复制该行或全部 | 实用能力补齐 |
| LY-4 | 封面无处查看/放大（H-09） | 点击播放条或面板封面 → 封面模态（大图 + 曲目信息 + 保存） | 展示力提升 |
| LY-5 | 元数据编辑入口分散、标签页不一致 | 统一"编辑标签"对话框 `dialog-lg`（基础/封面/歌词/高级/路径 5 页签），多选时体现批量字段 | 编辑一致、可批量 |

### 8.6 设置、更新与系统集成

| 编号 | 旧版做法 | v2 做法 | 收益 |
|---|---|---|---|
| ST-1 | 设置项存在"改了不生效/无消费者"（B 系列） | 每个设置项必须绑定消费者；开关即改即存；失败回滚 + Toast；无消费者的项**不得出现**在界面 | 消除空转设置与信任损伤 |
| ST-2 | 更新链路断裂（无可用更新入口） | 「关于」页提供：当前版本、检查更新（含手动）、更新通道、更新日志；检查中 loading、失败给可执行提示（手动下载链接） | 更新可用、可诊断 |
| ST-3 | 无系统媒体控制（B-15/K-14） | SMTC/MediaSession 双向同步（≤300ms），系统侧操作与界面状态一致 | 系统级体验补齐 |
| ST-4 | 无全局快捷键 | 全局快捷键表（媒体键 + 5 个可配置组合），设置页支持录制/冲突检测/恢复默认 | 免切窗控制 |
| ST-5 | 无关联文件打开（双击音频不进入应用） | 支持双击打开：前台激活 + 立即播放 + "临时播放"队列标注，不污染用户队列 | 与系统集成闭环 |
| ST-6 | 关闭窗口行为不明确，易误退出（含后台任务丢失） | 默认最小化到托盘并继续播放（首次说明 + 记住选择）；有进行中任务时确认框 | 防误退、任务不丢 |
| ST-7 | 在线服务不可用反复弹窗（A-15） | 顶部窄条一次性提示，可关闭且本次会话不再出现；本地功能不受影响 | 不打扰 |
| ST-8 | 状态记忆不健壮，可能启动即异常（B-12） | 记忆项集中管理 + 非法值回落默认；解析异常不得导致启动崩溃（§5.10） | 启动稳定 |
| ST-9 | 错误提示只有技术信息或完全静默 | 四级错误分级 + "起因/影响/下一步"文案 + 可复制技术详情（§5.4） | 可诊断、可自助 |

### 8.7 无障碍与一致性（旧版基本缺失）

| 编号 | 旧版做法 | v2 做法 | 收益 |
|---|---|---|---|
| A11Y-1 | 无键盘操作路径（播放/搜索/设置） | 完整快捷键表 + 键盘可达验收清单（§6.1/§6.5） | 可全键盘操作 |
| A11Y-2 | 无焦点管理与 `:focus-visible` 规范 | 焦点落点/返回/圈定规则（§6.2）+ 统一焦点环 token | 键盘用户不迷失 |
| A11Y-3 | 图标按钮无标签、列表无语义 | SR 标签策略表（§6.4）+ `role="grid"`/`aria-sort`/`aria-live` | 屏幕阅读器可用 |
| A11Y-4 | 状态靠颜色表达 | 多通道表达 + 对比度下限（§6.3/§3.1.2） | 色盲/低视力可用 |
| A11Y-5 | 固定 px 布局，缩放下控件被裁 | 200% 缩放下功能不丢（§7.4）+ 触摸/无悬停适配（§7.5） | 大字号/触摸设备可用 |
| A11Y-6 | 动效无减弱策略 | `prefers-reduced-motion` + 手动开关双路径（§3.6） | 前庭敏感用户可用 |

---

## 附录 A：Token 快速索引（实现自检用）

| 类别 | 必用 token（示例） | 常见误用 |
|---|---|---|
| 背景 | `--color-bg-surface` / `-elevated` / `-sidebar` / `-hover` / `-active` / `-selected` | 直接写 `#1e2128` |
| 文本 | `--color-text-primary` / `-secondary` / `-tertiary` / `-disabled` | 用 `opacity` 代替 `-secondary` |
| 边框 | `--color-border-subtle` / `--color-border` / `--color-border-strong` | 用 box-shadow 造分割线 |
| 品牌 | `--color-brand`（前景）/ `--color-brand-solid`（填充） | 用 `--color-brand` 当按钮底色（对比度不足） |
| 状态 | `--color-{success|warning|danger|info}` + `-bg` | 自造绿色/红色 |
| 字号 | `--font-size-{2xs…2xl}` 成对 `--line-height-*` | 只给 font-size 不给 line-height |
| 间距 | `--space-{1…16}` | `margin: 13px` |
| 圆角 | `--radius-{xs…full}` | `border-radius: 10px` |
| 层级 | `--shadow-elevation-{1…4}` | 自写 `box-shadow` |
| 动效 | `--duration-*` + `--ease-*` | `transition: all .3s` |
| 尺寸 | `--size-*` | 硬编码侧栏宽 |

## 附录 B：实现完成定义（DoD）

1. `src/styles/theme.css` 为唯一 token 定义处；全库检索不到十六进制色值与 `px` 魔法间距（例外清单：1px 描边、0、100%、50%）。
2. 主题切换（含启动首帧）无闪烁、无重挂载、无数据重取；明暗两套均通过 §3.1.2 对比度表。
3. §2 的 5 个区域组件、§2.4 的 8 类主视图线框全部落地；每个取数视图具备 loading / empty / error 三态。
4. §4 的组件全部具备规范中的状态（含 disabled 与 loading），且可用键盘完整操作。
5. §5 的交互契约（播放反馈、右键菜单、批量选择、破坏性确认一次化、拖拽三态）逐条实现。
6. §6 快捷键表全部可用；全局快捷键冲突可检测并阻止保存。
7. 窗口缩到 1024×640 与放大到 200% 文本时，功能入口无缺失、无遮挡、无横向滚动。
8. 长列表（≥1000 行）滚动不卡顿，批量操作只局部更新。

## 附录 C：与 FEATURES-INVENTORY 功能域的对应关系

| 功能域（FEATURES-INVENTORY） | 本文档主要章节 | 界面落点 |
|---|---|---|
| A 播放内核 | §2.5、§4.9、§5.2、§7.6 | 播放条 / 进度条 / 传输控制 |
| B 媒体库 | §2.4（2-4-1/2/3/4）、§4.5–4.8、§5.6、§5.8 | 曲库 / 专辑 / 艺术家 / 分轨专辑 |
| C 歌单与收藏 | §2.4（2-4-4）、§4.13、§5.6、§5.8 | 歌单详情 / 收藏视图 / 队列 |
| D 歌词 | §2.6、§4.11、§5.2 | 右面板歌词页签 / 顶态歌词 / 全屏歌词 |
| E 元数据与封面 | §2.4（2-4-4）、§4.13、§4.15（编辑标签） | 详情页 / 编辑标签对话框 / 封面模态 |
| F 外观与体验 | §3 全部、§2.3、§5.10 | 主题 / 密度 / 动效 / 面板 / 记忆 |
| G 系统集成 | §5.11、§6.1.4/6.1.5、§7.3 | 媒体控制 / 全局快捷键 / 托盘 / 关联打开 |
| H 在线服务 | §2.4（2-4-7）、§4.11、§4.15（在线匹配）、§5.11 | 搜索 / 歌词匹配 / 封面匹配 / 更新 |

---

**文档状态**：设计规范定稿（可执行依据）。本文档仅定义界面与布局规范，不涉及业务逻辑与数据层实现；所有"新旧对比"结论以 `FEATURES-INVENTORY.md` 为准。





