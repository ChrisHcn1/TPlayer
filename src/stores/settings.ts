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
 * M2 改造点：把 localStorage 换成应用数据目录下的 config.json（%APPDATA%\TPlayer Next），
 * 由 Rust 侧读写，避免 WebView 存储被清理后丢失曲库目录配置。
 */
import { ref } from 'vue'
import { defineStore } from 'pinia'

import { DEFAULT_LOCALE } from '../constants'
import { DEFAULT_ONLINE_CONFIG } from '../services/online'
import { loadSettings, saveSettings } from '../services/settings'
import type { OnlineModuleConfig } from '../types'

/** 持久化结构（与 localStorage / 未来的 config.json 一一对应；不含 theme） */
export interface PersistedSettings {
  locale: string
  libraryDirs: string[]
  online: OnlineModuleConfig
  scanOnStartup: boolean
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0
}

export const useSettingsStore = defineStore('settings', () => {
  const locale = ref<string>(DEFAULT_LOCALE)
  const libraryDirs = ref<string[]>([])
  const online = ref<OnlineModuleConfig>({ ...DEFAULT_ONLINE_CONFIG })
  const scanOnStartup = ref(false)

  /** 导出当前设置为持久化结构 */
  function toPersisted(): PersistedSettings {
    return {
      locale: locale.value,
      libraryDirs: [...libraryDirs.value],
      online: { ...online.value },
      scanOnStartup: scanOnStartup.value,
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
  }

  async function persist(): Promise<void> {
    await saveSettings(toPersisted())
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

  async function addLibraryDir(dir: string): Promise<void> {
    if (!isNonEmptyString(dir) || libraryDirs.value.includes(dir)) return
    libraryDirs.value = [...libraryDirs.value, dir]
    persist()
  }

  async function removeLibraryDir(dir: string): Promise<void> {
    libraryDirs.value = libraryDirs.value.filter((item) => item !== dir)
    persist()
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

  /**
   * 恢复出厂默认（仅重置"设置"，不影响曲库文件与播放历史）
   * 主题不在此重置：由 useTheme.resetTheme() 单独负责，保证主题只有一个写入方。
   */
  async function reset(): Promise<void> {
    locale.value = DEFAULT_LOCALE
    libraryDirs.value = []
    online.value = { ...DEFAULT_ONLINE_CONFIG }
    scanOnStartup.value = false
    persist()
  }

  return {
    locale,
    libraryDirs,
    online,
    scanOnStartup,
    restore,
    persist,
    setLocale,
    setScanOnStartup,
    addLibraryDir,
    removeLibraryDir,
    setOnlineEnabled,
    setAllowLibraryUpload,
    reset,
  }
})
