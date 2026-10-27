/**
 * 歌词服务：运行时实现选择
 *
 * 约定：
 *  - 歌词来源优先级由后端决定：内嵌 > 同名 .lrc 边车文件 > 在线（在线来源需在线模块已开启）；
 *  - 前端只负责展示与偏移校正，不解析 LRC 文本；
 *  - Tauri 环境走命令（M3 接通），浏览器环境使用 services/web 的本地预览数据。
 */
import type { Lyrics, SidecarMeta } from '../types'

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

/**
 * 读取同名 .lrc 边车里的元数据标签（[ar:]/[al:]/[ti:]/[au:]/[by:]）。
 * 用于「歌词元数据补全」实验室功能；浏览器预览无 Tauri 运行时返回 null，调用失败也返回 null。
 */
export async function getSidecarMetadata(trackId: string): Promise<SidecarMeta | null> {
  if (!isTauriRuntime()) return null
  try {
    return await callCommand<SidecarMeta | null>('lyrics_sidecar_metadata', { trackId })
  } catch {
    return null
  }
}
