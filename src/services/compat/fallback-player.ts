/**
 * 兜底播放通道 —— 用隐藏的 <audio> 元素播放解码后的标准 WAV blob。
 *
 * 定位：仅当原生播放（Rust 引擎 / Web 引擎）失败时由 CompatPlayer 启用，
 * 对外通过 onEvent 回调发出与 IPlayer.subscribe 同构的 PlaybackEvent，使 UI 无感切换。
 * 在 Tauri WebView 中，<audio> 经由系统 WebView 输出声音，与 Rust 音频通道相互独立，
 * 且兜底场景中原生通道已停止，不存在双声道冲突。
 */
import type { PlaybackEvent, PlaybackStatus } from '../../types'

export class FallbackPlayer {
  private audio: HTMLAudioElement | null = null
  private objectUrl: string | null = null
  private currentTrackId: string | null = null
  private durationMs = 0
  private positionMs = 0
  private status: PlaybackStatus = 'idle'
  private volume = 1
  private muted = false
  /** 播放倍速（1.0 = 原速） */
  private rate = 1

  constructor(private readonly onEvent: (e: PlaybackEvent) => void) {}

  get trackId(): string | null {
    return this.currentTrackId
  }

  get isActive(): boolean {
    return this.audio !== null && this.currentTrackId !== null
  }

  get snapshotFields(): {
    trackId: string | null
    positionMs: number
    durationMs: number
    status: PlaybackStatus
    volume: number
    muted: boolean
  } {
    return {
      trackId: this.currentTrackId,
      positionMs: this.positionMs,
      durationMs: this.durationMs,
      status: this.status,
      volume: this.volume,
      muted: this.muted,
    }
  }

  play(trackId: string, blob: Blob, durationMs: number): void {
    this.stopInternal()
    this.currentTrackId = trackId
    this.durationMs = durationMs
    this.status = 'loading'
    this.objectUrl = URL.createObjectURL(blob)
    const audio = new Audio()
    this.audio = audio
    audio.src = this.objectUrl
    audio.volume = this.muted ? 0 : this.volume
    audio.playbackRate = this.rate

    // 守卫：仅当该元素仍是当前播放元素时才向上发事件。
    // stop/切换曲目会对旧元素 pause + 清 src，可能异步触发 error/pause 等事件，
    // 不加守卫会误报「兜底播放通道异常」并污染新曲目状态。
    const isCurrent = () => this.audio === audio
    audio.addEventListener('loadedmetadata', () => {
      if (!isCurrent()) return
      const realDurationMs = Math.round(audio.duration * 1000)
      if (Number.isFinite(audio.duration) && realDurationMs > 0) {
        // HTMLAudioElement 是最终真相源：如果它拿到了有效时长，
        // 用它覆盖 Worker 计算值（Worker 在某些格式上会算成 0）
        if (realDurationMs !== this.durationMs) {
          this.durationMs = realDurationMs
          this.emit({ type: 'track', payload: { trackId, durationMs: realDurationMs } })
        }
      }
    })
    audio.addEventListener('playing', () => {
      if (!isCurrent()) return
      this.status = 'playing'
      this.emit({ type: 'status', payload: { status: 'playing' } })
    })
    audio.addEventListener('pause', () => {
      if (!isCurrent()) return
      this.status = 'paused'
      this.emit({ type: 'status', payload: { status: 'paused' } })
    })
    audio.addEventListener('timeupdate', () => {
      if (!isCurrent()) return
      this.positionMs = Math.round(audio.currentTime * 1000)
      this.emit({ type: 'progress', payload: { positionMs: this.positionMs } })
    })
    audio.addEventListener('ended', () => {
      if (!isCurrent()) return
      this.status = 'stopped'
      this.emit({ type: 'status', payload: { status: 'stopped' } })
      this.emit({ type: 'ended', payload: { trackId } })
    })
    audio.addEventListener('error', () => {
      if (!isCurrent()) return
      this.status = 'error'
      this.emit({ type: 'error', payload: { message: '兜底播放通道异常', trackId } })
    })

    this.emit({ type: 'track', payload: { trackId, durationMs } })
    void audio.play().catch(() => {
      this.status = 'error'
      this.emit({ type: 'error', payload: { message: '兜底播放启动失败', trackId } })
    })
  }

  pause(): void {
    this.audio?.pause()
  }

  resume(): void {
    void this.audio?.play().catch(() => {})
  }

  seek(positionMs: number): void {
    if (this.audio) {
      this.audio.currentTime = Math.max(0, positionMs) / 1000
      this.positionMs = Math.max(0, positionMs)
      this.emit({ type: 'progress', payload: { positionMs: this.positionMs } })
    }
  }

  setVolume(volume: number): void {
    this.volume = Math.min(1, Math.max(0, volume))
    if (this.audio && !this.muted) this.audio.volume = this.volume
  }

  setMuted(muted: boolean): void {
    this.muted = muted
    if (this.audio) this.audio.volume = muted ? 0 : this.volume
  }

  setRate(rate: number): void {
    this.rate = Math.max(0.25, Math.min(4.0, rate))
    if (this.audio) this.audio.playbackRate = this.rate
  }

  stop(): void {
    this.stopInternal()
    this.status = 'stopped'
    this.currentTrackId = null
  }

  private stopInternal(): void {
    if (this.audio) {
      this.audio.pause()
      this.audio.src = ''
      this.audio = null
    }
    if (this.objectUrl) {
      URL.revokeObjectURL(this.objectUrl)
      this.objectUrl = null
    }
  }

  private emit(e: PlaybackEvent): void {
    this.onEvent(e)
  }
}
