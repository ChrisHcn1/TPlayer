//! 曲库相关命令
//!
//! 安全约定：凡接收路径参数的命令，都必须经 `security::ensure_allowed` 校验
//! （白名单 = config.library_dirs）。library_add_dir 的路径来自 dialog 选择器，可信。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::cover;
use crate::error::{AppError, AppResult};
use crate::library::models::{
    disc_key, Playlist, RecentEntry, ScanProgress, ScanStage, Track,
};
use crate::library::scan::{self, collect_import_tracks, list_cue_files, parse_track, scan_directory};
use crate::library::verify;
use crate::library::sacd::{pre_export_sacd_dsf, SacdDisc};
use crate::library::cue::{parse_cue, parse_cue_light};
use crate::AppState;

/// 启动一次曲库扫描（增量）
///
/// `force = true` 时忽略「未变更则跳过」优化，强制重新遍历 + 解析（用于用户
/// 改了标签却未触碰目录 mtime 等需要强制刷新的场景）；默认 `false` 走智能跳过。
#[tauri::command]
pub async fn library_scan(app: AppHandle, force: bool) -> AppResult<()> {
    // 扫描是重活（遍历整个曲库树 + 全库去重 SQL + 写库），必须放到阻塞线程池，
    // 绝不能跑在主线程上——否则大曲库 / 网络盘扫描会把整个界面拖成「未响应」。
    tauri::async_runtime::spawn_blocking(move || scan_library_sync(&app, force))
        .await
        .map_err(|e| AppError::Library(format!("扫描任务执行失败: {e}")))?
}

/// `library_scan` 的实际同步实现（在阻塞线程池上运行）。
fn scan_library_sync(app: &AppHandle, force: bool) -> AppResult<()> {
    let state = app.state::<AppState>();
    let roots: Vec<PathBuf> = {
        let config = state.config.lock().unwrap();
        config
            .library_dirs
            .iter()
            .map(PathBuf::from)
            .collect()
    };
    eprintln!("[scan-debug] scan_library_sync ENTRY: roots={:?} force={}", roots, force);

    if roots.is_empty() {
        return Err(AppError::Library("未配置曲库目录，请先在设置中添加".into()));
    }

    // 扫描前先校验每个根目录是否可达：网络盘未挂载 / 凭据缺失 / 路径非法（如
    // SMB 共享写成 172.18.0.21/songs 而非常规 \\\\host\\share，或相对路径）都会被
    // 静默吞掉导致「扫完 0 文件」。这里显式报错让用户看到真实原因。
    //
    // Linux 补充：NTFS 分区（ntfs-3g / ntfs3）按挂载选项决定归属，目录可 stat
    // 但当前用户无读权限时 read_dir 返回 EACCES——metadata 探测不到这一点，
    // 会在扫描阶段被逐目录跳过。Linux 追加一次 read_dir 预检，把「挂载点存在
    // 但不可读」与「挂载点不存在」区分开；错误文案按平台分支，Windows 保持
    // 原有 SMB 提示逐字不变。
    let mut inaccessible = Vec::new();
    for root in &roots {
        if std::fs::metadata(root).is_err() {
            inaccessible.push(root.to_string_lossy().to_string());
            continue;
        }
        #[cfg(target_os = "linux")]
        if let Err(e) = std::fs::read_dir(root) {
            inaccessible.push(format!(
                "{}：目录不可读（{}）。NTFS 分区请检查挂载选项：\
                 以 root 挂载并指定 uid/gid（当前用户），或为 ntfs-3g 启用 permissions 选项",
                root.display(),
                e
            ));
        }
    }
    if !inaccessible.is_empty() {
        #[cfg(target_os = "linux")]
        let message = format!(
            "以下曲库目录无法访问（未挂载 / 无读取权限 / 路径非法；\
             请确认分区已挂载且当前用户有读取权限）：{}",
            inaccessible.join("；")
        );
        #[cfg(not(target_os = "linux"))]
        let message = format!(
            "以下曲库目录无法访问（网络盘未挂载 / 凭据缺失 / 路径非法；Windows SMB 共享需以 \\\\host\\share 形式，如 \\\\172.18.0.21\\songs，或将共享映射为盘符如 Y:\\songs）：{}",
            inaccessible.join("；")
        );
        return Err(AppError::Library(message));
    }

    // 计算各根目录自身 mtime（用于「未变更则跳过扫描」判断，开销仅为对根目录的 stat）
    let cur_roots: Vec<(String, i64)> = roots
        .iter()
        .map(|r| {
            let m = std::fs::metadata(r)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            (r.to_string_lossy().to_string(), m)
        })
        .collect();

    // —— 未变更则跳过扫描（直接复用曲库 DB 已缓存的解析 / 解码结果）——
    // 仅当所有根目录的「自身 mtime」都与上次扫描记录一致时才跳过：目录 mtime 在
    // 其下条目被增删 / 改名时会随之变化，故一致即代表自上次扫描以来曲目集合未变，
    // 无需重新遍历整棵目录树，也无需重新解析 / 解码任何文件，启动与重复扫描瞬时完成。
    // （`force` 或根目录配置发生变化、或无历史记录时走完整增量扫描。）
    if !force {
        let prev = {
            let g = state.library.lock().unwrap_or_else(|e| e.into_inner());
            g.load_scan_roots()
        };
        let all_stable = match &prev {
            Some(p) => {
                p.len() == cur_roots.len()
                    && p.iter().all(|(path, mtime)| {
                        cur_roots
                            .iter()
                            .any(|(cp, cm)| cp == path && cm == mtime && *cm != 0)
                    })
            }
            None => false,
        };
        eprintln!("[scan-debug] stable-check: force={} all_stable={}", force, all_stable);
        if all_stable {
            // 直接复用：从 DB 装载曲目到播放器（与上次一致），不发任何扫描进度，
            // 前端因此不进入「扫描中」态——用户无感知地即时可用，免去等候。
            let tracks = {
                let g = state.library.lock().unwrap_or_else(|e| e.into_inner());
                g.tracks()
            };
            eprintln!("[scan-debug] STABLE-SKIP: reusing DB (no scan). tracks by root:");
            {
                let mut bk: Vec<(String, usize)> =
                    roots.iter().map(|r| (r.to_string_lossy().to_string(), 0)).collect();
                for t in &tracks {
                    for b in bk.iter_mut() {
                        if t.file_path.starts_with(&b.0) {
                            b.1 += 1;
                            break;
                        }
                    }
                }
                for (r, c) in &bk {
                    eprintln!("[scan-debug]   root {r} = {c}");
                }
            }
            state.player.lock().unwrap().set_tracks(tracks);
            // 仅当确有未完成的 SACD 占位（如首次扫描中途被中断、ISO 尚未拆轨）
            // 才补跑一次解析；曲库未变且已解析完时完全跳过，**避免每次启动都无谓地
            // 触发 SACD 解析线程 —— 这是「曲库没变就不浪费系统资源」的关键一环。
            let has_pending_sacd = {
                let g = state.library.lock().unwrap_or_else(|e| e.into_inner());
                g.pending_tracks().iter().any(|t| t.iso_placeholder)
            };
            if has_pending_sacd {
                spawn_sacd_resolve(app);
            }
            return Ok(());
        }
    }

    // 一次性全量解析：目录遍历 + 逐文件元数据/时长(lofty/ffprobe) + CUE 精确拆分，
    // 一次完成、时长全部正确。整段在 `library_scan` 的 spawn_blocking 里跑（不阻塞
    // 主线程 / UI），进度经 `library://scan-progress` 上报；封面与 SACD 拆轨留按需。
    state.scan_cancel.store(false, std::sync::atomic::Ordering::SeqCst);
    // force = 真正的全量重解析（传标志，基线仍保留）：逐文件 mtime 跳过全部失效，
    // 否则 lab 开关切换后的强扫不会重新解析未变更文件，增强解析与入队都不生效。
    // 基线保留 → 删除检测照常；ISO 的 unchanged 仍按 mtime 判定 → 不触发重复拆轨。
    let snapshot = state.library.lock().unwrap_or_else(|e| e.into_inner()).scan_snapshot();
    let result = scan_directory(&roots, &snapshot, app, force, &state.scan_cancel);
    let changed = result.changed_count;

    let mut library = state.library.lock().unwrap_or_else(|e| e.into_inner());
    library.apply_scan(&result.upserts, &result.removed_ids)?;
    eprintln!("[scan-debug] FULL-SCAN committed: upserts={} removed={}", result.upserts.len(), result.removed_ids.len());
    {
        let mut bk: Vec<(String, usize)> =
            roots.iter().map(|r| (r.to_string_lossy().to_string(), 0)).collect();
        for t in &result.upserts {
            for b in bk.iter_mut() {
                if t.file_path.starts_with(&b.0) {
                    b.1 += 1;
                    break;
                }
            }
        }
        for (r, c) in &bk {
            eprintln!("[scan-debug]   upsert by root {r} = {c}");
        }
    }

    // 本次扫描成功提交 → 记为「好状态」快照（崩溃回退的还原点，内部 5 分钟节流）
    library.commit_good_snapshot();
    // 记录根目录指纹，供下次扫描判断「是否未变更可跳过」
    let _ = library.save_scan_roots(&cur_roots);

    // 刷新播放器曲目缓存（占位轨道同样进入缓存，列表立即可渲染）
    let tracks = library.tracks();
    drop(library);
    state.player.lock().unwrap().set_tracks(tracks);

    // 变更/删除曲目封面内存缓存失效（磁盘缓存保留，文件恢复后仍可复用）
    if changed > 0 || !result.removed_ids.is_empty() {
        cover_cache().lock().unwrap().clear();
    }

    // 实验室·元数据增强：扫描期入队的本地拆分候选 → 后台批量在线校验并回写
    verify::drain_and_spawn(app);

    // 不再常驻后台补解析普通曲目（那会把界面卡成「未响应」）。但 SACD ISO 拆轨
    // 很慢（sacd_extract 逐轨），放扫描里会拖住加载，故扫描后用一个**只处理 SACD**
    // 的后台线程按需拆：ISO 通常很少，逐个解析、让出 CPU、只短暂持库锁，
    // 不阻塞主加载与界面；结果经 library://tracks-resolved 增量更新前端。
    spawn_sacd_resolve(app);
    Ok(())
}

