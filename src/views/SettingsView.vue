<script setup lang="ts">
/**
 * 设置视图（DESIGN §2.4）：左侧分组页签 —— 按功能属性归类。
 *
 * 分组（每个分组名即功能属性）：
 *  - 界面与外观：外观 / 歌词
 *  - 播放与媒体：播放 / 媒体库
 *  - 服务与工具：在线服务 / 工具
 *  - 系统：更新 / 关于
 *
 * 约定：
 *  - 主题唯一事实源 useTheme，本页只做切换 UI；
 *  - 曲库目录白名单经 settings store；Tauri 下走系统目录选择器（services/dialog），
 *    浏览器降级为 prompt 文本输入；
 *  - 在线模块默认关闭；即使开启，allowLibraryUpload 仍需单独同意（privacy-first）。
 *  - 支持 /settings/:tab 直达（外观 / 播放 / 媒体库 / 歌词 / 工具 / 更新 / 关于）。
 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'

import pkg from '../../package.json'
import { useSettingsStore } from '../stores/settings'
import { useLibraryStore } from '../stores/library'
import { useTheme } from '../composables/useTheme'
import { useToastStore } from '../stores/toast'
import { useDialogStore } from '../stores/dialog'
import { pickDirectory, pickDepsPackage } from '../services/dialog'
import { lyricsService } from '../services/lyrics'
import { appService } from '../services/app'
import { isTauriRuntime, normalizeError } from '../services/ipc'
import { getWatcherStatus, subscribeWatcher, type WatcherStatus } from '../services/watcher'
import BaseButton from '../components/common/BaseButton.vue'
import Icon from '../components/common/Icon.vue'
import ConverterView from './ConverterView.vue'
import StatsView from './StatsView.vue'
import LabView from './LabView.vue'
import { useLayout } from '../composables/useLayout'
import { useUpdaterStore } from '../stores/updater'
import { useDepsStore } from '../stores/deps'
import { useRoute, useRouter } from 'vue-router'
import { depsService } from '../services/deps'
import { t, useI18n } from '../i18n'

type SettingsTab =
  | 'appearance'
  | 'playback'
  | 'library'
  | 'lyrics'
  | 'report'
  | 'tools'
  | 'lab'
  | 'update'
  | 'about'

interface TabDef {
  key: SettingsTab
  labelKey: string
  icon: string
}

interface TabGroup {
  key: string
  labelKey: string
  tabs: ReadonlyArray<TabDef>
}

/** 左侧导航分组（每个组名即功能属性） */
const TAB_GROUPS: ReadonlyArray<TabGroup> = [
  {
    key: 'interface',
    labelKey: 'settings.group.interface',
    tabs: [
      { key: 'appearance', labelKey: 'settings.appearance', icon: 'sun' },
    ],
  },
  {
    key: 'playback-media',
    labelKey: 'settings.group.playback',
    tabs: [
      { key: 'playback', labelKey: 'settings.playback', icon: 'play' },
      { key: 'library', labelKey: 'settings.library', icon: 'disc' },
      { key: 'lyrics', labelKey: 'settings.lyrics', icon: 'lyrics' },
    ],
  },
  {
    key: 'service',
    labelKey: 'settings.group.service',
    tabs: [
      { key: 'tools', labelKey: 'settings.tools', icon: 'swap' },
      { key: 'report', labelKey: 'settings.report', icon: 'clock' },
      { key: 'lab', labelKey: 'settings.lab', icon: 'settings' },
    ],
  },
  {
    key: 'system',
    labelKey: 'settings.group.system',
    tabs: [
      { key: 'update', labelKey: 'settings.update', icon: 'download' },
      { key: 'about', labelKey: 'settings.about', icon: 'info' },
    ],
  },
]

const TAB_KEYS = new Set<SettingsTab>(
  TAB_GROUPS.flatMap((group) => group.tabs.map((tab) => tab.key)),
)

function isSettingsTab(value: unknown): value is SettingsTab {
  return typeof value === 'string' && TAB_KEYS.has(value as SettingsTab)
}

const settingsStore = useSettingsStore()
const libraryStore = useLibraryStore()
const toastStore = useToastStore()
const dialogStore = useDialogStore()
const { theme, toggleTheme } = useTheme()
const { isCoverAccentEnabled, setCoverAccent } = useLayout()
const router = useRouter()
const route = useRoute()

/** 国际化：当前语言 / 可选语言清单 / 切换 */
const { currentLocale, availableLocales, setLocale } = useI18n()
function onLocaleChange(event: Event): void {
  const value = (event.target as HTMLSelectElement).value
  void setLocale(value)
}

const DEFAULT_TAB: SettingsTab = 'appearance'
const activeTab = ref<SettingsTab>(DEFAULT_TAB)
const scanning = ref(false)

/** 目录自动监控状态（仅 Tauri 运行时后端提供；浏览器预览为 null） */
const watcherStatus = ref<WatcherStatus | null>(null)
let unlistenWatcher: (() => void) | null = null
/** 解码组件安装进度事件取消订阅 */
let unlistenDeps: (() => void) | null = null

async function refreshWatcherStatus(): Promise<void> {
  watcherStatus.value = await getWatcherStatus()
}

onMounted(() => {
  // 支持 /settings/:tab 直达
  if (isSettingsTab(route.params.tab)) {
    activeTab.value = route.params.tab
  }
  void refreshWatcherStatus()
  // 增量同步发生后实时更新状态展示
  unlistenWatcher = subscribeWatcher((payload) => {
    watcherStatus.value = {
      enabled: watcherStatus.value?.enabled ?? true,
      dirs: watcherStatus.value?.dirs ?? [],
      last_event_at: payload.at,
      last_event_desc: payload.desc,
    }
  })
  // 解码组件：进入设置页时检测一次；订阅安装进度事件
  void depsStore.refresh()
  void depsService.onProgress((progress) => depsStore.onProgress(progress)).then((unlisten) => {
    unlistenDeps = unlisten
  })
})

// 浏览器前进/后退或地址栏变化时同步页签
watch(
  () => route.params.tab,
  (tab) => {
    if (isSettingsTab(tab)) activeTab.value = tab
  },
)

onUnmounted(() => {
  unlistenWatcher?.()
  unlistenDeps?.()
})

/**
 * 主题控件：全应用只有标题栏一枚主题图标与设置页这一个主题控件，
 * 原先「深色 / 浅色」两个选项按钮收敛为单个切换按钮（点击即在两者间切换），
 * 图标随当前主题变化（深色=月亮，浅色=太阳）。
 */
