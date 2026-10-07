/**
 * Web 预览版曲库服务 —— services/library 在非 Tauri 环境下的实现
 *
 * 数据来源：services/web/dataset 的确定性 mock 曲目；
 * 用户数据（收藏 / 最近播放 / 歌单 / 已移除曲目）持久化到 localStorage。
 *
 * 与 Tauri 实现同构：方法签名、返回类型、错误语义一致，
 * M2 接通 Rust 命令后仅替换实现，store / 视图层零改动。
 */
import {
  WEB_FAVORITES_STORAGE_KEY,
  WEB_PLAYLISTS_STORAGE_KEY,
  WEB_RECENT_STORAGE_KEY,
  WEB_REMOVED_STORAGE_KEY,
} from '../../constants'
import type { Album, Artist, LibraryStats, Playlist, RecentEntry, Track } from '../../types'
import {
  aggregateAlbums,
  aggregateArtists,
  buildStats,
  CUE_ALBUM_IDS,
  getAllTracks,
} from './dataset'

const RECENT_LIMIT = 200

function readJSON<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key)
    return raw === null ? fallback : (JSON.parse(raw) as T)
  } catch {
    return fallback
  }
}

function writeJSON(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch {
    /* 配额 / 隐私模式：本次会话仍生效，仅放弃持久化 */
  }
}

function readStringSet(key: string): Set<string> {
  const raw = readJSON<unknown>(key, [])
  if (!Array.isArray(raw)) return new Set()
  return new Set(raw.filter((item): item is string => typeof item === 'string'))
}

function defaultPlaylists(): Playlist[] {
  const now = Math.floor(Date.now() / 1000)
  return [
    {
      id: 'p-seeded-1',
      name: '深夜独处',
      description: '演示歌单：适合夜晚的低缓曲目',
      trackIds: ['t1', 't11', 't26', 't33', 't44'],
      createdAt: now - 86_400 * 12,
      updatedAt: now - 86_400,
      isSystem: false,
    },
  ]
}

function readPlaylists(): Playlist[] {
  const raw = readJSON<unknown>(WEB_PLAYLISTS_STORAGE_KEY, null)
  if (!Array.isArray(raw)) return defaultPlaylists()
  return raw.filter(isPlaylist)
}

function isPlaylist(value: unknown): value is Playlist {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Partial<Playlist>
  return (
    typeof candidate.id === 'string' &&
    typeof candidate.name === 'string' &&
    Array.isArray(candidate.trackIds)
  )
}

/** 当前可见曲目 = 全量数据集 − 用户已从曲库移除的 id */
function visibleTracks(): Track[] {
  const removed = readStringSet(WEB_REMOVED_STORAGE_KEY)
  return getAllTracks().filter((track) => !removed.has(track.id))
}

/**
 * 确定性渐变 SVG 占位封面（web 预览专用，纯本地 data URL）。
 * seed 决定配色（同专辑曲目配色一致，模拟"专辑共用封面"），initial 为封面大字。
 */
