/**
 * useKeyboardShortcuts —— 全局快捷键组合式函数
 * 约定：快捷键映射集中在此处，避免旧版把按键判断散落在多个组件的 keydown 回调里。
 * M0 阶段只提供注册/注销骨架，不实现具体业务动作。
 */
import { onBeforeUnmount, onMounted } from 'vue'

/**
 * 快捷键动作标识（与 DESIGN §6.1.1 键位表一一对应）
 * 该联合类型是映射表的"唯一允许值集合"：新增键位必须先在此登记，
 * 否则 Record<string, ShortcutAction> 会在编译期报错（避免写了键位却没接线动作）。
 */
export type ShortcutAction =
  /* 播放控制（§6.1.1） */
  | 'toggle-play'
  | 'previous-track'
  | 'next-track'
  | 'seek-backward'
  | 'seek-forward'
  | 'volume-up'
  | 'volume-down'
  | 'toggle-mute'
  | 'toggle-favorite'
  | 'cycle-play-mode'
  /* 搜索、歌词与面板（§6.1.1 / §6.1.2） */
  | 'focus-search'
  | 'toggle-fullscreen-lyrics'
  | 'toggle-lyrics-panel'
  | 'toggle-queue-panel'
  | 'toggle-sidebar'

/**
 * 默认映射（DESIGN §6.1.1 全量键位表）。
 * 注意：M0 阶段 player / lyrics / search 尚未接线，直接注册本表会劫持按键而无动作，
 * 因此本表只作为规范基线，动作接线完成后才由调用方使用（当前外壳只注册 LAYOUT_SHORTCUTS）。
 */
export const DEFAULT_SHORTCUTS: Record<string, ShortcutAction> = {
  space: 'toggle-play',
  'ctrl+arrowright': 'next-track',
  'ctrl+arrowleft': 'previous-track',
  arrowright: 'seek-forward',
  arrowleft: 'seek-backward',
  arrowup: 'volume-up',
  arrowdown: 'volume-down',
  m: 'toggle-mute',
  f: 'toggle-favorite',
  r: 'cycle-play-mode',
  'ctrl+k': 'focus-search',
  '/': 'focus-search',
  'ctrl+l': 'toggle-fullscreen-lyrics',
  q: 'toggle-queue-panel',
}

/**
 * 布局外壳快捷键（DESIGN §6.1.2）
 * Ctrl+B 折叠侧栏 / Ctrl+U 歌词面板开合 / Ctrl+Shift+U 队列面板开合。
 * 单独导出：这是 M0 唯一"已接线、可闭环"的一组动作，由 App.vue 注册。
 */
export const LAYOUT_SHORTCUTS: Record<string, ShortcutAction> = {
  'ctrl+b': 'toggle-sidebar',
  'ctrl+u': 'toggle-lyrics-panel',
  'ctrl+shift+u': 'toggle-queue-panel',
}

export interface UseKeyboardShortcutsOptions {
  /** 自定义映射，默认使用 DEFAULT_SHORTCUTS */
  shortcuts?: Record<string, ShortcutAction>
  /** 命中动作时的回调；M0 阶段由调用方注入（通常是 player store 的方法） */
  handler: (action: ShortcutAction, event: KeyboardEvent) => void
}

export function useKeyboardShortcuts(options: UseKeyboardShortcutsOptions) {
  const shortcuts = options.shortcuts ?? DEFAULT_SHORTCUTS

  /** 归一化事件为映射表键：修饰键顺序固定，字符键小写，空格归一为 space */
  function toShortcutKey(event: KeyboardEvent): string {
    const parts: string[] = []
    if (event.ctrlKey) parts.push('ctrl')
    if (event.altKey) parts.push('alt')
    if (event.shiftKey) parts.push('shift')
    parts.push(event.key === ' ' ? 'space' : event.key.toLowerCase())
    return parts.join('+')
  }

  function onKeydown(event: KeyboardEvent) {
    // 输入法组合期间（中文输入）不触发快捷键，避免误吞按键
    if (event.isComposing) return

    // 输入类元素中不劫持按键，避免影响文本输入（搜索框 / 表单 / contenteditable）
    const target = event.target as HTMLElement | null
    if (
      target &&
      (target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.tagName === 'SELECT' ||
        target.isContentEditable)
    ) {
      return
    }

    const action = shortcuts[toShortcutKey(event)]
    if (!action) return

    event.preventDefault()
    options.handler(action, event)
  }

  onMounted(() => {
    window.addEventListener('keydown', onKeydown)
  })

  onBeforeUnmount(() => {
    window.removeEventListener('keydown', onKeydown)
  })

  return { onKeydown }
}
