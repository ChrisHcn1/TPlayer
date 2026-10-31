# 项目依赖声明与合规管理规范

**版本**：1.0  
**生效日期**：2026-10-29  
**适用对象**：本组织所有软件项目（无论是否商业化、无论开源或闭源）  
**维护责任**：技术负责人（Tech Lead）与项目维护者  
**审核责任**：法务/合规负责人（如无则由技术负责人兼任）

---

## 1. 目的与原则

本规范旨在统一项目对第三方依赖的声明、标识、核查与维护方式，确保：

- **完整**：所有进入构建、测试、分发、部署环节的依赖均有据可查；
- **一致**：不同项目、不同语言栈的依赖声明格式与字段保持统一；
- **可核查**：任何人可通过声明文件与锁文件快速定位依赖来源、版本与许可证；
- **可落地**：可直接纳入 CI/CD 与日常开发流程，无需额外专用工具即可开展基线合规。

核心原则：**“声明在先，锁定版本，许可证透明，定期审计”**。

---

## 2. 依赖声明范围

本规范要求声明的依赖分为四类，缺一不可：

### 2.1 直接依赖（Direct Dependencies）

项目源码中 `import` / `use` / `require` 或在构建配置中显式列出的第三方库、框架、SDK、插件。

- 示例：Rust `Cargo.toml` 的 `[dependencies]`；Node.js `package.json` 的 `dependencies`；
- 必须声明：名称、版本约束、许可证、来源仓库/注册表、用途简述。

### 2.2 间接依赖（Transitive / Indirect Dependencies）

直接依赖进一步依赖的库，通过依赖锁文件（`Cargo.lock`、`package-lock.json`、`poetry.lock` 等）固定。

- 不需要在源码清单中逐条列出，但**必须**通过锁文件完整记录，并随版本控制提交；
- 合规审计时，需从锁文件导出完整依赖树并标注许可证。

### 2.3 第三方引用代码（Third-Party Code References）

从互联网、开源仓库、AI 生成工具、技术博客、Stack Overflow 等处复制、改写或引入的代码片段、算法实现、配置模板。

- 必须在代码文件头部或 `THIRD_PARTY_NOTICES` 中标注：来源 URL、原作者、原许可证、引入日期、是否有修改；
- AI 生成代码若未经人工审查，应标注 `AI-Generated: unreviewed`，并在合规审计时重点核查是否混入受保护代码片段。

### 2.4 程序内置组件（Bundled / Built-in Components）

随安装包或程序包一起分发的非源码组件，包括但不限于：

- 随包二进制可执行文件（如 `ffmpeg.exe`、`sacd_extract.exe`）；
- 内置字体、图标、图片、音频/视频素材、LOGO；
- 内置数据库引擎（如 SQLite amalgamation）、内置脚本、内置配置模板；
- 操作系统运行时、WebView2、Tauri 运行时等随包分发的组件。

- 必须在 `THIRD_PARTY_NOTICES` 或随包 `LICENSES/` 目录中列出名称、版本、许可证、获取方式、是否修改。

---

## 3. 格式规范

### 3.1 许可证标识

全部使用 **SPDX 许可证标识符**（https://spdx.org/licenses/），例如：

| 类型 | 示例 SPDX ID | 说明 |
|------|-------------|------|
| 宽松许可 | `MIT`, `Apache-2.0`, `BSD-3-Clause`, `ISC` | 可商用，需保留版权声明 |
| 弱 copyleft | `MPL-2.0`, `LGPL-2.1`, `LGPL-3.0` | 修改该库本身需开源，作为依赖调用可闭源 |
| 强 copyleft | `GPL-2.0`, `GPL-3.0`, `AGPL-3.0` | 衍生作品需开源，商业闭源分发需谨慎 |
| 商业/专有 | `Commercial`, `Proprietary` | 需附购买凭证或授权协议 |
| 未知/待审 | `LicenseRef-unknown` | 禁止进入生产构建，直至法务确认 |

### 3.2 版本标识方式

