import { describe, expect, it } from 'vitest'

import type { Track } from '../types'
import { classifyQuality } from './audio-quality'

function make(partial: Partial<Track>): Track {
  return {
    id: 't',
    title: 't',
    artists: [],
    album: 'a',
    albumId: null,
    duration: 0,
    trackNumber: null,
    year: null,
    filePath: '',
    fileSize: 0,
    format: '',
    bitrate: null,
    sampleRate: null,
    hasCover: false,
    addedAt: 0,
    modifiedAt: 0,
    ...partial,
  } as Track
}

describe('classifyQuality', () => {
  it('解析占位 / 元数据未就绪时返回 null', () => {
    expect(classifyQuality(make({ metadataPending: true }))).toBeNull()
    expect(classifyQuality(make({ isoPlaceholder: true }))).toBeNull()
  })

  it('DSD：原生 DSD 容器', () => {
    expect(classifyQuality(make({ format: 'dsf' }))).toBe('dsd')
    expect(classifyQuality(make({ format: 'dff' }))).toBe('dsd')
  })

  it('DSD：SACD 虚拟分轨带原始 DSD 采样率', () => {
    expect(classifyQuality(make({ isSacd: true, sacdDsdRate: 2822400 }))).toBe('dsd')
  })

  it('无损按采样率分 SQ / HR / Hi-Res', () => {
    expect(classifyQuality(make({ format: 'flac', sampleRate: 44100 }))).toBe('sq')
    expect(classifyQuality(make({ format: 'flac', sampleRate: 48000 }))).toBe('hr')
    expect(classifyQuality(make({ format: 'flac', sampleRate: 96000 }))).toBe('hires')
  })

  it('有损按比特率分 HQ / 标准', () => {
    expect(classifyQuality(make({ format: 'mp3', bitrate: 320000 }))).toBe('hq')
    expect(classifyQuality(make({ format: 'mp3', bitrate: 192000 }))).toBe('standard')
    expect(classifyQuality(make({ format: 'aac', bitrate: 128000 }))).toBe('standard')
  })

  it('高采样率 m4a 视为 ALAC 无损', () => {
    expect(classifyQuality(make({ format: 'm4a', sampleRate: 96000 }))).toBe('hires')
  })

  it('无损但缺采样率回落 SQ', () => {
    expect(classifyQuality(make({ format: 'flac', sampleRate: null }))).toBe('sq')
  })

  it('有损但缺比特率无法判定', () => {
    expect(classifyQuality(make({ format: 'mp3', bitrate: null }))).toBeNull()
  })

  it('DTS 核心（有损）按比特率分 HQ / 标准', () => {
    expect(classifyQuality(make({ format: 'dts', codec: 'dts', codecProfile: 'DTS', bitrate: 320000 }))).toBe('hq')
    expect(classifyQuality(make({ format: 'dts', codec: 'dts', codecProfile: 'DTS', bitrate: 192000 }))).toBe('standard')
  })

  it('DTS-HD MA（无损母带）按采样率分 hires / hr / sq', () => {
    expect(
      classifyQuality(make({ format: 'dtshd', codec: 'dts', codecProfile: 'DTS-HD MA', sampleRate: 96000 })),
    ).toBe('hires')
    expect(
      classifyQuality(make({ format: 'dtshd', codec: 'dts', codecProfile: 'DTS-HD MA', sampleRate: 48000 })),
    ).toBe('hr')
    expect(
      classifyQuality(make({ format: 'dtshd', codec: 'dts', codecProfile: 'DTS-HD MA', sampleRate: 44100 })),
    ).toBe('sq')
  })

  it('DTS-HD HRA（有损高解析）按采样率分 hires / hr', () => {
    expect(
      classifyQuality(make({ format: 'dts', codec: 'dts', codecProfile: 'DTS-HD HRA', sampleRate: 96000 })),
    ).toBe('hires')
    expect(
      classifyQuality(make({ format: 'dts', codec: 'dts', codecProfile: 'DTS-HD HRA', sampleRate: 48000 })),
    ).toBe('hr')
  })

  it('DTS 容器包裹在 wav 中也能按 profile 识别', () => {
    expect(
      classifyQuality(make({ format: 'wav', codec: 'dts', codecProfile: 'DTS-HD MA', sampleRate: 96000 })),
    ).toBe('hires')
  })

  it('DTS 缺 profile 且缺比特率无法判定', () => {
    expect(classifyQuality(make({ format: 'dts', codec: 'dts', codecProfile: null, bitrate: null }))).toBeNull()
  })
})
