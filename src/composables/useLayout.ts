/**
 * useLayout —— 布局外壳状态（侧栏折叠 / 右面板开合与页签）
 *
 * DESIGN 依据：§2.1 窗口骨架、§2.3 折叠三原则（按钮固定在侧栏底部 · 状态由外壳统一持有 ·
 * 记忆恢复失败回落标准模式）、§5.10 状态记忆（localStorage）。
 *
 * 单一事实源约定（与 useTheme 同构）：
 *  - 状态由本模块的模块级单例持有：App.vue（外壳）据此映射网格列宽，
 *    SideBar / RightPanel 只读取状态并调用此处方法，不各自维护折叠标志；
 *  - 持久化键唯一：LAYOUT_STORAGE_KEY；键缺失、JSON 损坏、字段类型非法
 *    一律整体回落到 LAYOUT_DEFAULTS（§5.10 红线：禁止"启动即 0 宽侧栏"等恢复失败形态）；
 *  - 只收纳外壳自身当下能闭环的字段。侧栏/右面板宽度（拖拽调整）与紧凑档自动折叠属 M1，
 *    届时在本文件扩展，不预先塞入无人读写的字段。
 *
 * 未实现（M1，见 §2.3 / §2.8 / §4.10）：
 *  - 窗口高度 < 560px 时底部组转图标行；紧凑/窄档自动折叠侧栏，以及"手动锁定后不再自动切换"；
 *  - 侧栏与右面板宽度拖拽（--size-sidebar-w-min/max、--size-panel-w-min/max）；
 *  - 紧凑档下右面板浮层形态（覆盖内容区）。
 */
import { computed, reactive, ref, watch } from 'vue'

import { LAYOUT_DEFAULTS, LAYOUT_STORAGE_KEY } from '../constants'

/** 右面板页签（§4.10：歌词 / 队列，顺序固定） */
export type PanelTab = 'lyrics' | 'queue'

/** 布局状态：可持久化的全部字段（新增字段须同步读取校验、默认值与本注释） */
interface LayoutState {
  /** 侧栏是否折叠（列宽 --size-sidebar-collapsed ↔ --size-sidebar-w） */
  sidebarCollapsed: boolean
  /** 右面板是否展开（列宽 --size-panel-w ↔ 0） */
  panelOpen: boolean
  /** 右面板当前页签 */
  panelTab: PanelTab
  /** 播放列表上方的实时歌词条是否展开（与右面板歌词页签相互独立） */
  desktopLyrics: boolean
  /** 皮肤强调色是否跟随当前曲目封面 */
  coverAccent: boolean
}

function isPanelTab(value: unknown): value is PanelTab {
  return value === 'lyrics' || value === 'queue'
}

/**
 * 读取持久化状态：逐字段守卫，而非整体信任 JSON。
 * 任何异常（无 localStorage、非法 JSON、字段类型不符）都回落到默认值。
 */
function readPersisted(): LayoutState {
  const fallback: LayoutState = { ...LAYOUT_DEFAULTS }
  try {
    const raw = localStorage.getItem(LAYOUT_STORAGE_KEY)
    if (!raw) return fallback

    const parsed: unknown = JSON.parse(raw)
    if (typeof parsed !== 'object' || parsed === null) return fallback

    const candidate = parsed as Record<string, unknown>
    return {
      sidebarCollapsed:
        typeof candidate.sidebarCollapsed === 'boolean'
          ? candidate.sidebarCollapsed
          : fallback.sidebarCollapsed,
      panelOpen:
        typeof candidate.panelOpen === 'boolean' ? candidate.panelOpen : fallback.panelOpen,
      panelTab: isPanelTab(candidate.panelTab) ? candidate.panelTab : fallback.panelTab,
      desktopLyrics:
        typeof candidate.desktopLyrics === 'boolean'
          ? candidate.desktopLyrics
          : fallback.desktopLyrics,
      coverAccent:
        typeof candidate.coverAccent === 'boolean'
          ? candidate.coverAccent
          : fallback.coverAccent,
    }
  } catch {
    // localStorage 不可用（隐私模式 / 受限 WebView）或内容损坏
    return fallback
  }
}

/** 模块级单例：应用生命周期内唯一，冷启动时完成一次"读取 + 校验 + 回落" */
const state = reactive<LayoutState>(readPersisted())

/**
 * 封面放大层（NowPlaying）开合：临时视图状态，不持久化（下次启动不应全屏遮挡）。
 */
const nowPlayingOpen = ref(false)

/**
 * 变更即持久化：写失败（配额 / 受限环境）静默忽略——
 * 状态记忆属"有则更好"，不得因持久化失败抛错或阻断交互。
 */
watch(
  state,
  () => {
    try {
      localStorage.setItem(LAYOUT_STORAGE_KEY, JSON.stringify(state))
    } catch {
      /* 忽略：本次会话内状态仍然生效 */
    }
  },
  { deep: true },
)

/** 折叠切换：侧栏底部按钮与快捷键 Ctrl+B 共用同一入口（§2.3） */
function toggleSidebar(): void {
  state.sidebarCollapsed = !state.sidebarCollapsed
}

/**
 * 面板页签开关（Ctrl+U / Ctrl+Shift+U）：
 * 已展开且正显示该页签 → 关闭；否则切到该页签并展开。
 */
function togglePanelTab(tab: PanelTab): void {
  if (state.panelOpen && state.panelTab === tab) {
    state.panelOpen = false
    return
  }
  state.panelTab = tab
  state.panelOpen = true
}

/** 切换页签（RightPanel 页签点击）；面板已展开时保持展开 */
function setPanelTab(tab: PanelTab): void {
  state.panelTab = tab
  state.panelOpen = true
}

/** 关闭面板（RightPanel 右上关闭按钮）；列宽归 0 由外壳网格完成 */
function closePanel(): void {
  state.panelOpen = false
}

/** 播放列表上方实时歌词条的开合（与右面板歌词页签独立，互不影响） */
function toggleDesktopLyrics(): void {
  state.desktopLyrics = !state.desktopLyrics
}

/** 封面取色开关 */
function setCoverAccent(enabled: boolean): void {
  state.coverAccent = enabled
}

/** 封面放大层（点击播放条封面打开，展示大封面与叠加歌词） */
function openNowPlaying(): void {
  nowPlayingOpen.value = true
}

function closeNowPlaying(): void {
  nowPlayingOpen.value = false
}

function toggleNowPlaying(): void {
  nowPlayingOpen.value = !nowPlayingOpen.value
}

export function useLayout() {
  return {
    isSidebarCollapsed: computed(() => state.sidebarCollapsed),
    isPanelOpen: computed(() => state.panelOpen),
    panelTab: computed(() => state.panelTab),
    isDesktopLyricsVisible: computed(() => state.desktopLyrics),
    isCoverAccentEnabled: computed(() => state.coverAccent),
    isNowPlayingOpen: computed(() => nowPlayingOpen.value),
    toggleSidebar,
    togglePanelTab,
    setPanelTab,
    closePanel,
    toggleDesktopLyrics,
    setCoverAccent,
    openNowPlaying,
    closeNowPlaying,
    toggleNowPlaying,
  }
}
