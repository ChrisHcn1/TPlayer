/**
 * 解码组件（第三方依赖）状态编排 store
 *
 * 职责边界（与 services/deps 的分工）：
 *  - services/deps 只负责「与后端对话」（检测 / 安装 / 打开目录 / 卸载）；
 *  - 本 store 负责**编排**：何时弹引导窗、安装进度展示、失败原因归类、
 *    以及「用户已忽略引导」的持久化。
 *
 * 设计原则：
 *  - 缺失依赖**不阻断基础播放**（MP3/FLAC/WAV 等原生格式照常可播），
 *    因此引导是「非阻塞弹窗」：只有用户主动点播放冷门格式失败时才会打断；
 *  - 首次启动时静默检测，只在缺失且用户未忽略时才提示一次；
 *  - 更新场景下检测到已有依赖即直接复用（后端按公共目录优先查找）。
 */
import { ref } from 'vue'
import { defineStore } from 'pinia'

import {
  depsService,
  type DepInstallProgress,
  type DepsStatus,
} from '../services/deps'
import { normalizeError } from '../services/ipc'
import { useToastStore } from './toast'
import { t } from '../i18n'

/** 忽略引导的持久化键（用户勾选「不再提示」后写入） */
const IGNORED_KEY = 'tplayer-next.deps-guide-ignored'

/** 安装阶段 */
export type DepsPhase = 'idle' | 'checking' | 'installing' | 'error'

/** 工具展示名（专有名词，与语言无关） */
const TOOL_LABELS: Record<string, string> = {
  ffmpeg: 'FFmpeg',
  ffprobe: 'FFprobe',
  sacd_extract: 'SACD Extract',
}

