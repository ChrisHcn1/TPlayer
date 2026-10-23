<script setup lang="ts">
/**
 * 根组件（布局壳）
 *
 * 职责边界（相对旧版的关键改动）：
 *  - 旧版 App.vue 达 8985 行，塞入了导航、播放、在线、设置等全部逻辑；
 *  - v2 根组件只做四件事：主题初始化 —— 设置恢复 —— 五区网格 + 路由出口 —— 全局快捷键注册；
 *  - 业务状态一律下沉到 stores / services，本文件不出现任何业务字段。
 *
 * 布局状态（§2.3 / §5.10）：
 *  - 侧栏折叠、右面板开合与页签，全部来自 composables/useLayout.ts 的模块级单例，
 *    本文件只做"状态 → 网格列宽"的映射（§2.3 折叠三原则 1：状态由外壳统一持有）；
 *  - 状态记忆的读取 / 校验 / 回落默认值均在 useLayout 内完成，外壳不重复实现。
 *
 * 主题单一事实源：useTheme（持久化键 THEME_STORAGE_KEY）。
 * 本文件不读 settings store 的 theme（该字段已移除），避免双写双源。
 */
import { computed, defineComponent, onMounted, onUnmounted, ref, watch } from 'vue'
import { RouterView, useRoute, type RouteMeta } from 'vue-router'

import { router } from './router'
import { useSettingsStore } from './stores/settings'
import { usePlayerStore } from './stores/player'
import { useLibraryStore } from './stores/library'
import { usePlaylistStore } from './stores/playlist'
import { useToastStore } from './stores/toast'
import { useStatsStore } from './stores/stats'
import { useTheme } from './composables/useTheme'
import { useLayout } from './composables/useLayout'
import { useCommandPalette } from './composables/useCommandPalette'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { listen } from '@tauri-apps/api/event'
import { updateService } from './services/update'
import { subscribeWatcher } from './services/watcher'
import { useTrackCover } from './composables/useTrackCover'
import { useCoverTheme } from './composables/useCoverTheme'
import type { Track } from './types'
import { isTauriRuntime, normalizeError } from './services/ipc'
import {
  DEFAULT_SHORTCUTS,
  LAYOUT_SHORTCUTS,
  useKeyboardShortcuts,
  type ShortcutAction,
} from './composables/useKeyboardShortcuts'
import TitleBar from './components/layout/TitleBar.vue'
import SideBar from './components/layout/SideBar.vue'
import RightPanel from './components/layout/RightPanel.vue'
import PlayerBar from './components/player/PlayerBar.vue'
import DesktopLyricsBar from './components/player/DesktopLyricsBar.vue'
import NowPlayingOverlay from './components/player/NowPlayingOverlay.vue'
import ToastContainer from './components/common/ToastContainer.vue'
import DialogHost from './components/common/DialogHost.vue'
import ContextMenuHost from './components/common/ContextMenuHost.vue'
import CommandPalette from './components/common/CommandPalette.vue'
import Icon from './components/common/Icon.vue'

const settingsStore = useSettingsStore()
const playerStore = usePlayerStore()
const libraryStore = useLibraryStore()
const playlistStore = usePlaylistStore()
const toastStore = useToastStore()
// applyTheme 为幂等操作：index.html 首屏脚本已按同一键写入 data-theme，
// 这里再执行一次是为了把运行时响应式状态与 DOM 对齐。
const { theme, applyTheme } = useTheme()
const {
  isSidebarCollapsed,
  isPanelOpen,
  isDesktopLyricsVisible,
  isNowPlayingOpen,
  toggleSidebar,
  togglePanelTab,
  toggleNowPlaying,
} = useLayout()
const commandPalette = useCommandPalette()
const route = useRoute()
const contentRef = ref<HTMLElement | null>(null)

/** 拖拽导入遮罩是否可见（仅 Tauri 环境注册拖放监听） */
const isDragging = ref(false)
/** 卸载时注销拖放监听 */
let unlistenDrag: (() => void) | undefined
/** 卸载时注销懒加载解析完成事件监听 */
let unlistenResolved: (() => void) | undefined

/**
 * 注册 Tauri 拖放事件：文件/目录拖入窗口时显示遮罩，松手后将真实路径交给曲库入库。
 * 浏览器预览无 Tauri 运行时，不注册（HTML5 drop 拿不到真实路径）。
 */
