import { invoke } from '@tauri-apps/api/core'

export interface CacheMetadata {
  key: string
  category: 'lyrics' | 'covers' | 'metadata'
  source: string
  size: number
  createdAt: number
  lastAccessedAt: number
  accessCount: number
}

export interface CacheIndex {
  version: number
  lastCleanup: number
  entries: Record<string, CacheMetadata>
}

class CacheService {
  private readonly INDEX_KEY = 'cache_index'
  private readonly MAX_COVERS = 100
  private index: CacheIndex | null = null

  constructor() {
    this.loadIndex()
  }

  private async loadIndex() {
    try {
      const cached = localStorage.getItem(this.INDEX_KEY)
      if (cached) {
        this.index = JSON.parse(cached)
      } else {
        this.index = {
          version: 1,
          lastCleanup: Date.now(),
          entries: {}
        }
      }
    } catch {
      this.index = {
        version: 1,
        lastCleanup: Date.now(),
        entries: {}
      }
    }
  }

  private saveIndex() {
    if (this.index) {
      localStorage.setItem(this.INDEX_KEY, JSON.stringify(this.index))
    }
  }

  /**
   * 保存歌词到缓存
   */
  async saveLyric(key: string, source: string, data: any): Promise<void> {
    try {
      const dataStr = JSON.stringify(data)
      const encoder = new TextEncoder()
      const bytes = encoder.encode(dataStr)
      
      await invoke<string>('save_to_cache', {
        category: 'lyrics',
        filename: `${key}.json`,
        data: Array.from(bytes)
      })

      this.updateIndex(key, 'lyrics', source, bytes.length)
    } catch (error) {
      console.error('保存歌词缓存失败:', error)
    }
  }

  /**
   * 从缓存读取歌词
   */
  async getLyric(key: string): Promise<any | null> {
    try {
      const bytes = await invoke<number[]>('get_cached_file', {
        category: 'lyrics',
        filename: `${key}.json`
      })

      const decoder = new TextDecoder()
      const dataStr = decoder.decode(new Uint8Array(bytes))
      const data = JSON.parse(dataStr)

      this.updateAccess(key)
      return data
    } catch {
      return null
    }
  }

  /**
   * 保存封面到缓存
   */
  async saveCover(key: string, source: string, data: Uint8Array): Promise<void> {
    try {
      await invoke<string>('save_to_cache', {
        category: 'covers',
        filename: `${key}.jpg`,
        data: Array.from(data)
      })

      this.updateIndex(key, 'covers', source, data.length)
      await this.enforceCoverLimit()
    } catch (error) {
      console.error('保存封面缓存失败:', error)
    }
  }

  /**
   * 从缓存读取封面
   */
  async getCover(key: string): Promise<string | null> {
    try {
      const bytes = await invoke<number[]>('get_cached_file', {
        category: 'covers',
        filename: `${key}.jpg`
      })

      const blob = new Blob([new Uint8Array(bytes)], { type: 'image/jpeg' })
      const url = URL.createObjectURL(blob)

      this.updateAccess(key)
      return url
    } catch {
      return null
    }
  }

  /**
   * 清理过期缓存
   */
  async clearCache(olderThanDays: number = 30): Promise<number> {
    try {
      const cleaned = await invoke<number>('clear_cache', {
        category: 'covers',
        olderThanDays
      })
      
      // 清理索引
      if (this.index) {
        const now = Date.now()
        const threshold = olderThanDays * 86400000
        
        for (const [key, meta] of Object.entries(this.index.entries)) {
          if (now - meta.lastAccessedAt > threshold) {
            delete this.index.entries[key]
          }
        }
        
        this.index.lastCleanup = now
        this.saveIndex()
      }

      return cleaned
    } catch {
      return 0
    }
  }

  private updateIndex(key: string, category: string, source: string, size: number) {
    if (!this.index) return

    const now = Date.now()
    this.index.entries[key] = {
      key,
      category: category as any,
      source,
      size,
      createdAt: now,
      lastAccessedAt: now,
      accessCount: 1
    }

    this.saveIndex()
  }

  private updateAccess(key: string) {
    if (!this.index?.entries[key]) return

    this.index.entries[key].lastAccessedAt = Date.now()
    this.index.entries[key].accessCount++
    this.saveIndex()
  }

  private async enforceCoverLimit() {
    if (!this.index) return

    const coverEntries = Object.entries(this.index.entries)
      .filter(([_, meta]) => meta.category === 'covers')
      .sort((a, b) => a[1].lastAccessedAt - b[1].lastAccessedAt)

    if (coverEntries.length > this.MAX_COVERS) {
      const toDelete = coverEntries.slice(0, coverEntries.length - this.MAX_COVERS)
      
      for (const [key] of toDelete) {
        try {
          await invoke('clear_cache', {
            category: 'covers',
            olderThanDays: 0
          })
          delete this.index.entries[key]
        } catch {
          // 忽略错误
        }
      }

      this.saveIndex()
    }
  }
}

export const cacheService = new CacheService()
