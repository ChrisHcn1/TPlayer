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

/** 保存设置：Tauri 走命令，浏览器走 localStorage */
export async function saveSettings(settings: PersistedSettings): Promise<void> {
  if (isTauriRuntime()) {
    try {
      await callCommand('settings_save', { config: settings })
    } catch {
      /* 持久化失败不影响当前会话 */
    }
    return
  }
  try {
    localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings))
  } catch {
    /* 忽略 */
  }
}
