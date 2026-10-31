/**
 * 应用级服务：打开外部网址（TPlayer 官网入口）
 *
 * 为什么不用 `<a href>` / `window.open`：
 *  - Tauri v2 会拦截 WebView 内的外部导航（防止应用被诱导到任意站点）；
 *  - 因此打开官网必须走后端，由系统默认浏览器接管（见 src-tauri/src/website.rs）。
 * 浏览器预览环境（vite dev）没有后端，降级为 window.open。
 */
import { callCommand, isTauriRuntime } from './ipc'

/** 官方网站地址（与后端 `website::WEBSITE_URL` 保持一致） */
export const WEBSITE_URL = 'http://tplayer.gleckes.cn'

/** 用系统默认浏览器打开外部网址（后端仅允许 http / https） */
export async function openExternalUrl(url: string): Promise<void> {
  if (!isTauriRuntime()) {
    window.open(url, '_blank', 'noopener,noreferrer')
    return
  }
  await callCommand<void>('open_external_url', { url })
}

/** 打开 TPlayer 官网 */
export async function openWebsite(): Promise<void> {
  await openExternalUrl(WEBSITE_URL)
}

export const appService = {
  openExternalUrl,
  openWebsite,
}
