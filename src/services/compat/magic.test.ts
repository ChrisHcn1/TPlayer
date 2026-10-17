/**
 * magic.ts 魔数识别单测 —— 覆盖 18 种支持格式 + WAV 容器深检（DTS-in-WAV）场景。
 * 目标：冷门格式识别无漏判、常规格式无误判。
 */
import { describe, expect, it } from 'vitest'

import { detectFormat, isNativePlayable } from './magic'

function u8(...bytes: number[]): ArrayBuffer {
  return new Uint8Array(bytes).buffer
}

function asciiBytes(s: string): number[] {
  return [...s].map((c) => c.charCodeAt(0) & 0xff)
}

/** 组装 RIFF/WAVE 文件头：fmt(16B PCM 44.1k/16bit/stereo) + 可选前置 chunk + data */
function wavFile(data: number[], preChunk?: { id: string; payload: number[] }): ArrayBuffer {
  const body: number[] = [...asciiBytes('WAVE')]
  body.push(...asciiBytes('fmt '), 16, 0, 0, 0)
  body.push(1, 0, 2, 0) // PCM, stereo
  body.push(0x44, 0xac, 0, 0) // 44100
  body.push(0x10, 0xb1, 2, 0) // byte rate
  body.push(4, 0, 16, 0) // align, bits
  if (preChunk) {
    const payload = preChunk.payload
    body.push(...asciiBytes(preChunk.id), payload.length & 0xff, (payload.length >> 8) & 0xff, 0, 0)
    body.push(...payload)
    if (payload.length % 2 !== 0) body.push(0)
  }
  const n = data.length
  body.push(...asciiBytes('data'), n & 0xff, (n >> 8) & 0xff, (n >> 16) & 0xff, (n >> 24) & 0xff)
  body.push(...data)
  const riff: number[] = [...asciiBytes('RIFF')]
  const len = body.length
  riff.push(len & 0xff, (len >> 8) & 0xff, (len >> 16) & 0xff, (len >> 24) & 0xff)
  return new Uint8Array([...riff, ...body]).buffer
}

/** 标准 DTS-CD 帧（16-bit BE 同步字） */
const DTS_SYNC_BE = [0x7f, 0xfe, 0x80, 0x01]
const DTS_PAYLOAD = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66]

describe('常规标准格式 → 原生流程（isNativePlayable）', () => {
  it('真 PCM WAV 识别为 wav，不被误判为 DTS', () => {
    // 真 PCM 音乐样本起始（非 DTS 同步字）
    const f = wavFile([0x00, 0x00, 0xfe, 0xff, 0x24, 0x17, 0xc4, 0x9b], {
      id: 'LIST',
      payload: [...asciiBytes('INFOIART20:00:00:00TPlayer')],
    })
    expect(detectFormat(f).kind).toBe('wav')
    expect(isNativePlayable(detectFormat(f).kind)).toBe(true)
  })

  it('MP3（ID3v2 / 裸帧同步字）', () => {
    expect(detectFormat(u8(...asciiBytes('ID3'), 3, 0, 0, 0, 0, 0)).kind).toBe('mp3')
    expect(detectFormat(u8(0xff, 0xfb, 0x90, 0x00)).kind).toBe('mp3')
  })

  it('FLAC / OGG', () => {
    expect(detectFormat(u8(...asciiBytes('fLaC'))).kind).toBe('flac')
    expect(detectFormat(u8(...asciiBytes('OggS'), 0, 2)).kind).toBe('ogg')
  })

  it('AAC（ADIF / ADTS）/ M4A', () => {
    expect(detectFormat(u8(...asciiBytes('ADIF'))).kind).toBe('aac')
    expect(detectFormat(u8(0xff, 0xf1, 0x50, 0x80)).kind).toBe('aac')
    expect(detectFormat(u8(0, 0, 0, 0x20, ...asciiBytes('ftypM4A '))).kind).toBe('aac')
  })
})