/// 扫描后按需拆解 SACD ISO 占位（虚拟分轨）并**预导出每轨 DSF 到缓存**。
///
/// 仅处理 `iso_placeholder` 条目（数量少），解析慢但逐个来、每轮让出 CPU，写库只短暂持锁，
/// 不阻塞主加载与界面；结果经 `library://tracks-resolved` 增量更新前端。拆轨后的 ISO 不再是
/// 占位，下次扫描自动跳过，幂等。
///
/// 健壮性：单碟坏数据（坏 ISO / 解析 panic）用 `catch_unwind` 隔离，不会把整条后台线程带崩
/// （否则排在后面的 ISO 全部漏解析）；单轨导出失败只记日志，不影响其余轨与整体拆轨。
/// 进度经 `library://scan-progress`（stage = `backfilling`）上报，前端据此显示「后台解析 x/y」。
pub(crate) fn spawn_sacd_resolve(app: &AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || {
        let Some(state) = handle.try_state::<AppState>() else {
            return;
        };
        let isos: Vec<Track> = {
            let library = state.library.lock().unwrap_or_else(|e| e.into_inner());
            library
                .pending_tracks()
                .into_iter()
                .filter(|t| t.iso_placeholder)
                .collect()
        };
        let total = isos.len() as u32;
        let mut done = 0u32;
        for iso in &isos {
            // 单碟坏数据不能带崩整条后台线程：panic 隔离 + 记日志，后续 ISO 仍照常拆轨
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                resolve_sacd_iso(iso)
            }));
            match outcome {
                Ok(Some(o)) => {
                    if !o.upserts.is_empty() || !o.removed.is_empty() {
                        flush_resolved(
                            state.inner(),
                            &handle,
                            o.upserts.clone(),
                            o.upserts,
                            o.removed,
                            o.remap,
                        );
                    }
                }
                Ok(None) => {
                    eprintln!(
                        "[library] SACD 拆轨无结果（可能非 SACD 镜像）：{}",
                        iso.file_path
                    );
                }
                Err(e) => {
                    eprintln!(
                        "[library] SACD 拆轨线程 panic（{}）：{:?}",
                        iso.file_path, e
                    );
                }
            }
            done += 1;
            // 上报进度：复用 backfilling 阶段，前端据此显示「后台解析 x/y」（含 SACD 拆轨）
            emit_backfill_progress(&handle, done, total, Some(iso.file_path.clone()));
            // 让出：ISO 拆轨慢，避免长时间占用 CPU / IO
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        // 收尾：SACD 拆轨完成回到 Done（仅在有 SACD 时才发，避免无谓事件）
        if total > 0 {
            emit_scan_stage(&handle, ScanStage::Done, done, total, None);
        }
    });
}

/// 拆一个 SACD ISO 占位为虚拟分轨，并在扫描期**预导出每轨 DSF 到缓存**，
/// 使后续播放无需再花长时间拆轨 / 解码。返回待写库的解析产物；解析失败返回 `None`。
///
/// 与 `resolve_placeholder` 的 SACD 分支等价（建虚拟轨 + 占位→分轨重映射），额外在写库前
/// 预导出每轨 DSF：播放链路 `spawn_sacd_decoder` 看到缓存命中即跳过导出，起播更快。
fn resolve_sacd_iso(iso: &Track) -> Option<ResolveOutcome> {
    let path = Path::new(&iso.file_path);
    let mut disc = SacdDisc::open(path)?;
    disc.probe_durations();
    let tracks = disc.to_tracks();
    if tracks.is_empty() {
        return None;
    }
    // 预导出每轨 DSF（即使个别轨失败也不影响其余轨与整体拆轨）
    for t in &tracks {
        let idx = t.sacd_track_index as usize;
        if let Err(e) = pre_export_sacd_dsf(path, &disc, idx) {
            eprintln!(
                "[library] SACD 预导出失败（{} 轨 {}）：{}",
                iso.file_path, idx, e
            );
        }
    }
    let mut remap = HashMap::new();
    remap.insert(iso.id.clone(), tracks.iter().map(|t| t.id.clone()).collect());
    Some(ResolveOutcome {
        upserts: tracks,
        removed: vec![iso.id.clone()],
        existing: Vec::new(),
        remap,
    })
}

