<script setup lang="ts">
/**
 * 设置视图（DESIGN §2.4）：左侧六个页签 —— 播放 / 外观 / 歌词 / 在线 / 更新 / 关于。
 *
 * 约定：
 *  - 主题唯一事实源 useTheme，本页只做切换 UI；
 *  - 曲库目录白名单经 settings store；Tauri 下走系统目录选择器（services/dialog），
 *    浏览器降级为 prompt 文本输入；
 *  - 在线模块默认关闭；即使开启，allowLibraryUpload 仍需单独同意（privacy-first）。
 */
import { computed, ref } from 'vue'

import pkg from '../../package.json'
import { useSettingsStore } from '../stores/settings'
import { useLibraryStore } from '../stores/library'
import { useTheme, type ThemeName } from '../composables/useTheme'
import { useToastStore } from '../stores/toast'
import { useDialogStore } from '../stores/dialog'
import { pickDirectory } from '../services/dialog'
import { lyricsService } from '../services/lyrics'
import { isTauriRuntime } from '../services/ipc'
import { updateService, type Update, type UpdateProgress } from '../services/update'
import BaseButton from '../components/common/BaseButton.vue'
import Icon from '../components/common/Icon.vue'
import ConverterView from './ConverterView.vue'
import { useLayout } from '../composables/useLayout'

type SettingsTab = 'playback' | 'appearance' | 'lyrics' | 'online' | 'tools' | 'update' | 'about'

const TABS: ReadonlyArray<{ key: SettingsTab; label: string; icon: string }> = [
  { key: 'playback', label: '播放与曲库', icon: 'play' },
  { key: 'appearance', label: '外观', icon: 'sun' },
  { key: 'lyrics', label: '歌词', icon: 'lyrics' },
  { key: 'online', label: '在线服务', icon: 'globe' },
  { key: 'tools', label: '工具', icon: 'swap' },
  { key: 'update', label: '更新', icon: 'download' },
  { key: 'about', label: '关于', icon: 'info' },
]

const settingsStore = useSettingsStore()
const libraryStore = useLibraryStore()
const toastStore = useToastStore()
const dialogStore = useDialogStore()
const { theme, applyTheme } = useTheme()
const { isCoverAccentEnabled, setCoverAccent } = useLayout()

const activeTab = ref<SettingsTab>('playback')
const scanning = ref(false)

const themeOptions: ReadonlyArray<{ key: ThemeName; label: string; icon: string }> = [
  { key: 'dark', label: '深色', icon: 'moon' },
  { key: 'light', label: '浅色', icon: 'sun' },
]

const isTauri = computed(() => isTauriRuntime())
const appVersion = computed(() => pkg.version)

async function addLibraryDir(): Promise<void> {
  const picked = await pickDirectory()
  if (picked) {
    await settingsStore.addLibraryDir(picked)
    toastStore.success('已添加曲库目录')
    return
  }
  // 浏览器（或选择器不可用）：降级为手动输入路径
  const input = await dialogStore.prompt({
    title: '添加曲库目录',
    message: isTauri.value
      ? '未能打开系统目录选择器，请手动输入目录路径。'
      : '浏览器预览环境不支持系统目录选择，请输入一个模拟目录路径。',
    placeholder: '例如 D:\\Music',
    validate: (value) => (value.trim().length === 0 ? '路径不能为空' : null),
  })
  if (input === null) return
  const dir = input.trim()
  if (settingsStore.libraryDirs.includes(dir)) {
    toastStore.warning('该目录已在白名单中')
    return
  }
  await settingsStore.addLibraryDir(dir)
  toastStore.success('已添加曲库目录')
}

