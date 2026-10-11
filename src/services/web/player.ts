/**
 * Web 预览播放引擎 —— services/player 在非 Tauri 环境下的 IPlayer 实现
 *
 * 定位：M1 阶段 Rust 统一播放引擎（M2）尚未落地，浏览器预览需要一条
 * “能驱动全部 UI 状态”的播放链路。本引擎以逻辑时钟推进播放位置，
 * 完整实现队列上下文、四种播放模式与自动连播，并通过 subscribe 推送
 * 与未来 Tauri 事件同构的 PlaybackEvent —— 不输出声音（无真实音频文件），
 * 进度 / 切歌 / 歌词滚动 / 队列等交互均可真实联调。
 *
 * M2 接通 Rust 后仅替换 services/player 的实现选择，store / 组件零改动。
 */
import type {
  IPlayer,
  PlayContext,
  PlayMode,
  PlaybackEvent,
  PlaybackSnapshot,
  PlayerCapabilities,
  SleepTimerState,
} from '../../types'
import { createIdleSnapshot } from '../player'
import { getTrackById } from './dataset'

const TICK_MS = 250

const CAPABILITIES: PlayerCapabilities = {
  seek: true,
  volumeControl: true,
  rateControl: false,
  supportedFormats: ['mp3', 'flac', 'wav', 'ogg', 'aac', 'm4a'],
}

function durationMsOf(trackId: string | null): number {
  if (!trackId) return 0
  return (getTrackById(trackId)?.duration ?? 0) * 1000
}

export class WebPlayer implements IPlayer {
  private snapshotState: PlaybackSnapshot = createIdleSnapshot()
  private context: PlayContext | null = null
  private index = -1
  private timer: ReturnType<typeof setInterval> | null = null
  private listeners = new Set<(event: PlaybackEvent) => void>()
  /** 睡眠定时器：到期时刻（epoch 毫秒）+ 是否播完当前曲；fired 表示已到期等待曲终 */
  private sleepTimer: { endsAtMs: number; waitTrackEnd: boolean; fired: boolean } | null = null
  private sleepTimerHandle: ReturnType<typeof setTimeout> | null = null