const themeIcon = computed(() => (theme.value === 'dark' ? 'moon' : 'sun'))
const themeLabel = computed(() =>
  theme.value === 'dark' ? t('nav.theme-to-light') : t('nav.theme-to-dark'),
)
const themeToggleLabel = computed(() =>
  theme.value === 'dark' ? t('settings.themeToLight') : t('settings.themeToDark'),
)

const isTauri = computed(() => isTauriRuntime())
const appVersion = computed(() => pkg.version)

function selectTab(tab: SettingsTab): void {
  activeTab.value = tab
  // 同步到地址栏，便于直达与分享
  if (route.params.tab !== tab) {
    void router.replace({ name: 'settings-tab', params: { tab } })
  }
}

async function addLibraryDir(): Promise<void> {
  const picked = await pickDirectory()
  if (picked) {
    await persistLibraryDir(() => settingsStore.addLibraryDir(picked), t('settings.dirAdded'))
    return
  }
  // 浏览器（或选择器不可用）：降级为手动输入路径
  const input = await dialogStore.prompt({
    title: t('settings.addDirDialogTitle'),
    message: isTauri.value
      ? t('settings.addDirDialogTauriMsg')
      : t('settings.addDirDialogBrowserMsg'),
    placeholder: t('settings.addDirPlaceholder'),
    validate: (value) => {
      const v = value.trim()
      if (!v) return t('settings.pathRequired')
      // Windows 绝对路径需盘符（C:\）或 UNC（\\host\share）；网络共享若写成
      // 172.18.0.21/songs 这类非法形式，扫描时会静默失败，这里提前提示正确写法
      if (!/^[A-Za-z]:\\|\\\\/.test(v)) {
        return t('settings.pathInvalid')
      }
      return null
    },
  })
  if (input === null) return
  const dir = input.trim()
  if (settingsStore.libraryDirs.includes(dir)) {
    toastStore.warning(t('settings.dirExists'))
    return
  }
  await persistLibraryDir(() => settingsStore.addLibraryDir(dir), t('settings.dirAdded'))
}

/**
 * 统一处理曲库目录写入：成功提示，失败给出可定位错误。
 * 原先 store 的持久化失败被静默吞掉，UI 仍提示"已添加"，
 * 导致扫描时才发现"未配置曲库目录"，极具误导性。
 */
async function persistLibraryDir(action: () => Promise<void>, successText: string): Promise<void> {
  try {
    await action()
    toastStore.success(successText)
  } catch (cause) {
    toastStore.error(t('settings.dirSaveFailed') + normalizeError(cause).message)
  }
}

async function removeLibraryDir(dir: string): Promise<void> {
  const confirmed = await dialogStore.confirm({
    title: t('settings.removeDirDialogTitle'),
    message: t('settings.removeDirDialogMessage', { dir }),
    confirmText: t('settings.removeConfirm'),
  })
  if (!confirmed) return
  await persistLibraryDir(() => settingsStore.removeLibraryDir(dir), t('settings.dirRemoved'))
}

async function scanNow(): Promise<void> {
  scanning.value = true
  try {
    // 智能扫描：曲目自上次扫描未变更时直接复用曲库 DB（瞬时），否则增量刷新
    await libraryStore.scan(false)
    if (libraryStore.error) toastStore.error(t('settings.scanFailed') + libraryStore.error)
    else toastStore.success(t('settings.scanDone'))
  } finally {
    scanning.value = false
  }
}

/**
 * 取消当前扫描：后端置位取消标志，快扫占位循环与后台补解析随即停止。
 * 已写入的批次保留，剩余文件保持占位，下次扫描或播放时再解析——
 * 网络共享 / 海量小文件场景下用户可主动中止，不必等待自然结束。
 */
async function cancelScanNow(): Promise<void> {
  await libraryStore.cancelScan()
  toastStore.info(t('settings.scanCancelRequested'))
}

async function clearLyricsCache(): Promise<void> {
  await lyricsService.clearCache()
  toastStore.success(t('settings.lyricsCacheCleared'))
}

// ---- 应用更新（Tauri Updater，委托给 updater store）----------------
const updaterStore = useUpdaterStore()

// ---- 解码组件（第三方可选依赖）--------------------------------------
const depsStore = useDepsStore()

/** 解码组件状态（null = 尚未检测） */
const depsStatus = computed(() => depsStore.status)
/** 是否正在检测 */
const depsChecking = computed(() => depsStore.checking)
/** 是否正在安装 */
const depsInstalling = computed(() => depsStore.installing)
/** 当前阶段 */
const depsPhase = computed(() => depsStore.phase)
/** 状态摘要 */
const depsSummary = computed(() => depsStore.summary())
/** 工具展示名 */
const depsToolLabel = (name: string): string => depsStore.toolLabel(name)
/** 工具用途 */
const depsToolPurpose = (name: string): string => depsStore.toolPurpose(name)
/** 来源说明 */
const depsSourceLabel = (source: string): string => depsStore.sourceLabel(source)

/** 下载百分比（总大小未知时为 null → 不定进度条） */
const depsPercent = computed<number | null>(() => {
  const p = depsStore.progress
  if (!p || !p.total || p.total <= 0) return null
  return Math.min(100, Math.round((p.downloaded / p.total) * 100))
})

/** 阶段提示文案 */
const depsProgressHint = computed(() => {
  if (depsPhase.value === 'error') return depsStore.error || t('settings.depsInstallFailed')
  if (depsStore.progress?.message) return depsStore.progress.message
  return t('settings.depsPreparing')
})

/** 重新检测 */
async function depsRefresh(): Promise<void> {
  await depsStore.refresh()
}

/** 安装 / 更新解码组件 */
async function depsInstall(): Promise<void> {
  await depsStore.install()
}

/** 从本地 zip 安装依赖包（用户自行下载后选择） */
async function depsInstallLocal(): Promise<void> {
  const picked = await pickDepsPackage()
  if (!picked) return
  await depsStore.installLocal(picked)
}

/** 打开公共依赖目录 */
async function depsOpenFolder(): Promise<void> {
  await depsStore.openFolder()
}

/** 卸载解码组件 */
async function depsRemove(): Promise<void> {
  await depsStore.remove()
}

