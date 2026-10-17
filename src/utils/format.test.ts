/**
 * formatMs / progressRatio 单元测试
 *
 * 覆盖场景：正常路径、边界值（0、负数、NaN、Infinity）、精度与截断。
 */
import { describe, it, expect } from 'vitest'

import { formatMs, progressRatio } from './format'

describe('formatMs', () => {
  it('正常路径：整分钟内', () => {
    expect(formatMs(0)).toBe('0:00')
    expect(formatMs(999)).toBe('0:01')   // 0.999s → round 到 1s
    expect(formatMs(1_000)).toBe('0:01')
    expect(formatMs(59_000)).toBe('0:59')
  })

  it('跨分钟', () => {
    expect(formatMs(60_000)).toBe('1:00')
    expect(formatMs(125_500)).toBe('2:06') // 2m5.5s → round 2m6s
    expect(formatMs(3_599_000)).toBe('59:59')
    expect(formatMs(3_600_000)).toBe('60:00') // 60 分钟（format 不封顶）
  })

  it('边界：负数当作 0', () => {
    expect(formatMs(-1)).toBe('0:00')
    expect(formatMs(-999_999)).toBe('0:00')
  })

  it('边界：非有限数当作 0', () => {
    expect(formatMs(NaN)).toBe('0:00')
    expect(formatMs(Infinity)).toBe('0:00')
    expect(formatMs(-Infinity)).toBe('0:00')
  })

  it('秒数向最近整数取整', () => {
    expect(formatMs(1_499)).toBe('0:01') // 1.499s → round 1s
    expect(formatMs(1_500)).toBe('0:02') // 1.5s → round 2s
  })
})

describe('progressRatio', () => {
  it('正常路径', () => {
    expect(progressRatio(0, 100)).toBe(0)
    expect(progressRatio(50, 100)).toBeCloseTo(0.5)
    expect(progressRatio(100, 100)).toBe(1)
    expect(progressRatio(250, 1000)).toBeCloseTo(0.25)
  })

  it('clamp 到 [0, 1]', () => {
    // position > duration → 1（进度条不溢出）
    expect(progressRatio(200, 100)).toBe(1)
    // 负数 position → 0
    expect(progressRatio(-50, 100)).toBe(0)
  })

  it('duration = 0 / 负数 / NaN → 0（避免除零）', () => {
    expect(progressRatio(50, 0)).toBe(0)
    expect(progressRatio(50, -100)).toBe(0)
    expect(progressRatio(50, NaN)).toBe(0)
    expect(progressRatio(50, Infinity)).toBe(0)
  })

  it('position 非有限数 → 0', () => {
    expect(progressRatio(NaN, 100)).toBe(0)
    expect(progressRatio(Infinity, 100)).toBe(0)
    expect(progressRatio(-Infinity, 100)).toBe(0)
  })
})
