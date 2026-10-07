/**
 * 全局命令浮层（Ctrl+K / ⌘+K）开关单例（DESIGN §2.4）
 */
import { ref } from 'vue'

const isOpen = ref(false)

function open(): void {
  isOpen.value = true
}

function close(): void {
  isOpen.value = false
}

function toggle(): void {
  isOpen.value = !isOpen.value
}

export function useCommandPalette() {
  return { isOpen, open, close, toggle }
}