// 与模板兼容的状态别名（模板变量名保持不变，零改动）
const updateStatus = computed(() => {
  if (!updaterStore.available) return 'unsupported'
  switch (updaterStore.phase) {
    case 'checking':
      return 'checking'
    case 'available':
      return 'available'
    case 'downloading':
      return 'downloading'
    case 'installing':
      return 'downloading'
    case 'error':
      return 'error'
    default:
      return updaterStore.update ? 'available' : 'up-to-date'
  }
})

const availableUpdate = computed(() => updaterStore.update)
const updateError = computed(() => updaterStore.error)
const updateProgress = computed(() => updaterStore.progress)

/** 下载百分比（服务端未返回总大小时为 null，只显示已下载量） */
const updatePercent = computed<number | null>(() => {
  const { downloaded, total } = updateProgress.value
  if (!total || total <= 0) return null
  return Math.min(100, Math.round((downloaded / total) * 100))
})

/** 已下载/总大小文本（MB） */
const updateSizeText = computed(() => {
  const toMb = (bytes: number): string => (bytes / 1024 / 1024).toFixed(1)
  const { downloaded, total } = updateProgress.value
  return total
    ? `${toMb(downloaded)} / ${toMb(total)} MB`
    : `${toMb(downloaded)} MB`
})

async function checkUpdates(): Promise<void> {
  await updaterStore.manualCheck()
}

async function installUpdate(): Promise<void> {
  await updaterStore.confirmInstall()
}

/**
 * 打开 TPlayer 官网（用系统默认浏览器）。
 *
 * 走后端命令而非 <a href>：Tauri v2 会拦截 WebView 内的外部导航，
 * 由后端调系统默认浏览器打开才可靠（见 src-tauri/src/website.rs）。
 */
async function openOfficialSite(): Promise<void> {
  try {
    await appService.openWebsite()
  } catch (cause) {
    toastStore.error(t('settings.openOfficialFailed') + normalizeError(cause).message)
  }
}

async function resetSettings(): Promise<void> {
  const confirmed = await dialogStore.confirm({
    title: t('settings.resetConfirm'),
    message: t('settings.resetDesc'),
    confirmText: t('settings.resetConfirmText'),
    danger: true,
  })
  if (!confirmed) return
  await settingsStore.reset()
  toastStore.success(t('settings.resetDone'))
}

/** 复制版本信息（应用名 + 版本 + 运行环境）到剪贴板 */
async function copyVersionInfo(): Promise<void> {
  const text = `TPlayer v${appVersion} · ${isTauri ? t('settings.runtimeTauri') : t('settings.runtimeBrowser')}`
  try {
    await navigator.clipboard.writeText(text)
    toastStore.success(t('settings.versionCopied'))
  } catch {
    toastStore.error(t('settings.copyFailed'))
  }
}
</script>

