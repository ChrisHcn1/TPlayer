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
 * 曲目关键词匹配（调用方保证 key 已 trim + lowercase）。
 * 主匹配域：标题 / 艺术家 / 专辑；
 * 附加匹配域：文件后缀名（如 "flac"、".flac"、"dsf"），供内部按格式筛选用，UI 不提示。
 */
export function matchTrackKeyword(track: Track, key: string): boolean {
  if (`${track.title} ${track.artists.join(' ')} ${track.album}`.toLowerCase().includes(key)) {
    return true
  }
  const ext = track.filePath.split('.').pop()?.toLowerCase() ?? ''
  return ext.length > 0 && ext === key.replace(/^\.+/, '')
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
    try {
      const [trackList, albumList, cueList, sacdList, artistList, libraryStats, favs, recent] =
        await Promise.all([
          libraryService.tracks(),
          libraryService.albums(),
          libraryService.cueAlbums(),
          libraryService.sacdAlbums(),
          libraryService.artists(),
          libraryService.stats(),
          libraryService.favoriteIds(),
          libraryService.recent(),
        ])
      allTracks.value = trackList
      // 普通列表剔除 SACD 5.1 环绕轨（与立体声区同曲目，只差 `[5.1]` 后缀）
      tracks.value = trackList.filter((track) => !isSacdMultich(track))
      albums.value = albumList
      cueAlbums.value = cueList
      sacdAlbums.value = sacdList
      artists.value = artistList
      stats.value = libraryStats
      favoriteIds.value = favs
      recentEntries.value = recent
    } catch (cause) {
      error.value = normalizeError(cause).message
    } finally {
      loading.value = false
    }
  }

  /** 触发一次扫描（扫描根目录取 settings.libraryDirs，由 Rust 侧读取配置） */
  async function scan(): Promise<void> {
    error.value = null
    // 监听一次后端扫描进度事件：
    //  - 中间态写入 scanProgress（既有能力，此前无人订阅，顺带接通）；
    //  - Linux：NTFS 分区上的目录可能因挂载权限无法读取，后端在完成事件里
    //    带回聚合错误（progress.error）。这里捕获并向用户明示，避免
    //    「部分目录被静默跳过、曲目缺失但无任何提示」。Windows 该字段恒为
    //    null，不产生任何提示，行为零变化。
    let scanError: string | null = null
    let unlisten: (() => void) | null = null
    if (isTauriRuntime()) {
      try {
        unlisten = await listen<ScanProgress>('library://scan-progress', (e) => {
          applyScanProgress(e.payload)
          if (e.payload.stage === 'done' && e.payload.error) {
            scanError = e.payload.error
          }
        })
      } catch {
        /* 事件订阅失败不阻断扫描本身 */
      }
    }
    try {
      await libraryService.scan()
      await refresh()
      scanProgress.value = { ...scanProgress.value, stage: 'done' }
      if (scanError) {
        toast.warning(scanError)
      }
    } catch (cause) {
      error.value = normalizeError(cause).message
      scanProgress.value = { ...scanProgress.value, stage: 'failed', error: error.value }
    } finally {
      unlisten?.()
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
    setKeyword,
    toggleFavorite,
    recordPlayed,
    removeTracks,
    clear,
  }
})