  subscribe(listener: (event: PlaybackEvent) => void): () => void {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  private emit(event: PlaybackEvent): void {
    this.listeners.forEach((listener) => listener(event))
  }

  async init(): Promise<PlayContext | null> {
    /* Web 引擎无异步资源需要初始化，也无断点恢复 */
    return null
  }

  async capabilities(): Promise<PlayerCapabilities> {
    return CAPABILITIES
  }

  async snapshot(): Promise<PlaybackSnapshot> {
    return { ...this.snapshotState }
  }

  async play(trackId: string, context?: PlayContext): Promise<void> {
    if (context) this.context = context
    if (this.context) {
      const idx = this.context.trackIds.indexOf(trackId)
      if (idx >= 0) this.index = idx
    }
    this.loadTrack(trackId, true)
  }

  async pause(): Promise<void> {
    if (this.snapshotState.status !== 'playing') return
    this.stopClock()
    this.patch({ status: 'paused' })
    this.emit({ type: 'status', payload: { status: 'paused' } })
  }

  async resume(): Promise<void> {
    if (!this.snapshotState.trackId || this.snapshotState.status === 'playing') return
    this.patch({ status: 'playing' })
    this.emit({ type: 'status', payload: { status: 'playing' } })
    this.startClock()
  }

  async stop(): Promise<void> {
    this.stopClock()
    this.patch({ status: 'stopped', positionMs: 0 })
    this.emit({ type: 'status', payload: { status: 'stopped' } })
  }

  async next(): Promise<void> {
    this.step(1)
  }

  async previous(): Promise<void> {
    // 播放超过 3 秒时先回到本曲开头（桌面播放器通行语义），否则才切上一首
    if (this.snapshotState.positionMs > 3000) {
      this.seek(0)
      return
    }
    this.step(-1)
  }

  async seek(positionMs: number): Promise<void> {
    const position = Math.max(0, Math.min(positionMs, this.snapshotState.durationMs))
    this.patch({ positionMs: position })
    this.emit({ type: 'progress', payload: { positionMs: position } })
  }

  async setVolume(volume: number): Promise<void> {
    const next = Math.min(1, Math.max(0, volume))
    this.patch({ volume: next, muted: next === 0 ? false : this.snapshotState.muted })
    this.emit({
      type: 'volume',
      payload: { volume: this.snapshotState.volume, muted: this.snapshotState.muted },
    })
  }

  async setMuted(muted: boolean): Promise<void> {
    this.patch({ muted })
    this.emit({ type: 'volume', payload: { volume: this.snapshotState.volume, muted } })
  }

  async setMode(mode: PlayMode): Promise<void> {
    this.patch({ mode })
    this.emit({ type: 'mode', payload: { mode } })
  }

  async setCrossfade(_enabled: boolean, _durationMs: number): Promise<void> {
    /* 浏览器 mock 引擎无真实音频输出，交叉淡化参数不生效，忽略即可 */
  }

  async setReplaygain(_enabled: boolean): Promise<void> {
    /* 浏览器 mock 引擎无真实音频输出，ReplayGain 不生效，忽略即可 */
  }

  async setSleepTimer(delayMs: number, waitTrackEnd: boolean): Promise<void> {
    this.clearSleepTimerHandle()
    this.sleepTimer = {
      endsAtMs: Date.now() + Math.max(delayMs, 1000),
      waitTrackEnd,
      fired: waitTrackEnd && delayMs === 0,
    }
    if (!this.sleepTimer.fired) {
      const remaining = this.sleepTimer.endsAtMs - Date.now()
      this.sleepTimerHandle = setTimeout(() => {
        if (!this.sleepTimer) return
        if (this.sleepTimer.waitTrackEnd) {
          this.sleepTimer.fired = true
        } else {
          this.sleepTimer = null
          void this.pause()
          this.emitSleepTimer()
        }
      }, remaining)
    }
    this.emitSleepTimer()
  }

  async cancelSleepTimer(): Promise<void> {
    if (!this.sleepTimer) return
    this.clearSleepTimerHandle()
    this.sleepTimer = null
    this.emitSleepTimer()
  }

  async sleepTimerStatus(): Promise<SleepTimerState> {
    if (!this.sleepTimer) return { active: false, waitTrackEnd: false, endsAtMs: null }
    return {
      active: true,
      waitTrackEnd: this.sleepTimer.waitTrackEnd,
      endsAtMs: this.sleepTimer.endsAtMs,
    }
  }

  private clearSleepTimerHandle(): void {
    if (this.sleepTimerHandle !== null) {
      clearTimeout(this.sleepTimerHandle)
      this.sleepTimerHandle = null
    }
  }

  async removeFromQueue(index: number): Promise<void> {
    if (!this.context) throw new Error('没有可操作的播放队列')
    const ids = this.context.trackIds
    if (index < 0 || index >= ids.length) throw new Error(`队列下标越界: ${index}`)
    ids.splice(index, 1)
    // 修正当前曲目下标：移除当前曲之前的项则前移；移除当前曲本身不打断播放
    if (index < this.index && this.index > 0) this.index -= 1
    if (this.index >= ids.length) this.index = ids.length - 1
    this.emit({ type: 'queue', payload: { ...this.context, trackIds: [...ids] } })
  }

  private emitSleepTimer(): void {
    void this.sleepTimerStatus().then((state) => {
      this.emit({ type: 'sleepTimer', payload: state })
    })
  }

  async dispose(): Promise<void> {
    this.stopClock()
    this.clearSleepTimerHandle()
    this.sleepTimer = null
    this.listeners.clear()
  }

  /* ---- 内部实现 ---- */

  private patch(partial: Partial<PlaybackSnapshot>): void {
    this.snapshotState = { ...this.snapshotState, ...partial }
  }

  private loadTrack(trackId: string, playing: boolean): void {
    const durationMs = durationMsOf(trackId)
    this.patch({
      status: playing ? 'playing' : 'paused',
      trackId,
      positionMs: 0,
      durationMs,
      error: null,
    })
    this.emit({ type: 'track', payload: { trackId, durationMs } })
    this.emit({ type: 'status', payload: { status: this.snapshotState.status } })
    if (playing) this.startClock()
  }

  private startClock(): void {
    this.stopClock()
    this.timer = setInterval(() => this.tick(), TICK_MS)
  }

  private stopClock(): void {
    if (this.timer !== null) {
      clearInterval(this.timer)
      this.timer = null
    }
  }

  private tick(): void {
    if (this.snapshotState.status !== 'playing' || !this.snapshotState.trackId) return
    const position = this.snapshotState.positionMs + TICK_MS
    if (position < this.snapshotState.durationMs) {
      this.patch({ positionMs: position })
      this.emit({ type: 'progress', payload: { positionMs: position } })
      return
    }
    this.handleEnded()
  }

  /** 一曲播完：按播放模式决定单曲重播 / 连播 / 结束 */
  private handleEnded(): void {
    // 睡眠定时器"播完当前曲目"已到期：停止而不连播
    if (this.sleepTimer?.fired && this.sleepTimer.waitTrackEnd) {
      this.sleepTimer = null
      this.stopClock()
      this.patch({ status: 'stopped', positionMs: 0 })
      this.emit({ type: 'status', payload: { status: 'stopped' } })
      this.emitSleepTimer()
      return
    }

    const mode = this.snapshotState.mode
    const ids = this.context?.trackIds ?? []
    const trackId = this.snapshotState.trackId

    if (mode === 'repeat-one' && trackId) {
      this.loadTrack(trackId, true)
      return
    }

    if (mode === 'shuffle' && ids.length > 1) {
      let nextIndex = this.index
      while (nextIndex === this.index) nextIndex = Math.floor(Math.random() * ids.length)
      this.index = nextIndex
      this.loadTrack(ids[nextIndex], true)
      return
    }

    const nextIndex = this.index + 1
    if (nextIndex < ids.length) {
      this.index = nextIndex
      this.loadTrack(ids[nextIndex], true)
      return
    }

    // 列表循环回到开头；顺序播放到此结束
    if (mode === 'repeat-all' && ids.length > 0) {
      this.index = 0
      this.loadTrack(ids[0], true)
      return
    }

    this.stopClock()
    this.patch({ status: 'stopped', positionMs: 0 })
    if (trackId) this.emit({ type: 'ended', payload: { trackId } })
    this.emit({ type: 'status', payload: { status: 'stopped' } })
  }

  /** 手动上一首 / 下一首（单曲循环下手动切歌仍移动到相邻曲） */
  private step(direction: 1 | -1): void {
    const ids = this.context?.trackIds ?? []
    if (ids.length === 0 || !this.snapshotState.trackId) return

    const wrap = this.snapshotState.mode === 'repeat-all'
    let nextIndex = this.index + direction
    if (nextIndex < 0 || nextIndex >= ids.length) {
      if (!wrap) return
      nextIndex = (nextIndex + ids.length) % ids.length
    }
    this.index = nextIndex
    this.loadTrack(ids[nextIndex], this.snapshotState.status === 'playing')
  }
}
