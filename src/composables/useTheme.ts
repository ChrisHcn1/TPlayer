/**
 * useTheme —— 主题切换组合式函数
 * 约定：主题唯一载体是 <html data-theme>，唯一持久化键是 THEME_STORAGE_KEY；
 * 这里不引入第二套 class 机制（旧版同时用 class + 内联样式，已废弃）。
 */
import { readonly, ref } from 'vue'

import { DEFAULT_THEME, THEME_STORAGE_KEY } from '../constants'
import type { AppError } from '../types'

export type ThemeName = 'dark' | 'light'

export function useTheme() {
  const theme = ref<ThemeName>(readTheme())

  function readTheme(): ThemeName {
    try {
      const stored = localStorage.getItem(THEME_STORAGE_KEY)
      return stored === 'light' || stored === 'dark' ? stored : DEFAULT_THEME
    } catch {
      return DEFAULT_THEME
    }
  }

  /** 应用主题到 DOM 并持久化；index.html 首屏脚本读取同一键做预加载 */
  function applyTheme(next: ThemeName): void {
    theme.value = next
    document.documentElement.setAttribute('data-theme', next)
    try {
      localStorage.setItem(THEME_STORAGE_KEY, next)
    } catch {
      /* 持久化失败不影响当前会话的主题生效 */
    }
  }

  function toggleTheme(): void {
    applyTheme(theme.value === 'dark' ? 'light' : 'dark')
  }

  /** 系统主题跟随（M0 预留，M3 接入设置项） */
  function prefersDark(): boolean {
    return window.matchMedia('(prefers-color-scheme: dark)').matches
  }

  return {
    theme: readonly(theme),
    applyTheme,
    toggleTheme,
    prefersDark,
  }
}

/** 统一的错误提示占位：M0 阶段仅返回结构，不接入通知组件 */
export function toAppError(error: unknown): AppError {
  return {
    code: 'UNKNOWN',
    message: '操作失败',
    detail: error instanceof Error ? error.message : String(error),
  }
}
