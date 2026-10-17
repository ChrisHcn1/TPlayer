# TPlayer v2.0.0 发布说明

- **发布日期**：2026-10-17
- **平台**：Windows x64（`TPlayer_2.0.0_x64-setup.exe`）
- **应用标识**：`com.tplayer`（对外名称统一为 **TPlayer**，不再使用 "TPlayer Next" 后缀）
- **应用数据目录**：`%APPDATA%\com.tplayer`

## 重要变更：产品命名与升级方式

本次起产品对外名称统一为 **TPlayer**（去掉 "Next" 后缀）。由于安装标识 `productName`
由 `TPlayer Next` 变更为 `TPlayer`，NSIS 安装 GUID 随之改变：

- **已安装旧版 "TPlayer Next" 的用户**：新版不会原地覆盖旧版，而是作为独立条目安装
  （可能出现"双装"）。请在确认新版 `TPlayer` 运行正常后，手动卸载旧的 "TPlayer Next"。
- **旧曲库与设置不会丢失**：新版首次启动时会自动把旧数据目录 `%APPDATA%\TPlayer Next`
  下的 `config.json`、`library.db`（曲库）、歌词偏移、播放状态等**一次性迁移**到新目录
  `%APPDATA%\com.tplayer`。旧目录保留，迁移失败仅记录日志、不影响启动。
- 与更早的历史版本 `TPlayer`（v1，标识 `D57E920A.TPlayer`）仍按设计共存。

## 升级签名密钥更换

本版起更换了自动升级签名密钥对（旧密钥密码遗失，无法继续使用）。影响：

- **v2.0.0-beta 及更早版本**内置的旧公钥无法校验本版及后续版本的更新签名，
  这些用户**无法通过应用内自动更新升级到 v2.0.0**，请从本发布页手动下载安装。
- 从 v2.0.0 起安装的用户不受影响，后续版本可正常自动更新。

## 新特性与改进（相对 v2.0.0-beta）

### 1. 单实例运行
- 通过命名互斥体（`TPlayer_SingleInstance`）检测已有实例；若已有实例运行，新进程仅聚焦
  已有窗口并退出，系统中始终仅一个常驻进程。

### 2. 静默启动 / 续播接管
- 续播或连播到原生引擎难以稳定解码的曲目（DTS / WAV 容器等）时，不再原生播放（避免噪声），
  改由前端 core 解码层静默接管，core 就绪后自动接播且不打断当前曲。

### 3. 拖拽加载播放列表
- 从资源管理器拖入文件 / 文件夹：自动入库 → 生成「拖放列表」歌单 → 立即播放首曲，支持 NAS / 网盘路径。

### 4. NAS / 网盘路径支持
- `\\host\share`（SMB）与映射盘符均可在「曲库目录」中加入并扫描；直接拖入的文件经入库后亦可播放。

### 5. 曲库扫描稳定性修复
- 修复「设置页已添加目录但扫描报"未配置曲库目录"」的错位：曲库白名单现通过专用命令
  `library_add_dir` 直接写入配置，且设置保存对字段缺失更健壮（不再静默吞掉失败）。

## 修复与异常处理
- 设置持久化（`settings_save`）此前因前后端结构不匹配（前端缺 `replaygain.targetLufs`）
  整体反序列化失败、被静默吞掉，导致配置（含曲库白名单）从未落盘；现后端结构加
  `#[serde(default)]` 容错、前端补齐字段、保存失败显式记录并提示。
- 新增 Rust 单元测试覆盖：前端载荷缺字段、旧配置缺可选段、缺 `libraryDirs` 被拒、落盘往返、
  损坏 JSON、文件缺失等边界（6 项全部通过）。

## 安装与升级
- **全新安装**：运行 `TPlayer_2.0.0_x64-setup.exe`，按向导完成。
- **已安装含升级按钮的版本**：打开应用 → `设置` → `更新` → `检查更新` → `下载并安装`（自动重启）。
  - 注意：旧 "TPlayer Next" 的升级按钮会拉取到新 "TPlayer" 安装包并作为新条目安装（见上方"重要变更"）。
- **手动升级**：前往 GitHub Releases 下载 `TPlayer_2.0.0_x64-setup.exe` 安装即可。

## 已知事项
- 安装包当前未做 Authenticode 代码签名，Windows SmartScreen 可能提示"未知发布者"，
  选择「仍要运行」即可（不影响功能）；建议正式发布补充代码签名证书。
- **自动更新仓库地址**：`tauri.conf.json` 的 updater `endpoints` 当前指向
  `https://github.com/ChrisHcn1/TPlayer/releases/latest/download/latest.json`。发布前请确认该仓库
  即为本项目的发版仓库，且 `pubkey` 与签名私钥匹配，否则自动更新会因校验失败而无法安装。

## 构建产物
位于 `src-tauri/target/release/bundle/nsis/`：
- `TPlayer_2.0.0_x64-setup.exe`
- `TPlayer_2.0.0_x64-setup.exe.sig`
- `latest.json`
