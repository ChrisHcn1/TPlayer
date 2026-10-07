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
}

const state = reactive<MenuState>({
  open: false,
  x: 0,
  y: 0,
  title: null,
  items: [],
})

/** 一级菜单键盘高亮（-1 表示无高亮） */
const activeIndex = ref(-1)
/** 当前展开子菜单的父项 key */
const openSubmenuKey = ref<string | null>(null)

function openAt(x: number, y: number, items: MenuItem[], title?: string): void {
  close()
  state.items = items
  state.title = title ?? null
  state.x = x
  state.y = y
  state.open = true
  activeIndex.value = -1
}

function open(event: MouseEvent, items: MenuItem[], title?: string): void {
  event.preventDefault()
  event.stopPropagation()
  openAt(event.clientX, event.clientY, items, title)
}

function close(): void {
  state.open = false
  state.items = []
  state.title = null
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
