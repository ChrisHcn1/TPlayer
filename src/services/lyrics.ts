/**
 * 歌词服务：运行时实现选择
 *
 * 约定：
 *  - 歌词来源优先级由后端决定：内嵌 > 同名 .lrc 边车文件 > 在线（在线来源需在线模块已开启）；
 *  - 前端只负责展示与偏移校正，不解析 LRC 文本；
 *  - Tauri 环境走命令（M3 接通），浏览器环境使用 services/web 的本地预览数据。
 */
import type { Lyrics } from '../types'

import { callCommand, isTauriRuntime } from './ipc'
import { webLyrics } from './web/lyrics'

export interface LyricsService {
  load(trackId: string): Promise<Lyrics | null>
  saveOffset(trackId: string, offsetMs: number): Promise<void>
  clearCache(): Promise<void>
}

const tauriLyrics: LyricsService = {
  load: (trackId) => callCommand<Lyrics | null>('lyrics_load', { trackId }),
  saveOffset: (trackId, offsetMs) =>
    callCommand('lyrics_save_offset', { trackId, offsetMs }),
  clearCache: () => callCommand('lyrics_clear_cache'),
}

export const lyricsService: LyricsService = isTauriRuntime() ? tauriLyrics : webLyrics
