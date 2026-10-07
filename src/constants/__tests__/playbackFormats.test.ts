import { describe, it, expect } from 'vitest'
import { needsFFplayEngine, FFPLAY_ENGINE_FORMATS } from '../playbackFormats'

describe('needsFFplayEngine', () => {
  it('HTML5 Audio 可原生解码的格式不走 FFplay（回归：m4a/aac 拖动进度条无效）', () => {
    // 这是本次修复的核心：m4a/aac 必须由 HTML5 Audio 播放与 seek
    expect(needsFFplayEngine('D:\\music\\song.m4a')).toBe(false)
    expect(needsFFplayEngine('D:/music/song.aac')).toBe(false)
    expect(needsFFplayEngine('track.mp3')).toBe(false)
    expect(needsFFplayEngine('track.flac')).toBe(false)
  })

  it('HTML5 Audio 无法解码的无损格式走 FFplay', () => {
    for (const ext of FFPLAY_ENGINE_FORMATS) {
      expect(needsFFplayEngine(`D:\\music\\song${ext}`)).toBe(true)
    }
  })

  it('扩展名匹配大小写不敏感', () => {
    expect(needsFFplayEngine('D:\\MUSIC\\SONG.APE')).toBe(true)
    expect(needsFFplayEngine('song.DSF')).toBe(true)
    expect(needsFFplayEngine('SONG.M4A')).toBe(false)
  })

  it('路径中含类似扩展名的目录名不误判', () => {
    expect(needsFFplayEngine('D:\\music\\ape.backup\\song.mp3')).toBe(false)
  })
})