/// 两阶段懒加载 - 阶段二「后台补解析」（异步线程，不阻塞 UI）。
///
/// **按本地 / 网络分流**（用户明确要求区分两类路径）：
///  - 本地磁盘：**深度解析**——元数据 + CUE 分轨 + SACD 虚拟分轨一次到位，
///    用户在播放列表里立刻能看到分轨（不必等到点播放）；
///  - 网络共享：**浅解析**——只读文件头取标签 / 时长（lofty / ffprobe 头部），
///    CUE 分轨与 SACD 虚拟分轨需要读取整碟数据，留待播放时按需解析，
///    避免一次把共享带宽打满。
///
/// 解析结果按批写库，并通过 `library://tracks-resolved` 推送前端增量更新
/// （带 `removedIds`：整碟镜像被分轨取代时，占位条目需同步移除）；
/// 进度经 `library://scan-progress`（stage = `backfilling`）上报，
/// 前端据此显示「后台解析中 x/y」。
///
/// 单趟处理全部待解析条目（本地在前、网络在后）；解析失败的文件保留占位，
/// 下次扫描 / 播放时再试。`cancel` 由 `library_cancel_scan` 置位，用户主动
/// 取消时立即中止，已写入的批次保留，剩余条目留待下次扫描 / 启动继续。
#[allow(dead_code)]
pub(crate) fn spawn_lazy_backfill(
    app: &AppHandle,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let handle = app.clone();
    std::thread::spawn(move || {
        let Some(app_state) = handle.try_state::<AppState>() else {
            return;
        };

        // 待解析条目：本地优先（让用户尽快看到 CUE / SACD 分轨），网络随后
        let pending = match app_state.library.lock() {
            Ok(g) => g.pending_tracks(),
            Err(_) => return,
        };
        if pending.is_empty() {
            return;
        }
        let (local, remote): (Vec<Track>, Vec<Track>) = pending
            .into_iter()
            .partition(|t| !is_network_path(&t.file_path));

        let total = (local.len() + remote.len()) as u32;
        let mut processed = 0u32;
        // 库内已入库的 CUE 分轨（增量局部发现时用于清理失效分轨）
        let known_cue_tracks = {
            let library = app_state.library.lock().unwrap_or_else(|e| e.into_inner());
            library.cue_tracks()
        };
        let mut ctx = ResolveContext::new(known_cue_tracks, true);

        for (batch, depth) in [(local, ResolveDepth::Deep), (remote, ResolveDepth::Shallow)] {
            let mut upserts: Vec<Track> = Vec::new();
            let mut resolved: Vec<Track> = Vec::new();
            let mut removed: Vec<String> = Vec::new();
            let mut remaps: HashMap<String, Vec<String>> = HashMap::new();
            for placeholder in batch {
                if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                // 单条坏数据（坏 ISO / 坏 CUE）不能把整条后台线程带崩，
                // 否则排在后面的曲目（含 CUE 分轨）会全部漏解析。
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    resolve_placeholder(&placeholder, depth, &mut ctx)
                }));
                let outcome = match outcome {
                    Ok(o) => o,
                    Err(_) => {
                        eprintln!(
                            "[library] 解析占位时 panic，跳过该条目: {}",
                            placeholder.file_path
                        );
                        ResolveOutcome::empty()
                    }
                };
                if !outcome.upserts.is_empty() {
                    upserts.extend(outcome.upserts.iter().cloned());
                    resolved.extend(outcome.upserts.iter().cloned());
                }
                removed.extend(outcome.removed.iter().cloned());
                for (k, v) in outcome.remap {
                    remaps.insert(k, v);
                }
                processed += 1;

                // 进度：每 8 条上报一次（避免事件洪泛）
                if processed % 8 == 0 || processed == total {
                    emit_backfill_progress(
                        &handle,
                        processed,
                        total,
                        Some(placeholder.file_path.clone()),
                    );
                }
                // 每 32 条提交一批，避免长时间占用库锁
                if upserts.len() >= 32 {
                    flush_resolved(
                        app_state.inner(),
                        &handle,
                        std::mem::take(&mut upserts),
                        std::mem::take(&mut resolved),
                        std::mem::take(&mut removed),
                        std::mem::take(&mut remaps),
                    );
                    // 让出 CPU / IO：后台解析是低优先级任务，主动歇一下，
                    // 避免把机器跑满、影响正在进行的音频解码与 UI 交互。
                    std::thread::sleep(std::time::Duration::from_millis(12));
                }
                // 每条之间也让出时间片，进一步降低与播放线程的争抢
                std::thread::yield_now();
            }
            flush_resolved(
                app_state.inner(),
                &handle,
                upserts,
                resolved,
                removed,
                remaps,
            );
        }

        // 收尾：进度回到 done（前端把「后台解析」指示器收起）
        emit_scan_stage(&handle, ScanStage::Done, processed, total, None);
    });
}

/// 后台补解析的深度策略：本地与网络走完全不同的解析路径。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ResolveDepth {
    /// 深度解析（本地磁盘）：元数据 + CUE 分轨 + SACD 虚拟分轨。
    Deep,
    /// 浅解析（网络共享）：只读文件头取标签 / 时长；虚拟分轨留待播放时按需解析。
    Shallow,
}

/// 单个占位轨道的解析产物。
struct ResolveOutcome {
    /// 本次解析产出、需要写入曲库的曲目
    upserts: Vec<Track>,
    /// 需要从曲库移除的占位 id（整碟镜像被 CUE 分轨取代时）
    removed: Vec<String>,
    /// 分轨此前已入库（无需重复写入）；播放路径直接复用这一份
    existing: Vec<Track>,
    /// 重映射：被替换的占位 id → 新分轨 id（CUE / SACD 场景）。
    /// 用于同步播放队列，使后台解析不打断正在进行的播放。
    remap: HashMap<String, Vec<String>>,
}

impl ResolveOutcome {
    fn empty() -> Self {
        Self {
            upserts: Vec::new(),
            removed: Vec::new(),
            existing: Vec::new(),
            remap: HashMap::new(),
        }
    }
}

/// 解析上下文：持有「整碟键 → CUE 分轨」索引与「库内已入库分轨」索引，
/// 供 `resolve_placeholder` 判定一个占位轨道应被拆成哪些 CUE 分轨。
///
/// 索引采用**增量局部发现**：`ensure_cue_for(audio_path)` 只扫该音频文件所在目录
/// 及有限层父目录（已扫目录缓存，整库每个目录至多扫一次），**绝不递归整棵曲库树**。
/// 因此：
/// - 点播时按需解析只扫相关几个目录，瞬时返回，不再阻塞 UI / 播放控制线程；
/// - 后台补解析线程复用同一逻辑，覆盖度与旧版一致（镜像与 CUE 不必同级，
///   只要 CUE 位于音频文件的祖先目录内即可），同时把 CUE 发现的 IO 成本摊到
///   各占位解析上，而非启动时一次性全树扫描。
struct ResolveContext {
    /// 整碟键 → CUE 分轨（按需增量填充）
    cue_by_disc: HashMap<(String, String), Vec<Track>>,
    /// 库内已入库的 CUE 分轨（按整碟键索引），用于清理「CUE 已删 / 改名」后
    /// 残留的失效分轨（避免列表里留下重复条目）。
    known_discs: HashMap<(String, String), Vec<Track>>,
    /// 已扫描过 CUE 的目录（非递归列举 + 解析），避免重复扫描
    scanned_dirs: HashSet<PathBuf>,
    /// 廉价模式：后台线程只做「CUE 文本分轨检测」，**不探测音频 / 不读封面 /
    /// 不解析 SACD**——这些昂贵工作全部改为按需（滚动 / 播放时才做）。
    /// 点播路径 `resolve_one` 用全量模式（probe 精确时长）。
    cheap: bool,
}

impl ResolveContext {
    /// 以「库内已入库的 CUE 分轨」构建上下文（不扫描文件系统）。
    /// `cheap=true` 为后台廉价检测模式（只见 CUE 文本，不碰音频）。
    fn new(known_cue_tracks: Vec<Track>, cheap: bool) -> Self {
        // 库内已入库分轨：按整碟键索引（清理失效分轨用）
        let mut known_discs: HashMap<(String, String), Vec<Track>> = HashMap::new();
        for t in known_cue_tracks {
            if let Some(key) = disc_key(&t.file_path) {
                known_discs.entry(key).or_default().push(t);
            }
        }
        Self {
            cue_by_disc: HashMap::new(),
            known_discs,
            scanned_dirs: HashSet::new(),
            cheap,
        }
    }

    /// 确保 `audio_path` 所在目录及**全部祖先目录**内的 `.cue` 已解析并入索引。
    ///
    /// 只在尚未扫描过的目录上做「非递归列举 .cue + 解析」，整库每个目录至多扫一次。
    /// 沿音频文件的祖先链向上扫到根（不递归整棵子树），因此：
    /// - 覆盖度与旧版全树扫描一致——CUE 位于镜像任意上级目录都能命中
    ///   （如 `Album.cue` 在根、`FILE "sub/CDImage.flac"`）；
    /// - 开销极小：每个相关目录只扫一次；廉价模式只读 .cue 文本、不探测音频。
    fn ensure_cue_for(&mut self, audio_path: &str) {
        let mut dir = Path::new(audio_path).parent().map(|p| p.to_path_buf());
        while let Some(d) = dir {
            if !self.scanned_dirs.contains(&d) {
                self.scanned_dirs.insert(d.clone());
                for cue in list_cue_files(&d) {
                    let parsed = if self.cheap {
                        parse_cue_light(&cue)
                    } else {
                        parse_cue(&cue)
                    };
                    for track in parsed {
                        if let Some(key) = disc_key(&track.file_path) {
                            self.cue_by_disc.entry(key).or_default().push(track);
                        }
                    }
                }
            }
            // 向上一层；根目录的 parent 指向自身，需判等防死循环
            match d.parent() {
                Some(p) if p != d => dir = Some(p.to_path_buf()),
                _ => break,
            }
        }
    }
}

