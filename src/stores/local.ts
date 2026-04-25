import localforage from 'localforage'

// 类型定义
export interface Song {
  id: string
  title: string
  artist: string
  album: string
  path: string
  duration: string
  cover: string
  year: string
  genre: string
  lyric?: string
  isFavorite?: boolean
  // CUE相关字段
  isCueTrack?: boolean
  startTime?: string
  endTime?: string
  trackNumber?: string
  parentFile?: string
  cueInfo?: string
  // 转码相关
  needs_transcode: boolean
}

export interface Playlist {
  id: string
  name: string
  songs: string[] // 存储歌曲ID
  createdAt: string // 使用ISO字符串而不是Date对象
  updatedAt: string // 使用ISO字符串而不是Date对象
}

export interface PlaybackProgress {
  songId: string
  position: number
  isPlaying: boolean
  timestamp: string // 使用ISO字符串而不是Date对象
}

export interface UserSettings {
  theme: 'light' | 'dark'
  language: string
  musicDirectory: string
  volume: number
  playbackMode: 'order' | 'random' | 'repeat'
  equalizerPreset: string
  equalizerBands: number[]
  autoPlay: boolean
  rememberProgress: boolean
  crossfadeEnabled: boolean
  crossfadeDuration: number
  autoPlayNext: boolean
  showLyrics: boolean
  enableTranscode: boolean
  forceTranscode: boolean
}

// 初始化localforage
localforage.config({
  name: 'TPlayer',
  storeName: 'tplayer_data',
  description: 'TPlayer music player data storage'
})

// 存储键名常量
const STORAGE_KEYS = {
  SONGS: 'songs',
  PLAYLISTS: 'playlists',
  FAVORITES: 'favorites',
  PLAYBACK_PROGRESS: 'playback_progress',
  SETTINGS: 'settings',
  LYRIC_CACHE: 'lyric_cache'  // 在线匹配歌词缓存
}

// 默认设置
const DEFAULT_SETTINGS: UserSettings = {
  theme: 'dark',
  language: 'zh-CN',
  musicDirectory: '',
  volume: 80,
  playbackMode: 'order',
  equalizerPreset: 'flat',
  equalizerBands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  autoPlay: true,
  rememberProgress: true,
  crossfadeEnabled: false,
  crossfadeDuration: 1,
  autoPlayNext: true,
  showLyrics: true,
  enableTranscode: true,
  forceTranscode: false
}

// 本地存储服务
class LocalStorageService {
  // 歌曲相关
  async saveSongs(songs: Song[]): Promise<void> {
    await localforage.setItem(STORAGE_KEYS.SONGS, songs)
  }

  async getSongs(): Promise<Song[]> {
    const songs = await localforage.getItem<Song[]>(STORAGE_KEYS.SONGS)
    return songs || []
  }

  async clearSongs(): Promise<void> {
    await localforage.removeItem(STORAGE_KEYS.SONGS)
  }

  // 歌单相关
  async savePlaylists(playlists: Playlist[]): Promise<void> {
    await localforage.setItem(STORAGE_KEYS.PLAYLISTS, playlists)
  }

  async getPlaylists(): Promise<Playlist[]> {
    const playlists = await localforage.getItem<Playlist[]>(STORAGE_KEYS.PLAYLISTS)
    return playlists || []
  }

  async createPlaylist(name: string): Promise<Playlist> {
    const playlists = await this.getPlaylists()
    const now = new Date().toISOString()
    const newPlaylist: Playlist = {
      id: `playlist_${Date.now()}`,
      name,
      songs: [],
      createdAt: now,
      updatedAt: now
    }
    playlists.push(newPlaylist)
    await this.savePlaylists(playlists)
    return newPlaylist
  }

  async updatePlaylist(id: string, updates: Partial<Playlist>): Promise<void> {
    const playlists = await this.getPlaylists()
    const index = playlists.findIndex(p => p.id === id)
    if (index !== -1) {
      playlists[index] = {
        ...playlists[index],
        ...updates,
        updatedAt: new Date().toISOString()
      }
      await this.savePlaylists(playlists)
    }
  }

  async deletePlaylist(id: string): Promise<void> {
    const playlists = await this.getPlaylists()
    const filteredPlaylists = playlists.filter(p => p.id !== id)
    await this.savePlaylists(filteredPlaylists)
  }

  // 收藏歌曲相关
  async saveFavorites(songIds: string[]): Promise<void> {
    await localforage.setItem(STORAGE_KEYS.FAVORITES, songIds)
  }

  async getFavorites(): Promise<string[]> {
    const favorites = await localforage.getItem<string[]>(STORAGE_KEYS.FAVORITES)
    return favorites || []
  }

  async addToFavorites(songId: string): Promise<void> {
    const favorites = await this.getFavorites()
    if (!favorites.includes(songId)) {
      favorites.push(songId)
      await this.saveFavorites(favorites)
    }
  }

  async removeFromFavorites(songId: string): Promise<void> {
    const favorites = await this.getFavorites()
    const filteredFavorites = favorites.filter(id => id !== songId)
    await this.saveFavorites(filteredFavorites)
  }

  async isFavorite(songId: string): Promise<boolean> {
    const favorites = await this.getFavorites()
    return favorites.includes(songId)
  }

  // 播放进度相关
  async savePlaybackProgress(progress: PlaybackProgress): Promise<void> {
    await localforage.setItem(STORAGE_KEYS.PLAYBACK_PROGRESS, progress)
  }

  async getPlaybackProgress(): Promise<PlaybackProgress | null> {
    return await localforage.getItem<PlaybackProgress>(STORAGE_KEYS.PLAYBACK_PROGRESS)
  }

  async clearPlaybackProgress(): Promise<void> {
    await localforage.removeItem(STORAGE_KEYS.PLAYBACK_PROGRESS)
  }

  // 设置相关
  async saveSettings(settings: UserSettings): Promise<void> {
    await localforage.setItem(STORAGE_KEYS.SETTINGS, settings)
  }

  async getSettings(): Promise<UserSettings> {
    const settings = await localforage.getItem<UserSettings>(STORAGE_KEYS.SETTINGS)
    return settings || DEFAULT_SETTINGS
  }

  async updateSettings(updates: Partial<UserSettings>): Promise<void> {
    const currentSettings = await this.getSettings()
    const newSettings = {
      ...currentSettings,
      ...updates
    }
    await this.saveSettings(newSettings)
  }

  // 歌词缓存相关
  async getCachedLyric(songId: string): Promise<string> {
    const cache = await localforage.getItem<Record<string, string>>(STORAGE_KEYS.LYRIC_CACHE)
    return cache?.[songId] || ''
  }

  async saveCachedLyric(songId: string, lyric: string): Promise<void> {
    const cache = await localforage.getItem<Record<string, string>>(STORAGE_KEYS.LYRIC_CACHE) || {}
    cache[songId] = lyric
    await localforage.setItem(STORAGE_KEYS.LYRIC_CACHE, cache)
  }

  async clearCachedLyric(songId: string): Promise<void> {
    const cache = await localforage.getItem<Record<string, string>>(STORAGE_KEYS.LYRIC_CACHE) || {}
    delete cache[songId]
    await localforage.setItem(STORAGE_KEYS.LYRIC_CACHE, cache)
  }

  async clearAllCachedLyrics(): Promise<void> {
    await localforage.removeItem(STORAGE_KEYS.LYRIC_CACHE)
  }

  // 清空所有数据
  async clearAll(): Promise<void> {
    await localforage.clear()
  }
}

// 导出单例实例
export const localStorageService = new LocalStorageService()
