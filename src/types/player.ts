/**
 * 播放器领域模型（前端）
 *
 * 设计要点：前端只依赖下面这组稳定接口，不感知底层是 rodio 还是外部进程；
 * 引擎差异全部由 Rust 侧 `PlayerEngine` trait 吸收（见 ARCHITECTURE.md）。
 */

/** 播放状态机状态 */
export type PlaybackStatus = 'idle' | 'loading' | 'playing' | 'paused' | 'stopped' | 'error'

/** 播放模式 */
export type PlayMode = 'order' | 'repeat-all' | 'repeat-one' | 'shuffle'

/** 播放器能力声明：UI 据此决定控件启用态，避免对引擎能力做硬编码假设 */
export interface PlayerCapabilities {
  /** 是否支持精确 seek */
  seek: boolean
  /** 是否支持软件音量（否则只能调用系统音量） */
  volumeControl: boolean
  /** 是否支持变速播放 */
  rateControl: boolean
  /** 支持的无缝解码格式（小写扩展名） */
  supportedFormats: string[]
}

/** 播放快照（后端为唯一真相源，前端只读） */
export interface PlaybackSnapshot {
  status: PlaybackStatus
  /** 当前曲目 ID */
  trackId: string | null
  /** 当前播放位置（毫秒，整数，避免浮点抖动） */
  positionMs: number
  /** 总时长（毫秒，未知为 0） */
  durationMs: number
  /** 音量 0.0 ~ 1.0 */
  volume: number
  muted: boolean
  mode: PlayMode
  /** 播放错误信息 */
  error: string | null
}

/** 播放上下文：决定"下一首"的语义 */
export interface PlayContext {
  /** 上下文类型：曲库 / 专辑 / 歌单 / 搜索结果 */
  source: 'library' | 'album' | 'artist' | 'playlist' | 'search'
  /** 上下文标识（专辑 ID / 歌单 ID 等，library 与 search 为 null） */
  sourceId: string | null
  /** 上下文曲目 ID 有序列表 */
  trackIds: string[]
}

/**
 * 播放器接口（前端侧契约）
 * 所有实现（Tauri 命令桥接、未来的 WebAudio 降级实现）都必须满足该接口。
 */
export interface IPlayer {
  /**
   * 初始化播放器。Tauri 实现返回上次会话恢复出的播放上下文（断点续播），
   * 无恢复内容时为 null；恢复后的完整快照由随后的 snapshot() 读取。
   */
  init(): Promise<PlayContext | null>
  capabilities(): Promise<PlayerCapabilities>
  snapshot(): Promise<PlaybackSnapshot>
  /**
   * 订阅后端推送的播放事件（进度 / 曲目切换 / 结束等）。
   * 返回取消订阅函数；Tauri 实现桥接 playback://event，Web 实现由本地引擎推送。
   */
  subscribe(listener: (event: PlaybackEvent) => void): () => void
  play(trackId: string, context?: PlayContext): Promise<void>
  pause(): Promise<void>
  resume(): Promise<void>
  stop(): Promise<void>
  next(): Promise<void>
  previous(): Promise<void>
  seek(positionMs: number): Promise<void>
  setVolume(volume: number): Promise<void>
  setMuted(muted: boolean): Promise<void>
  setMode(mode: PlayMode): Promise<void>
  dispose(): Promise<void>
}

/** 播放状态事件（后端 -> 前端推送） */
export type PlaybackEvent =
  | { type: 'status'; payload: { status: PlaybackStatus } }
  | { type: 'track'; payload: { trackId: string; durationMs: number } }
  | { type: 'progress'; payload: { positionMs: number } }
  | { type: 'volume'; payload: { volume: number; muted: boolean } }
  | { type: 'mode'; payload: { mode: PlayMode } }
  | { type: 'ended'; payload: { trackId: string } }
  | { type: 'error'; payload: { message: string; trackId: string | null } }
