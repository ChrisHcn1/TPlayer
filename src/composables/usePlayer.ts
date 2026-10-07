/**
 * usePlayer —— 播放器组合式函数
 * 定位：把 player store 的细粒度状态整理成 UI 直接可用的一组派生值，
 * 使组件不重复书写进度换算/按钮禁用逻辑。
 */
import { computed } from 'vue'
import { storeToRefs } from 'pinia'

import { usePlayerStore } from '../stores/player'

export function usePlayer() {
  const playerStore = usePlayerStore()
  const { snapshot } = storeToRefs(playerStore)

  const isPlaying = computed(() => snapshot.value.status === 'playing')
  const isLoading = computed(() => snapshot.value.status === 'loading')
  const hasError = computed(() => snapshot.value.status === 'error')

  /** 进度百分比 0~100，时长未知时为 0 */
  const progressPercent = computed(() => {
    const { positionMs, durationMs } = snapshot.value
    if (durationMs <= 0) return 0
    return Math.min(100, Math.max(0, (positionMs / durationMs) * 100))
  })

  /** 剩余时间（毫秒），用于 UI 展示 */
  const remainingMs = computed(() => {
    const { positionMs, durationMs } = snapshot.value
    return Math.max(0, durationMs - positionMs)
  })

  return {
    snapshot,
    isPlaying,
    isLoading,
    hasError,
    progressPercent,
    remainingMs,
    play: playerStore.play,
    toggle: playerStore.toggle,
    next: playerStore.next,
    previous: playerStore.previous,
    seek: playerStore.seek,
    setVolume: playerStore.setVolume,
    setMode: playerStore.setMode,
  }
}
