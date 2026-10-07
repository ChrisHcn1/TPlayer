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

import { libraryService } from '../services/library'
import { normalizeError } from '../services/ipc'
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

export const useLibraryStore = defineStore('library', () => {
  const tracks = ref<Track[]>([])
  const albums = ref<Album[]>([])
  const cueAlbums = ref<Album[]>([])
  const artists = ref<Artist[]>([])
  const stats = ref<LibraryStats | null>(null)
  const favoriteIds = ref<string[]>([])
  const recentEntries = ref<RecentEntry[]>([])
  const scanProgress = ref<ScanProgress>(createIdleScanProgress())
  const keyword = ref('')
  const loading = ref(false)
  const error = ref<string | null>(null)

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
    for (const track of tracks.value) map.set(track.id, track)
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
    return [...albums.value, ...cueAlbums.value].find((album) => album.id === id)
  }

  function artistByName(name: string | null | undefined): Artist | undefined {
    if (!name) return undefined
    return artists.value.find((artist) => artist.name === name)
  }

  /** 专辑下的曲目（按音轨序号排序；找不到时回落到空数组） */
  function tracksOfAlbum(albumId: string): Track[] {
    return tracks.value
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
      const [trackList, albumList, cueList, artistList, libraryStats, favs, recent] =
        await Promise.all([
          libraryService.tracks(),
          libraryService.albums(),
          libraryService.cueAlbums(),
          libraryService.artists(),
          libraryService.stats(),
          libraryService.favoriteIds(),
          libraryService.recent(),
        ])
      tracks.value = trackList
      albums.value = albumList
      cueAlbums.value = cueList
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
    try {
      await libraryService.scan()
      await refresh()
      scanProgress.value = { ...scanProgress.value, stage: 'done' }
    } catch (cause) {
      error.value = normalizeError(cause).message
      scanProgress.value = { ...scanProgress.value, stage: 'failed', error: error.value }
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
    albums.value = []
    cueAlbums.value = []
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
    cancelScan,
    applyScanProgress,
    setKeyword,
    toggleFavorite,
    recordPlayed,
    removeTracks,
    clear,
  }
})
