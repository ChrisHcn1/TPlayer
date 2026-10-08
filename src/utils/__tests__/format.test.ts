import { describe, it, expect } from 'vitest'
import { formatTime, formatTimeForLrc, formatTimeWithHours, parseDurationToSeconds } from '../format'

describe('formatTime', () => {
  it('0 秒输出 0:00', () => {
    expect(formatTime(0)).toBe('0:00')
  })

  it('不足 1 分钟输出 m:ss', () => {
    expect(formatTime(5)).toBe('0:05')
    expect(formatTime(59)).toBe('0:59')
  })

  it('整分钟秒数补零', () => {
    expect(formatTime(60)).toBe('1:00')
    expect(formatTime(125)).toBe('2:05')
  })

  it('浮点秒数向下取整', () => {
    expect(formatTime(65.7)).toBe('1:05')
  })
})

describe('formatTimeWithHours', () => {
  it('不足 1 小时输出 m:ss', () => {
    expect(formatTimeWithHours(0)).toBe('0:00')
    expect(formatTimeWithHours(59)).toBe('0:59')
    expect(formatTimeWithHours(60)).toBe('1:00')
    expect(formatTimeWithHours(125)).toBe('2:05')
  })

  it('超过 1 小时输出 h:mm:ss', () => {
    expect(formatTimeWithHours(3600)).toBe('1:00:00')
    expect(formatTimeWithHours(3661)).toBe('1:01:01')
  })

  it('不足 10 的分秒补零（带小时分支）', () => {
    expect(formatTimeWithHours(3605)).toBe('1:00:05')
    expect(formatTimeWithHours(3665)).toBe('1:01:05')
  })

  it('多小时不补零', () => {
    expect(formatTimeWithHours(36000)).toBe('10:00:00')
  })
})

describe('parseDurationToSeconds', () => {
  it('解析 mm:ss 为秒', () => {
    expect(parseDurationToSeconds('0:00')).toBe(0)
    expect(parseDurationToSeconds('3:05')).toBe(185)
    expect(parseDurationToSeconds('12:34')).toBe(754)
  })

  it('空串返回 0', () => {
    expect(parseDurationToSeconds('')).toBe(0)
    expect(parseDurationToSeconds('未知')).toBe(0)
  })

  it('非两段格式返回 0（保留原内联行为）', () => {
    expect(parseDurationToSeconds('1:00:00')).toBe(0)
    expect(parseDurationToSeconds('abc')).toBe(0)
  })
})

describe('formatTimeForLrc', () => {
  it('毫秒转 mm:ss.cs 两位补零', () => {
    expect(formatTimeForLrc(0)).toBe('00:00.00')
    expect(formatTimeForLrc(1850)).toBe('00:01.85')
    expect(formatTimeForLrc(62500)).toBe('01:02.50')
  })
})
