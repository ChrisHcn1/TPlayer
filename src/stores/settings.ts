/**
 * settings store（M0 骨架）
 *
 * 约束：
 *  - 持久化键统一取自 constants（与 index.html 首屏脚本、composables/useTheme 共用）；
 *  - **主题不归本 store 管**：主题的唯一事实源是 composables/useTheme（唯一持久化键
 *    THEME_STORAGE_KEY = 'tplayer-next.theme'），本 store 既不持有 theme 状态，
 *    也不再把 theme 写进 SETTINGS_STORAGE_KEY，彻底消除双写双源；
 *  - 在线模块默认关闭，且 allowLibraryUpload 默认 false：即使开启在线能力，
 *    也不得自动把本地曲库信息上报给外部服务（见 services/online.ts 的准入检查）；
 *  - 曲库目录是"白名单"的来源：只有在此登记的目录才允许被扫描与读取（见 ARCHITECTURE.md §9）。
 *
 * M2 改造点：把 localStorage 换成应用数据目录下的 config.json（%APPDATA%\com.tplayer），
 * 由 Rust 侧读写，避免 WebView 存储被清理后丢失曲库目录配置。
 */
import { ref } from 'vue'
import { defineStore } from 'pinia'

import { DEFAULT_LOCALE } from '../constants'
import { DEFAULT_ONLINE_CONFIG } from '../services/online'
import { loadSettings, saveSettings } from '../services/settings'
import { playerService } from '../services/player'
import { libraryService } from '../services/library'
import type { OnlineModuleConfig } from '../types'

/** 持久化结构（与 localStorage / 未来的 config.json 一一对应；不含 theme） */
export interface PersistedSettings {
  locale: string
  libraryDirs: string[]
  online: OnlineModuleConfig
  scanOnStartup: boolean
  crossfade: { enabled: boolean; duration_ms: number }
  replaygain: { enabled: boolean; targetLufs: number }
  watch: { enabled: boolean }
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0
}

