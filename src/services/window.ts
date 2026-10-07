/**
 * 窗口控制服务（最小化 / 最大化切换 / 关闭）
 *
 * 为什么单独成服务：组件不得直接 import @tauri-apps/api（见 services/index.ts 约定），
 * 且必须能在浏览器（npm run dev）中安全降级——非 Tauri 环境为 no-op，不抛错。
 *
 * 运行前提（属 src-tauri 侧配置）：
 *  1) capabilities/default.json 增加 core:window:allow-minimize / allow-toggle-maximize /
 *     allow-close / allow-start-dragging（core:default 不包含这四个写权限）；
 *  2) tauri.conf.json 的 windows[0].decorations 为 false，自绘标题栏取代系统标题栏（§4.3）。
 * 在配置补齐前调用会被 Tauri 拒绝；本服务捕获后只记录警告，不向 UI 抛出
 * （避免产生未捕获的 Promise 拒绝）。
 */
import { isTauriRuntime } from './ipc'

type WindowAction = 'minimize' | 'toggleMaximize' | 'close' | 'startDragging'

async function invokeWindowAction(action: WindowAction): Promise<void> {
  if (!isTauriRuntime()) return

  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    const current = getCurrentWindow()
    if (action === 'minimize') {
      await current.minimize()
    } else if (action === 'toggleMaximize') {
      await current.toggleMaximize()
    } else if (action === 'startDragging') {
      await current.startDragging()
    } else {
      await current.close()
    }
  } catch (error) {
    console.warn(`[window] ${action} 调用失败（请核对 src-tauri 窗口权限与 decorations 配置）`, error)
  }
}

export const windowService = {
  minimize: (): Promise<void> => invokeWindowAction('minimize'),
  toggleMaximize: (): Promise<void> => invokeWindowAction('toggleMaximize'),
  close: (): Promise<void> => invokeWindowAction('close'),
  /** 无边框窗口拖拽移动（WebView2 不识别 CSS app-region，必须用 Tauri API） */
  startDragging: (): Promise<void> => invokeWindowAction('startDragging'),
}