function setupDragImport(): void {
  if (!isTauriRuntime()) return
  void getCurrentWebview()
    .onDragDropEvent((event) => {
      const payload = event.payload
      switch (payload.type) {
        case 'enter':
        case 'over':
          isDragging.value = true
          break
        case 'drop':
          isDragging.value = false
          if (payload.paths.length > 0) {
            void loadDroppedAsPlaylist(payload.paths)
          }
          break
        default:
          // leave / exit
          isDragging.value = false
      }
    })
    .then((unlisten) => {
      unlistenDrag = unlisten
    })
}

/**
 * 拖入文件/目录：解析入库后自动建成「拖放列表」歌单并立即播放，
 * 满足"拖入即加载播放列表"的便捷操作；NAS / 网盘路径同样适用
 * （入库即被 compat 的 has_file_path 放行，无需白名单目录）。
 */
async function loadDroppedAsPlaylist(paths: string[]): Promise<void> {
  const ids = await libraryStore.importDropped(paths)
  if (ids.length === 0) {
    toastStore.info('拖入的内容中没有可识别的音频文件')
    return
  }
  try {
    await playlistStore.refresh()
    let playlist = playlistStore.playlists.find((p) => p.name === '拖放列表')
    if (!playlist) {
      await playlistStore.create('拖放列表')
      playlist = playlistStore.playlists.find((p) => p.name === '拖放列表')
    }
    if (!playlist) {
      toastStore.warning('创建播放列表失败')
      return
    }
    // 追加模式 + 全量去重：trackId 由后端按文件绝对路径哈希生成（唯一标识），
    // 已存在于播放列表中的重复文件自动过滤，列表既有内容不被清空重加
    const existing = new Set(playlist.trackIds)
    const newIds = ids.filter((id) => !existing.has(id))
    await playlistStore.addTracks(playlist.id, ids)
    if (newIds.length === 0) {
      toastStore.info(`所选音频均已存在于「拖放列表」，已跳过 ${ids.length} 个重复文件`)
      return
    }
    // 从首个新增曲目开始播放；队列上下文为去重合并后的完整列表
    const merged = playlistStore.byId(playlist.id)?.trackIds ?? [...playlist.trackIds, ...newIds]
    await playerStore.playFromContext(newIds[0], {
      source: 'playlist',
      sourceId: playlist.id,
      trackIds: merged,
    })
    toastStore.success(`已追加 ${newIds.length} 首到「拖放列表」并开始播放`)
  } catch (cause) {
    toastStore.warning(normalizeError(cause).message)
  }
}

onUnmounted(() => {
  unlistenDrag?.()
  unlistenResolved?.()
})

// 封面取色：跟随当前曲目封面维护全局强调色变量（播放条氛围光 / 放大层）
const { coverUrl: currentCoverUrl } = useTrackCover(() => playerStore.snapshot.trackId)
useCoverTheme(() => currentCoverUrl.value)

/**
 * 非缓存路由期间 KeepAlive 内部的占位组件。
 *
 * 为什么必须存在：KeepAlive 一旦被条件分支卸载，其缓存会一并销毁——
 * 若用 v-if 把 KeepAlive 与"非缓存路由"分支并列，进入详情页（meta.keepAlive=false）时
 * 列表页的缓存即告失效，返回后需要重建（§2.1 硬性规则 3、§2.7 滚动位置保持）。
 * 因此让 KeepAlive 常驻，非缓存路由期间只把它的子节点替换为空占位。
 */
const IdlePlaceholder = defineComponent({
  name: 'IdlePlaceholder',
  render: () => null,
})

/** meta 为弱类型（RouteMeta 索引签名），显式布尔守卫，避免脏 meta 直接参与分支 */
function isCacheable(meta: RouteMeta): boolean {
  return meta.keepAlive === true
}

/**
 * 五区网格列宽只在本文件映射，且取值一律是 theme.css 的 CSS 变量本身
 * （像素值只存在于 theme.css，遵守 §3 token 单一事实源）：
 *  - 侧栏列：--size-sidebar-w ↔ --size-sidebar-collapsed（折叠）；
 *  - 右面板列：--size-panel-w ↔ 0（关闭时为 0 而非 display:none，§2.1 硬性规则 3）。
 * 说明：紧凑/窄档（data-layout）下 theme.css 已将 --size-sidebar-w 覆写为折叠宽度，
 * 因此窄档下"展开"同样呈现 64px，无需在外壳重复判断断点。
 */
