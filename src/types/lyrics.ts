/**
 * 歌词领域模型（前端）
 */

/** 歌词行（已解析时间轴） */
export interface LyricLine {
  /** 起始时间（毫秒） */
  timeMs: number
  /** 该行文本 */
  text: string
  /** 是否为翻译行 */
  translation: string | null
}

/** 歌词来源 */
export type LyricsSource = 'embedded' | 'sidecar' | 'online' | 'manual' | 'none'

/** 歌词文档 */
export interface Lyrics {
  trackId: string
  source: LyricsSource
  /** 是否含时间轴（false 表示纯文本歌词） */
  synced: boolean
  lines: LyricLine[]
  /** 歌词偏移校正（毫秒，正数表示整体延后） */
  offsetMs: number
}

/** 歌词加载状态 */
export type LyricsStatus = 'idle' | 'loading' | 'ready' | 'empty' | 'error'
