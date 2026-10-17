/**
 * 全局上下文菜单单例（DESIGN §4.17 / §5.5）
 *
 * 组件在 @contextmenu 中调用 open(event, items)；宿主 ContextMenuHost 负责
 * 定位翻转、外点/Esc 关闭、键盘导航。支持一层子菜单（用于“添加到歌单”）。
 */
import { reactive, ref } from 'vue'

export interface MenuItem {
  /** 唯一 key；分隔线可省略 */
  key?: string
  label?: string
  icon?: string
  /** 右侧快捷键提示文本（如 “Delete”），仅展示，不负责绑定 */
  shortcut?: string
  danger?: boolean
  disabled?: boolean
  divider?: boolean
  /** 一层子菜单（设置后点击该项仅展开子菜单） */
  children?: MenuItem[]
  onClick?: () => void
}

interface MenuState {
  open: boolean
  x: number
  y: number
  title: string | null
  items: MenuItem[]
  /**
   * 曲目信息头（DESIGN §5.5 内容展示模块）：右键单曲目时传入 trackId，
   * 宿主在菜单顶部渲染完整元数据 / 歌词 / 封面 / 文件路径；null 表示纯动作菜单。
   */
  trackId: string | null
}

const state = reactive<MenuState>({
  open: false,
  x: 0,
  y: 0,
  title: null,
  items: [],
  trackId: null,
})

/** 一级菜单键盘高亮（-1 表示无高亮） */
const activeIndex = ref(-1)
/** 当前展开子菜单的父项 key */
const openSubmenuKey = ref<string | null>(null)

/** 打开菜单的附加选项 */
export interface MenuOpenOptions {
  /** 传入曲目 id 时，菜单顶部渲染该曲目的完整信息头 */
  trackId?: string | null
}

function openAt(x: number, y: number, items: MenuItem[], title?: string, options?: MenuOpenOptions): void {
  close()
  state.items = items
  state.title = title ?? null
  state.trackId = options?.trackId ?? null
  state.x = x
  state.y = y
  state.open = true
  activeIndex.value = -1
}

function open(event: MouseEvent, items: MenuItem[], title?: string, options?: MenuOpenOptions): void {
  event.preventDefault()
  event.stopPropagation()
  openAt(event.clientX, event.clientY, items, title, options)
}

function close(): void {
  state.open = false
  state.items = []
  state.title = null
  state.trackId = null
  openSubmenuKey.value = null
  activeIndex.value = -1
}

function select(item: MenuItem): void {
  if (item.disabled || item.divider) return
  if (item.children) {
    openSubmenuKey.value = item.key ?? null
    return
  }
  const action = item.onClick
  close()
  action?.()
}

function setActiveIndex(index: number): void {
  activeIndex.value = index
  const item = state.items[index]
  openSubmenuKey.value = item?.children ? (item.key ?? null) : null
}

export function useContextMenu() {
  return {
    state,
    activeIndex,
    openSubmenuKey,
    open,
    openAt,
    close,
    select,
    setActiveIndex,
  }
}
