/**
 * library store
 *
 * 职责：持有曲库曲目 / 专辑 / 艺术家 / CUE 专辑聚合视图、收藏集合、最近播放、
 * 扫描进度与搜索关键词；数据来源为 services/library（Tauri 命令或 Web 预览实现），
 * 本 store 不做文件系统访问。
 *
 * 与旧版的差异：
 *  - 旧版曲目数据与 UI 逻辑混在 App.vue，扫描进度靠轮询；
 *  - v2 扫描进度由后端事件推送，前端只做展示；
 *  - 收藏 / 最近播放为独立用户数据，不再寄生在曲目对象上。
 */
import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { listen } from '@tauri-apps/api/event'

import { libraryService } from '../services/library'
import { isTauriRuntime, normalizeError } from '../services/ipc'
import { useToastStore } from './toast'
import type { Album, Artist, LibraryStats, RecentEntry, ScanProgress, Track } from '../types'

function createIdleScanProgress(): ScanProgress {
  return { stage: 'idle', processed: 0, total: 0, currentPath: null, error: null }
}

/**
 * 搜索用小写关键词串的缓存（WeakMap：曲目对象被替换 / 回收后自动失效）。
 *
 * 记录生成时的字段快照：标题 / 专辑按值比较，艺术家按数组引用 + 长度比较，
 * 字段变化（如元数据回填）时自动重建。这样每次按键只需对全库曲目做
 * 一次 `includes`，不再为每首曲目重复拼接 + toLowerCase 分配新字符串。
 */
interface SearchHaystack {
  title: string
  album: string
  artists: string[]
  artistCount: number
  hay: string
}

const haystackCache = new WeakMap<Track, SearchHaystack>()

function trackHaystack(track: Track): string {
  const cached = haystackCache.get(track)
  if (
    cached &&
    cached.title === track.title &&
    cached.album === track.album &&
    cached.artists === track.artists &&
    cached.artistCount === track.artists.length
  ) {
    return cached.hay
  }
  const hay = `${track.title} ${track.artists.join(' ')} ${track.album}`.toLowerCase()
  haystackCache.set(track, {
    title: track.title,
    album: track.album,
    artists: track.artists,
    artistCount: track.artists.length,
    hay,
  })
  return hay
}

/**
 * 曲目关键词匹配（调用方保证 key 已 trim + lowercase）。
 * 主匹配域：标题 / 艺术家 / 专辑；
 * 附加匹配域：文件后缀名（如 "flac"、".flac"、"dsf"），供内部按格式筛选用，UI 不提示。
 */
export function matchTrackKeyword(track: Track, key: string): boolean {
  if (trackHaystack(track).includes(key)) {
    return true
  }
  // 后缀名匹配：取最后一个点之后的片段（无点时沿用整个文件名，与旧实现一致）
  const filePath = track.filePath
  const dot = filePath.lastIndexOf('.')
  const ext = (dot >= 0 ? filePath.slice(dot + 1) : filePath).toLowerCase()
  if (ext.length === 0) return false
  // 去掉 key 的前导点（等价于旧的 `key.replace(/^\.+/, '')`）
  let bare = key
  let i = 0
  while (i < bare.length && bare.charCodeAt(i) === 46) i++
  if (i > 0) bare = bare.slice(i)
  return ext === bare
}

/**
 * 是否为 SACD 的 5.1 环绕轨（后端 `SacdArea` 序列化为 `"MultiCh"`）。
 *
 * 与立体声区逐曲对应、标题仅差 `[5.1]` 后缀，普通列表需要去重，
 * 只有 SACD 专辑页的区域切换才展示。
 */
export function isSacdMultich(track: Track): boolean {
  return String(track.sacdArea ?? '').toLowerCase() === 'multich'
}

/**
 * 「同一首歌」去重键（与后端 `not_duplicate_song()` 口径一致）：
 * 主艺人 + 标题 + 专辑 + 格式。
 *  - 格式不同（flac / mp3 / ape 各一份）视为不同版本，都保留；
 *  - SACD 虚拟分轨不参与（其立体声 / 5.1 两套由专辑页区域切换表达）；
 *  - CUE 分轨标题带「曲目号 - 标题」且指向同一镜像，按此键也不相撞。
 */
