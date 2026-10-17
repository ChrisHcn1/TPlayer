import { describe, expect, it } from 'vitest'

import { computeDurationFromWav } from './index'

/** 构造 16-bit PCM WAV 的 ArrayBuffer（与 decoders.encodeWav 同布局，可插入额外 chunk） */
function buildWav(opts: {
  sampleRate: number
  channels: number
  /** 16-bit 样本帧数（每帧 channels 个样本） */
  frames: number
  /** 在 fmt 与 data 之间插入的额外 chunk，默认无 */
  extraChunks?: Array<{ id: string; payload: number[] }>
  /** 覆盖 header 中的 channels（模拟坏 header） */
  overrideChannels?: number
  /** 覆盖 header 中的 sampleRate（模拟坏 header） */
  overrideSampleRate?: number
}): ArrayBuffer {
  const bytesPerSample = 2
  const dataSize = opts.frames * opts.channels * bytesPerSample
  const extra = opts.extraChunks ?? []
  const extraSize = extra.reduce((sum, c) => sum + 8 + c.payload.length + (c.payload.length % 2), 0)
  const buffer = new ArrayBuffer(44 + extraSize + dataSize)
  const view = new DataView(buffer)
  const writeStr = (off: number, s: string) => {
    for (let i = 0; i < s.length; i++) view.setUint8(off + i, s.charCodeAt(i))
  }
  writeStr(0, 'RIFF')
  view.setUint32(4, buffer.byteLength - 8, true)
  writeStr(8, 'WAVE')
  writeStr(12, 'fmt ')
  view.setUint32(16, 16, true)
  view.setUint16(20, 1, true) // PCM
  view.setUint16(22, opts.overrideChannels ?? opts.channels, true)
  view.setUint32(24, opts.overrideSampleRate ?? opts.sampleRate, true)
  view.setUint32(28, (opts.overrideSampleRate ?? opts.sampleRate) * opts.channels * bytesPerSample, true)
  view.setUint16(32, opts.channels * bytesPerSample, true)
  view.setUint16(34, 16, true)
  let off = 36
  for (const chunk of extra) {
    writeStr(off, chunk.id)
    view.setUint32(off + 4, chunk.payload.length, true)
    chunk.payload.forEach((b, i) => view.setUint8(off + 8 + i, b))
    off += 8 + chunk.payload.length + (chunk.payload.length % 2)
  }
  writeStr(off, 'data')
  view.setUint32(off + 4, dataSize, true)
  return buffer
}

describe('computeDurationFromWav', () => {
  it('标准 44 字节头：2 秒 44.1kHz 立体声 → 2000ms', async () => {
    const wav = buildWav({ sampleRate: 44100, channels: 2, frames: 88200 })
    expect(await computeDurationFromWav(wav)).toBe(2000)
  })

  it('单声道 48kHz：3 秒 → 3000ms', async () => {
    const wav = buildWav({ sampleRate: 48000, channels: 1, frames: 144000 })
    expect(await computeDurationFromWav(wav)).toBe(3000)
  })

  it('fmt 与 data 之间有额外 chunk（LIST）时跳过并正确定位 data', async () => {
    const wav = buildWav({
      sampleRate: 44100,
      channels: 2,
      frames: 88200,
      extraChunks: [{ id: 'LIST', payload: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] }],
    })
    expect(await computeDurationFromWav(wav)).toBe(2000)
  })

  it('奇数长度 chunk 补齐 padding 后仍能正确定位 data', async () => {
    const wav = buildWav({
      sampleRate: 44100,
      channels: 2,
      frames: 88200,
      extraChunks: [{ id: 'cue ', payload: [1, 2, 3, 4, 5, 6, 7, 8, 9] }],
    })
    expect(await computeDurationFromWav(wav)).toBe(2000)
  })

  it('header channels 为 0 时回退到 fallbackChannels', async () => {
    const wav = buildWav({ sampleRate: 44100, channels: 2, frames: 88200, overrideChannels: 0 })
    expect(await computeDurationFromWav(wav, undefined, 2)).toBe(2000)
  })

  it('header sampleRate 为 0 时回退到 fallbackSampleRate', async () => {
    const wav = buildWav({ sampleRate: 44100, channels: 2, frames: 88200, overrideSampleRate: 0 })
    expect(await computeDurationFromWav(wav, 44100, undefined)).toBe(2000)
  })

  it('无 data chunk 返回 0', async () => {
    const buffer = new ArrayBuffer(44)
    const view = new DataView(buffer)
    const writeStr = (off: number, s: string) => {
      for (let i = 0; i < s.length; i++) view.setUint8(off + i, s.charCodeAt(i))
    }
    writeStr(0, 'RIFF')
    view.setUint32(4, 36, true)
    writeStr(8, 'WAVE')
    writeStr(12, 'fmt ')
    view.setUint32(16, 16, true)
    view.setUint16(20, 1, true)
    view.setUint16(22, 2, true)
    view.setUint32(24, 44100, true)
    expect(await computeDurationFromWav(buffer)).toBe(0)
  })

  it('接受 Blob 输入并得到相同结果', async () => {
    const wav = buildWav({ sampleRate: 44100, channels: 2, frames: 88200 })
    const blob = new Blob([new Uint8Array(wav)], { type: 'audio/wav' })
    // happy-dom 的 Blob 未实现 arrayBuffer()（真实 WebView2/Chromium 均支持），测试内补齐
    if (typeof blob.arrayBuffer !== 'function') {
      Object.defineProperty(Blob.prototype, 'arrayBuffer', {
        value: () => Promise.resolve(wav),
        configurable: true,
      })
    }
    expect(await computeDurationFromWav(blob)).toBe(2000)
  })
})
