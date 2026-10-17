/**
 * services/player.ts — createIdleSnapshot 单元测试
 *
 * 纯对象构造：验证默认快照的 shape 与值，避免 store 初始化漂移。
 */
import { describe, it, expect } from 'vitest'

import { createIdleSnapshot } from './player'

describe('createIdleSnapshot', () => {
  it('返回结构完整、可直接作为 store 初值', () => {
    const snap = createIdleSnapshot()
    // 字段存在性
    expect(snap).toHaveProperty('status')
    expect(snap).toHaveProperty('trackId')
    expect(snap).toHaveProperty('positionMs')
    expect(snap).toHaveProperty('durationMs')
    expect(snap).toHaveProperty('volume')
    expect(snap).toHaveProperty('muted')
    expect(snap).toHaveProperty('rate')
    expect(snap).toHaveProperty('mode')
    expect(snap).toHaveProperty('error')
  })

  it('默认值语义正确', () => {
    const snap = createIdleSnapshot()
    expect(snap.status).toBe('idle')
    expect(snap.trackId).toBeNull()
    expect(snap.positionMs).toBe(0)
    expect(snap.durationMs).toBe(0)
    expect(snap.volume).toBe(1)
    expect(snap.muted).toBe(false)
    expect(snap.rate).toBe(1)
    expect(snap.mode).toBe('order')
    expect(snap.error).toBeNull()
  })

  it('每次调用返回独立对象（非引用）', () => {
    const a = createIdleSnapshot()
    const b = createIdleSnapshot()
    expect(a).not.toBe(b)
    a.rate = 2
    expect(b.rate).toBe(1)
  })
})
