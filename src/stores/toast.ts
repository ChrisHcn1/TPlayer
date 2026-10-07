/**
 * toast store —— 全局轻量反馈（DESIGN §4.16）
 *
 * 只维护待展示的 toast 队列与显隐；自动消失计时由 ToastItem 组件负责
 * （需要支持悬停暂停）。同时最多 3 条，超出时最旧出队。
 */
import { ref } from 'vue'
import { defineStore } from 'pinia'

export type ToastVariant = 'info' | 'success' | 'warning' | 'error'

export interface ToastAction {
  label: string
  onClick: () => void
}

export interface ToastItem {
  id: number
  variant: ToastVariant
  title: string
  description: string | null
  action: ToastAction | null
  /** 自动停留时长（毫秒），由调用方或变体默认值决定 */
  duration: number
}

export interface ToastOptions {
  description?: string
  action?: ToastAction
  duration?: number
}

const DEFAULT_DURATION: Record<ToastVariant, number> = {
  info: 4000,
  success: 4000,
  warning: 6000,
  error: 8000,
}

const MAX_TOASTS = 3

export const useToastStore = defineStore('toast', () => {
  const toasts = ref<ToastItem[]>([])
  let seq = 0

  function push(variant: ToastVariant, title: string, options: ToastOptions = {}): number {
    const id = ++seq
    const item: ToastItem = {
      id,
      variant,
      title,
      description: options.description ?? null,
      action: options.action ?? null,
      duration: options.duration ?? DEFAULT_DURATION[variant],
    }
    toasts.value = [...toasts.value, item].slice(-MAX_TOASTS)
    return id
  }

  function dismiss(id: number): void {
    toasts.value = toasts.value.filter((item) => item.id !== id)
  }

  const info = (title: string, options?: ToastOptions) => push('info', title, options)
  const success = (title: string, options?: ToastOptions) => push('success', title, options)
  const warning = (title: string, options?: ToastOptions) => push('warning', title, options)
  const error = (title: string, options?: ToastOptions) => push('error', title, options)

  return { toasts, push, dismiss, info, success, warning, error }
})
