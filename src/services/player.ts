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
  IPlayer,
  PlayContext,
  PlayMode,
  PlaybackEvent,
  PlaybackSnapshot,
  PlayerCapabilities,
} from '../types'

import { callCommand, isTauriRuntime } from './ipc'
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
    mode: 'order',
    error: null,
  }
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
  init: () => callCommand<PlayContext | null>('player_init'),
  capabilities: () => callCommand<PlayerCapabilities>('player_capabilities'),
  snapshot: () => callCommand<PlaybackSnapshot>('player_snapshot'),
  play: (trackId: string, context?: PlayContext) =>
    callCommand('player_play', { trackId, context }),
  pause: () => callCommand('player_pause'),
  resume: () => callCommand('player_resume'),
  stop: () => callCommand('player_stop'),
  next: () => callCommand('player_next'),
  previous: () => callCommand('player_previous'),
  seek: (positionMs: number) => callCommand('player_seek', { positionMs }),
  setVolume: (volume: number) => callCommand('player_set_volume', { volume }),
  setMuted: (muted: boolean) => callCommand('player_set_muted', { muted }),
  setMode: (mode: PlayMode) => callCommand('player_set_mode', { mode }),
  dispose: () => callCommand('player_dispose'),
}

/** 单例：Web 引擎在应用生命周期内复用同一个时钟与监听器集合 */
const webPlayer = new WebPlayer()

export const playerService: IPlayer = isTauriRuntime() ? tauriPlayer : webPlayer