const shellColumns = computed(() => ({
  '--app-shell-sidebar-col': isSidebarCollapsed.value
    ? 'var(--size-sidebar-collapsed)'
    : 'var(--size-sidebar-w)',
  '--app-shell-panel-col': isPanelOpen.value ? 'var(--size-panel-w)' : '0px',
}))

/** 快退/快进步长（毫秒） */
const SEEK_STEP_MS = 5_000
/** 音量单步 */
const VOLUME_STEP = 0.1

/**
 * 全局快捷键（§6.1）：布局键 + 默认播放键全量注册。
 * 播放类动作经 player store；F（收藏）作用于当前曲目；Ctrl+K / / 唤起命令浮层。
 */
useKeyboardShortcuts({
  shortcuts: { ...DEFAULT_SHORTCUTS, ...LAYOUT_SHORTCUTS },
  handler: (action: ShortcutAction) => {
    const snapshot = playerStore.snapshot
    switch (action) {
      case 'toggle-sidebar':
        toggleSidebar()
        return
      case 'toggle-lyrics-panel':
        togglePanelTab('lyrics')
        return
      case 'toggle-queue-panel':
        togglePanelTab('queue')
        return
      case 'toggle-fullscreen-lyrics':
        // 封面放大层（大封面 + 叠加歌词）；无曲目时退化为打开歌词面板
        if (snapshot.trackId) toggleNowPlaying()
        else togglePanelTab('lyrics')
        return
      case 'focus-search':
        commandPalette.toggle()
        return
      case 'toggle-play':
        void playerStore.toggle()
        return
      case 'previous-track':
        void playerStore.previous()
        return
      case 'next-track':
        void playerStore.next()
        return
      case 'seek-backward':
        void playerStore.seek(snapshot.positionMs - SEEK_STEP_MS)
        return
      case 'seek-forward':
        void playerStore.seek(snapshot.positionMs + SEEK_STEP_MS)
        return
      case 'volume-up':
        void playerStore.setVolume(snapshot.volume + VOLUME_STEP)
        return
      case 'volume-down':
        void playerStore.setVolume(snapshot.volume - VOLUME_STEP)
        return
      case 'toggle-mute':
        void playerStore.setMuted(!snapshot.muted)
        return
      case 'cycle-play-mode':
        void playerStore.cycleMode()
        return
      case 'toggle-favorite':
        if (snapshot.trackId) void libraryStore.toggleFavorite(snapshot.trackId)
        return
      default:
        return
    }
  },
})

/**
 * 路由切换后内容区回到顶部。
 * M0 说明：滚动容器是外壳的 <main>（各视图自身不带滚动），若不重置，
 * 从长列表的深滚位置切到另一个视图会"以已滚到底部的位置"出现；
 * §2.7 的"按视图保存 / 恢复滚动位置"需要把滚动容器下移到各视图内部，属 M1，
 * 届时本处改为接入滚动记忆机制。
 */
watch(
  () => route.fullPath,
  () => {
    if (contentRef.value !== null) contentRef.value.scrollTop = 0
  },
)

/**
 * 切歌联动：
 *  - 记录"最近播放"（由 library store 落用户数据，服务端/本地持久化）；
 *  - 歌词的加载由常驻的 DesktopLyricsBar watch trackId 触发，
 *    LyricsPanel 的同名 watch 经 lyrics store 的同曲目去重不会双取。
 */
watch(
  () => playerStore.snapshot.trackId,
  (trackId, previousId) => {
    if (trackId && trackId !== previousId) {
      void libraryStore.recordPlayed(trackId)
    }
  },
)

/** 播放错误 → toast（store 已把异常收敛进 snapshot.error，不在此弹窗打断） */
watch(
  () => [playerStore.snapshot.status, playerStore.snapshot.error] as const,
  ([status, error]) => {
    if (status === 'error' && error) toastStore.error(`播放失败：${error}`)
  },
)