export const useDepsStore = defineStore('deps', () => {
  /** 当前状态（null = 尚未检测） */
  const status = ref<DepsStatus | null>(null)
  /** 是否正在检测 */
  const checking = ref(false)
  /** 是否正在安装 */
  const installing = ref(false)
  /** 当前阶段 */
  const phase = ref<DepsPhase>('idle')
  /** 安装进度 */
  const progress = ref<DepInstallProgress | null>(null)
  /** 失败原因 */
  const error = ref('')
  /** 引导弹窗是否打开 */
  const guideOpen = ref(false)
  /** 用户是否已忽略引导（不再主动弹出） */
  const guideIgnored = ref(false)
  /** 检测失败（后端不可用等）时的降级标记 */
  const unavailable = ref(false)

  const toast = useToastStore()

  /**
   * 把后端异常归一化为可读文案。
   *
   * 后端 `AppError` 经 IPC 返回的是普通对象 `{ code, message, detail }`，
   * 直接 `String(对象)` 会得到 `[object Object]`；统一走 `normalizeError`
   * 取出真实 `message`（兜底为可读文本），组件与 toast 只消费字符串。
   */
  function messageOf(cause: unknown): string {
    return normalizeError(cause).message
  }

  /** 读取「已忽略引导」标记 */
  function loadIgnored(): boolean {
    try {
      return localStorage.getItem(IGNORED_KEY) === '1'
    } catch {
      return false
    }
  }

  /** 写入「已忽略引导」标记 */
  function saveIgnored(value: boolean): void {
    guideIgnored.value = value
    try {
      if (value) {
        localStorage.setItem(IGNORED_KEY, '1')
      } else {
        localStorage.removeItem(IGNORED_KEY)
      }
    } catch {
      /* 存储不可用时仅保留内存状态 */
    }
  }

  /**
   * 检测依赖状态。
   * 静默失败：检测本身失败不应打扰用户（可能只是浏览器预览环境）。
   */
  async function refresh(): Promise<DepsStatus | null> {
    checking.value = true
    try {
      const next = await depsService.status()
      status.value = next
      unavailable.value = false
      error.value = ''
      // 重新检测成功后清除上一次安装失败残留的 error 阶段，避免旧错误横幅常驻
      if (!installing.value) phase.value = 'idle'
      return next
    } catch (cause) {
      unavailable.value = true
      error.value = messageOf(cause)
      return null
    } finally {
      checking.value = false
    }
  }

  /**
   * 启动期静默检测：缺失且用户未忽略时才弹引导。
   * 已安装（或系统已提供）时不做任何提示——这是「更新场景直接复用」的体现。
   */
  async function init(): Promise<void> {
    guideIgnored.value = loadIgnored()
    const next = await refresh()
    if (!next) return
    if (next.ok) return
    if (guideIgnored.value) return
    // 必需组件缺失才提示；仅 ffprobe / sacd_extract 缺失属部分功能降级，不打扰
    const missingRequired = next.tools.some((tool) => tool.required && !tool.installed)
    if (missingRequired) {
      guideOpen.value = true
    }
  }

  /** 打开引导弹窗（设置页手动点击 / 播放失败后触发） */
  function openGuide(): void {
    guideOpen.value = true
  }

  /** 关闭引导弹窗（不忽略，下次仍会提示） */
  function closeGuide(): void {
    guideOpen.value = false
  }

  /** 关闭并忽略引导（不再主动弹出） */
  function dismissGuide(): void {
    saveIgnored(true)
    guideOpen.value = false
  }

  /**
   * 下载并安装依赖包。
   * 失败时保留具体原因（网络 / 校验 / 解压），并把「打开目录手动放置」
   * 作为兜底出口——弱网或 GitHub 不可达时用户仍能自行完成安装。
   */
  async function install(): Promise<boolean> {
    if (installing.value) return false
    installing.value = true
    phase.value = 'installing'
    progress.value = null
    error.value = ''
    try {
      const result = await depsService.install()
      status.value = result.status
      guideOpen.value = false
      toast.success(
        result.changed
          ? t('settings.depsInstalledVersion', { version: result.version })
          : t('settings.depsAlreadyUpToDate'),
      )
      return true
    } catch (cause) {
      const message = messageOf(cause)
      error.value = message
      phase.value = 'error'
      toast.error(t('settings.depsInstallFailedPrefix') + message)
      return false
    } finally {
      installing.value = false
      if (phase.value !== 'error') {
        phase.value = 'idle'
      }
    }
  }

  /** 从本地 zip 安装依赖包（用户自行下载后选择） */
  async function installLocal(path: string): Promise<boolean> {
    if (installing.value) return false
    installing.value = true
    phase.value = 'installing'
    progress.value = null
    error.value = ''
    try {
      const result = await depsService.installLocal(path)
      status.value = result.status
      guideOpen.value = false
      toast.success(t('settings.depsInstalledVersion', { version: result.version }))
      return true
    } catch (cause) {
      const message = messageOf(cause)
      error.value = message
      phase.value = 'error'
      toast.error(t('settings.depsInstallFailedPrefix') + message)
      return false
    } finally {
      installing.value = false
      if (phase.value !== 'error') {
        phase.value = 'idle'
      }
    }
  }

  /** 接收后端进度事件 */
  function onProgress(next: DepInstallProgress): void {
    progress.value = next
    if (next.phase === 'failed') {
      error.value = next.message
      phase.value = 'error'
    }
  }

  /** 打开公共依赖目录（手动放置文件） */
  async function openFolder(): Promise<void> {
    try {
      await depsService.openFolder()
      toast.info(t('settings.depsOpenFolderHint'))
    } catch (cause) {
      const message = messageOf(cause)
      toast.error(t('settings.depsOpenFolderFailedPrefix') + message)
    }
  }

  /** 卸载已安装的依赖包 */
  async function remove(): Promise<void> {
    try {
      status.value = await depsService.remove()
      toast.success(t('settings.depsUninstalled'))
    } catch (cause) {
      const message = messageOf(cause)
      toast.error(t('settings.depsUninstallFailedPrefix') + message)
    }
  }

  /** 工具展示名（专有名词，与语言无关） */
  function toolLabel(name: string): string {
    return TOOL_LABELS[name] ?? name
  }

  /** 工具用途说明 */
  function toolPurpose(name: string): string {
    return t(`settings.depsPurpose.${name}`)
  }

  /** 来源说明 */
  function sourceLabel(source: string): string {
    return t(`settings.depsSource.${source}`)
  }

  /** 依赖包体积提示 */
  function sizeHint(): string {
    return t('settings.depsSizeHint')
  }

  /** 状态摘要（设置页卡片顶部一行） */
  function summary(): string {
    if (!status.value) return checking.value ? t('settings.depsChecking') : t('settings.depsNotChecked')
    if (status.value.ok) {
      const pack = status.value.installedPack ? `v${status.value.installedPack}` : ''
      return pack ? t('settings.depsInstalledPack', { pack }) : t('settings.depsReady')
    }
    const missing = status.value.missing.map(toolLabel).join(t('settings.depsListSep'))
    return t('settings.depsMissing', { missing })
  }

  return {
    status,
    checking,
    installing,
    phase,
    progress,
    error,
    guideOpen,
    guideIgnored,
    unavailable,
    refresh,
    init,
    openGuide,
    closeGuide,
    dismissGuide,
    install,
    installLocal,
    onProgress,
    openFolder,
    remove,
    toolLabel,
    toolPurpose,
    sourceLabel,
    sizeHint,
    summary,
  }
})