/// 解析一个占位轨道（**不写库**），返回待入库曲目与待移除的占位 id。
///
/// - 普通音频占位：解析完整元数据；若所在目录的 CUE 引用了它（整碟镜像），
///   则产出 CUE 分轨并移除镜像占位；
/// - SACD ISO 占位（iso_placeholder）：`SacdDisc::open` 完整解析，产出虚拟
///   分轨并移除占位；解析失败保留占位（前端提示暂不可解析）；
/// - 网络共享（`ResolveDepth::Shallow`）：只取元数据，虚拟分轨留待播放时解析。
fn resolve_placeholder(
    placeholder: &Track,
    depth: ResolveDepth,
    ctx: &mut ResolveContext,
) -> ResolveOutcome {
    let path = Path::new(&placeholder.file_path);

    // SACD ISO 占位：拆虚拟分轨要读整张镜像——本地深度解析，
    // 网络共享留待播放时按需解析（否则一次就把共享带宽打满）。
    // 廉价（后台检测）模式：跳过，留待播放时按需解析。
    if placeholder.iso_placeholder {
        if depth == ResolveDepth::Shallow || ctx.cheap {
            return ResolveOutcome::empty();
        }
        let Some(mut disc) = SacdDisc::open(path) else {
            return ResolveOutcome::empty();
        };
        disc.probe_durations();
        let tracks = disc.to_tracks();
        if tracks.is_empty() {
            return ResolveOutcome::empty();
        }
        let mut remap = HashMap::new();
        remap.insert(
            placeholder.id.clone(),
            tracks.iter().map(|t| t.id.clone()).collect(),
        );
        return ResolveOutcome {
            upserts: tracks,
            removed: vec![placeholder.id.clone()],
            existing: Vec::new(),
            remap,
        };
    }

    // CUE 分轨（仅本地深度解析）。
    // 分轨的标题 / 专辑 / 时长全部来自 CUE 本身，不依赖镜像能否解析出元数据，
    // 故这一步排在 parse_track 之前（整碟镜像的元数据常为空）。
    //
    // 匹配基于「整碟键」（目录 + 基名），与镜像 / CUE 是否同级无关——只要 CUE
    // 引用的音频文件就是库内这个镜像即可（复刻旧版 scan_directory 的覆盖度）。
    if depth == ResolveDepth::Deep {
        // 增量局部发现：只扫本曲目所在目录及有限层父目录的 .cue（已扫目录缓存），
        // 不递归整棵曲库树——点播路径只扫相关几个目录、瞬时返回，不阻塞 UI；
        // 后台补解析线程复用同一逻辑，把 CUE 发现的 IO 成本摊到各占位解析上。
        // 网络共享走 Shallow 分支，不在此触发任何 CUE 发现（避免打满带宽）。
        ctx.ensure_cue_for(&placeholder.file_path);
        if let Some(key) = disc_key(&placeholder.file_path) {
            // 情况一：某个 CUE 引用了该镜像 → 用分轨取代整碟占位
            if let Some(matching) = ctx.cue_by_disc.get(&key) {
                let matching = matching.clone();
                // 旧分轨（cue 改名 / 改内容后 id 变化的那些）需清理；
                // 注意**不能**删除本轮即将写入的同 id 分轨——apply_scan 先 upsert
                // 后 remove，同 id 会把自己删掉。
                let matching_ids: std::collections::HashSet<String> =
                    matching.iter().map(|t| t.id.clone()).collect();
                let mut removed: Vec<String> = ctx
                    .known_discs
                    .get(&key)
                    .map(|tracks| tracks.iter().map(|t| t.id.clone()).collect::<Vec<_>>())
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|id| !matching_ids.contains(id))
                    .collect();
                removed.push(placeholder.id.clone());
                let mut remap = HashMap::new();
                remap.insert(
                    placeholder.id.clone(),
                    matching.iter().map(|t| t.id.clone()).collect(),
                );
                return ResolveOutcome {
                    upserts: matching,
                    removed,
                    existing: Vec::new(),
                    remap,
                };
            }
            // 情况二：该整碟已不被任何 CUE 引用（cue 被删 / 改名）→ 退回普通单曲，
            // 同时清掉失效的旧分轨，避免列表里留下重复条目。
            // 注意：占位 id **不能**进本分支的「去重」移除——普通单曲用同一个 id
            // （upsert 覆盖占位），同 id 先 upsert 后 remove 会把自己删掉。
            if let Some(stale) = ctx.known_discs.get(&key) {
                let mut removed: Vec<String> = stale.iter().map(|t| t.id.clone()).collect();
                let Some(mut full) = parse_track(path) else {
                    // 解析失败：保留占位（含失效旧分轨），交由播放时按需解析
                    return ResolveOutcome {
                        upserts: Vec::new(),
                        removed,
                        existing: Vec::new(),
                        remap: HashMap::new(),
                    };
                };
                full.id = placeholder.id.clone();
                removed.push(placeholder.id.clone());
                return ResolveOutcome {
                    upserts: vec![full],
                    removed,
                    existing: Vec::new(),
                    remap: HashMap::new(),
                };
            }
        }
    }

    // 普通单曲：同 id 覆盖占位。
    // 注意**不能**同时把该 id 放进 removed——`apply_scan` 先 upsert 后 remove，
    // 同 id 会把自己删掉（历史 bug：解析完成的曲目在库里凭空消失）。
    //
    // 廉价（后台检测）模式：跳过音频标签解析——普通曲目元数据改为按需
    // （滚动可见 / 播放时才解析），避免后台把整库音频读一遍拖垮性能。
    if ctx.cheap {
        return ResolveOutcome::empty();
    }
    let Some(mut full) = parse_track(path) else {
        // 解析失败：保留占位，交由播放时按需解析
        return ResolveOutcome::empty();
    };
    full.id = placeholder.id.clone();
    ResolveOutcome {
        upserts: vec![full],
        removed: Vec::new(),
        existing: Vec::new(),
        remap: HashMap::new(),
    }
}

/// 提交一批解析结果到曲库，并推送前端增量事件。
#[allow(dead_code)]
fn flush_resolved(
    app_state: &AppState,
    handle: &AppHandle,
    upserts: Vec<Track>,
    resolved: Vec<Track>,
    removed: Vec<String>,
    remaps: HashMap<String, Vec<String>>,
) {
    if upserts.is_empty() && removed.is_empty() {
        return;
    }
    // 防御：`apply_scan` 先 upsert 后 remove，同 id 会把自己删掉。
    // 移除清单里与本批待写入曲目同 id 的条目（历史 bug：解析完成的曲目凭空消失）。
    let upsert_ids: std::collections::HashSet<&str> =
        upserts.iter().map(|t| t.id.as_str()).collect();
    let removed: Vec<String> = removed
        .into_iter()
        .filter(|id| !upsert_ids.contains(id.as_str()))
        .collect();
    {
        let Ok(mut library) = app_state.library.lock() else {
            return;
        };
        if let Err(e) = library.apply_scan(&upserts, &removed) {
            eprintln!("[library] 后台补解析写库失败: {e}");
        }
        // 注意：不在此调用 library.tracks()。全库去重 SQL 在大曲库下开销可观，
        // 每批（约 32 条）都跑一遍会在持锁期间长时间拖住主线程，造成「未响应」。
        // 改为增量维护播放器缓存（只增删本批变更），library 锁在此尽快释放。
        drop(library);
        // 播放列表缓存增量同步；并平滑迁移被替换占位的队列引用，不打断播放
        let mut player = app_state.player.lock().unwrap();
        player.apply_track_changes(&upserts, &removed);
        player.remap_context(&remaps);
    }
    if !resolved.is_empty() || !removed.is_empty() {
        let _ = handle.emit(
            "library://tracks-resolved",
            serde_json::json!({ "tracks": &resolved, "removedIds": &removed }),
        );
    }
}

/// 后台补解析进度上报（stage = backfilling）。
#[allow(dead_code)]
fn emit_backfill_progress(
    app: &AppHandle,
    processed: u32,
    total: u32,
    current_path: Option<String>,
) {
    emit_scan_stage(
        app,
        ScanStage::Backfilling,
        processed,
        total,
        current_path,
    );
}

/// 扫描 / 补解析进度事件上报。
#[allow(dead_code)]
fn emit_scan_stage(
    app: &AppHandle,
    stage: ScanStage,
    processed: u32,
    total: u32,
    current_path: Option<String>,
) {
    let _ = app.emit(
        "library://scan-progress",
        ScanProgress {
            stage,
            processed,
            total,
            current_path,
            error: None,
        },
    );
}