describe('WAV 容器深检 → 伪装 WAV 识别为冷门格式', () => {
  it('DTS-ES 封装在 WAV（标准 44B 头 + 7FFE8001）识别为 dts', () => {
    const f = wavFile([...DTS_SYNC_BE, ...DTS_PAYLOAD])
    const d = detectFormat(f)
    expect(d.kind).toBe('dts')
    expect(d.description).toContain('WAV')
    expect(isNativePlayable(d.kind)).toBe(false)
  })

  it('带 LIST/INFO 前置 chunk 的 DTS-in-WAV（data 偏移 >64B）仍能识别', () => {
    const f = wavFile([...DTS_SYNC_BE, ...DTS_PAYLOAD], {
      id: 'LIST',
      payload: [...asciiBytes('INFOIART40:00:00:00G.E.M.邓紫棋 泡沫 DTS-ES')],
    })
    expect(detectFormat(f).kind).toBe('dts')
  })

  it('DTS 同步字全部 5 种变体（16/14-bit × 大小端 + DTS-HD）', () => {
    const syncs = [
      [0x7f, 0xfe, 0x80, 0x01], // 16-bit BE
      [0xff, 0x1f, 0x00, 0xe8], // 14-bit BE
      [0xe8, 0x00, 0x1f, 0xff], // 14-bit LE
      [0x01, 0x80, 0xfe, 0x7f], // 16-bit LE
      [0x64, 0x58, 0x20, 0x25], // DTS-HD "dX %"
    ]
    for (const s of syncs) {
      expect(detectFormat(wavFile([...s, ...DTS_PAYLOAD])).kind).toBe('dts')
      expect(detectFormat(u8(...s, ...DTS_PAYLOAD)).kind).toBe('dts') // 裸流同表
    }
  })

  it('WavPack 封装在 WAV 中识别为 wv', () => {
    const f = wavFile([...asciiBytes('wvpk'), 0x28, 0x00, 0x00, 0x00])
    expect(detectFormat(f).kind).toBe('wv')
  })

  it('头部截断（data chunk 不可见）保持保守判为 wav', () => {
    // 只有 RIFF+WAVE 头，未读到 data —— 不做无依据的冷门判定
    const f = u8(...asciiBytes('RIFF'), 0x24, 0, 0, 0, ...asciiBytes('WAVEfmt '), 16, 0, 0, 0)
    expect(detectFormat(f).kind).toBe('wav')
  })
})

describe('冷门格式魔数 → 兜底流程', () => {
  it('裸 DTS：ASCII 头与二进制同步字', () => {
    expect(detectFormat(u8(...asciiBytes('DTS '), 0, 1)).kind).toBe('dts')
    expect(detectFormat(u8(...asciiBytes('dtshd'), 0, 1)).kind).toBe('dts')
    expect(detectFormat(u8(...DTS_SYNC_BE, 0x00, 0x01)).kind).toBe('dts')
  })

  it('APE / WavPack / TTA / TAK / DSF / DFF', () => {
    expect(detectFormat(u8(...asciiBytes('MAC '), 0x96, 0x0f)).kind).toBe('ape')
    expect(detectFormat(u8(...asciiBytes('wvpk'), 0x28, 0x00)).kind).toBe('wv')
    expect(detectFormat(u8(...asciiBytes('TTA1'), 0, 1)).kind).toBe('tta')
    expect(detectFormat(u8(...asciiBytes('tBaK'), 0, 1)).kind).toBe('tak')
    expect(detectFormat(u8(...asciiBytes('DSD '), 1, 0)).kind).toBe('dsf')
    expect(detectFormat(u8(...asciiBytes('FRM8'), 0, 1)).kind).toBe('dff')
  })

  it('TAK 魔数大小写精确匹配（tBaK，此前小写错误导致漏判）', () => {
    // 小写 tbaK 不应命中 TAK（避免与真实格式混淆）
    expect(detectFormat(u8(...asciiBytes('tBaK'))).kind).toBe('tak')
  })

  it('WMA (ASF GUID) / AIFF / AMR / MIDI', () => {
    expect(detectFormat(u8(0x30, 0x26, 0xb2, 0x75, 0x8e, 0x66, 0xcf, 0x11)).kind).toBe('wma')
    expect(detectFormat(u8(...asciiBytes('FORM'), 0, 0, 0, 0, ...asciiBytes('AIFF'))).kind).toBe(
      'aiff',
    )
    expect(detectFormat(u8(...asciiBytes('FORM'), 0, 0, 0, 0, ...asciiBytes('AIFC'))).kind).toBe(
      'aiff',
    )
    expect(detectFormat(u8(...asciiBytes('#!AMR'))).kind).toBe('amr')
    expect(detectFormat(u8(...asciiBytes('MThd'))).kind).toBe('midi')
  })

  it('未知格式 → unknown（不误判）', () => {
    expect(detectFormat(u8(0xde, 0xad, 0xbe, 0xef, 0x00, 0x01)).kind).toBe('unknown')
    expect(
      detectFormat(u8(0x00, 0x00, 0x00, 0x18, ...asciiBytes('ftypisom'), 0, 0, 0)).kind,
    ).toBe('aac') // ftyp 在偏移 4 即 ISO 容器
  })
})

describe('识别结论与路由一致性', () => {
  it('isNativePlayable 仅放行 mp3/flac/wav/ogg', () => {
    for (const kind of [
      'mp3',
      'flac',
      'wav',
      'ogg',
    ] as const) {
      expect(isNativePlayable(kind)).toBe(true)
    }
    for (const kind of [
      'aac',
      'amr',
      'wma',
      'aiff',
      'dsf',
      'dff',
      'dts',
      'ape',
      'wv',
      'tta',
      'tak',
      'midi',
      'unknown',
    ] as const) {
      expect(isNativePlayable(kind)).toBe(false)
    }
  })
})
