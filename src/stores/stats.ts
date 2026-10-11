/**
 * stats store（收听报告数据源）
 *
 * 职责：维护"播放行为"的本地轻量统计（播放次数 / 累计时长 / 最后播放时间 / 小时分布），
 * 并通过 watch 跟踪 player store 快照自动记录：切歌与暂停恢复不重复计数，
 * 播放时长仅累计实际处于 playing 状态的时间（seek 跳变 / 暂停不产生时长）。
 *
 * 持久化：localStorage（键 STATS_STORAGE_KEY），本地优先、不上传（privacy-first）。
 * 曲目元数据（标题 / 艺术家）不冗余存储，展示时由视图 join library store。
 *
 * 依赖方向：components -> stores/stats -> stores/player / stores/library
 * （player / library 不反向依赖 stats，无循环）。
 */
import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'

import { STATS_STORAGE_KEY } from '../constants'
import { usePlayerStore } from './player'
import { useLibraryStore } from './library'
import type {
  ShareCardData,
  StatsSnapshot,
  TopArtistItem,
  TopTrackItem,
  TrackStat,
} from '../types/stats'

/** 播放时长累计写盘节流阈值（毫秒），避免每次进度事件都写 localStorage */
const PERSIST_THROTTLE_MS = 5_000

function createEmptySnapshot(): StatsSnapshot {
  return { version: 1, tracks: {}, hourCounts: new Array<number>(24).fill(0), updatedAt: 0 }
}

/** 从 localStorage 恢复快照；缺失 / 损坏 / 版本不符时回落空快照（对齐 useLayout 恢复策略） */
function loadSnapshot(): StatsSnapshot {
  try {
    const raw = localStorage.getItem(STATS_STORAGE_KEY)
    if (!raw) return createEmptySnapshot()
    const parsed = JSON.parse(raw) as Partial<StatsSnapshot>
    if (parsed.version !== 1 || typeof parsed.tracks !== 'object' || parsed.tracks === null) {
      return createEmptySnapshot()
    }
    const hourCounts =
      Array.isArray(parsed.hourCounts) && parsed.hourCounts.length === 24
        ? parsed.hourCounts.map((n) => (Number.isFinite(n) && n > 0 ? n : 0))
        : new Array<number>(24).fill(0)
    return { version: 1, tracks: parsed.tracks, hourCounts, updatedAt: parsed.updatedAt ?? 0 }
  } catch {
    return createEmptySnapshot()
  }
}

/** 小时桶计数 +1（返回新数组，保持不可变更新） */
function bumpHour(hourCounts: number[], hour: number): number[] {
  const next = hourCounts.slice()
  next[hour] = (next[hour] ?? 0) + 1
  return next
}

