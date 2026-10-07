/**
 * dialog store —— 程序化确认框 / 输入框（DESIGN §5.7 统一确认流）
 *
 * 组件层不自行 new 模态框，统一调用：
 *   const ok = await dialog.confirm({ title, message, danger: true })
 *   const name = await dialog.prompt({ title, initialValue })
 * 确认弹窗一次确认即执行；危险确认默认焦点落在“取消”。
 */
import { ref } from 'vue'
import { defineStore } from 'pinia'

export interface ConfirmOptions {
  title: string
  message?: string
  confirmText?: string
  cancelText?: string
  /** 破坏性操作：红色主按钮 + 禁点遮罩关闭 + 焦点落取消 */
  danger?: boolean
}

export interface PromptOptions {
  title: string
  message?: string
  placeholder?: string
  initialValue?: string
  confirmText?: string
  cancelText?: string
  /** 输入校验：返回错误文案则禁止确认（null/空串表示通过） */
  validate?: (value: string) => string | null
}

interface DialogState {
  kind: 'confirm' | 'prompt'
  title: string
  message: string
  confirmText: string
  cancelText: string
  danger: boolean
  placeholder: string
  value: string
  validate: ((value: string) => string | null) | null
}

export const useDialogStore = defineStore('dialog', () => {
  const state = ref<DialogState | null>(null)
  /** 输入校验错误（仅 prompt） */
  const error = ref<string | null>(null)
  /** confirm/prompt 共用的 resolver，settle 时按当前弹窗语义传值 */
  type DialogResolver = ((value: boolean) => void) | ((value: string | null) => void)
  let resolver: DialogResolver | null = null

  function settle(value: boolean | string | null): void {
    const done = resolver as ((value: boolean | string | null) => void) | null
    resolver = null
    state.value = null
    error.value = null
    done?.(value)
  }

  /** 新弹窗打开前，用兜底值结算上一个未决弹窗 */
  function dismissExisting(fallback: boolean | string | null): void {
    const done = resolver as ((value: boolean | string | null) => void) | null
    resolver = null
    done?.(fallback)
  }

  function confirm(options: ConfirmOptions): Promise<boolean> {
    dismissExisting(false)
    state.value = {
      kind: 'confirm',
      title: options.title,
      message: options.message ?? '',
      confirmText: options.confirmText ?? '确定',
      cancelText: options.cancelText ?? '取消',
      danger: options.danger ?? false,
      placeholder: '',
      value: '',
      validate: null,
    }
    return new Promise<boolean>((resolve) => {
      resolver = resolve
    })
  }

  function prompt(options: PromptOptions): Promise<string | null> {
    dismissExisting(null)
    state.value = {
      kind: 'prompt',
      title: options.title,
      message: options.message ?? '',
      confirmText: options.confirmText ?? '确定',
      cancelText: options.cancelText ?? '取消',
      danger: false,
      placeholder: options.placeholder ?? '',
      value: options.initialValue ?? '',
      validate: options.validate ?? null,
    }
    error.value = null
    return new Promise<string | null>((resolve) => {
      resolver = resolve
    })
  }

  function setValue(value: string): void {
    if (!state.value || state.value.kind !== 'prompt') return
    state.value.value = value
    if (error.value) error.value = null
  }

  function accept(): void {
    if (!state.value) return
    if (state.value.kind === 'prompt') {
      const message = state.value.validate?.(state.value.value.trim()) ?? null
      if (message) {
        error.value = message
        return
      }
      settle(state.value.value.trim())
      return
    }
    settle(true)
  }

  function cancel(): void {
    settle(state.value?.kind === 'prompt' ? null : false)
  }

  return { state, error, confirm, prompt, setValue, accept, cancel }
})