function syntheticCover(seed: string, initialSource: string): string {
  let hash = 0
  for (let i = 0; i < seed.length; i += 1) {
    hash = (hash * 31 + seed.charCodeAt(i)) >>> 0
  }
  const hue1 = hash % 360
  const hue2 = (hue1 + 40 + (hash % 50)) % 360
  const initial = (initialSource.trim()[0] ?? '♪').toUpperCase()
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200">` +
    `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">` +
    `<stop offset="0" stop-color="hsl(${hue1},52%,52%)"/>` +
    `<stop offset="1" stop-color="hsl(${hue2},58%,34%)"/>` +
    `</linearGradient></defs>` +
    `<rect width="200" height="200" fill="url(#g)"/>` +
    `<text x="100" y="100" dy="0.36em" text-anchor="middle" font-family="Segoe UI,sans-serif" ` +
    `font-size="92" font-weight="600" fill="rgba(255,255,255,0.92)">${initial}</text></svg>`
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`
}

function delay(): Promise<void> {
  return Promise.resolve()
}

function makePlaylistId(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) return crypto.randomUUID()
  return `p-${Date.now()}-${Math.floor(Math.random() * 1e6)}`
}

export const webLibrary = {
  async scan(): Promise<void> {
    // Web 预览无真实扫描：重新扫描视为"恢复全部 mock 曲目"（清空移除记录）
    writeJSON(WEB_REMOVED_STORAGE_KEY, [])
    await delay()
  },

  async cancelScan(): Promise<void> {},

  async tracks(): Promise<Track[]> {
    return visibleTracks()
  },

  /**
   * Web 预览无真实内嵌封面：按专辑/标题生成确定性的渐变 SVG 封面，
   * 纯本地 data URL（离线可用），仅用于界面联调，与 Tauri 实现同构返回。
   */
  async cover(trackId: string): Promise<string | null> {
    const track = visibleTracks().find((item) => item.id === trackId)
    if (!track) return null
    return syntheticCover(track.album || track.title, track.title)
  },

  /** Web 预览的专辑聚合封面：同专辑种子与单曲封面一致，视觉上即为共用封面 */
  async albumCover(albumId: string): Promise<string | null> {
    const album = aggregateAlbums(visibleTracks()).find((item) => item.id === albumId)
    if (!album) return null
    return syntheticCover(album.name, album.name)
  },

  async artistCover(artist: string): Promise<string | null> {
    const exists = visibleTracks().some((item) => item.artists[0] === artist)
    return exists ? syntheticCover(artist, artist) : null
  },

  async albums(): Promise<Album[]> {
    const tracks = visibleTracks()
    // 普通专辑视图排除 CUE 专辑（CUE 由独立入口承载）
    return aggregateAlbums(tracks).filter((album) => !CUE_ALBUM_IDS.has(album.id))
  },

  async cueAlbums(): Promise<Album[]> {
    const tracks = visibleTracks()
    return aggregateAlbums(tracks).filter((album) => CUE_ALBUM_IDS.has(album.id))
  },

  async artists(): Promise<Artist[]> {
    return aggregateArtists(visibleTracks())
  },

  async stats(): Promise<LibraryStats> {
    const tracks = visibleTracks()
    const albums = aggregateAlbums(tracks)
    const artists = aggregateArtists(tracks)
    return buildStats(tracks, albums, artists)
  },

  /* ---- 收藏 ---- */
  async favoriteIds(): Promise<string[]> {
    return [...readStringSet(WEB_FAVORITES_STORAGE_KEY)]
  },

  async setFavorite(trackId: string, favorite: boolean): Promise<void> {
    const set = readStringSet(WEB_FAVORITES_STORAGE_KEY)
    if (favorite) set.add(trackId)
    else set.delete(trackId)
    writeJSON(WEB_FAVORITES_STORAGE_KEY, [...set])
  },

  /* ---- 最近播放 ---- */
  async recent(): Promise<RecentEntry[]> {
    const raw = readJSON<unknown>(WEB_RECENT_STORAGE_KEY, [])
    if (!Array.isArray(raw)) return []
    return raw
      .filter(
        (item): item is RecentEntry =>
          typeof item === 'object' &&
          item !== null &&
          typeof (item as RecentEntry).trackId === 'string' &&
          typeof (item as RecentEntry).playedAt === 'number',
      )
      .slice(0, RECENT_LIMIT)
  },

  async recordPlayed(trackId: string): Promise<void> {
    const playedAt = Math.floor(Date.now() / 1000)
    const current = await webLibrary.recent()
    const next = [{ trackId, playedAt }, ...current.filter((entry) => entry.trackId !== trackId)]
    writeJSON(WEB_RECENT_STORAGE_KEY, next.slice(0, RECENT_LIMIT))
  },

  /* ---- 从曲库移除（仅移除索引，不删除磁盘文件；重新扫描可恢复 mock） ---- */
  async removeTracks(ids: string[]): Promise<void> {
    const set = readStringSet(WEB_REMOVED_STORAGE_KEY)
    for (const id of ids) set.add(id)
    writeJSON(WEB_REMOVED_STORAGE_KEY, [...set])
  },

  /* ---- 曲库目录（Web 预览无白名单概念，保留 no-op 以对齐契约） ---- */
  async addLibraryDir(_path: string): Promise<void> {},
  async removeLibraryDir(_path: string): Promise<void> {},

  /* ---- 歌单 ---- */
  async listPlaylists(): Promise<Playlist[]> {
    return readPlaylists()
  },

  async createPlaylist(name: string): Promise<Playlist> {
    const now = Math.floor(Date.now() / 1000)
    const playlist: Playlist = {
      id: makePlaylistId(),
      name,
      description: '',
      trackIds: [],
      createdAt: now,
      updatedAt: now,
      isSystem: false,
    }
    const playlists = readPlaylists()
    writeJSON(WEB_PLAYLISTS_STORAGE_KEY, [playlist, ...playlists])
    return playlist
  },

  async renamePlaylist(id: string, name: string): Promise<void> {
    const playlists = readPlaylists().map((playlist) =>
      playlist.id === id
        ? { ...playlist, name, updatedAt: Math.floor(Date.now() / 1000) }
        : playlist,
    )
    writeJSON(WEB_PLAYLISTS_STORAGE_KEY, playlists)
  },

  async deletePlaylist(id: string): Promise<void> {
    writeJSON(
      WEB_PLAYLISTS_STORAGE_KEY,
      readPlaylists().filter((playlist) => playlist.id !== id),
    )
  },

  async setPlaylistTracks(id: string, trackIds: string[]): Promise<void> {
    const playlists = readPlaylists().map((playlist) =>
      playlist.id === id
        ? { ...playlist, trackIds: [...trackIds], updatedAt: Math.floor(Date.now() / 1000) }
        : playlist,
    )
    writeJSON(WEB_PLAYLISTS_STORAGE_KEY, playlists)
  },

  async addTracksToPlaylist(id: string, trackIds: string[]): Promise<void> {
    const playlists = readPlaylists().map((playlist) => {
      if (playlist.id !== id) return playlist
      const merged = [...playlist.trackIds]
      for (const trackId of trackIds) {
        if (!merged.includes(trackId)) merged.push(trackId)
      }
      return { ...playlist, trackIds: merged, updatedAt: Math.floor(Date.now() / 1000) }
    })
    writeJSON(WEB_PLAYLISTS_STORAGE_KEY, playlists)
  },

  async removeTrackFromPlaylist(id: string, trackId: string): Promise<void> {
    const playlists = readPlaylists().map((playlist) =>
      playlist.id === id
        ? {
            ...playlist,
            trackIds: playlist.trackIds.filter((item) => item !== trackId),
            updatedAt: Math.floor(Date.now() / 1000),
          }
        : playlist,
    )
    writeJSON(WEB_PLAYLISTS_STORAGE_KEY, playlists)
  },
}