async function removeLibraryDir(dir: string): Promise<void> {
  const confirmed = await dialogStore.confirm({
    title: '移除曲库目录',
    message: `确定将「${dir}」从曲库白名单中移除吗？已扫描的曲目不受影响。`,
    confirmText: '移除',
  })
  if (confirmed) await settingsStore.removeLibraryDir(dir)
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
    <!-- 左侧页签 -->
    <nav class="settings__nav" aria-label="设置页签">
      <button
        v-for="tab in TABS"
        :key="tab.key"
        type="button"
        class="settings__nav-item"
        :class="{ 'settings__nav-item--active': activeTab === tab.key }"
        @click="activeTab = tab.key"
      >
        <Icon :name="tab.icon" :size="17" />
        {{ tab.label }}
      </button>
    </nav>

    <!-- 右侧内容 -->
    <div class="settings__content">
      <!-- 播放与曲库 -->
      <div v-if="activeTab === 'playback'" class="panel">
        <h2 class="panel__title">曲库目录</h2>
        <p class="panel__desc">只有白名单内的目录才允许被扫描与读取。</p>
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
        </div>

        <h2 class="panel__title panel__title--gap">启动行为</h2>
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
      </div>

      <!-- 外观 -->
      <div v-else-if="activeTab === 'appearance'" class="panel">
        <h2 class="panel__title">主题</h2>
        <p class="panel__desc">主题选择会立即生效并持久化。</p>
        <div class="theme-options">
          <button
            v-for="option in themeOptions"
            :key="option.key"
            type="button"
            class="theme-option"
            :class="{ 'theme-option--active': theme === option.key }"
            @click="applyTheme(option.key)"
          >
            <Icon :name="option.icon" :size="20" />
            <span>{{ option.label }}</span>
            <Icon v-if="theme === option.key" name="check" :size="15" class="theme-option__check" />
          </button>
        </div>

        <h2 class="panel__title panel__title--gap">氛围</h2>
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
      </div>

      <!-- 歌词 -->
      <div v-else-if="activeTab === 'lyrics'" class="panel">
        <h2 class="panel__title">歌词来源</h2>
        <p class="panel__desc">
          歌词优先级为：音频内嵌歌词 &gt; 同名 .lrc 边车文件 &gt; 在线匹配（需开启在线服务，M3 落地）。
          逐行偏移校正在右侧歌词面板底部进行，按曲目单独保存。
        </p>
        <div class="panel__actions">
          <BaseButton variant="subtle" size="sm" @click="clearLyricsCache">
            <Icon name="trash-2" :size="15" />
            清除歌词缓存
          </BaseButton>
        </div>
      </div>

      <!-- 在线服务 -->
      <div v-else-if="activeTab === 'online'" class="panel">
        <h2 class="panel__title">在线能力</h2>
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
      </div>

      <!-- 工具：音频转换器（原侧栏"工具"入口移入设置页，命令面板仍可直达） -->
      <div v-else-if="activeTab === 'tools'" class="settings__tools">
        <ConverterView />
      </div>

      <!-- 更新 -->
      <div v-else-if="activeTab === 'update'" class="panel">
        <h2 class="panel__title">检查更新</h2>
        <p class="panel__desc">
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
            浏览器预览环境不支持应用内升级，请使用安装版 TPlayer Next。
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
          <p class="panel__desc">下载完成后安装器将自动启动，应用会重启进入新版本。</p>
        </div>
      </div>

      <!-- 关于 -->
      <div v-else class="panel">
        <h2 class="panel__title">TPlayer Next</h2>
        <p class="about__tagline">本地优先的桌面音乐播放器</p>

        <p class="about__intro">
          TPlayer Next 是 TPlayer 的下一代版本，基于 Tauri 2、Vue 3 与 Rust 重新构建。
          所有曲目数据保存在本机 SQLite 数据库中，不依赖任何云服务即可完成曲库管理与播放；
          在线能力作为可选增强存在，默认关闭。与旧版采用不同的应用标识与数据目录，可共存安装。
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
          曲库文件、收藏、歌单与播放记录只存储在本机（<span class="u-tabular">%APPDATA%\TPlayer Next</span>）。
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

/* 工具页签：转换器占满内容列（不受 .panel 560px 限宽约束） */
.settings__tools {
  min-width: 0;
}

/* ---- 左侧导航 ---- */
.settings__nav {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
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

.panel__title {
  margin: 0 0 var(--space-1);
  font-size: var(--font-size-md);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
}

.panel__title--gap {
  margin-top: var(--space-6);
}

.panel__desc {
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
  background-color: var(--color-bg-elevated);
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

.setting-row--column {
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-2);
}

.setting-row--disabled {
  opacity: 0.5;
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
.theme-options {
  display: flex;
  gap: var(--space-3);
}

.theme-option {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  width: 120px;
  padding: var(--space-4) var(--space-3);
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  background-color: var(--color-bg-elevated);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-md);
  cursor: pointer;
}

.theme-option:hover {
  color: var(--color-text-primary);
  border-color: var(--color-border-strong);
}

.theme-option--active {
  color: var(--color-brand);
  border-color: var(--color-brand);
}

.theme-option__check {
  position: absolute;
  top: var(--space-2);
  right: var(--space-2);
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
  background: var(--color-bg-elevated);
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
