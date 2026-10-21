/**
 * 运行时平台识别（Linux 桌面端命名隔离的基础设施）
 *
 * 背景：产品已统一更名为 TPlayer，但 Windows 线上版本的持久化标识
 * （localStorage / IndexedDB 键）仍是历史前缀 `tplayer-next.*`。直接全局
 * 改键名会让已上线 Windows 用户的主题、设置、播放统计静默丢失（读到空键
 * 后整体回落默认值）——这是不可接受的线上回归。
 *
 * 因此共享的前端常量按平台分支：
 *  - Linux 桌面端（Tauri WebView + Linux UA）：使用 `tplayer.*` 新键
 *    （与产品名一致，彻底移除 Next 字样）；
 *  - Windows 桌面端与浏览器预览：保持 `tplayer-next.*` 历史键，零影响。
 *
 * 判定方式：Tauri 各平台 WebView 的 UA 携带宿主系统信息
 * （Linux/WebKitGTK 形如 `... X11; Linux x86_64 ...`；Windows 形如
 * `... Windows NT 10.0; Win64; x64 ...`）。仅在确认运行于 Tauri WebView
 * 时才按平台分支，浏览器预览（含 Linux 上的浏览器预览）一律归为 web，
 * 保持历史键不变。不引入 tauri-plugin-os 依赖，避免为命名隔离新增插件面。
 */

export type AppPlatform = 'linux' | 'windows' | 'web'

/** 是否运行在 Tauri WebView 中（与 services/ipc.ts 同一判定，内联以避免低层工具模块反向依赖 IPC 层） */
function isTauriWebView(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

/** 当前平台：桌面端按 WebView UA 判定；浏览器预览归为 web */
export function currentPlatform(): AppPlatform {
  if (typeof navigator === 'undefined') return 'web'
  if (!isTauriWebView()) return 'web'
  const ua = navigator.userAgent
  if (/X11|Linux/i.test(ua)) return 'linux'
  if (/Windows/i.test(ua)) return 'windows'
  return 'web'
}

/** 是否 Linux 桌面端（决定存储键等用户可见标识的命名分支） */
export function isLinuxDesktop(): boolean {
  return currentPlatform() === 'linux'
}

/**
 * 存储键前缀：Linux 桌面端为 `tplayer`（更名 TPlayer），
 * Windows 桌面端与浏览器预览保持历史值 `tplayer-next`（线上用户数据零影响）。
 */
export const STORAGE_KEY_PREFIX: string = isLinuxDesktop() ? 'tplayer' : 'tplayer-next'
