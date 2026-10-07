import { describe, it, expect } from 'vitest'
import {
  getFileExtension,
  isBrowserScannableAudio,
  BROWSER_SCANNABLE_AUDIO_EXTENSIONS
} from '../fileTypes'

describe('getFileExtension', () => {
  it('提取小写扩展名（含点号）', () => {
    expect(getFileExtension('song.mp3')).toBe('.mp3')
    expect(getFileExtension('artist - title.FLAC')).toBe('.flac')
  })

  it('多 dot 文件名只取最后一段', () => {
    expect(getFileExtension('artist - song.v2.m4a')).toBe('.m4a')
  })

  it('无扩展名返回空字符串', () => {
    expect(getFileExtension('README')).toBe('')
    expect(getFileExtension('.bashrc')).toBe('')
  })
})

describe('isBrowserScannableAudio', () => {
  it('浏览器可播放格式返回 true（大小写不敏感）', () => {
    for (const ext of BROWSER_SCANNABLE_AUDIO_EXTENSIONS) {
      expect(isBrowserScannableAudio(`track${ext}`)).toBe(true)
    }
    expect(isBrowserScannableAudio('TRACK.MP3')).toBe(true)
  })

  it('浏览器无法处理的无损格式返回 false', () => {
    expect(isBrowserScannableAudio('song.dsf')).toBe(false)
    expect(isBrowserScannableAudio('song.ape')).toBe(false)
    expect(isBrowserScannableAudio('song.wma')).toBe(false)
  })

  it('非音频文件与无扩展名文件返回 false', () => {
    expect(isBrowserScannableAudio('cover.jpg')).toBe(false)
    expect(isBrowserScannableAudio('README')).toBe(false)
  })
})