- **直接依赖**：在构建清单中使用语义化版本范围（如 `^1.0`, `>=0.32, <0.33`）；
- **锁定版本**：锁文件中必须记录精确版本号（如 `1.2.3`）及内容哈希（如可用）；
- **随包二进制**：记录文件版本、文件 SHA-256 哈希、下载来源 URL。

### 3.3 依赖声明字段（通用模板）

每个直接依赖在项目合规清单（`DEPENDENCIES.yml` 或 `THIRD_PARTY_NOTICES`）中应包含以下字段：

| 字段 | 必填 | 说明 | 示例 |
|------|------|------|------|
| `name` | 是 | 依赖名称 | `rodio` |
| `version` | 是 | 声明版本范围 | `0.19` |
| `locked_version` | 是 | 锁文件精确版本 | `0.19.0` |
| `license` | 是 | SPDX 标识符 | `MIT` |
| `license_file` | 否 | 许可证文本路径 | `third_party/rodio/LICENSE` |
| `source` | 是 | 来源仓库/注册表 | `https://crates.io/crates/rodio` |
| `scope` | 是 | 使用阶段 | `runtime`, `build`, `test`, `dev` |
| `purpose` | 是 | 用途简述 | `音频播放引擎封装` |
| `maintainer` | 是 | 项目内负责人 | `@tech-lead` |
| `reviewed_at` | 是 | 最后合规审核日期 | `2026-10-29` |
| `review_status` | 是 | `approved` / `pending` / `rejected` | `approved` |
| `notes` | 否 | 特殊说明（如随包二进制义务） | `独立进程调用，需随包提供源码` |

### 3.4 示例：Rust 项目 `DEPENDENCIES.yml`

```yaml
schema_version: "1.0"
project: "tplayer-next"
generated_at: "2026-10-29"

direct_dependencies:
  - name: rodio
    version: "0.19"
    locked_version: "0.19.0"
    license: "MIT"
    source: "https://crates.io/crates/rodio"
    scope: runtime
    purpose: "音频播放引擎封装"
    maintainer: "@tech-lead"
    reviewed_at: "2026-10-29"
    review_status: approved

  - name: lofty
    version: "0.22"
    locked_version: "0.22.0"
    license: "MPL-2.0"
    source: "https://crates.io/crates/lofty"
    scope: runtime
    purpose: "音频元数据标签读取"
    maintainer: "@tech-lead"
    reviewed_at: "2026-10-29"
    review_status: approved

bundled_binaries:
  - name: ffmpeg
    version: "6.x"
    sha256: "PLACEHOLDER_SHA256"
    license: "LGPL-2.1"
    source: "https://ffmpeg.org/download.html"
    purpose: "非原生格式音频解码子进程"
    notes: "独立进程调用，随包分发需附带许可证文本与源码获取方式"
    reviewed_at: "2026-10-29"
    review_status: approved

third_party_code:
  - name: "scarletbook DSD demux reference"
    source: "https://github.com/sacd-ripper/sacd-ripper"
    license: "GPL-3.0 (reference only)"
    purpose: "DSD 扇区结构参考，本项目为独立重写，未复制代码"
    reviewed_at: "2026-10-29"
    review_status: approved
```

---

## 4. 适用场景矩阵

| 场景 | 必须声明的依赖 | 版本锁定 | 许可证审核 | 交付物 |
|------|--------------|---------|-----------|--------|
| **开发（Development）** | 直接依赖 + 开发依赖 | 建议锁文件提交 | 建议审核 | `Cargo.toml` / `package.json` + 锁文件 |
| **测试（Testing）** | 测试框架、Mock 库、测试数据 | 必须锁文件 | 建议审核 | 测试依赖清单 |
| **构建（Build）** | 构建插件、代码生成器、编译器插件 | 必须锁文件 | 必须审核 | 构建依赖清单 |
| **分发（Distribution）** | 运行时依赖 + 随包二进制 + 素材 | 必须精确版本 + 哈希 | 必须审核 | `THIRD_PARTY_NOTICES`、`LICENSES/` 目录 |
| **部署（Deployment）** | 生产环境运行时、容器基镜像、系统库 | 必须精确版本 | 必须审核 | 部署清单 + SBOM（软件物料清单） |