onMounted(async () => {
  // 0) 收听统计：实例化即启动对播放快照的自动跟踪（次数 / 时长 / 最后播放 / 时段分布）
  useStatsStore()
  // 0.5) 拖拽导入：注册 Tauri 拖放事件（浏览器预览环境自动跳过）
  setupDragImport()
  // 1) 主题：与 index.html 首屏脚本同键同步，无闪烁
  applyTheme(theme.value)
  // 2) 设置：恢复非主题类偏好（语言、曲库目录、在线开关等）
  //    布局状态（§5.10）在 useLayout 模块首次加载时已读取并校验，此处不重复恢复
  await settingsStore.restore()
  // 3) 数据初始化：曲库 / 歌单 / 播放引擎（web 预览走 services/web 本地实现）
  await Promise.all([libraryStore.refresh(), playlistStore.refresh(), playerStore.init()])

  // 3.5) 目录自动监控：订阅增量同步事件，文件变化后自动刷新曲库
  subscribeWatcher(() => {
    void libraryStore.refresh()
  })

  // 3.6) 两阶段懒加载：订阅后台补解析 / 播放时按需解析完成事件，
  //      增量合并曲目元数据（不触发全量刷新，网络大目录 UI 即时响应）。
  //      removedIds：整碟镜像被 CUE / SACD 分轨取代时，后端会移除占位条目，
  //      前端需同步移除，避免列表里残留「未知专辑」的幽灵条目。
  listen<{ tracks: Track[]; removedIds?: string[] }>('library://tracks-resolved', (e) => {
    const tracks = Array.isArray(e.payload?.tracks) ? e.payload.tracks : []
    const removedIds = Array.isArray(e.payload?.removedIds) ? e.payload.removedIds : []
    if (tracks.length === 0 && removedIds.length === 0) return
    libraryStore.applyResolvedTracks(tracks, removedIds)
  })
    .then((unsub) => {
      unlistenResolved = unsub
    })
    .catch(() => {
      unlistenResolved = undefined
    })

  // 4) 静默检查更新：延迟 8s 不拖慢启动；仅发现新版本时提示，
  //    离线 / 更新通道尚未配置 / 浏览器预览等失败一律静默忽略
  window.setTimeout(() => {
    updateService
      .check()
      .then((update) => {
        if (update) {
          toastStore.info(`发现新版本 v${update.version}，可在「设置 → 更新」中下载安装`)
        }
      })
      .catch(() => {
        /* 静默：升级检查永不影响正常使用 */
      })
  }, 8000)
})

void router
</script>

<template>
  <!--
    五区布局壳（DESIGN §2.1）：
      TitleBar（跨全宽）/ SideBar / Content / RightPanel / PlayerBar（跨全宽）
    右面板关闭时其列宽为 0（而非 display:none），面板实例始终保留，开合不重建、不丢内部状态。
  -->
  <div class="app-shell" :style="shellColumns">
    <header class="app-shell__titlebar u-drag-region">
      <TitleBar />
    </header>

    <aside class="app-shell__sidebar">
      <SideBar />
    </aside>

    <main class="app-shell__content">
      <!-- 播放列表上方的实时歌词条（可开合，与右面板歌词页签独立）；常驻保证切歌时歌词持续加载 -->
      <DesktopLyricsBar v-show="isDesktopLyricsVisible" />
      <!--
        滚动容器自身不带 padding：否则 sticky 表头在 Chromium/WebView2 中吸顶时
        停在容器 padding-top 之内，滚动中曲目行会从表头上方的带状区露出（“悬浮缝隙”）。
        内边距下移到 view-pad（普通流内容），表头 top:0 即可真正贴住滚动区顶边。
      -->
      <div ref="contentRef" class="app-shell__content-scroll u-scroll">
        <div class="app-shell__view-pad">
          <RouterView v-slot="{ Component, route: currentRoute }">
            <!-- 常驻 KeepAlive：缓存开关由路由 meta.keepAlive 决定（§1.4 / §2.7） -->
            <KeepAlive :max="12">
              <component
                :is="isCacheable(currentRoute.meta) ? Component : IdlePlaceholder"
                :key="isCacheable(currentRoute.meta) ? `view:${String(currentRoute.name)}` : 'idle'"
              />
            </KeepAlive>
            <!-- 非缓存路由（详情页等）：每次进入重新创建，配合上面 KeepAlive 保住列表页缓存 -->
            <component
              v-if="!isCacheable(currentRoute.meta)"
              :is="Component"
              :key="currentRoute.fullPath"
            />
          </RouterView>
        </div>
      </div>
    </main>

    <aside class="app-shell__panel">
      <RightPanel />
    </aside>

    <footer class="app-shell__player">
      <PlayerBar />
    </footer>

    <!-- 全局浮层：Teleport 到 body，z 序由 theme.css 统一管理（toast > modal > dropdown） -->
    <ToastContainer />
    <DialogHost />
    <ContextMenuHost />
    <CommandPalette />
    <NowPlayingOverlay v-if="isNowPlayingOpen" />

    <!-- 拖拽导入遮罩：文件/目录拖入窗口时覆盖全窗，松手即入库 -->
    <transition name="drag-overlay">
      <div v-if="isDragging" class="drag-overlay">
        <div class="drag-overlay__card">
          <Icon name="file-audio" :size="40" />
          <p class="drag-overlay__title">松开以加载播放列表</p>
          <p class="drag-overlay__hint">支持音频文件与文件夹（含 NAS / 网盘），自动建列表并播放</p>
        </div>
      </div>
    </transition>
  </div>
