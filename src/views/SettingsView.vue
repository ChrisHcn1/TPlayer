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
 *  - 支持 /settings/:tab 直达（外观 / 播放 / 媒体库 / 歌词 / 在线服务 / 工具 / 更新 / 关于）。
 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'

import pkg from '../../package.json'
import { useSettingsStore } from '../stores/settings'
import { useLibraryStore } from '../stores/library'
import { useTheme } from '../composables/useTheme'
import { useToastStore } from '../stores/toast'
import { useDialogStore } from '../stores/dialog'
import { pickDirectory } from '../services/dialog'
import { lyricsService } from '../services/lyrics'
import { isTauriRuntime, normalizeError } from '../services/ipc'
import { getWatcherStatus, subscribeWatcher, type WatcherStatus } from '../services/watcher'
import { updateService, type Update, type UpdateProgress } from '../services/update'
import BaseButton from '../components/common/BaseButton.vue'
import Icon from '../components/common/Icon.vue'
import ConverterView from './ConverterView.vue'
import StatsView from './StatsView.vue'
import { useLayout } from '../composables/useLayout'
import { useRoute, useRouter } from 'vue-router'

type SettingsTab =
  | 'appearance'
  | 'playback'
  | 'library'
  | 'report'
  | 'lyrics'
  | 'online'
  | 'tools'
  | 'window'
  | 'update'
  | 'about'

interface TabDef {
  key: SettingsTab
  label: string
  icon: string
}

interface TabGroup {
  key: string
  label: string
  tabs: ReadonlyArray<TabDef>
}

/** 左侧导航分组（每个组名即功能属性） */
const TAB_GROUPS: ReadonlyArray<TabGroup> = [
  {
    key: 'interface',
    label: '界面与外观',
    tabs: [
      { key: 'appearance', label: '外观', icon: 'sun' },
      { key: 'lyrics', label: '歌词', icon: 'lyrics' },
    ],
  },
  {
    key: 'playback-media',
    label: '播放与媒体',
    tabs: [
      { key: 'playback', label: '播放', icon: 'play' },
      { key: 'library', label: '媒体库', icon: 'disc' },
      { key: 'report', label: '收听报告', icon: 'clock' },
    ],
  },
  {
    key: 'service',
    label: '服务与工具',
    tabs: [
      { key: 'online', label: '在线服务', icon: 'globe' },
      { key: 'tools', label: '工具', icon: 'swap' },
    ],
  },
  {
    key: 'system',
    label: '系统',
    tabs: [
      { key: 'window', label: '窗口', icon: 'square' },
      { key: 'update', label: '更新', icon: 'download' },
      { key: 'about', label: '关于', icon: 'info' },
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

const DEFAULT_TAB: SettingsTab = 'appearance'
const activeTab = ref<SettingsTab>(DEFAULT_TAB)
const scanning = ref(false)

/** 目录自动监控状态（仅 Tauri 运行时后端提供；浏览器预览为 null） */
const watcherStatus = ref<WatcherStatus | null>(null)
let unlistenWatcher: (() => void) | null = null

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
})

/**
 * 主题控件：全应用只有标题栏一枚主题图标与设置页这一个主题控件，
 * 原先「深色 / 浅色」两个选项按钮收敛为单个切换按钮（点击即在两者间切换），
 * 图标随当前主题变化（深色=月亮，浅色=太阳）。
 */
const themeIcon = computed(() => (theme.value === 'dark' ? 'moon' : 'sun'))
const themeLabel = computed(() => (theme.value === 'dark' ? '深色' : '浅色'))
const themeToggleLabel = computed(() =>
  theme.value === 'dark' ? '切换到浅色主题' : '切换到深色主题',
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
    await persistLibraryDir(() => settingsStore.addLibraryDir(picked), '已添加曲库目录')
    return
  }
  // 浏览器（或选择器不可用）：降级为手动输入路径
  const input = await dialogStore.prompt({
    title: '添加曲库目录',
    message: isTauri.value
      ? '未能打开系统目录选择器，请手动输入目录路径。'
      : '浏览器预览环境不支持系统目录选择，请输入一个模拟目录路径。',
    placeholder: '例如 D:\\Music',
    validate: (value) => {
      const v = value.trim()
      if (!v) return '路径不能为空'
      // Windows 绝对路径需盘符（C:\）或 UNC（\\host\share）；网络共享若写成
      // 172.18.0.21/songs 这类非法形式，扫描时会静默失败，这里提前提示正确写法
      if (!/^[A-Za-z]:\\|\\\\/.test(v)) {
        return '请填绝对路径；Windows 网络共享需以 \\\\ 开头（如 \\\\172.18.0.21\\songs），或先映射为盘符（如 Y:\\songs）'
      }
      return null
    },
  })
  if (input === null) return
  const dir = input.trim()
  if (settingsStore.libraryDirs.includes(dir)) {
    toastStore.warning('该目录已在白名单中')
    return
  }
  await persistLibraryDir(() => settingsStore.addLibraryDir(dir), '已添加曲库目录')
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
    toastStore.error('保存曲库目录失败：' + normalizeError(cause).message)
  }
}