**分发阶段强制要求**：随安装包附带 `THIRD_PARTY_NOTICES` 文件，列出所有直接依赖、间接依赖许可证汇总、随包二进制许可证及源码获取方式。

---

## 5. 执行标准

### 5.1 责任主体

| 角色 | 职责 |
|------|------|
| **开发者** | 新增依赖前填写声明字段，提交锁文件，代码引用第三方片段时标注来源 |
| **技术负责人（Tech Lead）** | 审批新增依赖，维护 `DEPENDENCIES.yml`，定期运行合规检查 |
| **法务/合规负责人** | 审核强 copyleft、商业、未知许可证依赖，判定商业化可行性 |
| **CI/CD 系统** | 自动执行依赖扫描、许可证检查、漏洞扫描，不通过则阻断构建 |

### 5.2 核查机制

1. **提交前检查（Pre-commit）**：
   - 新增依赖必须更新锁文件并提交；
   - 第三方代码引用必须在文件头部标注来源与许可证。

2. **CI 自动化检查**：
   - Rust：使用 `cargo-deny` 检查许可证、漏洞、未维护 crate；
   - Node.js：使用 `license-checker` 或 `npm audit`；
   - 通用：比对 `DEPENDENCIES.yml` 与实际锁文件是否一致，缺失则构建失败。

3. **定期审计（每季度）**：
   - 导出完整依赖树，核对许可证是否发生变更；
   - 检查是否有新增的强 copyleft 依赖进入闭源产品；
   - 更新 `reviewed_at` 与 `review_status`。

4. **发布前审核（Release Gate）**：
   - 法务签署 `THIRD_PARTY_NOTICES`；
   - 确认随包二进制的源码提供方式（如需）；
   - 生成 SBOM（软件物料清单）随发布包交付。

### 5.3 更新与维护要求

- **版本升级**：直接依赖升级需重新评审许可证与变更日志，更新 `locked_version` 与 `reviewed_at`；
- **漏洞响应**：收到安全漏洞通知后 5 个工作日内完成评估与修复或缓解；
- **废弃依赖**：不再使用的依赖须从清单与代码中移除，并从锁文件清理；
- **异常处理**：若遇 `LicenseRef-unknown` 或无法确认许可证的依赖，禁止进入生产构建，升级法务审核。

---

## 6. 合规检查清单（发布前必填）

- [ ] 所有直接依赖已在 `DEPENDENCIES.yml` 中声明并标注 SPDX 许可证
- [ ] 锁文件（`Cargo.lock` / `package-lock.json` 等）已提交版本控制
- [ ] 随包二进制（ffmpeg、sacd_extract 等）已列入 `bundled_binaries` 并附源码获取方式
- [ ] 第三方代码引用已在源码头部或 `THIRD_PARTY_NOTICES` 标注来源
- [ ] 无 `LicenseRef-unknown` 依赖进入生产构建
- [ ] 强 copyleft（GPL/AGPL）依赖已由法务评估并签字确认
- [ ] `THIRD_PARTY_NOTICES` 已随安装包分发
- [ ] SBOM 已生成并归档

---

## 7. 例外处理

任何对本规范的例外（如紧急修复需引入未审核依赖）必须：

1. 由技术负责人书面批准；
2. 在 `DEPENDENCIES.yml` 中标注 `review_status: pending` 并注明例外原因与到期日；
3. 在 2 个工作周内完成补审，否则自动标记为 `rejected` 并从构建中移除。

---

## 8. 文档维护

本规范由技术负责人维护，如有变更需经法务与技术负责人共同审核，变更后重新发布版本并通知所有项目组。

---

**附录：快速模板**

新项目可直接复制以下文件作为起点：

- `DEPENDENCIES.yml`：直接依赖声明清单（见 3.4 节示例）
- `THIRD_PARTY_NOTICES`：随分发包交付的第三方许可证汇总
- `.github/workflows/dependency-check.yml`：CI 依赖合规检查工作流（需按语言栈定制）