</template>

<style scoped>
.app-shell {
  display: grid;
  /* 列宽取值一律是 theme.css 的区域级尺寸 token，禁止在此写死像素值 */
  grid-template-columns:
    var(--app-shell-sidebar-col, var(--size-sidebar-w))
    minmax(0, 1fr)
    var(--app-shell-panel-col, 0px);
  grid-template-rows:
    var(--size-titlebar-h)
    minmax(0, 1fr)
    var(--size-playerbar-h);
  grid-template-areas:
    'titlebar titlebar titlebar'
    'sidebar content panel'
    'player player player';
  height: 100vh;
  /* 整窗不滚动，滚动只发生在内容区与面板内部（DESIGN §2.1 硬性规则 1） */
  overflow: hidden;
  color: var(--color-text-primary);
  background-color: var(--color-bg-base);
}

.app-shell__titlebar {
  grid-area: titlebar;
  min-width: 0;
}

.app-shell__sidebar {
  grid-area: sidebar;
  min-width: 0;
  /* 边框分区，不用阴影（DESIGN §2.1 硬性规则 2） */
  border-right: 1px solid var(--color-border-subtle);
}

.app-shell__content {
  grid-area: content;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  background-color: var(--color-bg-surface);
}

/* 滚动容器：自身不带 padding（见模板注释，sticky 表头才能真正贴顶） */
.app-shell__content-scroll {
  flex: 1 1 auto;
  min-height: 0;
}

/* 视图统一内边距：普通流内容，sticky 表头可越过它贴住滚动区顶边 */
.app-shell__view-pad {
  min-height: 100%;
  padding: var(--space-5) var(--space-6);
}

/* 右面板：列宽为 0 时内容必须被裁剪（面板自身已 overflow: hidden，此处兜底） */
.app-shell__panel {
  grid-area: panel;
  min-width: 0;
  overflow: hidden;
}

.app-shell__player {
  grid-area: player;
  min-width: 0;
  /* 抬升区：阴影 + 顶部 1px 边框，保证与内容区层级稳定 */
  box-shadow: var(--shadow-elevation-2);
}

/* ---- 拖拽导入遮罩（覆盖全窗，置于内容之上、浮层之下） ---- */
.drag-overlay {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
  color: var(--color-text-primary);
  background-color: color-mix(in srgb, var(--color-bg-base) 78%, transparent);
  backdrop-filter: blur(2px);
}

.drag-overlay__card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-10) var(--space-12);
  text-align: center;
  color: var(--color-brand);
  background-color: var(--color-bg-elevated);
  border: 2px dashed var(--color-brand);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-elevation-2);
}

.drag-overlay__title {
  margin: 0;
  font-size: var(--font-size-md);
  font-weight: var(--font-weight-semibold);
}

.drag-overlay__hint {
  margin: 0;
  font-size: var(--font-size-xs);
  color: var(--color-text-secondary);
}

.drag-overlay-enter-active,
.drag-overlay-leave-active {
  transition: opacity var(--transition-colors);
}

.drag-overlay-enter-from,
.drag-overlay-leave-to {
  opacity: 0;
}
</style>
