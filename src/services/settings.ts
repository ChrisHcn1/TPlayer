/**
 * 设置持久化服务
 *
 *  - Tauri 环境：通过 settings_load / settings_save 命令读写 app 数据目录下的 config.json；
 *  - 浏览器环境：回退到 localStorage（SETTINGS_STORAGE_KEY）。
 *
 * store 不直接接触 localStorage 或 IPC，统一经本服务。
 */
import { SETTINGS_STORAGE_KEY } from '../constants'
import type { OnlineModuleConfig } from '../types'

import { callCommand, isTauriRuntime } from './ipc'

/** 持久化结构（与 Rust config::AppConfig 对齐，但不含前端不需要的字段） */
export interface PersistedSettings {
  locale: string
  libraryDirs: string[]
  online: OnlineModuleConfig
  scanOnStartup: boolean
  crossfade: { enabled: boolean; duration_ms: number }
  replaygain: { enabled: boolean; targetLufs: number }
  watch: { enabled: boolean }
}

/** 加载设置：Tauri 走命令，浏览器走 localStorage */
export async function loadSettings(): Promise<PersistedSettings | null> {
  if (isTauriRuntime()) {
    try {
      return await callCommand<PersistedSettings>('settings_load')
    } catch {
      return null
    }
  }
  try {
    const raw = localStorage.getItem(SETTINGS_STORAGE_KEY)
    return raw ? (JSON.parse(raw) as PersistedSettings) : null
  } catch {
    return null
  }
}

/**
 * 保存设置：Tauri 走命令，浏览器走 localStorage。
 *
 * 返回是否成功：Tauri 侧 `settings_save` 为整体替换语义，任一字段反序列化失败
 * 都会导致整份配置（含曲库白名单）无法落盘——历史 bug 正是 replaygain 缺
 * `targetLufs` 被静默吞掉，表现为"已添加目录却报未配置曲库目录"。
 * 这里显式返回结果并打印日志，方便定位；且**不抛出**，以免 fire-and-forget
 * 调用点（如 setLocale）产生未处理的 Promise 拒绝。
 */
export async function saveSettings(settings: PersistedSettings): Promise<boolean> {
  if (isTauriRuntime()) {
    try {
      await callCommand('settings_save', { config: settings })
      return true
    } catch (cause) {
      console.error('[settings] 保存配置失败（设置可能未持久化）：', cause)
      return false
    }
  }
  try {
    localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings))
    return true
  } catch (cause) {
    console.error('[settings] 写入 localStorage 失败：', cause)
    return false
  }
}
