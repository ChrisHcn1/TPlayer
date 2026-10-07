import { invoke } from '@tauri-apps/api/core'
import type { Ref } from 'vue'

type LogFn = (...args: any[]) => void

interface UseWindowControlsOptions {
  // 浏览器环境下不调用 Tauri 窗口命令
  isBrowser: Ref<boolean>
  logError: LogFn
}

// 自定义标题栏的窗口控制：最小化、最大化/还原切换、关闭（隐藏到托盘）。
// 关闭按钮不退出应用，而是 hide() 到系统托盘——这是既有产品行为，保持不变。
export function useWindowControls(options: UseWindowControlsOptions) {
  const { isBrowser, logError } = options

  const minimizeWindow = async () => {
    try {
      if (!isBrowser.value) {
        await invoke('minimize_window')
      }
    } catch (error) {
      logError('最小化窗口失败:', error)
    }
  }

  const toggleMaximizeWindow = async () => {
    try {
      if (!isBrowser.value) {
        await invoke('toggle_maximize_window')
      }
    } catch (error) {
      logError('切换最大化状态失败:', error)
    }
  }

  const closeWindow = async () => {
    try {
      // 隐藏窗口到托盘，而不是退出应用
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      const currentWindow = getCurrentWindow()
      await currentWindow.hide()
    } catch (error) {
      logError('隐藏窗口失败:', error)
    }
  }

  return {
    minimizeWindow,
    toggleMaximizeWindow,
    closeWindow
  }
}