<template>
  <section class="settings">
    <!-- 左侧分组页签 -->
    <nav class="settings__nav" :aria-label="t('settings.navLabel')">
      <template v-for="group in TAB_GROUPS" :key="group.key">
        <p class="settings__nav-group">{{ t(group.labelKey) }}</p>
        <button
          v-for="tab in group.tabs"
          :key="tab.key"
          type="button"
          class="settings__nav-item"
          :class="{ 'settings__nav-item--active': activeTab === tab.key }"
          @click="selectTab(tab.key)"
        >
          <Icon :name="tab.icon" :size="17" />
          {{ t(tab.labelKey) }}
        </button>
      </template>
    </nav>

    <!-- 右侧内容 -->
    <div class="settings__content">
      <!-- 外观：语言 / 主题 / 氛围 -->
      <div v-if="activeTab === 'appearance'" class="panel">
        <section class="setting-card">
          <h2 class="setting-card__title">{{ t('settings.language') }}</h2>
          <p class="setting-card__desc">{{ t('settings.languageDesc') }}</p>
          <div class="setting-row setting-row--end">
            <select class="lang-select" :value="currentLocale" @change="onLocaleChange">
              <option v-for="loc in availableLocales" :key="loc.code" :value="loc.code">
                {{ loc.nativeName }}
              </option>
            </select>
          </div>
        </section>

        <section class="setting-card">
          <h2 class="setting-card__title">{{ t('settings.theme') }}</h2>
          <p class="setting-card__desc">{{ t('settings.themeDesc') }}</p>
          <div class="theme-options">
            <button
              type="button"
              class="theme-option theme-option--active"
              :aria-label="themeToggleLabel"
              :title="themeToggleLabel"
              @click="toggleTheme()"
            >
              <Icon :name="themeIcon" :size="20" />
              <span>{{ themeLabel }}</span>
              <span class="theme-option__hint">{{ themeToggleLabel }}</span>
            </button>
          </div>
        </section>

        <section class="setting-card">
          <h2 class="setting-card__title">{{ t('settings.coverAccent') }}</h2>
          <p class="setting-card__desc">{{ t('settings.coverAccentDesc') }}</p>
          <div class="setting-row setting-row--end">
            <input
              type="checkbox"
              class="switch"
              :checked="isCoverAccentEnabled"
              @change="setCoverAccent(($event.target as HTMLInputElement).checked)"
            />
          </div>
        </section>

        <section class="setting-card">
          <h2 class="setting-card__title">{{ t('settings.windowCloseTitle') }}</h2>
          <p class="setting-card__desc">{{ t('settings.windowCloseDesc') }}</p>
          <label class="setting-row">
            <span class="setting-row__text">
              <span class="setting-row__label">{{ t('settings.quitOnClose') }}</span>
              <span class="setting-row__hint">{{ t('settings.quitOnCloseHint') }}</span>
            </span>
            <input
              type="checkbox"
              class="switch"
              :checked="settingsStore.quitOnClose"
              @change="settingsStore.setQuitOnClose(($event.target as HTMLInputElement).checked)"
            />
          </label>
        </section>
      </div>

      <!-- 歌词 + 在线服务 -->
      <div v-else-if="activeTab === 'lyrics'" class="panel">
        <section class="setting-card">
          <h2 class="setting-card__title">{{ t('settings.lyricsTitle') }}</h2>
          <p class="setting-card__desc">{{ t('settings.lyricsDesc1') }}</p>
          <p class="setting-card__desc">{{ t('settings.lyricsDesc2') }}</p>

          <p class="setting-card__desc">{{ t('settings.onlineDesc') }}</p>
          <label class="setting-row">
            <span class="setting-row__text">
              <span class="setting-row__label">{{ t('settings.onlineEnable') }}</span>
              <span class="setting-row__hint">{{ t('settings.onlineEnableHint') }}</span>
            </span>
            <input
              type="checkbox"
              class="switch"
              :checked="settingsStore.online.enabled"
              @change="settingsStore.setOnlineEnabled(($event.target as HTMLInputElement).checked)"
            />
          </label>
          <label class="setting-row" :class="{ 'setting-row--disabled': !settingsStore.online.enabled }">
            <span class="setting-row__text">
              <span class="setting-row__label">{{ t('settings.allowUpload') }}</span>
              <span class="setting-row__hint">{{ t('settings.allowUploadHint') }}</span>
            </span>
            <input
              type="checkbox"
              class="switch"
              :checked="settingsStore.online.allowLibraryUpload"
              :disabled="!settingsStore.online.enabled"
              @change="settingsStore.setAllowLibraryUpload(($event.target as HTMLInputElement).checked)"
            />
          </label>
          <div class="setting-row setting-row--column" :class="{ 'setting-row--disabled': !settingsStore.online.enabled }">
            <span class="setting-row__text">
              <span class="setting-row__label">{{ t('settings.provider') }}</span>
            </span>
            <select
              class="select"
              :value="settingsStore.online.provider"
              disabled
              :title="t('settings.providerMore')"
            >
              <option value="netease">{{ t('settings.providerNetease') }}</option>
              <option value="none">{{ t('settings.providerNone') }}</option>
            </select>
          </div>

          <div class="panel__actions">
            <BaseButton variant="subtle" size="sm" @click="clearLyricsCache">
              <Icon name="trash-2" :size="15" />
              {{ t('settings.clearLyricsCache') }}
            </BaseButton>
          </div>
        </section>
      </div>

      <!-- 播放：交叉淡化 / 音量归一化 -->
      <div v-else-if="activeTab === 'playback'" class="panel">
        <section class="setting-card">
          <h2 class="setting-card__title">{{ t('settings.crossfadeTitle') }}</h2>
          <p class="setting-card__desc">{{ t('settings.crossfadeDesc') }}</p>
          <label class="setting-row">
            <span class="setting-row__text">
              <span class="setting-row__label">{{ t('settings.crossfadeEnable') }}</span>
              <span class="setting-row__hint">{{ t('settings.crossfadeEnableHint') }}</span>
            </span>
            <input
              type="checkbox"
              class="switch"
              :checked="settingsStore.crossfade.enabled"
              @change="settingsStore.setCrossfadeEnabled(($event.target as HTMLInputElement).checked)"
            />
          </label>
          <div
            class="setting-row setting-row--column"
            :class="{ 'setting-row--disabled': !settingsStore.crossfade.enabled }"
          >
            <span class="setting-row__text">
              <span class="setting-row__label">{{ t('settings.crossfadeDuration') }}</span>
              <span class="setting-row__hint">{{ t('settings.crossfadeDurationHint') }}</span>
            </span>
            <input
              type="range"
              class="range"
              min="0"
              max="3000"
              step="100"
              :value="settingsStore.crossfade.duration_ms"
              :disabled="!settingsStore.crossfade.enabled"
              @input="
                settingsStore.setCrossfadeDuration(
                  Number(($event.target as HTMLInputElement).value),
                )
              "
            />
            <span class="setting-row__value u-tabular">
              {{ (settingsStore.crossfade.duration_ms / 1000).toFixed(1) }}s
            </span>
          </div>
        </section>

        <section class="setting-card">
          <h2 class="setting-card__title">{{ t('settings.normalizeTitle') }}</h2>
          <p class="setting-card__desc">{{ t('settings.normalizeDesc') }}</p>
          <label class="setting-row">
            <span class="setting-row__text">
              <span class="setting-row__label">{{ t('settings.normalizeEnable') }}</span>
              <span class="setting-row__hint">{{ t('settings.normalizeEnableHint') }}</span>
            </span>
            <input
              type="checkbox"
              class="switch"
              :checked="settingsStore.replaygain.enabled"
              @change="settingsStore.setReplaygainEnabled(($event.target as HTMLInputElement).checked)"
            />
          </label>
        </section>

        <section class="setting-card">
          <h2 class="setting-card__title">{{ t('settings.audioQualityBadgeTitle') }}</h2>
          <p class="setting-card__desc">{{ t('settings.audioQualityBadgeDesc') }}</p>
          <label class="setting-row">
            <span class="setting-row__text">
              <span class="setting-row__label">{{ t('settings.audioQualityBadgeEnable') }}</span>
              <span class="setting-row__hint">{{ t('settings.audioQualityBadgeEnableHint') }}</span>
            </span>
            <input
              type="checkbox"
              class="switch"
              :checked="settingsStore.showAudioQuality"
              @change="settingsStore.setShowAudioQuality(($event.target as HTMLInputElement).checked)"
            />
          </label>
        </section>
      </div>

      <!-- 媒体库：曲库目录 / 启动行为 / 目录自动监控 -->
      <div v-else-if="activeTab === 'library'" class="panel">
        <section class="setting-card">
          <h2 class="setting-card__title">{{ t('settings.dirTitle') }}</h2>
          <p class="setting-card__desc">{{ t('settings.dirDesc') }}</p>
          <ul v-if="settingsStore.libraryDirs.length > 0" class="dir-list">
            <li v-for="dir in settingsStore.libraryDirs" :key="dir" class="dir-list__item">
              <Icon name="folder" :size="17" />
              <span class="dir-list__path" :title="dir">{{ dir }}</span>
              <button
                type="button"
                class="dir-list__remove"
                :aria-label="t('settings.removeDir')"
                :title="t('settings.removeDir')"
                @click="removeLibraryDir(dir)"
              >
                <Icon name="trash-2" :size="15" />
              </button>
            </li>
          </ul>
          <p v-else class="panel__empty">{{ t('settings.noDir') }}</p>
          <div class="panel__actions">
            <BaseButton variant="primary" size="sm" @click="addLibraryDir">
              <Icon name="folder-plus" :size="15" />
              {{ t('settings.addDir') }}
            </BaseButton>
            <BaseButton variant="subtle" size="sm" :disabled="scanning" @click="scanNow">
              <Icon name="refresh" :size="15" />
              {{ scanning ? t('settings.scanning') : t('settings.scanNow') }}
            </BaseButton>
            <BaseButton
              v-if="scanning"
              variant="subtle"
              size="sm"
              :title="t('settings.stopScan')"
              @click="cancelScanNow"
            >
              <Icon name="close" :size="15" />
              {{ t('settings.cancelScan') }}
            </BaseButton>
          </div>
        </section>

        <section class="setting-card">
          <h2 class="setting-card__title">{{ t('settings.startupTitle') }}</h2>
          <label class="setting-row">
            <span class="setting-row__text">
              <span class="setting-row__label">{{ t('settings.scanOnStartup') }}</span>
              <span class="setting-row__hint">{{ t('settings.scanOnStartupHint') }}</span>
            </span>
            <input
              type="checkbox"
              class="switch"
              :checked="settingsStore.scanOnStartup"
              @change="settingsStore.setScanOnStartup(($event.target as HTMLInputElement).checked)"
            />
          </label>
          <label class="setting-row">
            <span class="setting-row__text">
              <span class="setting-row__label">{{ t('settings.watchEnable') }}</span>
              <span class="setting-row__hint">{{ t('settings.watchEnableHint') }}</span>
            </span>
            <input
              type="checkbox"
              class="switch"
              :checked="settingsStore.watch.enabled"
              @change="settingsStore.setWatchEnabled(($event.target as HTMLInputElement).checked)"
            />
          </label>
          <div v-if="isTauri && watcherStatus" class="setting-row setting-row--static">
            <span class="setting-row__text">
              <span class="setting-row__label">{{ t('settings.watchStatus') }}</span>
              <span class="setting-row__hint">
                {{
                  watcherStatus.dirs.length > 0
                    ? t('settings.watchStatusDirs', { count: watcherStatus.dirs.length })
                    : t('settings.watchStatusNone')
                }}
                <template v-if="watcherStatus.last_event_desc">
                  {{ t('settings.watchLast', { desc: watcherStatus.last_event_desc ?? '', time: watcherStatus.last_event_at ?? '' }) }}
                </template>
              </span>
            </span>
          </div>
          <div v-if="isTauri && !watcherStatus" class="setting-row setting-row--static">
            <span class="setting-row__text">
              <span class="setting-row__label">{{ t('settings.watchStatus') }}</span>
              <span class="setting-row__hint">{{ t('settings.watchUnavailable') }}</span>
            </span>
          </div>
        </section>
      </div>

      <!-- 收听报告（服务与工具组） -->
      <div v-else-if="activeTab === 'report'" class="panel">
        <StatsView embedded />
      </div>

      <!-- 工具：音频转换器 + 解码组件（可选依赖工具归并至此，避免散落在"更新"中） -->
      <div v-else-if="activeTab === 'tools'" class="settings__tools">
        <ConverterView />

        <!-- 解码组件（第三方可选依赖）：检测 / 安装 / 更新 / 打开目录 -->
        <section class="setting-card">
          <h2 class="setting-card__title">{{ t('settings.depsTitle') }}</h2>
          <p class="setting-card__desc">{{ t('settings.depsDesc') }}</p>

          <!-- 状态摘要 -->
          <div class="deps__summary" :class="{ 'deps__summary--warn': depsStatus && !depsStatus.ok }">
            <Icon
              :name="depsStatus && depsStatus.ok ? 'check' : 'info'"
              :size="15"
            />
            <span>{{ depsSummary }}</span>
          </div>

          <!-- 各工具明细 -->
          <ul v-if="depsStatus" class="deps__list">
            <li v-for="tool in depsStatus.tools" :key="tool.name" class="deps__item">
              <Icon
                :name="tool.installed ? 'check' : 'minus'"
                :size="14"
                :class="tool.installed ? 'deps__ok' : 'deps__miss'"
              />
              <span class="deps__name">{{ depsToolLabel(tool.name) }}</span>
              <span class="deps__purpose">{{ depsToolPurpose(tool.name) }}</span>
              <span class="deps__source">
                {{ tool.installed ? depsSourceLabel(tool.source) : t('settings.notInstalled') }}
              </span>
            </li>
          </ul>

          <!-- 安装进度 -->
          <div v-if="depsInstalling || depsPhase === 'error'" class="deps__progress">
            <div class="deps__hint" :class="{ 'deps__hint--error': depsPhase === 'error' }">
              <Icon :name="depsPhase === 'error' ? 'info' : 'loader'" :size="15" :spin="depsPhase !== 'error'" />
              <span>{{ depsProgressHint }}</span>
            </div>
            <div v-if="depsPhase !== 'error'" class="update__progress">
              <div class="update__progress-bar">
                <div
                  class="update__progress-fill"
                  :style="{ width: depsPercent === null ? '100%' : `${depsPercent}%` }"
                  :class="{ 'update__progress-fill--indeterminate': depsPercent === null }"
                />
              </div>
            </div>
          </div>

          <p class="deps__dir">
            {{ t('settings.depsDir') }}<code>{{ depsStatus?.depsDir ?? '—' }}</code>
          </p>

          <div class="panel__actions">
            <BaseButton
              variant="primary"
              size="sm"
              :disabled="depsInstalling"
              @click="depsInstall"
            >
              <Icon name="download" :size="15" />
              {{ depsStatus?.installedPack ? t('settings.depsUpdate') : t('settings.depsInstall') }}
            </BaseButton>
            <BaseButton variant="subtle" size="sm" :disabled="depsChecking" @click="depsRefresh">
              <Icon name="refresh" :size="15" />
              {{ t('settings.depsRecheck') }}
            </BaseButton>
            <BaseButton variant="subtle" size="sm" :disabled="depsInstalling" @click="depsInstallLocal">
              <Icon name="folder" :size="15" />
              {{ t('settings.depsInstallLocal') }}
            </BaseButton>
            <BaseButton variant="subtle" size="sm" @click="depsOpenFolder">
              <Icon name="folder" :size="15" />
              {{ t('settings.depsOpenFolder') }}
            </BaseButton>
            <BaseButton
              v-if="depsStatus?.installedPack"
              variant="ghost"
              size="sm"
              :disabled="depsInstalling"
              @click="depsRemove"
            >
              {{ t('settings.depsUninstall') }}
            </BaseButton>
          </div>
        </section>
      </div>

      <!-- 实验室：实验功能开关（默认关闭、与现有功能隔离，可上线 / 废除） -->
      <div v-else-if="activeTab === 'lab'" class="panel">
        <section class="setting-card">
          <h2 class="setting-card__title">{{ t('settings.lab') }}</h2>
          <LabView />
        </section>
      </div>

      <!-- 更新 -->
      <div v-else-if="activeTab === 'update'" class="panel">
        <section class="setting-card">
          <h2 class="setting-card__title">{{ t('settings.updateTitle') }}</h2>
          <p class="setting-card__desc">{{ t('settings.updateDesc', { version: appVersion }) }}</p>

          <!-- 未检查 / 检查中 / 已是最新 / 失败 / 浏览器环境 -->
          <div v-if="updateStatus !== 'available' && updateStatus !== 'downloading'" class="update">
            <div v-if="updateStatus === 'checking'" class="update__hint">
              <Icon name="loader" :size="15" spin />
              {{ t('settings.checking') }}
            </div>
            <div v-else-if="updateStatus === 'up-to-date'" class="update__hint update__hint--ok">
              <Icon name="check" :size="15" />
              {{ t('settings.upToDate') }}
            </div>
            <div v-else-if="updateStatus === 'error'" class="update__hint update__hint--error">
              <Icon name="info" :size="15" />
              {{ updateError ? t('settings.checkFailedWith', { error: updateError }) : t('settings.checkFailed') }}
            </div>
            <div v-else-if="updateStatus === 'unsupported'" class="update__hint">
              <Icon name="info" :size="15" />
              {{ t('settings.unsupported') }}
            </div>

            <div class="panel__actions">
              <BaseButton
                variant="primary"
                size="sm"
                :disabled="updateStatus === 'checking'"
                @click="checkUpdates"
              >
                <Icon name="refresh" :size="15" />
                {{ updateStatus === 'error' ? t('settings.retry') : t('settings.checkUpdate') }}
              </BaseButton>
            </div>
          </div>

          <!-- 发现新版本 -->
          <div v-else-if="updateStatus === 'available' && availableUpdate" class="update">
            <div class="update__card">
              <div class="update__version">
                {{ t('settings.newVersion', { version: availableUpdate.version }) }}
                <span class="update__date">{{ availableUpdate.date }}</span>
              </div>
              <pre v-if="availableUpdate.body" class="update__notes">{{ availableUpdate.body }}</pre>
            </div>
            <div class="panel__actions">
              <BaseButton variant="primary" size="sm" @click="installUpdate">
                <Icon name="download" :size="15" />
                {{ t('settings.downloadInstall') }}
              </BaseButton>
            </div>
          </div>

          <!-- 下载安装中 -->
          <div v-else class="update">
            <div class="update__hint">
              <Icon name="loader" :size="15" spin />
              {{ t('settings.downloading') }}
            </div>
            <div class="update__progress">
              <div class="update__progress-bar">
                <div
                  class="update__progress-fill"
                  :style="{ width: updatePercent === null ? '100%' : `${updatePercent}%` }"
                  :class="{ 'update__progress-fill--indeterminate': updatePercent === null }"
                />
              </div>
              <div class="update__progress-meta u-tabular">
                <span>{{ updatePercent === null ? t('settings.downloadProgress') : `${updatePercent}%` }}</span>
                <span>{{ updateSizeText }}</span>
              </div>
            </div>
            <p class="setting-card__desc">{{ t('settings.installNote') }}</p>
          </div>
        </section>
      </div>

      <!-- 关于 -->
      <div v-else class="panel">
        <section class="setting-card">
          <h2 class="setting-card__title">TPlayer</h2>
          <p class="about__tagline">{{ t('settings.aboutTagline') }}</p>

          <p class="about__intro">{{ t('settings.aboutIntro') }}</p>

          <h3 class="about__heading">{{ t('settings.aboutFeatures') }}</h3>
          <ul class="about__list">
            <li>
              <strong>{{ t('settings.featFormatTitle') }}</strong>{{ t('common.colon') }}{{ t('settings.featFormatDesc') }}
            </li>
            <li>
              <strong>{{ t('settings.featPlaybackTitle') }}</strong>{{ t('common.colon') }}{{ t('settings.featPlaybackDesc') }}
            </li>
            <li>
              <strong>{{ t('settings.featLibraryTitle') }}</strong>{{ t('common.colon') }}{{ t('settings.featLibraryDesc') }}
            </li>
            <li>
              <strong>{{ t('settings.featLyricsTitle') }}</strong>{{ t('common.colon') }}{{ t('settings.featLyricsDesc') }}
            </li>
            <li>
              <strong>{{ t('settings.featHistoryTitle') }}</strong>{{ t('common.colon') }}{{ t('settings.featHistoryDesc') }}
            </li>
            <li>
              <strong>{{ t('settings.featPerfTitle') }}</strong>{{ t('common.colon') }}{{ t('settings.featPerfDesc') }}
            </li>
            <li>
              <strong>{{ t('settings.featUpdateTitle') }}</strong>{{ t('common.colon') }}{{ t('settings.featUpdateDesc') }}
            </li>
            <li>
              <strong>{{ t('settings.featLabTitle') }}</strong>{{ t('common.colon') }}{{ t('settings.featLabDesc') }}
            </li>
          </ul>

          <h3 class="about__heading">{{ t('settings.aboutPrivacy') }}</h3>
          <p class="about__intro">{{ t('settings.aboutPrivacyText') }}</p>

          <h3 class="about__heading">{{ t('settings.aboutThanks') }}</h3>
          <p class="about__intro">{{ t('settings.aboutThanksText1') }}</p>
          <p class="about__intro">{{ t('settings.aboutThanksText2') }}</p>
        </section>

        <!-- 资源与操作：官网 / 检查更新 / 复制版本信息 -->
        <section class="setting-card">
          <h2 class="setting-card__title">{{ t('settings.resourcesTitle') }}</h2>
          <p class="setting-card__desc">{{ t('settings.resourcesDesc') }}</p>
          <div class="panel__actions">
            <BaseButton variant="subtle" size="sm" @click="openOfficialSite">
              <Icon name="external-link" :size="15" />
              {{ t('settings.openOfficial') }}
            </BaseButton>
            <BaseButton variant="subtle" size="sm" @click="checkUpdates">
              <Icon name="refresh" :size="15" />
              {{ t('settings.checkUpdate') }}
            </BaseButton>
            <BaseButton variant="subtle" size="sm" @click="copyVersionInfo">
              <Icon name="copy" :size="15" />
              {{ t('settings.copyInfo') }}
            </BaseButton>
          </div>
        </section>

        <!-- 键盘快捷键（当前已接线的布局快捷键） -->
        <section class="setting-card">
          <h2 class="setting-card__title">{{ t('settings.shortcutsTitle') }}</h2>
          <p class="setting-card__desc">{{ t('settings.shortcutsDesc') }}</p>
          <dl class="about__shortcuts">
            <div class="about__row">
              <dt><kbd>Ctrl</kbd><kbd>B</kbd></dt>
              <dd>{{ t('settings.shortcutSidebar') }}</dd>
            </div>
            <div class="about__row">
              <dt><kbd>Ctrl</kbd><kbd>U</kbd></dt>
              <dd>{{ t('settings.shortcutLyrics') }}</dd>
            </div>
            <div class="about__row">
              <dt><kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>U</kbd></dt>
              <dd>{{ t('settings.shortcutQueue') }}</dd>
            </div>
          </dl>
        </section>

        <section class="setting-card">
          <dl class="about">
            <div class="about__row">
              <dt>{{ t('settings.version') }}</dt>
              <dd class="u-tabular">{{ appVersion }}</dd>
            </div>
            <div class="about__row">
              <dt>{{ t('settings.runtime') }}</dt>
              <dd>{{ isTauri ? t('settings.runtimeTauri') : t('settings.runtimeBrowser') }}</dd>
            </div>
            <div class="about__row">
              <dt>{{ t('settings.techStack') }}</dt>
              <dd>Tauri 2 · Vue 3 · TypeScript · Rust</dd>
            </div>
          </dl>
          <div class="panel__actions">
            <BaseButton variant="danger" size="sm" @click="resetSettings">
              <Icon name="trash-2" :size="15" />
              {{ t('settings.restoreDefaults') }}
            </BaseButton>
          </div>
        </section>
      </div>
    </div>
  </section>
