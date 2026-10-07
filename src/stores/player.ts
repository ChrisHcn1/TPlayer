/**
 * player store（M0 骨架）
 *
 * 职责边界（相对旧版的关键改动）：
 *  - 旧版把播放状态、UI 状态、网络请求全部混在 App.vue（8985 行）中；
 *  - v2 中本 store 只维护"后端快照的本地镜像"与用户意图动作，不直接碰 IPC，
 *    所有 IPC 经 services/player 的 IPlayer 实现（见 ARCHITECTURE.md §4）；
 *  - 后端是唯一真相源：前端不自行推导播放状态，M1 起通过 playback://event 事件同步。
 *
 * 依赖方向：components -> stores/player -> services/player -> Rust commands。
 */
import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import { createIdleSnapshot, playerService } from '../services/player'
import { normalizeError } from '../services/ipc'
import type { PlayContext, PlayMode, PlaybackEvent, PlaybackSnapshot } from '../types'

/**
 * 播放上下文（含起始下标）。
 * types/player.ts 的 PlayContext 未声明 startIndex，而 TrackList 需要"从第 N 首开始播放"；
 * 这里以扩展类型补齐，避免改动已落盘的类型文件，M1 上游统一后再收敛到 types。
 */
export type PlayRequestContext = PlayContext & { startIndex?: number }

function clamp01(value: number): number {
  if (!Number.isFinite(value)) return 0
  return Math.min(1, Math.max(0, value))
}

