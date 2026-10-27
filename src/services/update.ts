/**
 * 自动升级服务（Tauri Updater 官方插件）
 *
 * 通道：GitHub Releases 上的 latest.json（地址见 tauri.conf.json plugins.updater.endpoints）。
 * 安装包经 minisign 私钥签名，客户端用内置公钥校验，校验失败拒绝安装。
 *
 * 约定：
 *  - 启动时只做静默检查（发现新版本才提示），不自动下载；
 *  - 下载/安装必须由用户在确认弹窗中显式触发，安装完成后经 process 插件重启；
 *  - 浏览器预览环境不支持升级（无 updater 插件宿主），调用会拒绝。
 *
 * 本模块只负责「与官方插件对话」，节流 / 跳过 / 状态同步等编排在 stores/updater。
 */
import { check, type Update, type DownloadEvent } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

import { callCommand, IpcUnavailableError, isTauriRuntime } from './ipc'

export type { Update } from '@tauri-apps/plugin-updater'

/** 下载进度（downloaded/total 均为字节；total 在服务器不返回 Content-Length 时缺省） */
export interface UpdateProgress {
  downloaded: number
  total?: number
}

/** 下载/安装阶段状态（供 UI 反馈重试与安装进度） */
export interface UpdateStatus {
  phase: 'downloading' | 'installing' | 'retrying'
  /** 第几次重试（phase = 'retrying' 时有效，从 1 开始） */
  attempt?: number
}

/** 最多重试次数（下载失败时退避重试，兼顾「断点续传」与弱网容错） */
const MAX_DOWNLOAD_ATTEMPTS = 3

/** 退避延迟（毫秒）：第 n 次重试等待 n 秒，避免瞬时抖动导致的频繁重试 */
function backoff(attempt: number): number {
  return 1000 * attempt
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

/** 最近一次检查发现的待安装版本；下载安装时复用，避免重复请求 manifest */
let pendingUpdate: Update | null = null

export const updateService = {
  /**
   * 本机构建是否支持应用内更新。
   *
   * 经 Microsoft Store 分发的 MSIX 版不会注册 updater 插件（Store 政策要求更新走 Store），
   * 此时返回 false，前端据此跳过检查，避免无效 IPC 与日志噪声。
   * 浏览器预览环境同样返回 false。
   */
  async available(): Promise<boolean> {
    if (!isTauriRuntime()) return false
    try {
      return await callCommand<boolean>('updater_available')
    } catch {
      return false
    }
  },

  /**
   * 检查更新。
   * @returns 有新版本返回 Update（含 version / body 发布说明 / date / currentVersion），否则 null。
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
   *
   * 容错：下载阶段失败按指数退避重试（默认 3 次）。官方插件本身支持断点续传
   * （下载到临时文件，中断后重调用会从已下载处继续），重试不会从头再来。
   * 安装阶段由 NSIS 安装器接管并重启进入新版本。
   *
   * @param onProgress 下载进度回调（字节）
   * @param onStatus  阶段状态回调（下载中 / 安装中 / 重试中），供 UI 反馈
   */
  async downloadAndInstall(
    onProgress?: (progress: UpdateProgress) => void,
    onStatus?: (status: UpdateStatus) => void,
  ): Promise<void> {
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
    let attempt = 0
    while (true) {
      attempt += 1
      try {
        onStatus?.({ phase: 'downloading', attempt })
        // 断点续传：插件复用临时文件，从中断处继续下载
        await update.download((event: DownloadEvent) => {
          if (event.event === 'Started') {
            // 总大小取自响应头 Content-Length（服务器不返回时缺省，UI 退化为不定进度）
            total = event.data.contentLength
            onProgress?.({ downloaded: 0, total })
          } else if (event.event === 'Progress') {
            downloaded += event.data.chunkLength
            onProgress?.({ downloaded, total })
          }
        })
        break
      } catch (cause) {
        if (attempt >= MAX_DOWNLOAD_ATTEMPTS) throw cause
        onStatus?.({ phase: 'retrying', attempt })
        await delay(backoff(attempt))
      }
    }

    onStatus?.({ phase: 'installing' })
    // NSIS passive 模式：安装包下载完成即由安装器接管，随后重启进入新版本
    await update.install()
    await relaunch()
  },

  /** 清除待安装版本缓存（安装失败/放弃升级时调用） */
  reset(): void {
    pendingUpdate = null
  },
}