</template>

<style scoped>
.settings {
  display: grid;
  grid-template-columns: 200px minmax(0, 1fr);
  gap: var(--space-5);
  /* 内外边距由外壳 .app-shell__view-pad 统一提供，避免双倍留白 */
}

/* 工具页签：收听报告入口卡片 + 转换器占满内容列（不受 .panel 560px 限宽约束） */
.settings__tools {
  min-width: 0;
}

.tool-entry {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  padding: var(--space-3) var(--space-4);
  margin-bottom: var(--space-3);
  background: var(--color-bg-surface);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-lg);
  color: var(--color-text-primary);
  cursor: pointer;
  text-align: left;
}

.tool-entry:hover {
  background: var(--color-bg-hover);
  border-color: var(--color-border);
}

.tool-entry:focus-visible {
  outline: 2px solid var(--color-focus-ring);
  outline-offset: 2px;
}

.tool-entry__icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  flex: 0 0 auto;
  border-radius: var(--radius-md);
  background: var(--color-brand-subtle);
  color: var(--color-brand);
}

.tool-entry__meta {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.tool-entry__title {
  font-size: var(--font-size-md);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-md);
}

.tool-entry__desc {
  font-size: var(--font-size-xs);
  line-height: var(--line-height-xs);
  color: var(--color-text-tertiary);
}