/// 判断路径是否为网络共享路径。
///
/// - UNC：`\\host\share` 或 `//host/share`；
/// - Windows 映射盘符（`Y:\music` 实际指向网络共享）——按 `GetDriveTypeW`
///   返回 `DRIVE_REMOTE` 判定，否则「把共享映射成盘符」的用户会被当成本地盘，
///   后台补解析会逐文件走网络 IO，重新引入卡顿；
/// - Linux：`/mnt/`、`/run/media/` 等非常驻挂载前缀。
#[allow(dead_code)]
fn is_network_path(path: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        path.starts_with("//") || path.starts_with("/mnt/") || path.starts_with("/run/media/")
    }
    #[cfg(not(target_os = "linux"))]
    {
        if path.starts_with("\\\\") || path.starts_with("//") {
            return true;
        }
        is_windows_remote_drive(path)
    }
}

/// Windows：路径带盘符且该盘是「网络驱动器」时为 true。
#[allow(dead_code)]
#[cfg(target_os = "windows")]
fn is_windows_remote_drive(path: &str) -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDriveTypeW;

    let bytes = path.as_bytes();
    if bytes.len() < 2 || bytes[1] != b':' || !bytes[0].is_ascii_alphabetic() {
        return false;
    }
    let root: Vec<u16> = format!("{}:\\", (bytes[0] as char).to_ascii_uppercase())
        .encode_utf16()
        .collect();
    // Windows 常量：DRIVE_FIXED = 3，DRIVE_REMOTE = 4。此前误写成 3 会把本地盘
    // 误判为网络盘（进而被后台补解析跳过，本地条目长期停在 0:00），此处修正为 4。
    unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) == 4 }
}

/// 两阶段懒加载 - 阶段二「播放时按需解析」。
///
/// 播放某曲目（或显式点击 SACD/CUE 条目）前调用：把占位轨道解析为完整轨道
/// （含元数据 / 封面 / CUE 分轨 / SACD 分轨），解析结果写入曲库并通过
/// `library://tracks-resolved` 事件推送前端增量更新。
/// 已解析曲目若文本元数据仍为未知占位，也会在此按同一套规则重读文件头补全。
///
/// 与后台补解析（`spawn_lazy_backfill`）共用同一套解析逻辑
/// （`resolve_placeholder`），区别只有两点：
///  - 一律按**深度**解析（本地 / 网络一视同仁——用户已经明确点开这首了）；
///  - 解析结果直接返回给调用方（播放流程需要拿到真实曲目才能起播）。
///
/// 返回解析后的完整曲目列表（前端可据此渲染 / 播放）。
#[tauri::command]
pub async fn library_resolve_track(
    state: State<'_, AppState>,
    app: AppHandle,
    track_id: String,
) -> AppResult<Vec<Track>> {
    let initial = state
        .library
        .lock()
        .unwrap()
        .get_track(&track_id)
        .ok_or_else(|| AppError::Library(format!("曲目不存在: {}", track_id)))?;

    // 已完成解析：直接返回（含占位解析失败等场景的幂等路径）。
    // 例外：文本元数据仍未知时，后台重读文件头补全（媒体播放 / 预加载入口）。
    if !initial.metadata_pending && !scan::needs_metadata_refresh(&initial) {
        return Ok(vec![initial]);
    }

    // 解析可能耗时（网络文件 / SACD ISO / CUE probe），放到阻塞池异步执行。
    // 注意：State 是函数级借用，不能跨 spawn_blocking；闭包内从 owned AppHandle 取状态。
    let handle = app.clone();
    let id = initial.id.clone();
    let resolve_id = id.clone();

    // 解析可能耗时（网络文件 / SACD ISO / CUE probe），放到阻塞池异步执行
    let resolved: Vec<Track> = tauri::async_runtime::spawn_blocking(move || {
        let app_state = handle.state::<AppState>().inner();
        resolve_one(app_state, &resolve_id)
    })
    .await
    .unwrap_or_else(|_| vec![initial.clone()]);

    // 占位被分轨取代时，前端需同步移除旧占位（避免幽灵条目）
    let replaced = !resolved.iter().any(|t| t.id == id);
    let _ = app.emit(
        "library://tracks-resolved",
        serde_json::json!({
            "tracks": &resolved,
            "removedIds": if replaced { vec![id.as_str()] } else { Vec::<&str>::new() },
        }),
    );
    Ok(resolved)
}

/// 按需批量解析（滚动可见项 / 预播放加载调用）：补**普通占位**的音频标签 / 时长；
/// 已解析但文本元数据仍未知的曲目也会重读文件头（标签 → 附属文件 → 文件名拆分）。
/// **不拆 CUE、不碰 SACD / ISO**——那些由后台廉价检测或播放时按需处理，
/// 避免滚动时触发重活。网络文件同样只读所需标签。结果写库 + 增量推前端。
#[tauri::command]
pub async fn library_resolve_tracks(app: AppHandle, ids: Vec<String>) -> AppResult<Vec<Track>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut upserts: Vec<Track> = Vec::new();
        for id in &ids {
            let Some(track) = state.library.lock().unwrap_or_else(|e| e.into_inner()).get_track(id) else {
                continue;
            };
            // 已解析曲目：仅当文本元数据仍为未知占位时重读文件头（预加载 / 滚动可见）；
            // CUE 分轨（已有标题）/ ISO·SACD 占位另按需处理
            if !track.metadata_pending {
                if let Some(refreshed) = scan::refresh_metadata_from_header(&track) {
                    upserts.push(refreshed);
                }
                continue;
            }
            if track.is_cue || track.iso_placeholder {
                continue;
            }
            if let Some(mut full) = parse_track(Path::new(&track.file_path)) {
                full.id = track.id.clone();
                upserts.push(full);
            }
        }
        if upserts.is_empty() {
            return Ok(Vec::new());
        }
        {
            let Ok(mut library) = state.library.lock() else {
                return Ok(upserts);
            };
            let _ = library.apply_scan(&upserts, &[]);
            drop(library);
            state.player.lock().unwrap().apply_track_changes(&upserts, &[]);
        }
        // 不在此发事件：结果由调用方（前端 store.resolveTracks）直接增量合并，
        // 避免与合帧监听重复应用。播放路径的 resolve_one 仍走事件（前端未直接合并）。
        // 实验室·元数据增强：懒解析期入队的拆分候选 → 后台在线校验
        verify::drain_and_spawn(&app);
        Ok(upserts)
    })
    .await
    .map_err(|e| AppError::Library(format!("批量解析任务执行失败: {e}")))?
}
///
/// 返回可播放的曲目列表：解析失败时返回原占位（前端提示暂不可解析）。
fn resolve_one(app_state: &AppState, id: &str) -> Vec<Track> {
    let (placeholder, known_cue_tracks) = {
        let library = app_state.library.lock().unwrap_or_else(|e| e.into_inner());
        (library.get_track(id), library.cue_tracks())
    };
    let Some(placeholder) = placeholder else {
        return Vec::new();
    };
    // 媒体播放 / 预播放加载：已解析曲目若文本元数据仍为未知占位，按与扫描相同的
    // 规则重读一次文件头（标签 → 附属文件 → 文件名拆分），有变化才写库并推事件。
    if !placeholder.metadata_pending {
        let Some(refreshed) = scan::refresh_metadata_from_header(&placeholder) else {
            return vec![placeholder];
        };
        {
            let Ok(mut library) = app_state.library.lock() else {
                return vec![refreshed];
            };
            let _ = library.apply_scan(std::slice::from_ref(&refreshed), &[]);
            drop(library);
            app_state
                .player
                .lock()
                .unwrap()
                .apply_track_changes(std::slice::from_ref(&refreshed), &[]);
        }
        return vec![refreshed];
    }
    // 增量局部发现：只扫本曲目附近目录的 CUE，点播路径不阻塞、瞬时返回
    let mut ctx = ResolveContext::new(known_cue_tracks, false);
    let outcome = resolve_placeholder(&placeholder, ResolveDepth::Deep, &mut ctx);
    if outcome.upserts.is_empty() && outcome.existing.is_empty() {
        // 解析失败：保留占位，交由前端提示
        return vec![placeholder];
    }
    let tracks = if outcome.upserts.is_empty() {
        // 分轨此前已入库：直接复用，不重复写库
        outcome.existing.clone()
    } else {
        outcome.upserts.clone()
    };
    {
        let Ok(mut library) = app_state.library.lock() else {
            return tracks;
        };
        let _ = library.apply_scan(&outcome.upserts, &outcome.removed);
        drop(library);
        // 增量更新播放器曲目缓存（避免每次点播都全库去重 + 整表克隆，大库下会拖慢点播）；
        // 若占位被 CUE/SACD 分轨取代，平滑迁移播放队列引用，不打断播放。
        let mut player = app_state.player.lock().unwrap();
        player.apply_track_changes(&outcome.upserts, &outcome.removed);
        player.remap_context(&outcome.remap);
    }
    tracks
}

