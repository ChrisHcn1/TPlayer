/**
 * useTrackCover —— 封面加载（曲目 / 专辑 / 艺术家）
 *
 * 与后端两级缓存配合：
 *  - 模块级 urlCache 缓存已取到的 data URL（null = 已确认无封面，做负缓存）；
 *  - inflight 合并并发请求（播放条封面 / 放大层同时挂载时只发一次 IPC）。
 *
 * 缓存 key 约定（与 Rust 命令层一致）：
 *  - `track:<id>`   单曲封面（内嵌 → 同名边车 → 目录图 → 在线，data URL）
 *  - `album:<专辑名>:<主艺术家>`  专辑聚合封面（album.id 原文即此值，asset URL）
 *  - `artist:<名>`  艺术家聚合封面（asset URL）
 *
 * 组件用法：
 *   const { coverUrl } = useTrackCover(() => playerStore.snapshot.trackId)
 *   const { coverUrl } = useGroupCover(() => album.id)          // 专辑
 *   const { coverUrl } = useGroupCover(() => `artist:${name}`)  // 艺术家
 */
import { shallowRef, watch, type Ref } from 'vue'

import { libraryService } from '../services/library'

/** 封面 URL 缓存上限（超出淘汰最久未用，防止网格滚过几千张封面后内存膨胀） */
const CACHE_LIMIT = 240

/** LRU Map：get/set 时 delete + set 把条目提到最新位，超限时淘汰队首（最旧） */
const urlCache = new Map<string, string | null>()
const inflight = new Map<string, Promise<string | null>>()

function cacheGet(key: string): string | null | undefined {
  const value = urlCache.get(key)
  if (value !== undefined) {
    urlCache.delete(key)
    urlCache.set(key, value)
  }
  return value
}

function cacheSet(key: string, value: string | null): void {
  urlCache.delete(key)
  urlCache.set(key, value)
  while (urlCache.size > CACHE_LIMIT) {
    const oldest = urlCache.keys().next().value
    if (oldest === undefined) break
    urlCache.delete(oldest)
  }
}

function fetchCover(key: string): Promise<string | null> {
  // 专辑 key 即完整 album_id（自带 album: 前缀），原样传后端
  if (key.startsWith('album:')) return libraryService.albumCover(key)
  if (key.startsWith('artist:')) return libraryService.artistCover(key.slice(7))
  // 兼容裸曲目 id 与 `track:` 前缀两种写法
  const trackId = key.startsWith('track:') ? key.slice(6) : key
  return libraryService.cover(trackId)
}

/** 按 key 加载封面（带 LRU 缓存与并发去重）；无封面 resolve null，不抛错 */
export function loadCover(key: string): Promise<string | null> {
  const cached = cacheGet(key)
  if (cached !== undefined) return Promise.resolve(cached)

  const pending = inflight.get(key)
  if (pending) return pending

  const task = fetchCover(key)
    .then((url) => {
      cacheSet(key, url)
      inflight.delete(key)
      return url
    })
    .catch(() => {
      // 失败按"无封面"处理并负缓存，避免反复弹错；重新扫描后如需刷新可另加失效逻辑
      cacheSet(key, null)
      inflight.delete(key)
      return null
    })
  inflight.set(key, task)
  return task
}

/** 加载单曲封面（track:<id> 语义的语义化封装） */
export function loadTrackCover(trackId: string): Promise<string | null> {
  return loadCover(`track:${trackId}`)
}

/**
 * 跟随 key 自动加载封面；key 变化时先清空（避免短暂显示上一封面）。
 * 入参为 getter，便于直接传 () => `album:${albumId}`。
 */
export function useCover(keyGetter: () => string | null | undefined): {
  coverUrl: Ref<string | null>
} {
  const coverUrl = shallowRef<string | null>(null)

  watch(
    keyGetter,
    (key) => {
      coverUrl.value = null
      if (!key) return
      void loadCover(key).then((url) => {
        // 加载期间 key 又变化：丢弃过期响应
        if (keyGetter() === key) coverUrl.value = url
      })
    },
    { immediate: true },
  )

  return { coverUrl }
}

/** 跟随单曲 id 自动加载封面（播放条/放大层用） */
export function useTrackCover(trackIdGetter: () => string | null | undefined): {
  coverUrl: Ref<string | null>
} {
  return useCover(() => {
    const id = trackIdGetter()
    return id ? `track:${id}` : null
  })
}

/** 跟随专辑/艺术家分组 key 自动加载封面（网格卡片/详情头部用） */
export function useGroupCover(keyGetter: () => string | null | undefined): {
  coverUrl: Ref<string | null>
} {
  return useCover(keyGetter)
}
