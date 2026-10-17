/**
 * 转码结果缓存 —— 按 `trackId:size:modifiedAt` 复用，避免重复解码。
 *
 * 两级：会话内内存 Map（最快）+ IndexedDB（跨会话持久）。
 * 正常文件根本不进入兜底流程，因此缓存只承载「被兜底过」的稀有文件，体积可控。
 */
export interface CacheEntry {
  wav: Blob
  sampleRate: number
  channels: number
  durationMs: number
  size: number
  modifiedAt: number
}

const DB_NAME = 'tplayer-next-compat'
const STORE = 'wav'
// v4: 丢弃 durationMs=0 的坏缓存（fallback 路径 Worker 曾在部分格式上计算出 0 时长）
const DB_VERSION = 4

function keyOf(trackId: string, size: number, modifiedAt: number): string {
  return `${trackId}:${size}:${modifiedAt}`
}

export class CompatCache {
  private mem = new Map<string, CacheEntry>()
  private dbPromise: Promise<IDBDatabase> | null = null

  private openDb(): Promise<IDBDatabase> {
    if (this.dbPromise) return this.dbPromise
    this.dbPromise = new Promise((resolve, reject) => {
      if (typeof indexedDB === 'undefined') {
        reject(new Error('IndexedDB 不可用'))
        return
      }
      const req = indexedDB.open(DB_NAME, DB_VERSION)
      req.onupgradeneeded = () => {
        const db = req.result
        // 升级时清空旧缓存（可能含修复前的错误解码结果），避免命中坏数据
        if (db.objectStoreNames.contains(STORE)) db.deleteObjectStore(STORE)
        db.createObjectStore(STORE)
      }
      req.onsuccess = () => resolve(req.result)
      req.onerror = () => reject(req.error)
    })
    return this.dbPromise
  }

  /** 取出缓存项（命中则复用，跳过解码）；未命中返回 null */
  async get(trackId: string, size: number, modifiedAt: number): Promise<CacheEntry | null> {
    const key = keyOf(trackId, size, modifiedAt)
    const memHit = this.mem.get(key)
    if (memHit) return memHit
    try {
      const db = await this.openDb()
      const entry = await new Promise<CacheEntry | null>((resolve, reject) => {
        const tx = db.transaction(STORE, 'readonly')
        const req = tx.objectStore(STORE).get(key)
        req.onsuccess = () => resolve((req.result as CacheEntry) ?? null)
        req.onerror = () => reject(req.error)
      })
      if (entry) this.mem.set(key, entry)
      return entry
    } catch {
      return null
    }
  }

  /** 写入缓存（内存 + IndexedDB，均失败也不影响播放） */
  async put(trackId: string, size: number, modifiedAt: number, entry: CacheEntry): Promise<void> {
    const key = keyOf(trackId, size, modifiedAt)
    this.mem.set(key, entry)
    try {
      const db = await this.openDb()
      await new Promise<void>((resolve) => {
        const tx = db.transaction(STORE, 'readwrite')
        tx.objectStore(STORE).put(entry, key)
        tx.oncomplete = () => resolve()
        tx.onerror = () => resolve()
      })
    } catch {
      /* 忽略持久化失败 */
    }
  }
}
