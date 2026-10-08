import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { useMessage } from '../useMessage'

// useMessage 内部为模块级单例状态（toasts 与 timers）。
// 测试间需要清理共享状态，避免互相污染。
function clearAll() {
  const { toasts, removeToast } = useMessage()
  // 拷贝 id 列表后逐个 removeToast，以便同时清掉 timers 句柄
  const ids = toasts.value.map(t => t.id)
  for (const id of ids) removeToast(id)
}

describe('useMessage', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    clearAll()
  })

  afterEach(() => {
    vi.useRealTimers()
    clearAll()
  })

  it('showError 添加 error 类型 toast', () => {
    const { toasts, showError } = useMessage()
    showError('网络请求失败')
    expect(toasts.value).toHaveLength(1)
    expect(toasts.value[0].type).toBe('error')
    expect(toasts.value[0].content).toBe('网络请求失败')
  })

  it('showSuccess / showWarning / showInfo 各自路由到对应类型', () => {
    const { toasts, showSuccess, showWarning, showInfo } = useMessage()
    showSuccess('已复制')
    showWarning('请先填写标题')
    showInfo('未找到歌词')
    expect(toasts.value.map(t => t.type)).toEqual(['success', 'warning', 'info'])
    expect(toasts.value.map(t => t.content)).toEqual(['已复制', '请先填写标题', '未找到歌词'])
  })

  it('默认时长：error 5s / success 3s / warning 4s / info 3s', () => {
    const { toasts, showError, showSuccess, showWarning, showInfo, removeToast } = useMessage()
    showError('e')
    expect(toasts.value[0].duration).toBe(5000)
    removeToast(toasts.value[0].id)
    showSuccess('s')
    expect(toasts.value[0].duration).toBe(3000)
    removeToast(toasts.value[0].id)
    showWarning('w')
    expect(toasts.value[0].duration).toBe(4000)
    removeToast(toasts.value[0].id)
    showInfo('i')
    expect(toasts.value[0].duration).toBe(3000)
  })

  it('自定义 duration 覆盖默认', () => {
    const { toasts, showError } = useMessage()
    showError('x', 1234)
    expect(toasts.value[0].duration).toBe(1234)
  })

  it('到时自动移除', () => {
    const { toasts, showInfo } = useMessage()
    showInfo('稍后消失', 1000)
    expect(toasts.value).toHaveLength(1)
    vi.advanceTimersByTime(999)
    expect(toasts.value).toHaveLength(1)
    vi.advanceTimersByTime(2)
    expect(toasts.value).toHaveLength(0)
  })

  it('手动 removeToast 立即移除并取消定时器', () => {
    const { toasts, showInfo, removeToast } = useMessage()
    showInfo('手动关闭', 5000)
    const id = toasts.value[0].id
    removeToast(id)
    expect(toasts.value).toHaveLength(0)
    // 即使定时器时间到也不会有副作用
    vi.advanceTimersByTime(6000)
    expect(toasts.value).toHaveLength(0)
  })

  it('同时最多显示 3 条，超出挤掉最早的', () => {
    const { toasts, showInfo } = useMessage()
    showInfo('第一条')
    showInfo('第二条')
    showInfo('第三条')
    showInfo('第四条')
    expect(toasts.value).toHaveLength(3)
    // 第一条已被挤出
    expect(toasts.value.map(t => t.content)).toEqual(['第二条', '第三条', '第四条'])
  })

  it('pauseToast 暂停自动消失，resumeToast 恢复', () => {
    const { toasts, showInfo, pauseToast, resumeToast } = useMessage()
    showInfo('悬停', 1000)
    const id = toasts.value[0].id
    // 走一半时间后暂停
    vi.advanceTimersByTime(500)
    pauseToast(id)
    vi.advanceTimersByTime(2000)
    expect(toasts.value).toHaveLength(1)
    // 恢复后从当前点重新计时 1000ms
    resumeToast(id)
    vi.advanceTimersByTime(999)
    expect(toasts.value).toHaveLength(1)
    vi.advanceTimersByTime(2)
    expect(toasts.value).toHaveLength(0)
  })

  it('每条 toast 拥有唯一 id', () => {
    const { toasts, showInfo } = useMessage()
    showInfo('a')
    showInfo('b')
    showInfo('c')
    const ids = toasts.value.map(t => t.id)
    expect(new Set(ids).size).toBe(ids.length)
  })

  it('confirmAction 透传原生 confirm 的返回值', () => {
    const { confirmAction } = useMessage()
    vi.stubGlobal('confirm', () => true)
    expect(confirmAction('确定？')).toBe(true)
    vi.stubGlobal('confirm', () => false)
    expect(confirmAction('确定？')).toBe(false)
    vi.unstubAllGlobals()
  })
})
