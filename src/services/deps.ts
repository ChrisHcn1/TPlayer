/**
 * 解码组件（第三方依赖）服务层
 *
 * 对应后端 `src-tauri/src/commands/deps.rs`。
 * 与 services/update 的分工类似：本模块只负责「与后端对话」，
 * 状态编排（弹窗时机、进度展示）放在 stores/deps。
 */
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import { callCommand, IpcUnavailableError, isTauriRuntime } from './ipc'

/** 依赖来源（与后端 ToolSource 对齐） */
export type DepSource = 'deps' | 'programDir' | 'system' | 'missing'

/** 单个工具状态 */
export interface DepToolStatus {
  name: string
  /** 是否为播放必需 */
  required: boolean
  installed: boolean
  path: string | null
  source: DepSource
  packVersion: string | null
}

/** 依赖整体状态 */
export interface DepsStatus {
  depsDir: string
  platform: string
  installedPack: string | null
  expectedPack: string
  outdated: boolean
  tools: DepToolStatus[]
  missing: string[]
  ok: boolean
  installable: boolean
}

/** 安装进度 */
export interface DepInstallProgress {
  phase: string
  downloaded: number
  total: number
  message: string
}

/** 安装结果 */
export interface DepInstallResult {
  version: string
  changed: boolean
  status: DepsStatus
}

/** 安装进度事件名 */
export const DEPS_PROGRESS_EVENT = 'deps://progress'

/** 依赖包体积提示 */
export const DEPS_SIZE_HINT = '约 430 MB（FFmpeg + FFprobe + SACD Extract）'

export const depsService = {
  /** 检测依赖状态 */
  async status(): Promise<DepsStatus> {
    if (!isTauriRuntime()) {
      throw new IpcUnavailableError()
    }
    return callCommand<DepsStatus>('deps_status')
  },

  /**
   * 下载并安装依赖包。
   *
   * 超时给足 30 分钟：依赖包数百 MB，慢速网络下 10s 默认超时必然先触发，
   * 会让用户看到「安装失败」而后台其实还在下。
   */
  async install(): Promise<DepInstallResult> {
    if (!isTauriRuntime()) {
      throw new IpcUnavailableError()
    }
    return callCommand<DepInstallResult>('deps_install', undefined, { timeoutMs: 30 * 60_000 })
  },

  /** 从本地 zip 文件安装依赖包 */
  async installLocal(path: string): Promise<DepInstallResult> {
    if (!isTauriRuntime()) {
      throw new IpcUnavailableError()
    }
    return callCommand<DepInstallResult>('deps_install_local', { path }, { timeoutMs: 30 * 60_000 })
  },

  /** 打开公共依赖目录（用户手动放置文件） */
  async openFolder(): Promise<void> {
    if (!isTauriRuntime()) {
      throw new IpcUnavailableError()
    }
    await callCommand('deps_open_folder')
  },

  /** 卸载已安装的依赖包 */
  async remove(): Promise<DepsStatus> {
    if (!isTauriRuntime()) {
      throw new IpcUnavailableError()
    }
    return callCommand<DepsStatus>('deps_remove')
  },

  /**
   * 订阅安装进度事件。
   * 返回取消订阅函数；非 Tauri 环境返回空函数（调用方无需分支）。
   */
  async onProgress(handler: (progress: DepInstallProgress) => void): Promise<UnlistenFn> {
    if (!isTauriRuntime()) {
      return () => {}
    }
    return listen<DepInstallProgress>(DEPS_PROGRESS_EVENT, (event) => {
      handler(event.payload)
    })
  },
}
