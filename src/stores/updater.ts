/**
 * 自动更新编排 store（M0 骨架）
 *
 * 职责边界（与 services/update 的分工）：
 *  - services/update 只负责「与官方 updater 插件对话」（check / download / install / available）；
 *  - 本 store 负责**编排**：每日节流、跳过记录、确认弹窗、下载进度与失败重试的 UI 状态、
 *    安装完成后「已是最新版本」的一次性提示。
 *
 * 设计原则：
 *  - 启动期静默检查在后台进行，绝不阻塞启动、绝不影响使用；
 *  - 无网络 / 通道未配置 / MSIX(Store) 版 / 浏览器预览：一律静默忽略；
 *  - 仅在「发现新版本且非用户跳过版本」时才弹确认框，由用户显式确认后才下载安装。
 */
import { ref } from 'vue'
import { defineStore } from 'pinia'

import { updateService, type Update, type UpdateProgress } from '../services/update'
import { useSettingsStore } from './settings'
import { useToastStore } from './toast'
import { t } from '../i18n'

/** 交互阶段 */
export type UpdatePhase =
  | 'idle'
  | 'checking'
  | 'available'
  | 'downloading'
  | 'installing'
  | 'error'

/** 每日节流窗口（毫秒）：24 小时内只检查一次 */
const DAILY_WINDOW_MS = 24 * 60 * 60 * 1000

export const useUpdaterStore = defineStore('updater', () => {
  /** 本机构建是否支持应用内更新（非 MSIX / 非浏览器） */
  const available = ref(false)
  /** 正在检查中（UI 禁用按钮用） */
  const checking = ref(false)
  /** 待安装版本（官方插件对象，含 version / body / date / currentVersion） */
  const update = ref<Update | null>(null)
  /** 确认弹窗是否打开 */
  const dialogOpen = ref(false)
  /** 当前交互阶段 */
  const phase = ref<UpdatePhase>('idle')
  /** 下载进度（字节） */
  const progress = ref<UpdateProgress>({ downloaded: 0 })
  /** 失败原因 */
  const error = ref('')
  /** 已重试次数（供 UI 反馈） */
  const retryAttempt = ref(0)
  /** 用户跳过的版本号（与 settings 同步） */
  const skippedVersion = ref<string | null>(null)

  const settings = useSettingsStore()
  const toast = useToastStore()

  /**
   * 初始化：读取本机构建能力 + 跳过版本记录。
   * 失败（如非 Tauri 环境）一律回落为「不可用」，静默忽略。
   */
  async function init(): Promise<void> {
    try {
      available.value = await updateService.available()
    } catch {
      available.value = false
    }
    skippedVersion.value = settings.skippedUpdateVersion
  }

  /**
   * 启动期每日首次检查（后台、非阻塞）。
   *
   * 首次启动判定：仅当距上次成功检查 ≥ 24h 才真正发起；检查成功才更新时间戳，
   * 这样「检测失败」会在次日重试，而非被节流窗口吞掉。
   * 发现新版本且非跳过版本时弹确认框；否则静默。
   */
  async function runDailyCheck(): Promise<void> {
    if (!available.value || !settings.updaterAutoCheck) return
    if (settings.lastUpdateCheckMs > 0 && Date.now() - settings.lastUpdateCheckMs < DAILY_WINDOW_MS) {
      return
    }
    await performCheck({ markChecked: true, silentIfUpToDate: true })
  }

  /** 设置页手动检查：忽略节流，始终提示结果（含「已是最新版本」） */
  async function manualCheck(): Promise<void> {
    if (!available.value) {
      toast.warning(t('updater.unsupported'))
      return
    }
    await performCheck({ markChecked: false, silentIfUpToDate: false })
  }

  async function performCheck(opts: {
    markChecked: boolean
    silentIfUpToDate: boolean
  }): Promise<void> {
    if (checking.value) return
    checking.value = true
    phase.value = 'checking'
    error.value = ''
    try {
      const result = await updateService.check()
      // 检查成功才更新时间戳（失败留待次日重试）
      if (opts.markChecked) await settings.markUpdateChecked()
      update.value = result
      if (!result) {
        if (!opts.silentIfUpToDate) toast.success(t('settings.upToDate'))
        phase.value = 'idle'
        return
      }
      // 跳过版本不重复提示；出现更新的版本才重新弹窗
      if (result.version === skippedVersion.value) {
        phase.value = 'idle'
        return
      }
      phase.value = 'available'
      dialogOpen.value = true
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause)
      phase.value = 'error'
      if (!opts.silentIfUpToDate) toast.error(t('settings.checkFailedWith', { error: error.value }))
    } finally {
      checking.value = false
    }
  }

  /** 用户确认升级：下载 + 安装（含重试/续传），完成前先记录已安装版本再重启 */
  async function confirmInstall(): Promise<void> {
    if (!update.value) return
    const latest = update.value.version
    // 先于重启持久化「已安装版本」：安装器接管后应用会重启，重启后据此一次性提示
    await settings.setLastInstalledVersion(latest)

    phase.value = 'downloading'
    progress.value = { downloaded: 0 }
    error.value = ''
    retryAttempt.value = 0
    try {
      await updateService.downloadAndInstall(
        (p) => {
          progress.value = p
        },
        (s) => {
          if (s.phase === 'installing') phase.value = 'installing'
          else if (s.phase === 'retrying') retryAttempt.value = s.attempt ?? retryAttempt.value + 1
          else phase.value = 'downloading'
        },
      )
      // 正常路径：install() 后由 relaunch 接管，不会返回；若返回说明无需重启
      phase.value = 'idle'
      update.value = null
      dialogOpen.value = false
      toast.success(t('updater.updateComplete'))
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause)
      phase.value = 'error'
    }
  }

  /** 用户跳过此版本：记录后不再提示该版本，关闭弹窗 */
  async function skipVersion(): Promise<void> {
    if (update.value) {
      await settings.setSkippedUpdateVersion(update.value.version)
      skippedVersion.value = update.value.version
    }
    dialogOpen.value = false
    update.value = null
    phase.value = 'idle'
  }

  /** 用户选择「稍后提醒」：仅关闭弹窗，下次每日检查（或手动检查）再提示 */
  function later(): void {
    dialogOpen.value = false
    update.value = null
    phase.value = 'idle'
  }

  /** 下载失败后的重试（复用确认安装流程） */
  function retry(): void {
    void confirmInstall()
  }

  /** 关闭弹窗（下载中关闭则转入后台继续，不中断安装） */
  function close(): void {
    dialogOpen.value = false
  }

  return {
    available,
    checking,
    update,
    dialogOpen,
    phase,
    progress,
    error,
    retryAttempt,
    skippedVersion,
    init,
    runDailyCheck,
    manualCheck,
    confirmInstall,
    skipVersion,
    later,
    retry,
    close,
  }
})