.tool-entry__chevron {
  color: var(--color-text-tertiary);
}

/* ---- 左侧导航（分组） ---- */
.settings__nav {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.settings__nav-group {
  margin: var(--space-3) var(--space-3) var(--space-1);
  font-size: var(--font-size-xs);
  line-height: 1;
  letter-spacing: 0.04em;
  color: var(--color-text-tertiary);
  user-select: none;
}

.settings__nav-group:first-child {
  margin-top: 0;
}

.settings__nav-item {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  background: transparent;
  border: none;
  border-left: 3px solid transparent;
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.settings__nav-item:hover {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}

.settings__nav-item--active {
  color: var(--color-brand);
  border-left-color: var(--color-brand);
  background-color: var(--color-bg-selected);
}

/* ---- 右侧面板 ---- */
.settings__content {
  min-width: 0;
}

.panel {
  max-width: 560px;
}

/* ---- 设置卡片（DESIGN §2.4：分组用卡片承载） ---- */
.setting-card {
  padding: var(--space-4) var(--space-5);
  margin-bottom: var(--space-4);
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-elevation-1);
}

.setting-card__title {
  margin: 0 0 var(--space-1);
  font-size: var(--font-size-md);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
}

.setting-card__desc {
  margin: 0 0 var(--space-4);
  font-size: var(--font-size-xs);
  line-height: var(--line-height-sm);
  color: var(--color-text-tertiary);
}

.panel__empty {
  margin: 0 0 var(--space-3);
  font-size: var(--font-size-xs);
  color: var(--color-text-disabled);
}

.panel__actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

/* ---- 曲库目录列表 ---- */
.dir-list {
  margin: 0 0 var(--space-3);
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.dir-list__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  color: var(--color-text-secondary);
  background-color: var(--color-bg-surface);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-sm);
}

