/**
 * useContextMenu · 曲目信息头选项单测（DESIGN §5.5 内容展示模块）
 *
 * 覆盖：单曲右键传 trackId（信息头渲染）、多选/卡片不传（纯动作菜单）、
 * close 全量重置。模块级单例状态由 close() 保证用例间隔离。
 */
import { describe, expect, it } from 'vitest'

import { useContextMenu } from './useContextMenu'

function fakeEvent(): MouseEvent {
  return new MouseEvent('contextmenu', {
    clientX: 10,
    clientY: 20,
    bubbles: true,
    cancelable: true,
  })
}

describe('useContextMenu · trackId options', () => {
  it('open 传入 trackId 时信息头状态被设置（单曲右键）', () => {
    const { open, state } = useContextMenu()
    open(fakeEvent(), [{ key: 'a', label: 'A' }], undefined, { trackId: 't1' })
    expect(state.open).toBe(true)
    expect(state.trackId).toBe('t1')
    expect(state.title).toBeNull()
  })

  it('open 未传 options 时 trackId 为 null（多选 / 卡片等纯动作菜单）', () => {
    const { open, state } = useContextMenu()
    open(fakeEvent(), [{ key: 'a', label: 'A' }], '已选 7 首')
    expect(state.open).toBe(true)
    expect(state.trackId).toBeNull()
    expect(state.title).toBe('已选 7 首')
  })

  it('close 重置 trackId / 标题 / items，用例间状态不残留', () => {
    const { open, close, state } = useContextMenu()
    open(fakeEvent(), [{ key: 'a', label: 'A' }], undefined, { trackId: 't2' })
    close()
    expect(state.open).toBe(false)
    expect(state.trackId).toBeNull()
    expect(state.title).toBeNull()
    expect(state.items).toHaveLength(0)
  })
})
