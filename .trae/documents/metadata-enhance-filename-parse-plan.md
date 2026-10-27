# 实验室·元数据增强：文件名解析 + 网易云后台校验

## Context

无损音频（及部分命名不规范文件）常见「艺术家 - 歌曲」或「歌曲 - 艺术家」混写在文件名/标题里，分隔符可能是 `-`、`&`、纯空格、甚至没有（符号周围还可能带空格），导致曲库显示标题与艺术家错乱或缺失。用户要求：**无论什么命名规则先拆出来，然后联网读取相关信息校验，能对上就基本正确（约 80%）**。

用户已拍板三个决策：
1. **开关**：跟随实验室 `enhanced-metadata` 开关直接联网（不另设在线子开关）
2. **时机**：扫描时做本地文件名解析；随后**后台批量在线校验**
3. **回写**：校验结果**永久写入**曲库 SQLite

现有可复用基础：
- 实验室开关链路：`src/lab/registry.ts` → `stores/lab.ts` → `commands/lab.rs::lab_set_feature` → `library::scan::ENHANCED_METADATA` 原子变量，且切换时自动 `library_scan(true)` 强扫
- 本地解析增强已有骨架：`src-tauri/src/library/scan.rs` 的 `enhanced_metadata` / `extract_featuring` / `parse_artists_enhanced`（feat. 拆分、`&`/`x` 拆艺、VA 归一），但**不拆**「A - B」合成一串
- 网易云搜索：`src-tauri/src/lyrics/online.rs::search_song` 与 `src-tauri/src/cover.rs::search_covers` 同用 `music.163.com/api/search/get`，均未取 `songs[].name/artists[].name/album.name`——需新函数
- 持久化回写：`commands/library.rs::flush_resolved`（L655，现 dead_code）已封装 `apply_scan → player.apply_track_changes → emit("library://tracks-resolved")`，前端监听该事件合帧刷新

## 关键发现（设计前核实过）

- `library_scan(force=true)` 只绕过**根目录级** mtime 跳过（scan_library_sync L112），传给 `scan_directory` 的基线仍是全量 `scan_snapshot()`，**逐文件仍按 mtime 跳过解析**（scan.rs L702-710）。→ lab 开关切换后的强制重扫实际不会重新解析未变更文件（既有隐患），本功能入队依赖 parse_track 执行，必须一并修复。
- 前端**没有**手动编辑曲目元数据的功能（无 update_track 类命令），回写只需防「覆盖队列外曲目 / 覆盖已变化标题」。
- `apply_scan` 按 id upsert 保留 added_at，收藏/歌单关联不受影响；但回写 title/artists/album 后必须重算 `album_id_of(&album, first_artist)`。

## 实施步骤

### Step 1 — 本地文件名解析模块（新文件 `src-tauri/src/library/filename.rs`）

纯函数，`library/mod.rs` 注册：

- `split_stem(stem) -> Option<(&str, &str)>`：按优先级找**首个**分隔点——` – ` / ` — ` / ` - `（及无空格 `-`，两侧非字母连词防 "Green-Day" 误拆）/ `_` / 连续多空格(≥2) / ` & `；单空格为最低优先级。两侧 trim 后须非空，否则不拆。
- `infer_direction(a, b, ctx) -> (Direction, score)`：判定艺术家侧 vs 标题侧。启发式：前导曲目编号 `^\d{1,3}[.\s-]*`（该侧为标题侧并剥号）、父目录名（复用 `infer_album_artist` 的艺人/专辑）、标签已有 artist、标题常见词（Live/Remix/Remaster/feat./现场/伴奏→标题侧）、CJK/ASCII 特征。
- `build_candidates(stem, ctx) -> Option<Vec<Candidate>>`：方向明确→1 个候选；拿不准→2 个候选交在线裁决。选中标题侧后衔接现有 `extract_featuring` / `parse_artists_enhanced`（需在 scan.rs 标 `pub(crate)`）。
- **触发门槛**：artist 缺失或为「未知艺术家」，且标题（标签或 file_stem）含分隔符；`is_cue / iso_placeholder` 跳过。

### Step 2 — 接入扫描与入队（`src-tauri/src/library/scan.rs`）

- `parse_track` 增强分支（L300-313）先走 `filename.rs` 拆分。
- 新增模块级 `static VERIFY_QUEUE: Mutex<Vec<PendingVerify>>`（`PendingVerify { track_id, local_title, candidates, duration_ms }`）：`parse_track` 生成候选时入队（覆盖 scan / 拖拽导入 / 懒补解析三类调用点），由扫描完成处 drain。
- **修复 force 语义**：`scan_library_sync` 中 `force=true` 时传空基线（`Vec::new()`）给 `scan_directory`，使强制扫描真正全量重解析（符合 L26 注释承诺，同时让 lab 开关切换生效）。副作用核查：基线为空 → removed 为空（不误删）；耗时增加仅发生在 force 场景，符合预期。

