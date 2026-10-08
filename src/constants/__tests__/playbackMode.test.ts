import { describe, it, expect } from 'vitest'
import { type PlaybackMode, PLAYBACK_MODES, getPlaybackModeImage } from '../playbackMode'

describe('PLAYBACK_MODES', () => {
  it('包含 order/random/repeat 三种模式', () => {
    expect(PLAYBACK_MODES).toContain('order')
    expect(PLAYBACK_MODES).toContain('random')
    expect(PLAYBACK_MODES).toContain('repeat')
  })

  it('长度为 3（轮转切换依赖此顺序）', () => {
    expect(PLAYBACK_MODES.length).toBe(3)
  })

  it('无重复项', () => {
    const set = new Set(PLAYBACK_MODES)
    expect(set.size).toBe(PLAYBACK_MODES.length)
  })

  it('首项为 order（默认值）', () => {
    expect(PLAYBACK_MODES[0]).toBe('order')
  })
})

describe('getPlaybackModeImage', () => {
  it('order 返回 play-button 图标', () => {
    expect(getPlaybackModeImage('order')).toBe('/play-button_25b6-fe0f.png')
  })

  it('random 返回 shuffle-tracks-button 图标', () => {
    expect(getPlaybackModeImage('random')).toBe('/shuffle-tracks-button_1f500.png')
  })

  it('repeat 返回 repeat-button 图标', () => {
    expect(getPlaybackModeImage('repeat')).toBe('/repeat-button_1f501.png')
  })

  it('每个 PLAYBACK_MODES 项都有对应图标（白名单漂移防护）', () => {
    for (const mode of PLAYBACK_MODES) {
      const icon = getPlaybackModeImage(mode)
      expect(icon).toMatch(/^\/.*\.png$/)
      expect(icon.length).toBeGreaterThan(0)
    }
  })
})

describe('PlaybackMode 类型', () => {
  it('编译期类型检查（运行时无操作，仅占位确保类型导出可用）', () => {
    const sample: PlaybackMode = 'order'
    expect(sample).toBe('order')
  })
})