/// 请求取消当前扫描：置位取消标志，快扫占位循环与后台补解析随即中止
/// （已写入的批次保留，剩余文件保持占位，下次扫描/播放时再处理）。
#[tauri::command]
pub fn library_cancel_scan(state: State<'_, AppState>) -> AppResult<()> {
    state
        .scan_cancel
        .store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn library_tracks(app: AppHandle) -> AppResult<Vec<Track>> {
    // 重查询放到阻塞线程池，避免加载期间阻塞主线程 / 事件循环导致界面卡住
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        Ok(state.library.lock().unwrap_or_else(|e| e.into_inner()).tracks())
    })
    .await
    .map_err(|e| AppError::Library(format!("查询曲目失败: {e}")))?
}

// ---- 封面（内嵌 → 同名边车 → 在线 → 前端默认图） ------------------------

/// 内存封面缓存：track_id → data URL；值为 None 表示本会话已确认无封面（负缓存）
fn cover_cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 磁盘缓存可能出现的扩展名（与 cover::resolve_cover 嗅探结果对齐）
const CACHE_EXTS: &[&str] = &["jpg", "png", "webp", "gif", "bmp"];

/// track_id → 磁盘缓存文件名（sha256 十六进制，避免非法路径字符）
fn cache_stem(track_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(track_id.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 读磁盘缓存原始字节，命中返回 (扩展名, 字节)
fn read_disk_cache_bytes(
    cache_dir: &std::path::Path,
    cache_key: &str,
) -> Option<(String, Vec<u8>)> {
    let stem = cache_stem(cache_key);
    for ext in CACHE_EXTS {
        let path = cache_dir.join(format!("{}.{}", stem, ext));
        if let Ok(data) = std::fs::read(&path) {
            if !data.is_empty() {
                return Some((ext.to_string(), data));
            }
        }
    }
    None
}

/// 读磁盘缓存（应用数据目录 cover-cache/<sha>.<ext>），命中返回 data URL
fn read_disk_cache(cache_dir: &std::path::Path, cache_key: &str) -> Option<String> {
    let (_, data) = read_disk_cache_bytes(cache_dir, cache_key)?;
    let mime = cover::sniff_image_mime(&data);
    Some(cover::to_data_url(mime, &data))
}

/// 写磁盘缓存（内嵌/边车/在线任一来源成功后都落盘，下次启动免解析免联网）
///
/// 缓存是纯优化：写失败（目录不可写 / 磁盘满）时静默返回，不影响本次取封面，
/// 也不向控制台刷诊断输出。
fn write_disk_cache(cache_dir: &std::path::Path, track_id: &str, mime: &str, data: &[u8]) {
    if std::fs::create_dir_all(cache_dir).is_err() {
        return;
    }
    let path = cache_dir.join(format!("{}.{}", cache_stem(track_id), cover::ext_for_mime(mime)));
    let _ = std::fs::write(&path, data);
}

///
/// 读取曲目封面，返回可直接用于 `<img src>` 的 data URL；整条链无封面返回 None
/// （由前端回落到内置默认封面）。
///
/// 来源顺序：内嵌标签 → 同名边车图片 → 在线匹配（online.enabled 时）。
/// 内存缓存（含负缓存）+ 应用数据目录磁盘缓存两级，同一曲目只解析/联网一次。
#[tauri::command]
pub async fn library_cover(
    state: State<'_, AppState>,
    track_id: String,
) -> AppResult<Option<String>> {
    {
        let cache = cover_cache().lock().unwrap();
        if let Some(hit) = cache.get(&track_id) {
            return Ok(hit.clone());
        }
    }

    // 磁盘缓存目录：%APPDATA%/com.tplayer/cover-cache
    let cache_dir = cover_cache_dir(&state);

    if let Some(url) = read_disk_cache(&cache_dir, &track_id) {
        cover_cache()
            .lock()
            .unwrap()
            .insert(track_id, Some(url.clone()));
        return Ok(Some(url));
    }

    // 取曲目与在线开关后立即释放锁（不得跨 await 持锁）
    let (track, online_enabled) = {
        let library = state.library.lock().unwrap_or_else(|e| e.into_inner());
        let config = state.config.lock().unwrap();
        let enabled = config.online.enabled
            && !config.online.provider.is_empty()
            && config.online.provider != "none";
        (library.get_track(&track_id), enabled)
    };
    let Some(track) = track else {
        cover_cache()
            .lock()
            .unwrap()
            .insert(track_id, None);
        return Ok(None);
    };

    let result = cover::resolve_cover(&track, online_enabled).await;
    let url = match result {
        Some((mime, data)) => {
            write_disk_cache(&cache_dir, &track_id, mime, &data);
            Some(cover::to_data_url(mime, &data))
        }
        None => None,
    };

    cover_cache()
        .lock()
        .unwrap()
        .insert(track_id, url.clone());
    Ok(url)
}

/// 封面磁盘缓存目录：%APPDATA%/com.tplayer/cover-cache
fn cover_cache_dir(state: &State<'_, AppState>) -> PathBuf {
    state
        .config_path
        .parent()
        .map(|dir| dir.join("cover-cache"))
        .unwrap_or_else(|| PathBuf::from("cover-cache"))
}

/// 分组封面路径内存缓存（含负缓存）；分组封面走 asset 文件 URL，不占 base64 内存
fn group_cover_cache() -> &'static Mutex<HashMap<String, Option<PathBuf>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<PathBuf>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 找到分组/单曲键对应的磁盘缓存文件路径（按扩展名探测）
fn find_disk_cache_path(cache_dir: &std::path::Path, cache_key: &str) -> Option<PathBuf> {
    let stem = cache_stem(cache_key);
    CACHE_EXTS
        .iter()
        .map(|ext| cache_dir.join(format!("{}.{}", stem, ext)))
        .find(|path| path.is_file())
}

///
/// 分组（专辑/艺术家）封面加载，返回**磁盘缓存文件绝对路径**（前端经 convertFileSrc
/// 以 asset 协议加载，避免 base64 data URL 长驻 JS 内存）：
///   内存缓存（含负缓存）→ 分组磁盘缓存 → 成员曲目磁盘缓存（复制一份到分组键）
///   → 重新解析（候选已由 SQL 按内嵌封面优先、音轨号升序取前 30 首；
///     至多一次在线匹配）→ 落盘。
///
/// 负缓存仅存内存：下次启动自动重试（用户可能补了 folder.jpg 或开启在线）。
async fn load_group_cover_path(
    state: &State<'_, AppState>,
    group_key: &str,
    tracks: Vec<Track>,
    online_enabled: bool,
) -> AppResult<Option<String>> {
    // 1. 内存缓存
    {
        let cache = group_cover_cache().lock().unwrap();
        if let Some(hit) = cache.get(group_key) {
            return Ok(hit.clone().map(|path| path.to_string_lossy().to_string()));
        }
    }

    let cache_dir = cover_cache_dir(state);

    // 2. 分组磁盘缓存（上一次启动已落盘）
    if let Some(path) = find_disk_cache_path(&cache_dir, group_key) {
        group_cover_cache()
            .lock()
            .unwrap()
            .insert(group_key.to_string(), Some(path.clone()));
        return Ok(Some(path.to_string_lossy().to_string()));
    }

    if tracks.is_empty() {
        group_cover_cache()
            .lock()
            .unwrap()
            .insert(group_key.to_string(), None);
        return Ok(None);
    }

    // 3. 成员曲目磁盘缓存：单曲播放/查看时已解析过（可能来自在线），复制镜像到分组键
    for track in tracks.iter().take(30) {
        if let Some(src) = find_disk_cache_path(&cache_dir, &track.id) {
            if let Some(ext) = src.extension().and_then(|ext| ext.to_str()) {
                let dest = cache_dir.join(format!("{}.{}", cache_stem(group_key), ext));
                if std::fs::copy(&src, &dest).is_ok() {
                    group_cover_cache()
                        .lock()
                        .unwrap()
                        .insert(group_key.to_string(), Some(dest.clone()));
                    return Ok(Some(dest.to_string_lossy().to_string()));
                }
            }
        }
    }

    // 4. 完整解析（逐曲目本地链 + 至多一次在线匹配）
    let resolved = cover::resolve_group_cover(&tracks, online_enabled).await;
    let path = match resolved {
        Some((mime, data)) => {
            write_disk_cache(&cache_dir, group_key, mime, &data);
            find_disk_cache_path(&cache_dir, group_key)
        }
        None => None,
    };

    group_cover_cache()
        .lock()
        .unwrap()
        .insert(group_key.to_string(), path.clone());
    Ok(path.map(|path| path.to_string_lossy().to_string()))
}

/// 专辑封面（含 CUE 分轨专辑）：组内任一曲目有封面即共用。
/// 返回缓存文件绝对路径（前端 convertFileSrc 为 asset URL）；无封面返回 null。
#[tauri::command]
pub async fn library_album_cover(
    state: State<'_, AppState>,
    album_id: String,
) -> AppResult<Option<String>> {
    let (tracks, online_enabled) = {
        let library = state.library.lock().unwrap_or_else(|e| e.into_inner());
        let config = state.config.lock().unwrap();
        let enabled = config.online.enabled
            && !config.online.provider.is_empty()
            && config.online.provider != "none";
        (library.cover_candidates_of_album(&album_id, 30), enabled)
    };
    load_group_cover_path(&state, &album_id, tracks, online_enabled).await
}

/// 艺术家封面：候选曲目（任一阵位艺术家匹配）的专辑封面为代表
#[tauri::command]
pub async fn library_artist_cover(
    state: State<'_, AppState>,
    artist: String,
) -> AppResult<Option<String>> {
    let (tracks, online_enabled) = {
        let library = state.library.lock().unwrap_or_else(|e| e.into_inner());
        let config = state.config.lock().unwrap();
        let enabled = config.online.enabled
            && !config.online.provider.is_empty()
            && config.online.provider != "none";
        (library.cover_candidates_of_artist(&artist, 30), enabled)
    };
    load_group_cover_path(&state, &format!("artist:{artist}"), tracks, online_enabled).await
}

/// 聚合查询：一次返回专辑 / CUE 专辑 / SACD 专辑 / 艺术家 / 统计，
/// 前端 `refreshAggregates()` 改为调用此单一命令，避免 5 次独立 IPC
/// 在大曲库下重复执行相似去重 SQL，减少主线程与数据库压力。
#[tauri::command]
pub async fn library_aggregates(app: AppHandle) -> AppResult<crate::library::store::LibraryAggregates> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>().inner();
        Ok(state.library.lock().unwrap_or_else(|e| e.into_inner()).aggregates())
    })
    .await
    .map_err(|e| AppError::Library(format!("查询聚合失败: {e}")))?
}

