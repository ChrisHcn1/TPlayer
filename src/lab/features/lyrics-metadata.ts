/**
 * 元数据增强的子能力：歌词边车元数据补全（并入 enhanced-metadata，无独立开关）
 *
 * 无损音频文件常因标签缺失被扫描为「未知艺术家 / 未知专辑」，且无法靠常规解析修复。
 * 本能力在**曲目行渲染时**（播放列表 / 全部歌曲 / 专辑等共用 TrackRow）按需读取同名
 * `.lrc` 边车，解析其中的 [ar:]/[al:]/[ti:]/[au:]/[by:] 标签，回填到曲库 store 的
 * 曲目对象上，使所有视图（含播放列表）就地显示真实艺术家/专辑。
 *
 * 开关：随「元数据增强」（enhanced-metadata）一起开启，无需单独开关与介绍。
 *
 * 设计要点：
 *  - 仅当 lab 开启、且当前字段确为占位（未知艺术家/未知专辑/未知标题）时才补全；
 *    已有真实值的字段绝不被覆盖（优先级：曲库现有值 > 边车歌词）。
 *  - 模块级 LRU 缓存 + inflight 去重：同一 track 只解析一次，滚动复用零额外 IO。
 *  - 非阻塞：先渲染占位，异步回填后响应式更新，不刷新整列。
 *  - 浏览器预览无 Tauri 运行时，返回 null（能力隐式关闭）。
 */

import { watch } from 'vue'

import { getSidecarMetadata } from '../../services/lyrics'
import type { SidecarMeta } from '../../types'
import { useLabStore } from '../../stores/lab'
import { useLibraryStore } from '../../stores/library'

/** 曲库扫描期产生的占位文本（与 src-tauri/src/library/scan.rs 保持一致） */
const UNKNOWN_ARTIST = '未知艺术家'
const UNKNOWN_ALBUM = '未知专辑'
const UNKNOWN_TITLE = '未知标题'

const MAX_CACHE = 2000
const metaCache = new Map<string, SidecarMeta | null>()
const inflight = new Map<string, Promise<SidecarMeta | null>>()

function cacheSet(id: string, meta: SidecarMeta | null): void {
  metaCache.set(id, meta)
  if (metaCache.size > MAX_CACHE) {
    const oldest = metaCache.keys().next().value
    if (oldest !== undefined) metaCache.delete(oldest)
  }
}

/**
 * 在曲目行挂载：trackId（或 lab 开关）变化时，按规则尝试从边车歌词补全元数据。
 * @param trackIdGetter 返回当前行曲目 id 的 getter（需是响应式的）
 */
export function useLyricsMetadata(trackIdGetter: () => string | undefined | null): void {
  const lab = useLabStore()
  const library = useLibraryStore()

  watch(
    () => [trackIdGetter(), lab.isEnabled('enhanced-metadata')] as const,
    async ([id]) => {
      if (!id) return
      if (!lab.isEnabled('enhanced-metadata')) return

      const track = library.getTrack(id)
      if (!track) return
      const needArtist = track.artists.includes(UNKNOWN_ARTIST)
      const needAlbum = track.album === UNKNOWN_ALBUM
      const needTitle = track.title === UNKNOWN_TITLE
      if (!needArtist && !needAlbum && !needTitle) return

      // 命中缓存 / 去重中的在途请求，避免重复 IO
      const cached = metaCache.get(id)
      if (cached !== undefined) {
        apply(id, cached)
        return
      }
      const pending = inflight.get(id)
      if (pending) {
        apply(id, await pending)
        return
      }

      const task = (async () => {
        try {
          return await getSidecarMetadata(id)
        } catch {
          return null
        }
      })()
      inflight.set(id, task)
      const meta = await task
      inflight.delete(id)
      cacheSet(id, meta)
      apply(id, meta)
    },
    { immediate: true },
  )

  function apply(id: string, meta: SidecarMeta | null): void {
    if (!meta) return
    const track = library.getTrack(id)
    if (!track) return
    const patch: { artists?: string[]; album?: string; title?: string } = {}
    // 艺术家：ar 优先，缺省回退 au（作者）/ by（制作者）
    if (track.artists.includes(UNKNOWN_ARTIST) && (meta.artist || meta.author || meta.by)) {
      patch.artists = [(meta.artist ?? meta.author ?? meta.by) as string]
    }
    // 专辑：仅 al
    if (track.album === UNKNOWN_ALBUM && meta.album) {
      patch.album = meta.album
    }
    // 标题：仅 ti（曲目本身确为未知时才补）
    if (track.title === UNKNOWN_TITLE && meta.title) {
      patch.title = meta.title
    }
    if (Object.keys(patch).length > 0) library.patchTrackMeta(id, patch)
  }
}