export const usePlayerStore = defineStore('player', () => {
  /** 当前播放快照（只读消费方：PlayerBar / usePlayer） */
  const snapshot = ref<PlaybackSnapshot>(createIdleSnapshot())
  const initialized = ref(false)
  /** 最近一次播放的上下文（决定队列与“下一首”语义）；RightPanel 队列据此渲染 */
  const currentContext = ref<PlayContext | null>(null)
  /** 取消后端事件订阅（init 时登记，dispose 时注销） */
  let unsubscribe: (() => void) | null = null

  /** 当前队列曲目 id 列表（无上下文时为空） */
  const queueTrackIds = computed(() => currentContext.value?.trackIds ?? [])
  /** 当前曲目在队列中的下标 */
  const currentIndex = computed(() => {
    const id = snapshot.value.trackId
    if (!id || !currentContext.value) return -1
    return currentContext.value.trackIds.indexOf(id)
  })

  /** 统一动作包装：成功后同步后端快照，失败则落入 error 状态（不向上抛，避免 UI 崩溃） */
  async function execute(action: () => Promise<void>): Promise<void> {
    try {
      await action()
    } catch (cause) {
      snapshot.value = {
        ...snapshot.value,
        status: 'error',
        error: normalizeError(cause).message,
      }
    }
  }

  /** 应用后端返回的完整快照（后端为唯一真相源） */
  function applySnapshot(next: PlaybackSnapshot): void {
    snapshot.value = next
  }

  /** 应用后端推送的增量事件（M1 接入 listen('playback://event', ...) 后调用） */
  function applyEvent(event: PlaybackEvent): void {
    switch (event.type) {
      case 'status':
        // 回到正常状态时清掉残留错误（自动跳过坏曲后只收到事件、无快照回拉的路径也能复位）
        snapshot.value = {
          ...snapshot.value,
          status: event.payload.status,
          error: event.payload.status === 'error' ? snapshot.value.error : null,
        }
        return
      case 'track':
        snapshot.value = {
          ...snapshot.value,
          trackId: event.payload.trackId,
          durationMs: event.payload.durationMs,
          positionMs: 0,
          error: null,
        }
        return
      case 'progress':
        snapshot.value = { ...snapshot.value, positionMs: event.payload.positionMs }
        return
      case 'volume':
        snapshot.value = {
          ...snapshot.value,
          volume: event.payload.volume,
          muted: event.payload.muted,
        }
        return
      case 'mode':
        snapshot.value = { ...snapshot.value, mode: event.payload.mode }
        return
      case 'ended':
        // 自动连播时后端紧随其后会发 Loading/Track/Playing，
        // 这里只重置进度、不动 status，避免播放条在切歌瞬间闪烁为 stopped
        snapshot.value = { ...snapshot.value, positionMs: 0 }
        return
      case 'error':
        snapshot.value = {
          ...snapshot.value,
          status: 'error',
          error: event.payload.message,
        }
        return
      default:
        return
    }
  }

  async function init(): Promise<void> {
    if (initialized.value) return
    await execute(async () => {
      // 先订阅再初始化：避免引擎就绪后的首个事件丢失（如恢复上次播放）
      unsubscribe = playerService.subscribe((event) => applyEvent(event))
      // 断点续播：后端恢复的上次播放上下文（队列显示据此还原）
      const restoredContext = await playerService.init()
      if (restoredContext) currentContext.value = restoredContext
      applySnapshot(await playerService.snapshot())
      initialized.value = true
    })
  }

  /** 播放单曲；context 缺省时后端按"曲库全量"作为上下文 */
  async function play(trackId: string, context?: PlayContext): Promise<void> {
    await execute(async () => {
      if (context) currentContext.value = context
      // 切歌时同步重置进度，避免旧曲目进度残留到新曲目 Loading 窗口期
      snapshot.value = {
        ...snapshot.value,
        status: 'loading',
        trackId,
        positionMs: 0,
        durationMs: 0,
        error: null,
      }
      await playerService.play(trackId, context)
      applySnapshot(await playerService.snapshot())
    })
  }

  /**
   * 从列表上下文播放（TrackList 双击时调用）。
   * startIndex 由前端传入，保证"下一首"语义与用户所见列表一致。
   */
  async function playFromContext(trackId: string, context: PlayRequestContext): Promise<void> {
    await play(trackId, context)
  }

  async function pause(): Promise<void> {
    await execute(async () => {
      await playerService.pause()
      applySnapshot(await playerService.snapshot())
    })
  }

  async function resume(): Promise<void> {
    await execute(async () => {
      await playerService.resume()
      applySnapshot(await playerService.snapshot())
    })
  }

  async function stop(): Promise<void> {
    await execute(async () => {
      await playerService.stop()
      applySnapshot(await playerService.snapshot())
    })
  }

  /** 播放/暂停切换（快捷键、播放条按钮共用入口） */
  async function toggle(): Promise<void> {
    if (snapshot.value.status === 'playing') {
      await pause()
      return
    }
    await resume()
  }

  async function next(): Promise<void> {
    await execute(async () => {
      await playerService.next()
      applySnapshot(await playerService.snapshot())
    })
  }

  async function previous(): Promise<void> {
    await execute(async () => {
      await playerService.previous()
      applySnapshot(await playerService.snapshot())
    })
  }

  async function seek(positionMs: number): Promise<void> {
    await execute(async () => {
      await playerService.seek(Math.max(0, Math.round(positionMs)))
      applySnapshot(await playerService.snapshot())
    })
  }

  async function setVolume(volume: number): Promise<void> {
    await execute(async () => {
      await playerService.setVolume(clamp01(volume))
      applySnapshot(await playerService.snapshot())
    })
  }

  async function setMuted(muted: boolean): Promise<void> {
    await execute(async () => {
      await playerService.setMuted(muted)
      applySnapshot(await playerService.snapshot())
    })
  }

  async function setMode(mode: PlayMode): Promise<void> {
    await execute(async () => {
      await playerService.setMode(mode)
      applySnapshot(await playerService.snapshot())
    })
  }

  /** 播放模式轮换：顺序 → 列表循环 → 单曲循环 → 随机（快捷键 R 使用） */
  const MODE_ORDER: PlayMode[] = ['order', 'repeat-all', 'repeat-one', 'shuffle']
  async function cycleMode(): Promise<void> {
    const current = MODE_ORDER.indexOf(snapshot.value.mode)
    const nextMode = MODE_ORDER[(current + 1) % MODE_ORDER.length] ?? 'order'
    await setMode(nextMode)
  }

  /** 退出前释放引擎资源与事件订阅 */
  async function dispose(): Promise<void> {
    await execute(async () => {
      await playerService.dispose()
      unsubscribe?.()
      unsubscribe = null
      initialized.value = false
    })
  }

  return {
    snapshot,
    initialized,
    currentContext,
    queueTrackIds,
    currentIndex,
    applySnapshot,
    applyEvent,
    init,
    play,
    playFromContext,
    pause,
    resume,
    stop,
    toggle,
    next,
    previous,
    seek,
    setVolume,
    setMuted,
    setMode,
    cycleMode,
    dispose,
  }
})
