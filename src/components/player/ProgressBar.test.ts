/**
 * ProgressBar.vue 组件测试
 *
 * 覆盖：props → ratio → fill width 绑定、时间码格式化、键盘 seek 事件、disabled 静默、duration=0 无进度。
 */
import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'

import ProgressBar from './ProgressBar.vue'

function mountBar(props: { positionMs: number; durationMs: number; disabled?: boolean }) {
  return mount(ProgressBar, {
    props,
    global: {
      // 样式中用到 CSS 变量，jsdom 下无真实样式，提供最小替身以避免计算崩溃
      stubs: {},
    },
  })
}

describe('ProgressBar.vue — props 绑定', () => {
  it('正常进度：fill width = ratio × 100%', () => {
    const wrapper = mountBar({ positionMs: 30_000, durationMs: 120_000 }) // 25%
    const fill = wrapper.find('.progress__fill')
    expect(fill.exists()).toBe(true)
    const width = (fill.attributes('style') || '').match(/width:\s*([\d.]+)%/)?.[1]
    expect(width).toBeCloseTo(25, 1)
  })

  it('进度 0 → fill width 0%', () => {
    const wrapper = mountBar({ positionMs: 0, durationMs: 180_000 })
    const width = wrapper.find('.progress__fill').attributes('style') || ''
    expect(width).toContain('width: 0%')
  })

  it('进度 ≥ 总时长 → fill width 100%（不溢出）', () => {
    const wrapper = mountBar({ positionMs: 200_000, durationMs: 180_000 })
    const width = wrapper.find('.progress__fill').attributes('style') || ''
    expect(width).toContain('width: 100%')
  })

  it('duration = 0 → fill width 0%（避免除零）', () => {
    const wrapper = mountBar({ positionMs: 999_999, durationMs: 0 })
    const width = wrapper.find('.progress__fill').attributes('style') || ''
    expect(width).toContain('width: 0%')
  })
})

describe('ProgressBar.vue — 时间码', () => {
  it('当前时间与总时长分别渲染', () => {
    const wrapper = mountBar({ positionMs: 45_000, durationMs: 180_000 })
    const times = wrapper.findAll('.progress__time')
    expect(times).toHaveLength(2)
    // 位置时间 = 0:45
    expect(times[0].text()).toBe('0:45')
    // 总时长 = 3:00
    expect(times[1].text()).toBe('3:00')
  })
})

describe('ProgressBar.vue — 键盘 seek', () => {
  it('ArrowRight → seek +5s', async () => {
    const wrapper = mountBar({ positionMs: 10_000, durationMs: 60_000 })
    await wrapper.find('.progress__track').trigger('keydown', { key: 'ArrowRight' })
    expect(wrapper.emitted('seek')?.[0]).toEqual([15_000])
  })

  it('Shift+ArrowRight → seek +30s', async () => {
    const wrapper = mountBar({ positionMs: 10_000, durationMs: 60_000 })
    await wrapper.find('.progress__track').trigger('keydown', { key: 'ArrowRight', shiftKey: true })
    expect(wrapper.emitted('seek')?.[0]).toEqual([40_000])
  })

  it('ArrowLeft → seek -5s，不越界', async () => {
    const wrapper = mountBar({ positionMs: 3_000, durationMs: 60_000 })
    await wrapper.find('.progress__track').trigger('keydown', { key: 'ArrowLeft' })
    expect(wrapper.emitted('seek')?.[0]).toEqual([0])
  })

  it('End → seek duration（末尾）', async () => {
    const wrapper = mountBar({ positionMs: 0, durationMs: 60_000 })
    await wrapper.find('.progress__track').trigger('keydown', { key: 'End' })
    expect(wrapper.emitted('seek')?.[0]).toEqual([60_000])
  })

  it('Home → seek 0', async () => {
    const wrapper = mountBar({ positionMs: 30_000, durationMs: 60_000 })
    await wrapper.find('.progress__track').trigger('keydown', { key: 'Home' })
    expect(wrapper.emitted('seek')?.[0]).toEqual([0])
  })
})

describe('ProgressBar.vue — disabled 静默', () => {
  it('disabled 时键盘不 emit seek', async () => {
    const wrapper = mountBar({ positionMs: 10_000, durationMs: 60_000, disabled: true })
    await wrapper.find('.progress__track').trigger('keydown', { key: 'ArrowRight' })
    await wrapper.find('.progress__track').trigger('keydown', { key: 'Home' })
    expect(wrapper.emitted('seek')).toBeFalsy()
  })

  it('disabled 时 tabindex = -1（不可聚焦）', () => {
    const wrapper = mountBar({ positionMs: 0, durationMs: 60_000, disabled: true })
    expect(wrapper.find('.progress__track').attributes('tabindex')).toBe('-1')
  })

  it('duration = 0 时键盘不 emit seek', async () => {
    const wrapper = mountBar({ positionMs: 0, durationMs: 0 })
    await wrapper.find('.progress__track').trigger('keydown', { key: 'ArrowRight' })
    expect(wrapper.emitted('seek')).toBeFalsy()
  })
})