export const useSettingsStore = defineStore('settings', () => {
  const locale = ref<string>(DEFAULT_LOCALE)
  const libraryDirs = ref<string[]>([])
  const online = ref<OnlineModuleConfig>({ ...DEFAULT_ONLINE_CONFIG })
  const scanOnStartup = ref(false)
  const crossfade = ref({ enabled: true, duration_ms: 1000 })
  const replaygain = ref({ enabled: true, targetLufs: -14 })
  const watch = ref({ enabled: true })

  /** 导出当前设置为持久化结构 */
  function toPersisted(): PersistedSettings {
    return {
      locale: locale.value,
      libraryDirs: [...libraryDirs.value],
      online: { ...online.value },
      scanOnStartup: scanOnStartup.value,
      crossfade: { ...crossfade.value },
      replaygain: { ...replaygain.value },
      watch: { ...watch.value },
    }
  }

  /**
   * 从持久化结构恢复（逐字段校验，脏数据一律回落默认值）
   * 注意：历史版本可能残留 theme 字段，这里显式忽略——主题只认 THEME_STORAGE_KEY。
   */
  function applyPersisted(persisted: Partial<PersistedSettings>): void {
    if (isNonEmptyString(persisted.locale)) {
      locale.value = persisted.locale
    }
    if (Array.isArray(persisted.libraryDirs)) {
      libraryDirs.value = [...new Set(persisted.libraryDirs.filter(isNonEmptyString))]
    }
    if (persisted.online) {
      // 安全默认：无论本地存了什么，只要在线能力未开启，上报开关必须回落 false
      const enabled = persisted.online.enabled === true
      online.value = {
        enabled,
        allowLibraryUpload: enabled && persisted.online.allowLibraryUpload === true,
        provider: enabled ? persisted.online.provider ?? 'none' : 'none',
      }
    }
    if (typeof persisted.scanOnStartup === 'boolean') {
      scanOnStartup.value = persisted.scanOnStartup
    }
    if (persisted.crossfade) {
      if (typeof persisted.crossfade.enabled === 'boolean') {
        crossfade.value.enabled = persisted.crossfade.enabled
      }
      if (typeof persisted.crossfade.duration_ms === 'number') {
        const d = persisted.crossfade.duration_ms
        if (d >= 0 && d <= 3000) {
          crossfade.value.duration_ms = d
        }
      }
    }
    if (persisted.replaygain) {
      if (typeof persisted.replaygain.enabled === 'boolean') {
        replaygain.value.enabled = persisted.replaygain.enabled
      }
      // targetLufs 兜底校验：仅接受合理区间（-40 ~ 0 LUFS），脏数据回落默认 -14
      const target = persisted.replaygain.targetLufs
      if (typeof target === 'number' && Number.isFinite(target) && target <= 0 && target >= -40) {
        replaygain.value.targetLufs = target
      }
    }
    if (persisted.watch && typeof persisted.watch.enabled === 'boolean') {
      watch.value.enabled = persisted.watch.enabled
    }
  }

  /**
   * 持久化当前设置；返回是否成功。
   * 失败已在服务层记录日志且不抛出（避免 fire-and-forget 调用点产生未处理拒绝）。
   */
  async function persist(): Promise<boolean> {
    return saveSettings(toPersisted())
  }

  /** 由 App.vue 在挂载后调用，恢复用户偏好 */
  async function restore(): Promise<void> {
    try {
      const persisted = await loadSettings()
      if (!persisted) return
      applyPersisted(persisted)
    } catch {
      /* 解析失败视为无历史设置 */
    }
  }

  async function setLocale(next: string): Promise<void> {
    if (!isNonEmptyString(next)) return
    locale.value = next
    persist()
  }

  async function setScanOnStartup(next: boolean): Promise<void> {
    scanOnStartup.value = next
    persist()
  }

  /**
   * 添加曲库目录。
   *
   * 必须调用后端专用命令 library_add_dir（直接写 config.json 白名单），
   * 不能只依赖整体 settings_save —— 后者任一字段不匹配即整体失败，会造成
   * 「UI 已显示目录、扫描却报未配置曲库目录」的状态错位（历史 bug）。
   * 顺序：先写后端白名单（失败即抛出，交由 UI 提示且不改本地状态），再同步本地与其余设置。
   */
  async function addLibraryDir(dir: string): Promise<void> {
    if (!isNonEmptyString(dir) || libraryDirs.value.includes(dir)) return
    await libraryService.addLibraryDir(dir)
    libraryDirs.value = [...libraryDirs.value, dir]
    await persist()
  }

  /** 移除曲库目录（同上：先改后端白名单，再同步本地） */
  async function removeLibraryDir(dir: string): Promise<void> {
    await libraryService.removeLibraryDir(dir)
    libraryDirs.value = libraryDirs.value.filter((item) => item !== dir)
    await persist()
  }

  async function setOnlineEnabled(enabled: boolean): Promise<void> {
    online.value = {
      enabled,
      // 关闭在线能力时同步关闭上报开关，避免下次开启时"继承"上报表意
      allowLibraryUpload: enabled ? online.value.allowLibraryUpload : false,
      provider: enabled ? 'netease' : 'none',
    }
    persist()
  }

  async function setAllowLibraryUpload(allowed: boolean): Promise<void> {
    online.value = {
      ...online.value,
      allowLibraryUpload: allowed && online.value.enabled,
    }
    persist()
  }

  async function setCrossfadeEnabled(enabled: boolean): Promise<void> {
    crossfade.value.enabled = enabled
    persist()
    void playerService.setCrossfade(enabled, crossfade.value.duration_ms)
  }

  async function setCrossfadeDuration(duration_ms: number): Promise<void> {
    if (duration_ms < 0 || duration_ms > 3000) return
    const d = Math.round(duration_ms)
    crossfade.value.duration_ms = d
    persist()
    void playerService.setCrossfade(crossfade.value.enabled, d)
  }

  async function setReplaygainEnabled(enabled: boolean): Promise<void> {
    replaygain.value.enabled = enabled
    persist()
    void playerService.setReplaygain(enabled)
  }

  async function setWatchEnabled(enabled: boolean): Promise<void> {
    watch.value.enabled = enabled
    persist()
  }

  /**
   * 恢复出厂默认（仅重置"设置"，不影响曲库文件与播放历史）
   * 主题不在此重置：由 useTheme.resetTheme() 单独负责，保证主题只有一个写入方。
   */
  async function reset(): Promise<void> {
    locale.value = DEFAULT_LOCALE
    libraryDirs.value = []
    online.value = { ...DEFAULT_ONLINE_CONFIG }
    scanOnStartup.value = false
    crossfade.value = { enabled: true, duration_ms: 1000 }
    replaygain.value = { enabled: true, targetLufs: -14 }
    watch.value = { enabled: true }
    persist()
    void playerService.setCrossfade(true, 1000)
    void playerService.setReplaygain(true)
  }

  return {
    locale,
    libraryDirs,
    online,
    scanOnStartup,
    crossfade,
    replaygain,
    watch,
    restore,
    persist,
    setLocale,
    setScanOnStartup,
    addLibraryDir,
    removeLibraryDir,
    setOnlineEnabled,
    setAllowLibraryUpload,
    setCrossfadeEnabled,
    setCrossfadeDuration,
    setReplaygainEnabled,
    setWatchEnabled,
    reset,
  }
})