function songDedupKey(track: Track): string {
  const primary = (track.artists[0] ?? '').trim().toLowerCase()
  return [primary, track.title.trim().toLowerCase(), track.album.trim().toLowerCase(), (track.format ?? '').trim().toLowerCase()].join('\u0001')
}

/**
 * 列表兜底去重：同一首歌只保留首条（后端 `library_tracks` 已做同键去重，
 * 这里再挡一层，防止旧库残留 / 增量合并路径漏折导致列表出现重复项）。
 * 保持传入顺序，不做重排。
 */
function dedupeSongs(list: Track[]): Track[] {
  const seen = new Set<string>()
  const out: Track[] = []
  for (const track of list) {
    if (track.isSacd === true) {
      out.push(track)
      continue
    }
    const key = songDedupKey(track)
    if (seen.has(key)) continue
    seen.add(key)
    out.push(track)
  }
  return out
}

export const useLibraryStore = defineStore('library', () => {
  /**
   * 全量曲目（含 SACD 的 5.1 环绕轨）。
   *
   * ISO 的立体声区与多声道区是同一张碟的两套曲目（标题仅差 `[5.1]` 后缀），
   * 直接并排展示会像"每首歌加载了两次"。这里保留全量供 SACD 专辑页做
   * 「立体声 / 5.1 环绕」切换，普通列表统一用下面的 `tracks`（已剔除 5.1）。
   */
  const allTracks = ref<Track[]>([])
  const tracks = ref<Track[]>([])
  const albums = ref<Album[]>([])
  const cueAlbums = ref<Album[]>([])
  const sacdAlbums = ref<Album[]>([])
  const artists = ref<Artist[]>([])
  const stats = ref<LibraryStats | null>(null)
  const favoriteIds = ref<string[]>([])
  const recentEntries = ref<RecentEntry[]>([])
  const scanProgress = ref<ScanProgress>(createIdleScanProgress())
  const keyword = ref('')
  const loading = ref(false)
  const error = ref<string | null>(null)
  const toast = useToastStore()

  const isEmpty = computed(() => tracks.value.length === 0)

  const isScanning = computed(
    () =>
      scanProgress.value.stage === 'discovering' ||
      scanProgress.value.stage === 'parsing' ||
      scanProgress.value.stage === 'writing',
  )

  /**
   * 是否处于「后台补解析」阶段。
   *
   * 扫描（快速占位入库）结束后，后台线程继续解析占位条目：
   * 本地做深度解析（元数据 + CUE 分轨 + SACD 虚拟分轨），网络只读文件头。
   * 该阶段的结果**增量**推给前端（`library://tracks-resolved`），
   * 用户无需等待全部解析完就能看到已解析好的条目。
   */
  const isBackfilling = computed(() => scanProgress.value.stage === 'backfilling')

  /** 后台解析进度文案（如「后台解析 128/540」）；不在该阶段时为空串 */
  const backfillLabel = computed(() => {
    if (!isBackfilling.value) return ''
    const { processed, total } = scanProgress.value
    return total > 0 ? `后台解析 ${processed}/${total}` : `后台解析 ${processed}`
  })

  /** id → 曲目 的索引，供各视图 O(1) 取当前播放 / 歌单曲目 */
  const tracksById = computed(() => {
    const map = new Map<string, Track>()
    // 用全量索引：播放中的 5.1 轨、专辑页切换后的曲目都要能取到元数据
    for (const track of allTracks.value) map.set(track.id, track)
    return map
  })

  function getTrack(id: string | null | undefined): Track | undefined {
    if (!id) return undefined
    return tracksById.value.get(id)
  }

  const favoriteSet = computed(() => new Set(favoriteIds.value))
  function isFavorite(trackId: string): boolean {
    return favoriteSet.value.has(trackId)
  }

  /** 收藏曲目（按入库顺序，与曲目列表稳定一致） */
  const favoriteTracks = computed(() =>
    tracks.value.filter((track) => favoriteSet.value.has(track.id)),
  )

  /** 最近播放曲目（已失效的 id 自动跳过，避免幽灵条目，对齐旧版 B-07） */
  const recentTracks = computed(() =>
    recentEntries.value
      .map((entry) => tracksById.value.get(entry.trackId))
      .filter((track): track is Track => Boolean(track)),
  )

  function albumById(id: string | null | undefined): Album | undefined {
    if (!id) return undefined
    return [...albums.value, ...cueAlbums.value, ...sacdAlbums.value].find(
      (album) => album.id === id,
    )
  }

  function artistByName(name: string | null | undefined): Artist | undefined {
    if (!name) return undefined
    return artists.value.find((artist) => artist.name === name)
  }

  /**
   * 专辑下的曲目（按音轨序号排序；找不到时回落到空数组）。
   * 取全量 `allTracks`：SACD 专辑页需要立体声 + 5.1 两套，按区域切换显示。
   */
  function tracksOfAlbum(albumId: string): Track[] {
    return allTracks.value
      .filter((track) => track.albumId === albumId)
      .sort((a, b) => (a.trackNumber ?? 0) - (b.trackNumber ?? 0))
  }

  /** 艺术家的曲目（按专辑 / 音轨排序） */
  function tracksOfArtist(name: string): Track[] {
    return tracks.value
      .filter((track) => track.artists.includes(name))
      .sort((a, b) => {
        if (a.album !== b.album) return a.album.localeCompare(b.album)
        return (a.trackNumber ?? 0) - (b.trackNumber ?? 0)
      })
  }

  /**
   * 当前展示的曲目列表（本地大小写不敏感过滤：标题/歌手/专辑，另支持后缀名筛选）。
   * 大曲库后端检索在 M2 落地，视图工具条的“筛选本视图”同样复用此关键词。
   */
  function filterTracks(list: Track[], kw?: string): Track[] {
    const key = (kw ?? keyword.value).trim().toLowerCase()
    if (!key) return list
    return list.filter((track) => matchTrackKeyword(track, key))
  }

  /** 拉取曲库全量视图 + 用户数据（收藏 / 最近播放） */
  async function refresh(): Promise<void> {
    loading.value = true
    error.value = null
    // 渐进式加载：先把**曲目列表**刷出来（用户立刻能看到、可交互、可播放），
    // 再把专辑 / 艺术家 / 统计等较慢的聚合转**后台**加载。避免「等所有聚合查询
    // 都跑完才出列表」——大库下那些 CTE 聚合可能好几秒，拖住整个首屏。
    let trackList: Track[] = []
    let favs: string[] = []
    try {
      ;[trackList, favs] = await Promise.all([
        libraryService.tracks(),
        libraryService.favoriteIds(),
      ])
    } catch (cause) {
      error.value = normalizeError(cause).message
      loading.value = false
      return
    }
    allTracks.value = dedupeSongs(trackList)
    // 普通列表剔除 SACD 5.1 环绕轨（与立体声区同曲目，只差 `[5.1]` 后缀）
    tracks.value = allTracks.value.filter((track) => !isSacdMultich(track))
    favoriteIds.value = favs
    loading.value = false
    // 聚合 / 最近播放：后台加载，不阻塞列表；单个失败不影响列表可用
    void (async () => {
      try {
        recentEntries.value = await libraryService.recent()
      } catch {
        /* 最近播放加载失败不阻断 */
      }
      await refreshAggregates()
    })()
  }

  /** 触发一次扫描（扫描根目录取 settings.libraryDirs，由 Rust 侧读取配置）
   *  @param force 为 true 时忽略「未变更则跳过」优化，强制重新遍历 + 解析 */
  async function scan(force = false): Promise<void> {
    error.value = null
    scanError.value = null
    try {
      await libraryService.scan(force)
      await refresh()
      // 后台补解析阶段（stage=backfilling）由后端事件驱动，
      // 不能被这里的 'done' 覆盖掉（否则「后台解析中 x/y」指示器会闪退）
      if (scanProgress.value.stage !== 'backfilling') {
        scanProgress.value = { ...scanProgress.value, stage: 'done' }
      }
      if (scanError.value) {
        toast.warning(scanError.value)
      }
    } catch (cause) {
      error.value = normalizeError(cause).message
      scanProgress.value = { ...scanProgress.value, stage: 'failed', error: error.value }
    }
  }

  /**
   * 拖拽导入落地：把拖入窗口的文件/目录路径交给后端解析入库，
   * 成功后刷新曲库视图并返回入库曲目 ID 列表（供调用方建播放列表）。
   * 浏览器预览无真实文件解析，importFiles 返回空列表。
   */
  async function importDropped(paths: string[]): Promise<string[]> {
    if (paths.length === 0) return []
    try {
      const ids = await libraryService.importFiles(paths)
      if (ids.length > 0) await refresh()
      return ids
    } catch (cause) {
      error.value = normalizeError(cause).message
      toast.warning(error.value)
      return []
    }
  }

  async function cancelScan(): Promise<void> {
    try {
      await libraryService.cancelScan()
    } catch (cause) {
      error.value = normalizeError(cause).message
    }
  }

  /** 由后端扫描事件写入进度 */
  function applyScanProgress(progress: ScanProgress): void {
    scanProgress.value = progress
  }

  /**
   * 后端在完成事件里带回的扫描错误。
   *
   * Linux：NTFS 分区上的目录可能因挂载权限无法读取，后端在完成事件里带回
   * 聚合错误（progress.error）。这里捕获并向用户明示，避免「部分目录被静默
   * 跳过、曲目缺失但无任何提示」。Windows 该字段恒为 null，不产生任何提示。
   */
  const scanError = ref<string | null>(null)

  /**
   * 常驻订阅扫描 / 后台补解析进度。
   *
   * 此前监听挂在 `scan()` 内部，扫描结束即注销 → 后台补解析阶段
   * （stage = `backfilling`）的进度完全收不到，前端既看不到
   * 「后台解析中 x/y」，也不知道解析结果何时陆续到达。
   */
  if (isTauriRuntime()) {
    void listen<ScanProgress>('library://scan-progress', (e) => {
      applyScanProgress(e.payload)
      if (e.payload.stage === 'done' && e.payload.error) {
        scanError.value = e.payload.error
      }
    }).catch(() => {
      /* 事件订阅失败不阻断扫描本身 */
    })
  }

  /**
   * 两阶段懒加载 - 增量更新：把后台补解析 / 播放时按需解析完成的新曲目
   * 合并进曲库视图（按 id 替换，不触发全量刷新），并同步刷新专辑聚合视图。
   * removedIds 用于替换场景：占位条目被后端删除（CUE 分轨 / SACD 虚拟分轨入库）时，
   * 前端同步移除旧占位，避免幽灵条目。
   *
   * 返回「旧占位 id → 新分轨 id 列表」的重映射（按同文件匹配），供播放器把当前曲目 /
   * 队列里指向旧占位的引用平滑迁移到新分轨，避免后台解析打断正在进行的播放。
   */
  function applyResolvedTracks(
    updated: Track[],
    removedIds: string[] = [],
  ): Record<string, string[]> {
    const removed = new Set(removedIds)
    // 记录被移除占位的 filePath：占位与分轨指向同一物理文件，据此映射到新分轨
    const removedPaths = new Map<string, string>()
    if (removed.size > 0) {
      for (const track of allTracks.value) {
        if (removed.has(track.id)) removedPaths.set(track.id, track.filePath)
      }
      allTracks.value = allTracks.value.filter((track) => !removed.has(track.id))
      tracks.value = tracks.value.filter((track) => !removed.has(track.id))
    }
    if (updated.length > 0) {
      const byId = new Map(allTracks.value.map((track) => [track.id, track]))
      for (const track of updated) byId.set(track.id, track)
      allTracks.value = dedupeSongs([...byId.values()])
      // 普通列表剔除 SACD 5.1 环绕轨，保持与 refresh() 的过滤口径一致
      tracks.value = allTracks.value.filter((track) => !isSacdMultich(track))
    }
    void refreshAggregates()

    // 构建重映射：占位 id → 同文件的新分轨 id（按碟内曲目顺序）
    const updatedByPath = new Map<string, Track[]>()
    for (const track of updated) {
      const arr = updatedByPath.get(track.filePath)
      if (arr) arr.push(track)
      else updatedByPath.set(track.filePath, [track])
    }
    const remap: Record<string, string[]> = {}
    for (const [id, path] of removedPaths) {
      const repl = updatedByPath.get(path)
      if (repl && repl.length > 0) remap[id] = repl.map((t) => t.id)
    }
    return remap
  }

  /**
   * 实验室「歌词元数据补全」：就地回填单条曲目的艺术家/专辑/标题（仅替换传入字段）。
   *
   * 性能：改用**就地变更对象属性**而非整体 `.map` 重建数组——避免每次回填触发
   * `tracksById`(O(n)) 重算与数组引用变更带来的连锁重渲，消除大库回填期的 O(n²) 抖动；
   * 行内显示因深层响应式仍即时更新，聚合视图按 id 关联不受影响。
   */
  function patchTrackMeta(
    id: string,
    patch: { artists?: string[]; album?: string; title?: string },
  ): void {
    const apply = (list: Track[]): void => {
      for (const track of list) {
        if (track.id !== id) continue
        if (patch.artists) track.artists = patch.artists
        if (patch.album !== undefined) track.album = patch.album
        if (patch.title !== undefined) track.title = patch.title
        break
      }
    }
    apply(allTracks.value)
    apply(tracks.value)
  }

  /** 播放前解析占位轨道（两阶段懒加载 - 阶段二按需解析）；返回解析后的曲目列表 */
  async function resolveTrack(trackId: string): Promise<Track[]> {
    try {
      const resolved = await libraryService.resolveTrack(trackId)
      if (resolved.length > 0) {
        // CUE / SACD 场景占位 id 已被后端替换为分轨 id：原占位不再存在于返回列表
        const replaced = resolved.every((track) => track.id !== trackId)
        applyResolvedTracks(resolved, replaced ? [trackId] : [])
      }
      return resolved
    } catch (cause) {
      error.value = normalizeError(cause).message
      return []
    }
  }

  /**
   * 列表滚动到可见区时按需批量补解析：只挑**还未解析的普通占位**（CUE 分轨已有
   * 标题、ISO/SACD 另按需处理），请求后端补标签 / 时长后增量合并进列表。
   * 这样"全部歌曲"无需后台整库解析——滚到哪补到哪，未滚动到的保持轻量占位。
   */
  async function resolveTracks(ids: string[]): Promise<void> {
    if (ids.length === 0) return
    const seen = new Set<string>()
    const pending = ids.filter((id) => {
      if (seen.has(id)) return false
      seen.add(id)
      const t = getTrack(id)
      return !!t && t.metadataPending && !t.isCue
    })
    if (pending.length === 0) return
    try {
      const resolved = await libraryService.resolveTracks(pending)
      if (resolved.length > 0) applyResolvedTracks(resolved, [])
    } catch (cause) {
      error.value = normalizeError(cause).message
    }
  }

  // 聚合刷新节流 + 合池：后台补解析期间 `tracks-resolved` 事件密集，若每次都同步
  // 拉 5 个全库聚合接口，会把主线程与后端 DB 拖垮（大曲库下表现为界面「未响应」）。
  // 策略：同一时刻最多一个在跑；运行中到来的调用只置脏，跑完补一次；距上次不足
  // AGG_THROTTLE_MS 的调用顺延到时间窗末尾。最终保证一定有一次反映最新数据的刷新。
  let aggInFlight = false
  let aggPending = false
  let aggTimer: ReturnType<typeof setTimeout> | null = null
  let aggLastRun = 0
  const AGG_THROTTLE_MS = 500

  async function runAggregates(): Promise<void> {
    aggLastRun = Date.now()
    try {
      const [albumList, cueList, sacdList, artistList, libraryStats] = await Promise.all([
        libraryService.albums(),
        libraryService.cueAlbums(),
        libraryService.sacdAlbums(),
        libraryService.artists(),
        libraryService.stats(),
      ])
      albums.value = albumList
      cueAlbums.value = cueList
      sacdAlbums.value = sacdList
      artists.value = artistList
      stats.value = libraryStats
    } catch (cause) {
      error.value = normalizeError(cause).message
    } finally {
      aggInFlight = false
      if (aggPending) {
        aggPending = false
        void refreshAggregates()
      }
    }
  }

  /** 轻量刷新专辑/艺术家/统计聚合（懒加载增量更新后调用，避免全量重拉曲目） */
  function refreshAggregates(): Promise<void> {
    const now = Date.now()
    // 正在跑：置脏，待其完成后补一次
    if (aggInFlight) {
      aggPending = true
      return Promise.resolve()
    }
    // 距上次不足节流窗：置脏并顺延
    if (now - aggLastRun < AGG_THROTTLE_MS) {
      aggPending = true
      if (aggTimer == null) {
        aggTimer = setTimeout(
          () => {
            aggTimer = null
            void refreshAggregates()
          },
          AGG_THROTTLE_MS - (now - aggLastRun),
        )
      }
      return Promise.resolve()
    }
    aggInFlight = true
    return runAggregates()
  }

  function setKeyword(value: string): void {
    keyword.value = value
  }

  /** 切换收藏（乐观更新；失败回滚并落入 error 态） */
  async function toggleFavorite(trackId: string): Promise<boolean> {
    const next = !isFavorite(trackId)
    const previous = favoriteIds.value
    favoriteIds.value = next
      ? [...previous, trackId]
      : previous.filter((id) => id !== trackId)
    try {
      await libraryService.setFavorite(trackId, next)
    } catch (cause) {
      favoriteIds.value = previous
      error.value = normalizeError(cause).message
    }
    return next
  }

  /** 记录一次播放（由 App 监听当前曲目变化时调用） */
  async function recordPlayed(trackId: string): Promise<void> {
    const playedAt = Math.floor(Date.now() / 1000)
    recentEntries.value = [
      { trackId, playedAt },
      ...recentEntries.value.filter((entry) => entry.trackId !== trackId),
    ].slice(0, 200)
    try {
      await libraryService.recordPlayed(trackId)
    } catch (cause) {
      error.value = normalizeError(cause).message
    }
  }

  /**
   * 从曲库移除（仅索引，不删除磁盘文件）。
   * 同步清理收藏与最近播放；歌单中的失效引用在渲染侧跳过（M2 由后端事务清理）。
   */
  async function removeTracks(ids: string[]): Promise<void> {
    const idSet = new Set(ids)
    try {
      await libraryService.removeTracks(ids)
      tracks.value = tracks.value.filter((track) => !idSet.has(track.id))
      albums.value = albums.value
        .map((album) => ({
          ...album,
          trackCount: tracks.value.filter(
            (track) => track.albumId === album.id && !idSet.has(track.id),
          ).length,
        }))
        .filter((album) => album.trackCount > 0)
      favoriteIds.value = favoriteIds.value.filter((id) => !idSet.has(id))
      recentEntries.value = recentEntries.value.filter((entry) => !idSet.has(entry.trackId))
    } catch (cause) {
      error.value = normalizeError(cause).message
    }
  }

  /** 清空内存中的曲库视图（不删除任何磁盘文件，重新扫描可恢复） */
  function clear(): void {
    tracks.value = []
    allTracks.value = []
    albums.value = []
    cueAlbums.value = []
    sacdAlbums.value = []
    artists.value = []
    stats.value = null
    favoriteIds.value = []
    recentEntries.value = []
    scanProgress.value = createIdleScanProgress()
    keyword.value = ''
    error.value = null
  }

  return {
    tracks,
    albums,
    cueAlbums,
    sacdAlbums,
    artists,
    stats,
    favoriteIds,
    recentEntries,
    scanProgress,
    keyword,
    loading,
    error,
    isEmpty,
    isScanning,
    isBackfilling,
    backfillLabel,
    tracksById,
    favoriteTracks,
    recentTracks,
    getTrack,
    isFavorite,
    albumById,
    artistByName,
    tracksOfAlbum,
    tracksOfArtist,
    filterTracks,
    refresh,
    scan,
    importDropped,
    cancelScan,
    applyScanProgress,
    applyResolvedTracks,
    resolveTrack,
    resolveTracks,
    patchTrackMeta,
    refreshAggregates,
    setKeyword,
    toggleFavorite,
    recordPlayed,
    removeTracks,
    clear,
  }
})
