# TPlayer Next (v2)

> 桌面音乐播放器 TPlayer 的**下一代重设计版本**，与旧版 `E:\TPlayer` 完全隔离、可共存、不互相覆盖。
> 旧版保持可用，待 v2 稳定后再进行版本迭代替换。

- 目标平台：Windows / macOS / Linux（桌面）
- 技术栈：Tauri 2 + Vue 3 + TypeScript + Pinia + Rust
- 当前阶段：**M0 项目骨架与架构设计**（不含业务实现，不执行 `npm install` / `cargo build`）

## 与旧版的关系

| 项目 | 旧版 | v2（本仓库） |
|---|---|---|
| 仓库/目录 | `E:\TPlayer` | `E:\TPlayerNext` |
| 版本线 | 1.x | 2.x |
| 前端形态 | 单体 `App.vue`（8985 行） | 分层 + Pinia，强制组件拆分 |
| 播放后端 | rodio 与外部 `ffplay` 进程双引擎并行 | 统一 `Player` 接口 + 可替换 Engine 后端 |
| Rust 结构 | 单文件最大 2420 行 | 按 `commands/player/library/lyrics/update` 拆分，单文件 ≤ 800 行 |
| 在线能力 | 与本地逻辑交织 | 独立可开关模块，默认不向外部发送本地库信息 |
| 权限 | `fs` / `assetProtocol` 全盘 `**/*` | 收窄到音乐库目录白名单 |
| FFmpeg | 约 240MB 全量随包分发 | 分场景策略（见 ARCHITECTURE.md §11） |

**对外身份保持一致**：应用名 `TPlayer`、窗口标题、关于页品牌、支持的文件关联格式（mp3/flac/wav/aac/ogg/m4a/wma/dsf/dff）与旧版相同；
**可共存命名**：`productName` / `identifier` / 安装包文件名均与旧版不同，详见 [命名与共存方案](#命名与共存方案)。

## 命名与共存方案

| 项 | 旧版取值 | v2 取值 | 共存说明 |
|---|---|---|---|
| `productName`（安装名/开始菜单/注册表） | `TPlayer` | **`TPlayer Next`** | 安装目录、开始菜单项、卸载项互不覆盖 |
| `identifier` | `D57E920A.TPlayer` | **`com.tplayer.next`** | 命名空间完全不同，MSIX/注册表不冲突 |
| 窗口标题 / UI 品牌 | `TPlayer` | **`TPlayer`**（保持一致） | 用户感知上仍是 TPlayer |
| 应用数据目录 | `%APPDATA%\TPlayer` | **`%APPDATA%\TPlayer Next`** | 曲库/设置/缓存完全隔离 |
| NSIS 安装包 | `TPlayer_1.0.4_x64-setup.exe` | **`TPlayerNext_2.0.0_x64-setup.exe`** | 文件名与安装路径均不同 |
| MSIX 包 | `D57E920A.TPlayer_1.0.6.0_x64.msix` | **`com.tplayer.next_2.0.0.0_x64.msix`** | 商店包身份独立 |
| 文件关联扩展名 | mp3/flac/wav/... | **与旧版相同** | 两版均注册为候选打开方式；v2 **不主动抢占系统默认**，由用户在「打开方式」中选择 |

> 说明：Windows 上同一扩展名的「默认应用」全局唯一。v2 安装时仅注册候选处理程序（`FileAssociations`），不写入 `UserChoice` 强制默认，避免影响旧版既有行为。

## 目录速览

```
E:\TPlayerNext\
├── ARCHITECTURE.md        # 架构分层 / Player 接口 / 数据流与状态机 / 安全边界
├── ROADMAP.md             # M0-M4 分期里程碑、验收标准、旧版功能映射
├── index.html  package.json  vite.config.ts  tsconfig*.json
├── src/                   # 前端：views / components / composables / stores / services / types
├── src-tauri/             # 后端：commands / player / library / lyrics / cover
├── public/locales/        # 多语言
└── bin/                  # 随安装包分发的运行时工具（ffmpeg/ffprobe）
```

## 开发

```bash
npm install
npm run tauri dev      # 桌面开发（Vite 端口 3100 + Rust 热重启）
npm run typecheck      # 仅前端类型检查
```

## 打包与发布（Windows）

产物为 NSIS 安装包，同时内置应用内自动升级（Tauri Updater + minisign 签名）。

**一次性准备：签名密钥**

```bash
npm run tauri signer -- generate --ci -w src-tauri/.tauri-signing/TPlayerNext.key -p "<密码>" -f
```

- 公钥（`.key.pub` 内容）写入 `src-tauri/tauri.conf.json` 的 `plugins.updater.pubkey`；
- 私钥目录已在 `.gitignore` 中，**切勿入库、切勿丢失**（丢失后无法签发可被旧版本接受的升级包）。

**本地打包**

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = "src-tauri/.tauri-signing/TPlayerNext.key"  # 私钥内容或路径
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "<密码>"
npm run build:win
```

产物位于 `src-tauri/target/release/bundle/nsis/`：`*-setup.exe`（安装包，
同时也是自动升级包）与 `*-setup.exe.sig`（minisign 签名）。
配置开关：`tauri.conf.json` 的 `bundle.createUpdaterArtifacts`。

**FFmpeg 运行时依赖（已内置，用户零配置）**

`bin/ffmpeg.exe`、`bin/ffprobe.exe` 为 gyan.dev ffmpeg 6.1.1 **essentials
静态构建**（单文件、零外部 DLL 依赖，合计约 158MB；安装包经 NSIS 压缩后
增量约 60MB），覆盖 wma/ape/wv/tta/tak/dsf/dff/dts/aiff 等 rodio 原生格式
之外的解码与时长探测。两个 exe 通过 `bundle.resources` 随安装包释放到安装
目录 `bin/`，用户无需自行安装 FFmpeg。

> 必须使用**静态构建**（static build）。共享库构建（shared build）的
> `ffmpeg.exe` 仅约 0.6MB，但运行时依赖同目录 `avcodec-*.dll`、
> `avfilter-*.dll` 等十余个 DLL；只打包 exe 会在用户机器上弹
> 「找不到 avfilter-*.dll」系统错误框。构建脚本 `scripts/build-win.ps1`
> 会在打包前检查两个 exe 是否存在（CI 首次提交需把 `bin/` 纳入版本库）。

**GitHub Releases 发布（自动）**

1. 在仓库 Settings → Secrets and variables → Actions 配置
   `TAURI_SIGNING_PRIVATE_KEY`（私钥字符串）与 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`；
2. `plugins.updater.endpoints` 已指向
   `https://github.com/ChrisHcn1/TPlayer/releases/latest/download/latest.json`；
3. 推送版本标签：`git tag v2.0.1 && git push origin v2.0.1`。

GitHub Actions（`.github/workflows/release.yml`）会自动构建、签名、
生成 `latest.json` 并创建 Release（草稿，确认后发布）。应用启动后即会检查该清单。

> Windows 下运行 `scripts/build-win.ps1` 需文件保存为带 BOM 的 UTF-8
>（Windows PowerShell 5.1 对无 BOM 脚本按 ANSI 解析，中文注释会导致语法错误）。

> 三处版本号发布前保持一致：`package.json` / `src-tauri/Cargo.toml` /
> `src-tauri/tauri.conf.json`。

## 文档索引

- [ARCHITECTURE.md](./ARCHITECTURE.md) — 架构与设计决策
- [ROADMAP.md](./ROADMAP.md) — 里程碑与验收

## 许可

ISC（与旧版一致）。
