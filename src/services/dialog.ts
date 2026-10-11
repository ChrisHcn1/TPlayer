/**
 * 系统对话框服务：仅做运行时选择，组件不得直接 import @tauri-apps/plugin-dialog（§7 约束 6）。
 * 浏览器环境没有原生目录选择器，返回 null，由调用方降级为文本输入。
 */
import { isTauriRuntime } from './ipc'

/** 选择一个目录；取消或非 Tauri 环境返回 null */
export async function pickDirectory(): Promise<string | null> {
  if (!isTauriRuntime()) return null
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const selected = await open({ directory: true, multiple: false })
    return typeof selected === 'string' ? selected : null
  } catch {
    return null
  }
}

/** 选择一个或多个音频文件（返回绝对路径）；取消或非 Tauri 环境返回 null */
export async function pickAudioFiles(): Promise<string[] | null> {
  if (!isTauriRuntime()) return null
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const selected = await open({
      multiple: true,
      filters: [
        {
          name: '音频文件',
          extensions: [
            'mp3', 'flac', 'wav', 'ogg', 'aac', 'm4a', 'opus', 'wma', 'aif', 'aiff',
            'ape', 'wv', 'tta', 'tak', 'dsf', 'dff', 'dts',
          ],
        },
      ],
    })
    if (selected === null) return null
    return Array.isArray(selected) ? selected : [selected]
  } catch {
    return null
  }
}