#[tauri::command]
pub fn library_favorite_ids(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    Ok(state.library.lock().unwrap_or_else(|e| e.into_inner()).favorite_ids())
}

#[tauri::command]
pub fn library_set_favorite(
    state: State<'_, AppState>,
    track_id: String,
    favorite: bool,
) -> AppResult<()> {
    let mut library = state.library.lock().unwrap_or_else(|e| e.into_inner());
    library.set_favorite(&track_id, favorite)?;
    Ok(())
}

#[tauri::command]
pub fn library_recent(state: State<'_, AppState>) -> AppResult<Vec<RecentEntry>> {
    Ok(state.library.lock().unwrap_or_else(|e| e.into_inner()).recent())
}

#[tauri::command]
pub fn library_record_played(state: State<'_, AppState>, track_id: String) -> AppResult<()> {
    let mut library = state.library.lock().unwrap_or_else(|e| e.into_inner());
    // 自动记录，失败（如曲目刚被删除导致外键不匹配）不应打断播放流程
    let _ = library.record_played(&track_id);
    Ok(())
}

#[tauri::command]
pub fn library_remove_tracks(state: State<'_, AppState>, ids: Vec<String>) -> AppResult<()> {
    let mut library = state.library.lock().unwrap_or_else(|e| e.into_inner());
    library.remove_tracks(&ids)?;
    Ok(())
}

/// 把目录加入曲库白名单
#[tauri::command]
pub fn library_add_dir(state: State<'_, AppState>, path: String) -> AppResult<()> {
    // 路径来自 dialog 选择器，可信；直接加入白名单
    let mut config = state.config.lock().unwrap();
    if !config.library_dirs.contains(&path) {
        config.library_dirs.push(path.clone());
        config.save(&state.config_path)?;
    }
    Ok(())
}

/// 从曲库白名单移除目录
#[tauri::command]
pub fn library_remove_dir(state: State<'_, AppState>, path: String) -> AppResult<()> {
    {
        let mut library = state.library.lock().unwrap_or_else(|e| e.into_inner());
        library.remove_tracks_under_dir(&path)?;
    }
    {
        let mut config = state.config.lock().unwrap();
        config.library_dirs.retain(|p| p != &path);
        config.save(&state.config_path)?;
    }
    Ok(())
}

#[tauri::command]
pub fn library_list_playlists(state: State<'_, AppState>) -> AppResult<Vec<Playlist>> {
    Ok(state.library.lock().unwrap_or_else(|e| e.into_inner()).playlists())
}

#[tauri::command]
pub fn library_create_playlist(
    state: State<'_, AppState>,
    name: String,
) -> AppResult<Playlist> {
    let mut library = state.library.lock().unwrap_or_else(|e| e.into_inner());
    let playlist = library.create_playlist(name)?;
    Ok(playlist)
}