### Step 3 — 在线搜索与裁决（`src-tauri/src/lyrics/online.rs` + 新 `src-tauri/src/library/verify.rs`）

- `online.rs` 新增 `pub struct SearchHit { id, name, artists: Vec<String>, album: Option<String>, duration_ms }` 与 `search_hits(title, artist) -> Vec<SearchHit>`：复用 `SEARCH_URL`/UA/8s 超时，解析 `result.songs[].name / artists[].name / album.name / duration`；不改现有 `search_song`。
- `verify.rs` 裁决纯函数 `judge(candidates, hits, duration_ms) -> Option<(Candidate, SearchHit)>`（可单测）：
  - 标题归一化（小写、去括号与品质后缀）相似度 ≥ 0.8，且（艺术家命中 或 时长差 ≤ 3s，沿用 `DURATION_TOLERANCE_MS`）才算命中
  - 两方向都命中取分高者；采信 `hit.artists`，仅当 album 为「未知专辑」时回填 `hit.album`；重算 `album_id`
  - 都不命中/网络失败 → **保持扫描时已写入的本地拆分结果**，不回填专辑（本地拆分已优于原样，避免噪声写库）
  - 校验前 `get_track` 复核 `title == local_title`，防覆盖期间已变化的数据

### Step 4 — 后台批量编排（`src-tauri/src/commands/library.rs`）

- 从 `flush_resolved` 提取公共 `commit_track_updates(app, upserts)`（apply_scan → apply_track_changes → emit tracks-resolved），`flush_resolved` 改调它。
- `scan_library_sync` 尾部：`ENHANCED_METADATA && !queue.is_empty()` 时 `tauri::async_runtime::spawn(spawn_meta_verify(app, queue))`（library_import_files 完成处同样 drain）。
- `spawn_meta_verify`：静态 generation + running 标志（同一时刻仅一个任务，新扫描令旧任务作废）；50 条/批，批内 `buffered(2)` 并发、批间 sleep 200-400ms；每批检查 `ENHANCED_METADATA`（关则停）与 generation；429/5xx 退避跳过该批；逐条 judge 命中后攒批 `commit_track_updates`。
- 退出安全：RunEvent::Exit 已有 save_state 兜底，中断只损失未确认条目（本地结果已持久化）。

### Step 5 — 前端（1 个文件）

- `src/lab/registry.ts` 的 `enhanced-metadata` 描述追加：「按文件名拆分 标题/艺术家/专辑（支持 -、&、空格等分隔与多种顺序），并联网搜索校验（跟随本开关）」。
- 进度提示不做新 UI：靠 `library://tracks-resolved` 增量事件实时刷新。

### Step 6 — 测试

- `filename.rs` `#[cfg(test)]`（沿用 scan.rs 风格）：`周杰伦 - 晴天` / `晴天 - 周杰伦`（借父目录定方向）、`01 - Song`（剥号）、`Artist-Song`、`Artist_Song`、`A & B`、双空格、无分隔符纯标题（不拆）、`A - B - C`（首分隔点）、AC/DC 不误伤、Green-Day 类连词不误拆。
- `verify.rs::judge`：命中、时长对齐、双不命中回退本地。

## 修改文件清单

| 文件 | 动作 |
|---|---|
| `src-tauri/src/library/filename.rs` | 新建（解析 + 单测） |
| `src-tauri/src/library/verify.rs` | 新建（judge + spawn_meta_verify 编排 + 单测） |
| `src-tauri/src/library/mod.rs` | 注册新模块 |
| `src-tauri/src/library/scan.rs` | parse_track 接入、VERIFY_QUEUE、expose 内部函数 |
| `src-tauri/src/lyrics/online.rs` | 新增 SearchHit / search_hits |
| `src-tauri/src/commands/library.rs` | force 空基线、commit_track_updates 提取、扫描/导入后 spawn 校验 |
| `src/lab/registry.ts` | 描述更新 |

## 已知取舍

- 校验未完成即退出：本地拆分已落库，下次强扫重新入队补校。
- force 全量重扫会用本地结果覆盖在线结果，随后自动重新在线校验写回（最终态正确，多一次网络往返）；日常非 force 扫描 mtime 跳过，无重复请求。
- 纯本地解析（关联网开关前）离线可用；联网步骤按用户决策由实验室开关授权。

## 验证

1. `cd src-tauri && cargo test`（filename/judge 单测全过）→ `cargo clippy`
2. `npm run build`（vue-tsc 通过）
3. dev 实测：放若干乱名无损文件（`周杰伦 - 晴天.flac`、`晴天 周杰伦.flac`、`01 - Song.ape` 等）→ 开实验室「元数据增强」（触发强扫）→ 观察列表实时刷新出正确 标题/艺术家/专辑，SQLite `library.db` 中 tracks 表已持久化、album_id 聚合正确 → 关开关再扫，行为回退正常
4. 网络异常容错：断网下开开关扫描，本地拆分照常、校验静默跳过、应用不报错
