/**
 * lyrics store（M0 骨架）
 *
 * 职责：持有当前曲目的歌词、加载状态与偏移校正。
 * 约定：歌词渲染只依赖本 store 的 lyrics + 播放位置（由 player store 提供），
 * 组件内不做时间轴计算。
 */
import { ref } from 'vue'
import { defineStore } from 'pinia'

import { lyricsService } from '../services/lyrics'
import { normalizeError } from '../services/ipc'
import type { Lyrics, LyricsStatus } from '../types'

export const useLyricsStore = defineStore('lyrics', () => {
  const lyrics = ref<Lyrics | null>(null)
  const status = ref<LyricsStatus>('idle')
  const error = ref<string | null>(null)

  /**
   * 加载指定曲目的歌词；无歌词时为 empty（非错误）。
   * 多个消费者（桌面歌词条 / 右面板歌词页签）会 watch 同一 trackId，
   * 默认对"同一曲目且已就绪"的请求去重；force=true 用于手动在线匹配强制重走歌词链。
   */
  async function load(trackId: string, force = false): Promise<void> {
    if (!force && lyrics.value?.trackId === trackId && status.value === 'ready') return
    status.value = 'loading'
    error.value = null
    try {
      const loaded = await lyricsService.load(trackId)
      lyrics.value = loaded
      status.value = loaded ? 'ready' : 'empty'
    } catch (cause) {
      lyrics.value = null
      status.value = 'error'
      error.value = normalizeError(cause).message
    }
  }

  /** 用户手动校正偏移（毫秒，正数表示整体延后） */
  async function setOffset(offsetMs: number): Promise<void> {
    if (!lyrics.value) return
    const next = { ...lyrics.value, offsetMs }
    lyrics.value = next
    try {
      await lyricsService.saveOffset(next.trackId, offsetMs)
    } catch (cause) {
      error.value = normalizeError(cause).message
    }
  }

  /**
   * 按播放位置取当前歌词行下标（纯函数，不修改状态）。
   * M0 采用线性扫描；M1 改为二分查找以支持长歌词（数千行）。
   */
  function activeLineIndexAt(positionMs: number): number {
    const lines = lyrics.value?.lines
    if (!lines || lines.length === 0) return -1
    const offset = lyrics.value?.offsetMs ?? 0
    const target = positionMs - offset
    let active = -1
    for (let index = 0; index < lines.length; index += 1) {
      if (lines[index].timeMs <= target) active = index
      else break
    }
    return active
  }

  function clear(): void {
    lyrics.value = null
    status.value = 'idle'
    error.value = null
  }

  return { lyrics, status, error, load, setOffset, activeLineIndexAt, clear }
})
