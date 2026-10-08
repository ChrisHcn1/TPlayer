import { ref } from 'vue'

// 消息类型：error 报错/停止、success 成功、warning 警告/前置条件、info 信息/未找到
export type MessageType = 'error' | 'success' | 'warning' | 'info'

export interface Toast {
  id: string
  type: MessageType
  content: string
  duration: number
}

// 各类型默认自动消失时长（毫秒）
const DEFAULT_DURATION: Record<MessageType, number> = {
  error: 5000,
  success: 3000,
  warning: 4000,
  info: 3000
}

// 同时显示的最多条数，超出挤掉最早的
const MAX_VISIBLE = 3

// ===== 模块级单例状态 =====
// 整个应用共享同一份 toasts，任何 composable/组件 import useMessage 都操作这份数据。
const toasts = ref<Toast[]>([])
// 每条 toast 的定时器句柄，id -> setTimeout 句柄；悬停时清掉、离开时重启
const timers = new Map<string, ReturnType<typeof setTimeout>>()

function clearTimer(id: string) {
  const handle = timers.get(id)
  if (handle !== undefined) {
    clearTimeout(handle)
    timers.delete(id)
  }
}

function removeToast(id: string) {
  clearTimer(id)
  const idx = toasts.value.findIndex(t => t.id === id)
  if (idx !== -1) toasts.value.splice(idx, 1)
}

function scheduleRemoval(id: string, duration: number) {
  clearTimer(id)
  const handle = setTimeout(() => removeToast(id), duration)
  timers.set(id, handle)
}

function showMessage(type: MessageType, content: string, duration?: number) {
  const id = (typeof crypto !== 'undefined' && crypto.randomUUID)
    ? crypto.randomUUID()
    : `${Date.now()}-${Math.random().toString(36).slice(2)}`

  const toast: Toast = { id, type, content, duration: duration ?? DEFAULT_DURATION[type] }
  toasts.value.push(toast)
  // 超过上限：从头移除最早未关闭的，保证最多 MAX_VISIBLE 条
  while (toasts.value.length > MAX_VISIBLE) {
    const first = toasts.value[0]
    removeToast(first.id)
  }
  scheduleRemoval(id, toast.duration)
}

function showError(content: string, duration?: number) {
  showMessage('error', content, duration)
}
function showSuccess(content: string, duration?: number) {
  showMessage('success', content, duration)
}
function showWarning(content: string, duration?: number) {
  showMessage('warning', content, duration)
}
function showInfo(content: string, duration?: number) {
  showMessage('info', content, duration)
}

// 确认对话保留原生 confirm：桌面端跨浏览器、简单可靠，YAGNI 不引入 Tauri dialog 插件
function confirmAction(message: string): boolean {
  return confirm(message)
}

// 供 ToastContainer 悬停时调用：暂停指定 toast 的自动消失
function pauseToast(id: string) {
  clearTimer(id)
}

// 供 ToastContainer 离开时调用：按原 duration 重启计时
function resumeToast(id: string) {
  const t = toasts.value.find(x => x.id === id)
  if (t) scheduleRemoval(t.id, t.duration)
}

// 工厂返回操作同一份单例状态的函数集合
export function useMessage() {
  return {
    toasts,
    showMessage,
    showError,
    showSuccess,
    showWarning,
    showInfo,
    confirmAction,
    removeToast,
    pauseToast,
    resumeToast
  }
}