async function removeLibraryDir(dir: string): Promise<void> {
  const confirmed = await dialogStore.confirm({
    title: '移除曲库目录',
    message: `确定将「${dir}」从曲库白名单中移除吗？已扫描的曲目不受影响。`,
    confirmText: '移除',
  })
  if (!confirmed) return
  await persistLibraryDir(() => settingsStore.removeLibraryDir(dir), '已移除曲库目录')
}

async function scanNow(): Promise<void> {
  scanning.value = true
  try {
    await libraryStore.scan()
    if (libraryStore.error) toastStore.error('扫描失败：' + libraryStore.error)
    else toastStore.success('曲库扫描完成')
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
  toastStore.info('已请求取消扫描（已入库部分保留）')
}

async function clearLyricsCache(): Promise<void> {
  await lyricsService.clearCache()
  toastStore.success('歌词缓存已清除')
}

// ---- 应用更新（Tauri Updater） -------------------------------------------
type UpdateStatus =
  | 'idle' // 尚未检查
  | 'checking' // 正在检查
  | 'up-to-date' // 已是最新
  | 'available' // 发现新版本，等待用户确认
  | 'downloading' // 下载安装中
  | 'error' // 检查或下载失败
  | 'unsupported' // 浏览器预览环境

const updateStatus = ref<UpdateStatus>('idle')
const availableUpdate = ref<Update | null>(null)
const updateError = ref('')
const updateProgress = ref<UpdateProgress>({ downloaded: 0 })

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
  if (!isTauri.value) {
    updateStatus.value = 'unsupported'
    return
  }
  updateStatus.value = 'checking'
  updateError.value = ''
  try {
    const update = await updateService.check()
    availableUpdate.value = update
    updateStatus.value = update ? 'available' : 'up-to-date'
  } catch (error) {
    updateStatus.value = 'error'
    updateError.value = error instanceof Error ? error.message : String(error)
  }
}

async function installUpdate(): Promise<void> {
  updateStatus.value = 'downloading'
  updateError.value = ''
  updateProgress.value = { downloaded: 0 }
  try {
    // 下载完成后由安装器接管并自动重启；正常情况下此调用之后不再返回
    await updateService.downloadAndInstall((progress) => {
      updateProgress.value = progress
    })
  } catch (error) {
    updateStatus.value = 'error'
    updateError.value = error instanceof Error ? error.message : String(error)
  }
}

async function resetSettings(): Promise<void> {
  const confirmed = await dialogStore.confirm({
    title: '恢复默认设置',
    message: '将重置语言、曲库目录、在线服务等全部设置（不影响曲库文件、收藏与播放历史，主题保持不变）。',
    confirmText: '恢复默认',
    danger: true,
  })
  if (!confirmed) return
  await settingsStore.reset()
  toastStore.success('设置已恢复默认')
}
</script>