export const useStatsStore = defineStore('stats', () => {
  const playerStore = usePlayerStore()

  const snapshot = ref<StatsSnapshot>(loadSnapshot())

  /* ---- 派生指标（视图直接消费） ---- */

  /** 累计播放次数 */
  const totalPlayCount = computed(() =>
    Object.values(snapshot.value.tracks).reduce((sum, s) => sum + s.playCount, 0),
  )

  /** 累计播放时长（毫秒） */
  const totalPlayMs = computed(() =>
    Object.values(snapshot.value.tracks).reduce((sum, s) => sum + s.totalMs, 0),
  )

  /** 播放曲目数（仍有统计的曲目数量） */
  const playedTrackCount = computed(() => Object.keys(snapshot.value.tracks).length)

  /** 最近一次播放时间（Unix 秒；无记录返回 null） */
  const lastPlayedAt = computed<number | null>(() => {
    let max = 0
    for (const s of Object.values(snapshot.value.tracks)) {
      if (s.lastPlayedAt > max) max = s.lastPlayedAt
    }
    return max > 0 ? max : null
  })

  /** 最常听时段（0-23，播放次数最多的小时；并列取最早出现的小时） */
  const peakHour = computed(() => {
    const counts = snapshot.value.hourCounts
    let best = 0
    let bestCount = counts[0] ?? 0
    for (let h = 1; h < 24; h += 1) {
      const count = counts[h] ?? 0
      if (count > bestCount) {
        best = h
        bestCount = count
      }
    }
    return best
  })

  /** 单曲统计（无记录返回 undefined） */
  function getTrackStat(trackId: string | null | undefined): TrackStat | undefined {
    if (!trackId) return undefined
    return snapshot.value.tracks[trackId]
  }

  /** TOP 曲目（按播放次数降序；次数相同按时长降序） */
  function topTracks(limit = 5): TopTrackItem[] {
    const library = useLibraryStore()
    return Object.values(snapshot.value.tracks)
      .filter((s) => s.playCount > 0)
      .sort((a, b) => b.playCount - a.playCount || b.totalMs - a.totalMs)
      .slice(0, limit)
      .map((s) => {
        const track = library.getTrack(s.trackId)
        return {
          trackId: s.trackId,
          title: track?.title ?? '未知曲目',
          artists: track?.artists ?? [],
          playCount: s.playCount,
          totalMs: s.totalMs,
        }
      })
  }

  /** TOP 艺术家（按曲目播放量聚合：每首曲目的播放次数 / 时长归入其全部艺术家） */
  function topArtists(limit = 5): TopArtistItem[] {
    const library = useLibraryStore()
    const map = new Map<string, TopArtistItem>()
    for (const s of Object.values(snapshot.value.tracks)) {
      if (s.playCount <= 0) continue
      const track = library.getTrack(s.trackId)
      const names = track && track.artists.length > 0 ? track.artists : ['未知艺术家']
      for (const name of names) {
        const entry = map.get(name) ?? { name, playCount: 0, totalMs: 0 }
        entry.playCount += s.playCount
        entry.totalMs += s.totalMs
        map.set(name, entry)
      }
    }
    return [...map.values()]
      .sort((a, b) => b.playCount - a.playCount || b.totalMs - a.totalMs)
      .slice(0, limit)
  }

  /* ---- 写入动作 ---- */

  /** 记录一次播放开始（进入 playing 且当前曲目尚未计次时调用） */
  function recordPlayStart(trackId: string): void {
    const now = Math.floor(Date.now() / 1000)
    const prev = snapshot.value.tracks[trackId]
    snapshot.value = {
      ...snapshot.value,
      tracks: {
        ...snapshot.value.tracks,
        [trackId]: prev
          ? { ...prev, playCount: prev.playCount + 1, lastPlayedAt: now }
          : { trackId, playCount: 1, totalMs: 0, lastPlayedAt: now },
      },
      hourCounts: bumpHour(snapshot.value.hourCounts, new Date().getHours()),
      updatedAt: now,
    }
    persist()
  }

  /** 累计播放时长（毫秒；仅由跟踪器传入正增量） */
  function accumulatePlayMs(trackId: string, ms: number): void {
    const prev = snapshot.value.tracks[trackId]
    if (!prev || ms <= 0) return
    snapshot.value = {
      ...snapshot.value,
      tracks: {
        ...snapshot.value.tracks,
        [trackId]: { ...prev, totalMs: prev.totalMs + ms },
      },
    }
    if (Date.now() - lastPersistAt >= PERSIST_THROTTLE_MS) persist()
  }

  /** 清空全部统计（用户主动重置；仅清除本地统计，不触碰曲库文件） */
  function reset(): void {
    snapshot.value = createEmptySnapshot()
    persist()
  }

  /** 写盘（localStorage 不可用或内容异常时静默忽略，与 useLayout 策略一致） */
  function persist(): void {
    lastPersistAt = Date.now()
    try {
      localStorage.setItem(STATS_STORAGE_KEY, JSON.stringify(snapshot.value))
    } catch {
      /* 隐私模式 / 受限 WebView：统计仅保留在内存 */
    }
  }

  let lastPersistAt = 0

  /* ---- 自动跟踪（player store 快照） ---- */
  // 播放次数：仅当 trackId 从未在当前播放会话中计次时记一次，切歌 / 暂停恢复不重复计数。
  let countedTrackId: string | null = null

  watch(
    () => [playerStore.snapshot.trackId, playerStore.snapshot.status] as const,
    ([trackId, status]) => {
      if (status === 'playing' && trackId) {
        if (countedTrackId !== trackId) {
          countedTrackId = trackId
          recordPlayStart(trackId)
        }
      }
    },
  )

  // 播放时长：仅 playing 状态下 positionMs 的单调正增量计入；
  // seek 跳变（负增量 / 超过 60s 的突变）与暂停不产生时长。
  let lastPosMs = 0

  watch(
    () => [playerStore.snapshot.status, playerStore.snapshot.positionMs] as const,
    ([status, positionMs]) => {
      if (status === 'playing') {
        const diff = positionMs - lastPosMs
        if (diff > 0 && diff < 60_000) {
          const trackId = playerStore.snapshot.trackId
          if (trackId) accumulatePlayMs(trackId, diff)
        }
      }
      lastPosMs = positionMs
    },
  )

  // 退出 / 隐藏窗口前兜底写盘，避免最后几秒时长丢失
  if (typeof window !== 'undefined') {
    window.addEventListener('beforeunload', () => persist())
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'hidden') persist()
    })
  }

  /** 构建分享卡片渲染数据（视图与导出共用同一数据源） */
  function buildShareCardData(title = '我的收听报告'): ShareCardData {
    return {
      title,
      periodText: '自启用统计以来',
      totalMs: totalPlayMs.value,
      trackCount: playedTrackCount.value,
      playCount: totalPlayCount.value,
      topTracks: topTracks(5),
      topArtists: topArtists(5),
      hourCounts: snapshot.value.hourCounts,
      peakHour: peakHour.value,
      generatedAt: Math.floor(Date.now() / 1000),
    }
  }

  return {
    snapshot,
    totalPlayCount,
    totalPlayMs,
    playedTrackCount,
    lastPlayedAt,
    peakHour,
    getTrackStat,
    topTracks,
    topArtists,
    recordPlayStart,
    accumulatePlayMs,
    reset,
    persist,
    buildShareCardData,
  }
})