#[tauri::command]
pub fn library_rename_playlist(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> AppResult<()> {
    let mut library = state.library.lock().unwrap_or_else(|e| e.into_inner());
    library.rename_playlist(&id, name)?;
    Ok(())
}

/// 删除歌单
#[tauri::command]
pub fn library_delete_playlist(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let mut library = state.library.lock().unwrap_or_else(|e| e.into_inner());
    library.delete_playlist(&id)?;
    Ok(())
}

#[tauri::command]
pub fn library_set_playlist_tracks(
    state: State<'_, AppState>,
    id: String,
    track_ids: Vec<String>,
) -> AppResult<()> {
    let mut library = state.library.lock().unwrap_or_else(|e| e.into_inner());
    library.set_playlist_tracks(&id, track_ids)?;
    Ok(())
}

#[tauri::command]
pub fn library_add_tracks_to_playlist(
    state: State<'_, AppState>,
    id: String,
    track_ids: Vec<String>,
) -> AppResult<()> {
    let mut library = state.library.lock().unwrap_or_else(|e| e.into_inner());
    library.add_tracks_to_playlist(&id, track_ids)?;
    Ok(())
}

#[tauri::command]
pub fn library_remove_track_from_playlist(
    state: State<'_, AppState>,
    id: String,
    track_id: String,
) -> AppResult<()> {
    let mut library = state.library.lock().unwrap_or_else(|e| e.into_inner());
    library.remove_track_from_playlist(&id, &track_id)?;
    Ok(())
}

/// 拖拽导入：将给定路径（文件或目录）中的音频文件直接解析入库，
/// 绕过白名单目录扫描（等价于用户显式把文件"放入"曲库）。
///
/// 安全约定：路径来自 Tauri 拖放事件（用户主动拖入窗口），视为可信，
/// 与 library_add_dir（dialog 选择器）同源，不强制白名单校验。
/// 返回成功入库的曲目 ID 列表；无可识别音频时返回空列表。
#[tauri::command]
pub fn library_import_files(
    app: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> AppResult<Vec<String>> {
    let path_bufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let upserts = collect_import_tracks(&path_bufs);
    if upserts.is_empty() {
        return Ok(Vec::new());
    }

    // 写入曲库并刷新播放器曲目缓存（与 library_scan 同构）
    let mut library = state.library.lock().unwrap_or_else(|e| e.into_inner());
    library.apply_scan(&upserts, &[])?;
    let tracks = library.tracks();
    drop(library);
    state.player.lock().unwrap().set_tracks(tracks);

    // 新导入曲目可能带来新封面，失效内存封面缓存（磁盘缓存保留）
    cover_cache().lock().unwrap().clear();

    // 实验室·元数据增强：导入期入队的拆分候选 → 后台在线校验
    verify::drain_and_spawn(&app);

    Ok(upserts.into_iter().map(|t| t.id).collect())
}

/// 元数据增强·批量在线校验状态（实验室页展示后台任务是否在跑）
#[tauri::command]
pub fn library_meta_verify_status() -> AppResult<serde_json::Value> {
    Ok(serde_json::json!({ "running": verify::is_running() }))
}

/// 元数据增强·单曲重试：只重新识别 `track_id` 指定的这一首曲目
/// （重新解析文件名 + 联网查找），不影响其他曲目；结果追加进重试日志。
#[tauri::command]
pub async fn library_meta_verify_retry_track(
    app: AppHandle,
    track_id: String,
) -> AppResult<verify::RetryRecord> {
    verify::retry_track(&app, &track_id)
        .await
        .map_err(AppError::Library)
}

/// 元数据增强·重试操作日志（完整历史，前端据此展示「已重试」标记与排查）
#[tauri::command]
pub fn library_meta_verify_log(app: AppHandle) -> AppResult<Vec<verify::RetryRecord>> {
    Ok(verify::retry_log(&app))
}

// ---------------- 曲目详情（track_info） ----------------

/// 曲目详情返回模型：面向前端 TrackInfoPanel 的全量元数据聚合。
/// 所有字段均 Option 化，lofty 读不到、文件缺失、解码失败一律降级为 None，
/// 严禁因单个字段缺失让整个接口报错导致前端崩溃。
#[derive(Debug, Clone, serde::Serialize)]
pub struct TrackInfo {
    // ---- 基础信息 ----
    pub title: String,
    #[serde(rename = "artists")]
    pub artists: Vec<String>,
    pub album: String,
    #[serde(rename = "trackNumber")]
    pub track_number: Option<u32>,
    pub year: Option<u32>,
    /// 时长（秒），时长探测失败时为 0
    pub duration: f64,

    // ---- 技术信息 ----
    /// 音频格式（小写扩展名）
    pub format: String,
    /// 比特率（bps）
    pub bitrate: Option<u32>,
    /// 采样率（Hz）
    #[serde(rename = "sampleRate")]
    pub sample_rate: Option<u32>,
    /// 位深（bit）
    #[serde(rename = "bitDepth")]
    pub bit_depth: Option<u8>,
    /// 声道数
    pub channels: Option<u8>,

    // ---- 存储信息 ----
    #[serde(rename = "filePath")]
    pub file_path: String,
    #[serde(rename = "fileSize")]
    pub file_size: u64,
    #[serde(rename = "modifiedAt")]
    pub modified_at: i64,
    #[serde(rename = "addedAt")]
    pub added_at: i64,

    // ---- 封面（data URL，与 library_cover 同链路；无封面为 None）----
    pub cover: Option<String>,

    // ---- 内嵌歌词（纯文本；无歌词为 None）----
    #[serde(rename = "embeddedLyrics")]
    pub embedded_lyrics: Option<String>,

    // ---- ReplayGain ----
    #[serde(rename = "replaygainDb")]
    pub replaygain_db: Option<f32>,
    #[serde(rename = "replaygainPeak")]
    pub replaygain_peak: Option<f32>,
}

/// 读音频内嵌歌词（lofty Lyrics 标签）；失败或空文本一律返回 None
fn read_embedded_lyrics(path: &std::path::Path) -> Option<String> {
    use lofty::file::TaggedFileExt;
    use lofty::tag::ItemKey;
    let tagged = lofty::read_from_path(path).ok()?;
    let mut tags: Vec<&lofty::tag::Tag> = Vec::new();
    if let Some(primary) = tagged.primary_tag() {
        tags.push(primary);
    }
    tags.extend(tagged.tags().iter());
    for tag in tags {
        if let Some(text) = tag.get_string(&ItemKey::Lyrics) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// 读取曲目元数据：先读曲库缓存（快），再用 lofty + ffprobe 补齐 bit_depth/channels 等技术字段。
/// 若文本元数据仍为未知占位，会重读文件头补全，并把补全后的曲目一并返回（供调用方写库）。
fn enrich_track(track: &Track) -> (TrackInfo, Option<Track>) {
    use lofty::file::AudioFile;
    // 媒体信息提取：库内文本仍为未知占位时，按与扫描相同的规则重读文件头
    // （标签 → 附属文件 → 文件名拆分），保证信息面板与播放侧展示一致。
    let refreshed = scan::refresh_metadata_from_header(track);
    let text = refreshed.as_ref().unwrap_or(track);
    // Linux：还原非 UTF-8 文件名的容错编码路径（NTFS 场景；合法路径零成本直连）
    #[cfg(target_os = "linux")]
    let pb = crate::library::scan_linux::resolve_path_str(&track.file_path);
    #[cfg(target_os = "linux")]
    let path = pb.as_path();
    #[cfg(not(target_os = "linux"))]
    let path = std::path::Path::new(&track.file_path);
    let mut bit_depth = None;
    let mut channels = None;
    let mut duration = text.duration;

    // 尝试 lofty 解析完整属性（bit_depth/channels 在 Track 模型中未持久化，每次实时解析）
    if let Ok(file) = lofty::read_from_path(path) {
        let props = file.properties();
        bit_depth = props.bit_depth();
        channels = props.channels();
        // lofty 读取的 duration 更精确时以它为准
        let lofty_dur = props.duration().as_secs_f64();
        if lofty_dur > 0.0 && (duration <= 0.0 || (lofty_dur - duration).abs() > 0.5) {
            duration = lofty_dur;
        }
    }

    // 内嵌歌词
    let embedded_lyrics = read_embedded_lyrics(path);

    let info = TrackInfo {
        title: text.title.clone(),
        artists: text.artists.clone(),
        album: text.album.clone(),
        track_number: text.track_number,
        year: text.year,
        duration,
        format: text.format.clone(),
        bitrate: text.bitrate,
        sample_rate: text.sample_rate,
        bit_depth,
        channels,
        file_path: track.file_path.clone(),
        file_size: track.file_size,
        modified_at: track.modified_at,
        added_at: track.added_at,
        cover: None, // 单独通过 library_cover 异步解析（含内存/磁盘两级缓存 + 在线回退）
        embedded_lyrics,
        replaygain_db: text.replaygain_db,
        replaygain_peak: text.replaygain_peak,
    };
    (info, refreshed)
}

/// 返回指定曲目的完整元数据（含技术参数、内嵌歌词、封面 data URL）。
/// 任何字段缺失或解析失败一律降级为 None / 默认值，不向上抛错。
#[tauri::command]
pub async fn library_track_info(
    state: State<'_, AppState>,
    track_id: String,
) -> AppResult<Option<TrackInfo>> {
    // 1. 查曲库缓存（路径 / 基础字段）
    let track = match state.library.lock().unwrap_or_else(|e| e.into_inner()).get_track(&track_id) {
        Some(t) => t,
        None => return Ok(None),
    };

    // 2. 技术参数 + 内嵌歌词（同步解析，lofty 读取极快）；
    //    文本元数据未知时会顺带重读文件头，结果写回曲库并通知前端增量更新。
    let (mut info, refreshed) = enrich_track(&track);
    if let Some(refreshed) = refreshed {
        {
            let Ok(mut library) = state.library.lock() else {
                return Ok(Some(info));
            };
            let _ = library.apply_scan(std::slice::from_ref(&refreshed), &[]);
            drop(library);
            state
                .player
                .lock()
                .unwrap()
                .apply_track_changes(std::slice::from_ref(&refreshed), &[]);
        }
        let _ = state.app.emit(
            "library://tracks-resolved",
            serde_json::json!({ "tracks": [refreshed], "removedIds": Vec::<String>::new() }),
        );
    }

    // 3. 封面：复用 library_cover 的完整解析链（含缓存 + 在线匹配）
    if let Ok(Some(url)) = library_cover(state.clone(), track_id.clone()).await {
        info.cover = Some(url);
    }

    Ok(Some(info))
}

// 解析 / 补解析的行为约束测试（拆到独立文件，避免主文件过长）
#[cfg(test)]
#[path = "library_tests.rs"]
mod tests;