.dir-list__path {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  font-size: var(--font-size-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dir-list__remove {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-h);
  height: var(--size-control-h);
  color: var(--color-text-tertiary);
  border-radius: var(--radius-xs);
}

.dir-list__remove:hover {
  color: var(--color-danger);
  background-color: var(--color-danger-bg);
}

/* ---- 设置行 ---- */
.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-3) 0;
  border-top: 1px solid var(--color-border-subtle);
}

.setting-row:first-child {
  border-top: none;
  padding-top: 0;
}

.setting-row--column {
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-2);
}

.setting-row--disabled {
  opacity: 0.5;
}

.setting-row--static {
  align-items: flex-start;
}

/* 仅含控件、无行内文案的行：控件靠右（避免与卡片标题/描述重复） */
.setting-row--end {
  justify-content: flex-end;
}

.setting-row__text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.setting-row__label {
  font-size: var(--font-size-sm);
  color: var(--color-text-primary);
}

.setting-row__hint {
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

/* ---- 开关 ---- */
.switch {
  position: relative;
  flex: 0 0 auto;
  width: 38px;
  height: 22px;
  margin: 0;
  appearance: none;
  -webkit-appearance: none;
  background-color: var(--color-bg-hover);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-full);
  cursor: pointer;
  transition: background-color var(--transition-colors);
}

.switch::after {
  content: '';
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  background-color: var(--color-text-inverse);
  border-radius: var(--radius-full);
  transition: transform var(--transition-colors);
}

.switch:checked {
  background-color: var(--color-brand-solid);
  border-color: var(--color-brand-solid);
}

/* 语言选择器：沿用主题色 token，匹配暗/亮主题 */
.lang-select {
  flex: 0 0 auto;
  min-width: 160px;
  padding: 6px 10px;
  font-size: var(--font-size-sm);
  color: var(--color-text-primary);
  background-color: var(--color-bg);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  cursor: pointer;
}
.lang-select:focus-visible {
  outline: 2px solid var(--color-brand-solid);
  outline-offset: 1px;
}