<template>
  <section class="settings">
    <!-- 左侧分组页签 -->
    <nav class="settings__nav" aria-label="设置页签">
      <template v-for="group in TAB_GROUPS" :key="group.key">
        <p class="settings__nav-group">{{ group.label }}</p>
        <button
          v-for="tab in group.tabs"
          :key="tab.key"
          type="button"
          class="settings__nav-item"
          :class="{ 'settings__nav-item--active': activeTab === tab.key }"
          @click="selectTab(tab.key)"
        >
          <Icon :name="tab.icon" :size="17" />
          {{ tab.label }}
        </button>
      </template>
    </nav>

    <!-- 右侧内容 -->
    <div class="settings__content">
      <!-- 外观：主题 / 氛围 -->
      <div v-if="activeTab === 'appearance'" class="panel">
        <section class="setting-card">
          <h2 class="setting-card__title">主题</h2>
          <p class="setting-card__desc">主题选择会立即生效并持久化。</p>
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
          <h2 class="setting-card__title">氛围</h2>
          <p class="setting-card__desc">让界面色调跟随当前歌曲封面，营造统一氛围。</p>
          <label class="setting-row">
            <span class="setting-row__text">
              <span class="setting-row__label">皮肤色跟随当前歌曲封面</span>
              <span class="setting-row__hint">从封面提取主色，接管按钮、选中态、进度条、歌词高亮等强调色，并为界面晕染同色氛围</span>
            </span>
            <input
              type="checkbox"
              class="switch"
              :checked="isCoverAccentEnabled"
              @change="setCoverAccent(($event.target as HTMLInputElement).checked)"
            />
          </label>
        </section>
      </div>

      <!-- 播放：交叉淡化 / 音量归一化 -->
      <div v-else-if="activeTab === 'playback'" class="panel">
        <section class="setting-card">
          <h2 class="setting-card__title">交叉淡化</h2>
          <p class="setting-card__desc">切换曲目时旧曲淡出、新曲淡入，避免突兀断音；实时生效并持久化。</p>
          <label class="setting-row">
            <span class="setting-row__text">
              <span class="setting-row__label">启用交叉淡化</span>
              <span class="setting-row__hint">关闭后切歌为即时切换</span>
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
              <span class="setting-row__label">淡入淡出时长</span>
              <span class="setting-row__hint">0 – 3 秒，默认 1 秒</span>
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
          <h2 class="setting-card__title">音量归一化</h2>
          <p class="setting-card__desc">
            按曲目响度自动调整播放音量：优先读取音频内嵌 ReplayGain 标签，无标签曲目首次播放时后台分析响度并持久化；开启后避免不同歌曲音量忽大忽小。
          </p>
          <label class="setting-row">
            <span class="setting-row__text">
              <span class="setting-row__label">启用音量归一化（ReplayGain）</span>
              <span class="setting-row__hint">目标响度 -14 LUFS，峰值防削波；实时生效并持久化</span>
            </span>
            <input
              type="checkbox"
              class="switch"
              :checked="settingsStore.replaygain.enabled"
              @change="settingsStore.setReplaygainEnabled(($event.target as HTMLInputElement).checked)"
            />
          </label>
        </section>
      </div>

      <!-- 媒体库：曲库目录 / 启动行为 / 目录自动监控 -->
      <div v-else-if="activeTab === 'library'" class="panel">
        <section class="setting-card">
          <h2 class="setting-card__title">曲库目录</h2>
          <p class="setting-card__desc">只有白名单内的目录才允许被扫描与读取。</p>
          <ul v-if="settingsStore.libraryDirs.length > 0" class="dir-list">
            <li v-for="dir in settingsStore.libraryDirs" :key="dir" class="dir-list__item">
              <Icon name="folder" :size="17" />
              <span class="dir-list__path" :title="dir">{{ dir }}</span>
              <button
                type="button"
                class="dir-list__remove"
                aria-label="移除目录"
                title="移除目录"
                @click="removeLibraryDir(dir)"
              >
                <Icon name="trash-2" :size="15" />
              </button>
            </li>
          </ul>
          <p v-else class="panel__empty">尚未添加曲库目录</p>
          <div class="panel__actions">
            <BaseButton variant="primary" size="sm" @click="addLibraryDir">
              <Icon name="folder-plus" :size="15" />
              添加目录
            </BaseButton>
            <BaseButton variant="subtle" size="sm" :disabled="scanning" @click="scanNow">
              <Icon name="refresh" :size="15" />
              {{ scanning ? '扫描中…' : '立即扫描' }}
            </BaseButton>
            <BaseButton
              v-if="scanning"
              variant="subtle"
              size="sm"
              title="停止扫描（已入库部分保留）"
              @click="cancelScanNow"
            >
              <Icon name="close" :size="15" />
              取消扫描
            </BaseButton>
          </div>
        </section>

        <section class="setting-card">
          <h2 class="setting-card__title">启动与同步</h2>
          <label class="setting-row">
            <span class="setting-row__text">
              <span class="setting-row__label">启动时自动扫描曲库</span>
              <span class="setting-row__hint">应用启动后在后台检查白名单目录的变化</span>
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
              <span class="setting-row__label">启用目录自动监控</span>
              <span class="setting-row__hint">默认开启；关闭后需手动「扫描曲库」更新</span>
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
              <span class="setting-row__label">监控状态</span>
              <span class="setting-row__hint">
                {{
                  watcherStatus.dirs.length > 0
                    ? `正在监控 ${watcherStatus.dirs.length} 个目录`
                    : '暂无已登记曲库目录'
                }}
                <template v-if="watcherStatus.last_event_desc">
                  ｜最近：{{ watcherStatus.last_event_desc }}（{{ watcherStatus.last_event_at }}）
                </template>
              </span>
            </span>
          </div>
          <div v-if="isTauri && !watcherStatus" class="setting-row setting-row--static">
            <span class="setting-row__text">
              <span class="setting-row__label">监控状态</span>
              <span class="setting-row__hint">状态查询暂不可用</span>
            </span>
          </div>
        </section>
      </div>

      <!-- 歌词 -->
      <div v-else-if="activeTab === 'lyrics'" class="panel">
        <section class="setting-card">
          <h2 class="setting-card__title">歌词来源</h2>
          <p class="setting-card__desc">
            歌词优先级为：音频内嵌歌词 &gt; 同名 .lrc 边车文件 &gt; 在线匹配（需开启在线服务，M3 落地）。
            逐行偏移校正在右侧歌词面板底部进行，按曲目单独保存。
          </p>
          <div class="panel__actions">
            <BaseButton variant="subtle" size="sm" @click="clearLyricsCache">
              <Icon name="trash-2" :size="15" />
              清除歌词缓存
            </BaseButton>
          </div>
        </section>
      </div>

      <!-- 收听报告（由"工具"页签独立为"播放与媒体"下的页签，置于媒体库之后） -->
      <div v-else-if="activeTab === 'report'" class="panel">
        <StatsView embedded />
      </div>

      <!-- 在线服务 -->
      <div v-else-if="activeTab === 'online'" class="panel">
        <section class="setting-card">
          <h2 class="setting-card__title">在线能力</h2>
          <p class="setting-card__desc">在线服务默认关闭；开启后按需使用，本地曲库默认不上报。</p>
          <label class="setting-row">
            <span class="setting-row__text">
              <span class="setting-row__label">启用在线服务</span>
              <span class="setting-row__hint">开启后可使用在线音源匹配与歌词匹配（M3 落地）</span>
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
              <span class="setting-row__label">允许上报本地曲库特征</span>
              <span class="setting-row__hint">用于提高在线匹配准确率；默认关闭，需单独同意</span>
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
              <span class="setting-row__label">服务提供方</span>
            </span>
            <select
              class="select"
              :value="settingsStore.online.provider"
              disabled
              title="更多服务提供方将在 M3 接入"
            >
              <option value="netease">网易云音乐</option>
              <option value="none">未选择</option>
            </select>
          </div>
        </section>
      </div>

      <!-- 工具：音频转换器（收听报告已独立为"播放与媒体"下的页签） -->
      <div v-else-if="activeTab === 'tools'" class="settings__tools">
        <ConverterView />
      </div>

      <!-- 窗口：关闭按钮行为 -->
      <div v-else-if="activeTab === 'window'" class="panel">
        <section class="setting-card">
          <h2 class="setting-card__title">关闭按钮</h2>
          <p class="setting-card__desc">决定点击窗口右上角 X 时的行为，立即生效并持久化。</p>
          <label class="setting-row">
            <span class="setting-row__text">
              <span class="setting-row__label">点击 X 直接关闭程序</span>
              <span class="setting-row__hint">
                开启后点击右上角 X 立即退出；关闭时（默认）仅隐藏到托盘后台驻留，可从托盘菜单恢复或退出
              </span>
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

      <!-- 更新 -->
      <div v-else-if="activeTab === 'update'" class="panel">
        <section class="setting-card">
          <h2 class="setting-card__title">检查更新</h2>
          <p class="setting-card__desc">
            当前版本：{{ appVersion }}。更新包经数字签名校验，仅从官方发布通道获取，
            下载与安装均需你手动确认，不会静默升级。
          </p>

          <!-- 未检查 / 检查中 / 已是最新 / 失败 / 浏览器环境 -->
          <div v-if="updateStatus !== 'available' && updateStatus !== 'downloading'" class="update">
            <div v-if="updateStatus === 'checking'" class="update__hint">
              <Icon name="loader" :size="15" spin />
              正在检查更新…
            </div>
            <div v-else-if="updateStatus === 'up-to-date'" class="update__hint update__hint--ok">
              <Icon name="check" :size="15" />
              已是最新版本
            </div>
            <div v-else-if="updateStatus === 'error'" class="update__hint update__hint--error">
              <Icon name="info" :size="15" />
              检查更新失败{{ updateError ? `：${updateError}` : '' }}
            </div>
            <div v-else-if="updateStatus === 'unsupported'" class="update__hint">
              <Icon name="info" :size="15" />
              浏览器预览环境不支持应用内升级，请使用安装版 TPlayer。
            </div>

            <div class="panel__actions">
              <BaseButton
                variant="primary"
                size="sm"
                :disabled="updateStatus === 'checking'"
                @click="checkUpdates"
              >
                <Icon name="refresh" :size="15" />
                {{ updateStatus === 'error' ? '重试' : '检查更新' }}
              </BaseButton>
            </div>
          </div>

          <!-- 发现新版本 -->
          <div v-else-if="updateStatus === 'available' && availableUpdate" class="update">
            <div class="update__card">
              <div class="update__version">
                新版本 v{{ availableUpdate.version }}
                <span class="update__date">{{ availableUpdate.date }}</span>
              </div>
              <pre v-if="availableUpdate.body" class="update__notes">{{ availableUpdate.body }}</pre>
            </div>
            <div class="panel__actions">
              <BaseButton variant="primary" size="sm" @click="installUpdate">
                <Icon name="download" :size="15" />
                下载并安装
              </BaseButton>
            </div>
          </div>

          <!-- 下载安装中 -->
          <div v-else class="update">
            <div class="update__hint">
              <Icon name="loader" :size="15" spin />
              正在下载更新…
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
                <span>{{ updatePercent === null ? '下载中' : `${updatePercent}%` }}</span>
                <span>{{ updateSizeText }}</span>
              </div>
            </div>
            <p class="setting-card__desc">下载完成后安装器将自动启动，应用会重启进入新版本。</p>
          </div>
        </section>
      </div>

      <!-- 关于 -->
      <div v-else class="panel">
        <section class="setting-card">
          <h2 class="setting-card__title">TPlayer</h2>
          <p class="about__tagline">本地优先的桌面音乐播放器</p>

          <p class="about__intro">
            TPlayer 是基于 Tauri 2、Vue 3 与 Rust 重新构建的本地优先桌面音乐播放器。
            所有曲目数据保存在本机 SQLite 数据库中，不依赖任何云服务即可完成曲库管理与播放；
            在线能力作为可选增强存在，默认关闭。
          </p>

          <h3 class="about__heading">主要特性</h3>
          <ul class="about__list">
            <li>支持 18 种音频格式：MP3、FLAC、WAV、AAC、OGG/Opus、M4A、WMA、APE、WV、TTA、TAK、DSF/DFF、DTS 等</li>
            <li>播放队列与交叉淡化、多种播放模式、托盘控制与全局快捷键</li>
            <li>专辑 / 艺术家自动聚合，CUE 分轨识别</li>
            <li>封面自动匹配（内嵌标签 → 同名图片 → 目录封面 → 可选在线补全），并按封面主色生成界面皮肤</li>
            <li>收藏、最近播放、自定义歌单，LRC 歌词与逐行偏移调整</li>
            <li>万级曲库的增量扫描与虚拟滚动，SQLite 持久化，启动与浏览更快</li>
            <li>内置应用内自动升级：更新包经数字签名校验，手动确认后安装，不会静默更新</li>
          </ul>

          <h3 class="about__heading">隐私说明</h3>
          <p class="about__intro">
            曲库文件、收藏、歌单与播放记录只存储在本机（<span class="u-tabular">%APPDATA%\com.tplayer</span>）。
            应用不会上传你的曲目信息；歌词、封面等在线增强默认关闭，即使手动开启，
            「上报本地曲库特征」仍需单独同意。
          </p>

          <h3 class="about__heading">致谢</h3>
          <p class="about__intro">
            感谢 rodio / Symphonia（音频解码）、lofty（标签读取）、FFmpeg（格式转换）、
            minisign（升级签名）、Vue 与 Tauri 等开源项目。
          </p>

          <dl class="about">
            <div class="about__row">
              <dt>版本</dt>
              <dd class="u-tabular">{{ appVersion }}</dd>
            </div>
            <div class="about__row">
              <dt>运行环境</dt>
              <dd>{{ isTauri ? 'Tauri 桌面环境' : '浏览器预览环境' }}</dd>
            </div>
            <div class="about__row">
              <dt>技术栈</dt>
              <dd>Tauri 2 · Vue 3 · TypeScript · Rust</dd>
            </div>
          </dl>
          <div class="panel__actions">
            <BaseButton variant="danger" size="sm" @click="resetSettings">
              <Icon name="trash-2" :size="15" />
              恢复默认设置
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