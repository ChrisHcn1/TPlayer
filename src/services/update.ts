/**
 * 自动升级服务（Tauri Updater 官方插件）
 *
 * 通道：GitHub Releases 上的 latest.json（地址见 tauri.conf.json plugins.updater.endpoints）。
 * 安装包经 minisign 私钥签名，客户端用内置公钥校验，校验失败拒绝安装。
 *
 * 约定：
 *  - 启动时只做静默检查（发现新版本才提示），不自动下载；
 *  - 下载/安装必须由用户在设置页显式触发，安装完成后经 process 插件重启；
 *  - 浏览器预览环境不支持升级（无 updater 插件宿主），调用会拒绝。
 */
import { check, type Update, type DownloadEvent } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

import { IpcUnavailableError, isTauriRuntime } from './ipc'

export type { Update } from '@tauri-apps/plugin-updater'

/** 下载进度（downloaded/total 均为字节；total 在服务器不返回 Content-Length 时缺省） */
export interface UpdateProgress {
  downloaded: number
  total?: number
}

/** 最近一次检查发现的待安装版本；下载安装时复用，避免重复请求 manifest */
let pendingUpdate: Update | null = null

export const updateService = {
  /**
   * 检查更新。
   * @returns 有新版本返回 Update（含 version / body 发布说明 / date），否则 null。
   */
  async check(): Promise<Update | null> {
    if (!isTauriRuntime()) {
      throw new IpcUnavailableError()
    }
    const update = await check()
    pendingUpdate = update
    return update
  },

  /**
   * 下载并安装更新，完成后自动重启应用。
   * 必须先调用 check() 得到新版本；进度通过回调上报。
   */
  async downloadAndInstall(onProgress?: (progress: UpdateProgress) => void): Promise<void> {
    if (!isTauriRuntime()) {
      throw new IpcUnavailableError()
    }
    const update = pendingUpdate ?? (await check())
    if (!update) {
      pendingUpdate = null
      return
    }

    let downloaded = 0
    let total: number | undefined
    await update.downloadAndInstall((event: DownloadEvent) => {
      if (event.event === 'Started') {
        // 总大小取自响应头 Content-Length（服务器不返回时缺省，UI 退化为不定进度）
        total = event.data.contentLength
        onProgress?.({ downloaded: 0, total })
      } else if (event.event === 'Progress') {
        downloaded += event.data.chunkLength
        onProgress?.({ downloaded, total })
      }
    })

    // NSIS passive 模式：安装包下载完成即由安装器接管，随后重启进入新版本
    await relaunch()
  },

  /** 清除待安装版本缓存（安装失败/放弃升级时调用） */
  reset(): void {
    pendingUpdate = null
  },
}