.switch:checked::after {
  transform: translateX(16px);
}

.switch:disabled {
  cursor: not-allowed;
}

.switch:focus-visible {
  outline: 2px solid var(--color-focus-ring);
  outline-offset: 2px;
}

/* ---- 滑块（交叉淡化时长） ---- */
.range {
  width: 100%;
  margin: 0;
  accent-color: var(--color-brand-solid);
  cursor: pointer;
}

.range:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.range:focus-visible {
  outline: 2px solid var(--color-focus-ring);
  outline-offset: 2px;
}

.setting-row__value {
  align-self: flex-end;
  font-size: var(--font-size-xs);
  color: var(--color-text-secondary);
}

/* ---- 下拉 ---- */
.select {
  width: 220px;
  padding: var(--space-2) var(--space-3);
  font-size: var(--font-size-sm);
  color: var(--color-text-primary);
  background-color: var(--color-bg-input);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
}

.select:focus-visible {
  outline: 2px solid var(--color-focus-ring);
  outline-offset: 1px;
}

/* ---- 主题卡片 ---- */
/* 单一主题控件（原为「深色 / 浅色」两个选项卡，收敛为一个切换按钮） */
.theme-options {
  display: flex;
  gap: var(--space-3);
}

.theme-option {
  position: relative;
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  padding: var(--space-4) var(--space-3);
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  background-color: var(--color-bg-surface);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-md);
  cursor: pointer;
  text-align: left;
}

.theme-option:hover {
  color: var(--color-text-primary);
  border-color: var(--color-border-strong);
}

.theme-option--active {
  color: var(--color-brand);
  border-color: var(--color-brand);
}

/* 控件内的当前主题名与「点击切换到 XX」提示 */
.theme-option__hint {
  margin-left: auto;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

/* ---- 关于 ---- */
.about {
  margin: 0 0 var(--space-4);
}

.about__tagline {
  margin: calc(-1 * var(--space-2)) 0 var(--space-3);
  font-size: var(--font-size-sm);
  color: var(--color-text-tertiary);
}

.about__intro {
  margin: 0 0 var(--space-4);
  font-size: var(--font-size-sm);
  line-height: 1.7;
  color: var(--color-text-secondary);
}

.about__heading {
  margin: var(--space-5) 0 var(--space-2);
  font-size: var(--font-size-sm);
  font-weight: 600;
  color: var(--color-text-primary);
}

.about__list {
  margin: 0 0 var(--space-2);
  padding-left: var(--space-5);
  font-size: var(--font-size-sm);
  line-height: 1.8;
  color: var(--color-text-secondary);
}

.about__list li::marker {
  color: var(--color-text-tertiary);
}

.about__row {
  display: flex;
  gap: var(--space-4);
  padding: var(--space-2) 0;
  font-size: var(--font-size-sm);
  border-top: 1px solid var(--color-border-subtle);
}

.about__row dt {
  flex: 0 0 96px;
  color: var(--color-text-tertiary);
}

.about__row dd {
  margin: 0;
  color: var(--color-text-primary);
}

.about__shortcuts .about__row dt {
  flex: 0 0 auto;
  min-width: 136px;
  display: flex;
  gap: 4px;
  align-items: center;
}

kbd {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 20px;
  padding: 2px 6px;
  font-family: var(--font-family-mono, monospace);
  font-size: 11px;
  line-height: 1.4;
  color: var(--color-text-primary);
  background: var(--color-bg-hover);
  border: 1px solid var(--color-border);
  border-bottom-width: 2px;
  border-radius: var(--radius-sm);
}

/* ---- 更新页签 ---- */
.update {
  margin-top: var(--space-3);
}

.update__hint {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-3);
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
}

.update__hint--ok {
  color: var(--color-success);
}

.update__hint--error {
  color: var(--color-danger);
}

.update__card {
  margin-bottom: var(--space-3);
  padding: var(--space-3) var(--space-4);
  background: var(--color-bg-surface);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-md);
}

.update__version {
  display: flex;
  align-items: baseline;
  gap: var(--space-3);
  font-size: var(--font-size-sm);
  font-weight: 600;
  color: var(--color-text-primary);
}

.update__date {
  font-size: var(--font-size-xs);
  font-weight: 400;
  color: var(--color-text-tertiary);
}

.update__notes {
  margin: var(--space-2) 0 0;
  font-family: inherit;
  font-size: var(--font-size-sm);
  line-height: 1.6;
  color: var(--color-text-secondary);
  white-space: pre-wrap;
  word-break: break-word;
}

.update__progress {
  margin: var(--space-3) 0;
}

.update__progress-bar {
  height: 6px;
  overflow: hidden;
  background: var(--color-bg-input);
  border-radius: var(--radius-pill, 999px);
}

.update__progress-fill {
  height: 100%;
  background: var(--color-brand);
  border-radius: inherit;
  transition: width 0.2s ease;
}

/* 无法获知总大小时的不定进度动画 */
.update__progress-fill--indeterminate {
  width: 40% !important;
  animation: update-progress-slide 1.2s ease-in-out infinite alternate;
}

/* ---- 解码组件（第三方可选依赖）------------------------------------ */
.deps__summary {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-3);
  padding: var(--space-2) var(--space-3);
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  background-color: var(--color-bg-base);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-sm);
}

.deps__summary--warn {
  color: var(--color-warning);
  border-color: var(--color-warning);
}

.deps__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0 0 var(--space-3);
  padding: 0;
  list-style: none;
}

.deps__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-sm);
}

.deps__ok {
  color: var(--color-success);
}

.deps__miss {
  color: var(--color-text-tertiary);
}

.deps__name {
  font-weight: var(--font-weight-medium);
  color: var(--color-text-primary);
}

.deps__purpose {
  flex: 1 1 auto;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

.deps__source {
  font-size: var(--font-size-xs);
  color: var(--color-text-secondary);
}

.deps__progress {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin-bottom: var(--space-3);
}

.deps__hint {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
}

.deps__hint--error {
  color: var(--color-danger);
}

.deps__dir {
  margin: 0 0 var(--space-3);
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
  word-break: break-all;
}

.deps__dir code {
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
}

@keyframes update-progress-slide {
  from {
    margin-left: 0;
  }
  to {
    margin-left: 60%;
  }
}

.update__progress-meta {
  display: flex;
  justify-content: space-between;
  margin-top: var(--space-2);
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}
</style>