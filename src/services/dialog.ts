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
