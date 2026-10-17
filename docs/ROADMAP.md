# TPlayer 路线图

> 记录里程碑回顾、当前状态与后续待办。进度台账见 [`PROJECT-STATUS.md`](./PROJECT-STATUS.md)，架构见 [`ARCHITECTURE.md`](./ARCHITECTURE.md)。
> 最后更新：2026-10-14

## 1. 里程碑回顾

| 阶段 | 主题 | 状态 | 关键交付 |
|---|---|---|---|
| M0 | 骨架与设计基线 | ✅ | 14 视图骨架、五区布局外壳、DESIGN/STATUS 文档、样式 token 单一事实源 |
| M1 | 浏览器实测闭环 | ✅ | 通用反馈件（Toast/Modal/ContextMenu）、列表体系、PlayerBar 全交互、RightPanel、设置六页签、Ctrl+K |
| M2 | Rust 后端落地 | ✅ | 40 条命令、rodio+symphonia 引擎、lofty 曲库扫描、lyrics parser、updater、security 白名单 |
| M3 | 功能闭环 | 🟡 | 歌词/封面真实来源链、断点续播、无损解码修复、系统托盘、交叉淡化沉浸视图、转换器接后端、拖拽导入 |
| M4 | 性能与发布闭环 | ✅ | SQLite 曲库持久化、封面性能优化、虚拟滚动、应用内自动升级、程序化打包、皮肤色分层、启动续播 |

## 2. M3 当前状态（2026-10-14）

**已完成**

- 歌词真实来源链（内嵌 > .lrc 边车 > 在线）+ 在线匹配
- 断点续播与播放习惯记忆、ffprobe 时长兜底
- 实时歌词展示、切歌状态同步/事件推送修复、无损解码杂音修复
- 系统托盘、当前播放列表定位、后缀名筛选
- 封面解析链与封面放大层、切歌交叉淡化与沉浸播放视图、表头吸顶修复、侧栏折叠/转换器入口归位、艺术家/专辑封面展示与聚合补全
- 转换器接 Rust 后端（ffmpeg 转码、进度/取消、事件驱动、真实路径）
- 队列项移除、`应用图标各平台生成`
- **拖拽导入曲库**（Tauri 拖放事件 + `library_import_files` 命令 + 全窗遮罩 + 入库刷新）

**未完成 / 决策中**

- 在线模块真实通道：`onlineService` 统一出口骨架尚未接线（歌词/封面已各自真实化，见 ARCHITECTURE §8）
- 本文档（ARCHITECTURE / ROADMAP）：本批补齐

## 3. 后续待办

### 3.1 功能增强（可选）

- **SMTC 封面（缩略图）**：当前 SMTC 上报了标题/艺术家/专辑与播放状态，但锁屏/任务栏未显示封面。后续可将封面图片（本地 data URL 或 asset 文件）转为 `RandomAccessStreamReference` 经 `DisplayUpdater.Thumbnail` 上报（需 `Storage_Streams` / `Graphics_Imaging` feature）。
- **在线模块收敛**（建议）
  - 将歌词/封面的在线调用收敛到 `services/online.ts` 的 `onlineService` 单一受控出口，兑现「唯一联网出口、便于审计与一键关闭」的 M0 设计意图。
  - 当前 `onlineService.search/fetchLyrics/fetchCover` 为 `NotImplementedError` 骨架，需接真实后端（可委托现有 `lyrics_match_online` / `library_cover` 命令）。
  - 在线搜索（search）超出本地播放器定位，可长期保持未实现或择期评估。

### 3.2 发布前必做

- 替换 `tauri.conf.json` 中 `updater` endpoint 的 `<OWNER>/<REPO>` 为实际 GitHub 仓库。
- 补全/复核 minisign 公钥与签名流程（`src-tauri/.tauri-signing`，已 gitignore）。
- 版本号三处对齐（当前 `2.0.0`）。

### 3.3 质量与维护

- `cargo` 侧仍有 1 个既有 `last_err` 未使用警告（非阻塞），可择期清理。
- Web 预览降级路径（无真实文件解析/转码/托盘）需在每次新增能力时同步维护 `web*` 实现，避免类型契约漂移。

## 4. 风险与约束

- **平台单一性**：以 Windows 为主，ffmpeg 进程与 WebView2 均 Windows 友好；其他平台需额外验证。
- **在线隐私硬约束**：任何在线能力默认关闭且默认不外发本地曲库信息，新增在线功能不得放宽（见 ARCHITECTURE §8）。
- **命令/事件契约稳定**：前端 store 强依赖 `rename_all="lowercase"` 等 serde 约定，改动须同步更新 `types/` 与 `services/`。
