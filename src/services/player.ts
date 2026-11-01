/**
 * 播放服务：`IPlayer` 的运行时实现选择
 *
 *  - Tauri 环境：每个方法映射 src-tauri/src/commands/player.rs 的命令，
 *    subscribe 桥接后端 playback://event 推送；
 *  - 浏览器环境：使用 services/web 的本地逻辑引擎（不发声，但完整驱动播放状态）。
 *
 * 前端只依赖 types/player.ts 的 IPlayer 契约，不感知底层引擎差异。
 */
import { listen } from '@tauri-apps/api/event'

import type {
  EqBand,
  IPlayer,
  PlayContext,
  PlayMode,
  PlaybackEvent,
  PlaybackSnapshot,
  PlayerCapabilities,
} from '../types'

import { LONG_RUNNING_TIMEOUT_MS, callCommand, isTauriRuntime } from './ipc'
import { WebPlayer } from './web/player'

/** 前端默认（未初始化）播放快照：后端快照到达前，UI 的稳定初值 */
export function createIdleSnapshot(): PlaybackSnapshot {
  return {
    status: 'idle',
    trackId: null,
    positionMs: 0,
    durationMs: 0,
    volume: 1,
    muted: false,
    rate: 1,
    mode: 'order',
    error: null,
    errorCode: null,
  }
}

/**
 * 在途快照请求（单飞）。
 *
 * 后端 `player` 是单一互斥体，切歌 / 连点 / 事件回拉叠加时会产生多个并发的
 * `player_snapshot`，全部排在播放命令后面争锁，反过来拉长"点下一首"的感知延迟。
 * 这里把并发拉取合并成一次；结果返回后自动清空，不会读到过期值。
 */
let snapshotInFlight: Promise<PlaybackSnapshot> | null = null

function requestSnapshot(): Promise<PlaybackSnapshot> {
  if (snapshotInFlight) return snapshotInFlight
  const pending = callCommand<PlaybackSnapshot>('player_snapshot')
  snapshotInFlight = pending
  const clear = (): void => {
    if (snapshotInFlight === pending) snapshotInFlight = null
  }
  pending.then(clear, clear)
  return pending
}

/** Tauri 实现：经 callCommand 调用 Rust 命令，subscribe 桥接 playback://event */
const tauriPlayer: IPlayer = {
  subscribe(listener: (event: PlaybackEvent) => void): () => void {
    let unlisten: (() => void) | null = null
    // listen 返回 Promise<UnlistenFn>；同步返回的清理函数在 Promise resolve 后生效
    void listen<PlaybackEvent>('playback://event', (e) => listener(e.payload)).then(
      (un) => {
        unlisten = un
      },
    )
    return () => {
      unlisten?.()
    }
  },
  init: () => callCommand<PlayContext | null>('player_init', undefined, { timeoutMs: LONG_RUNNING_TIMEOUT_MS }),
  capabilities: () => callCommand<PlayerCapabilities>('player_capabilities'),
  // 快照单飞：切歌、连点、事件回拉可能在同一瞬间发起多次拉取，
  // 合并为一次在途请求可显著减少对后端 player 锁的争用。
  snapshot: () => requestSnapshot(),
  play: (trackId: string, context?: PlayContext) =>
    callCommand('player_play', { trackId, context }, { timeoutMs: LONG_RUNNING_TIMEOUT_MS }),
  pause: () => callCommand('player_pause'),
  resume: () => callCommand('player_resume', undefined, { timeoutMs: LONG_RUNNING_TIMEOUT_MS }),
  stop: () => callCommand('player_stop'),
  next: () => callCommand('player_next', undefined, { timeoutMs: LONG_RUNNING_TIMEOUT_MS }),
  previous: () => callCommand('player_previous', undefined, { timeoutMs: LONG_RUNNING_TIMEOUT_MS }),
  seek: (positionMs: number) => callCommand('player_seek', { positionMs }),
  setVolume: (volume: number) => callCommand('player_set_volume', { volume }),
  setMuted: (muted: boolean) => callCommand('player_set_muted', { muted }),
  setMode: (mode: PlayMode) => callCommand('player_set_mode', { mode }),
  setRate: (rate: number) => callCommand('player_set_rate', { rate }),
  setCrossfade: (enabled: boolean, durationMs: number) =>
    callCommand('player_set_crossfade', { enabled, durationMs }),
  setReplaygain: (enabled: boolean) =>
    callCommand('player_set_replaygain', { enabled }),
  setEq: (enabled: boolean, bands: EqBand[]) =>
    callCommand('player_set_eq', { enabled, bands }),
  setSleepTimer: (delayMs: number, waitTrackEnd: boolean) =>
    callCommand('player_set_sleep_timer', { delayMs, waitTrackEnd }),
  cancelSleepTimer: () => callCommand('player_cancel_sleep_timer'),
  sleepTimerStatus: () => callCommand('player_sleep_timer_status'),
  removeFromQueue: (index: number) => callCommand('player_remove_from_queue', { index }),
  dispose: () => callCommand('player_dispose'),
}

/** 单例：Web 引擎在应用生命周期内复用同一个时钟与监听器集合 */
const webPlayer = new WebPlayer()

/**
 * 零侵入接入音频格式兼容补丁：
 * 把原生 IPlayer 包进 CompatPlayer —— 正常文件走原生（零开销），
 * 仅当原生 play 失败时才在 Web Worker 内魔数识别 + 分级解码 + 兜底播放。
 * 不修改 tauriPlayer / webPlayer / WebPlayer / Rust 引擎的任何代码。
 */
import { createCompatPlayer } from './compat'

const basePlayer: IPlayer = isTauriRuntime() ? tauriPlayer : webPlayer

export const playerService: IPlayer = createCompatPlayer(basePlayer)
