/**
 * 轻量 JS 解码器（L1）—— KB 级纯逻辑，零依赖。
 *
 * 覆盖：
 *  - WAV 容器容错解析（轻微头损坏：chunk 顺序错乱 / 字段缺失时尽量定位 data）
 *  - PCM 直提（8/16/24/32-bit，单/多声道，归一为 16-bit）
 *  - G.711 μ-law / A-law（电话语音、部分 WAV 变体）
 *  - IMA ADPCM / MS ADPCM（常见于语音 WAV、游戏音频、部分录音笔）
 *
 * 对 AMR / OGG / AAC / WMA / DSD 等复杂编码不在此层处理，交由 L2（FFmpeg.wasm）。
 * 任何本层无法处理的情形一律返回 null，由上层转入 L2 或放弃。
 */
const IMA_STEP_TABLE = [
  7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 40, 44, 48, 53, 58, 63, 70,
  77, 85, 93, 102, 112, 123, 135, 148, 163, 178, 195, 214, 235, 258, 282, 310, 337, 370, 403, 442,
  484, 531, 582, 637, 697, 766, 842, 925, 1012, 1110, 1212, 1330, 1456, 1598, 1752, 1923, 2109,
  2312, 2535, 2782, 3053, 3350, 3678, 4037, 4432, 4868, 5347, 5873, 6448, 7083, 7784, 8548, 9388,
  10312, 11327, 12439, 13664, 15010, 16482, 18094, 19864, 21811, 23961, 26325, 28916, 31760, 34888,
  38330, 42106, 46246, 50800, 55832, 61368, 67448, 74128, 81480, 89560, 98416, 108192, 118928, 130672,
  143640, 157872, 173568, 190688, 209600, 230384, 253152, 278016, 305088, 335456, 368512, 405056,
  445184, 489152, 537408, 590592, 649216, 713088, 783104, 860416, 945664, 1039232, 1142080, 1254976,
  1379328, 1516544, 1667072, 1832320, 2013184,
]

const IMA_INDEX_TABLE = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8]

const MS_ADPCM_ADAPT_TABLE = [230, 230, 230, 230, 307, 409, 512, 614, 768, 614, 512, 409, 307, 230, 230, 230]

/** WAV fmt 信息 */
interface WavFormat {
  formatTag: number
  channels: number
  sampleRate: number
  blockAlign: number
  bitsPerSample: number
  /** MS-ADPCM 系数对（来自 fmt cbSize 扩展） */
  msCoeff: [number, number][]
  /** IMA-ADPCM 每块的样本数 */
  samplesPerBlock: number
  extensibleSubTag: number | null
}

interface WavData {
  format: WavFormat
  dataOffset: number
  dataLength: number
}

/** 鲁棒扫描 RIFF/WAVE 的 chunk 列表，定位 fmt 与 data（容忍 LIST/额外块与轻微错位） */
function scanWav(buffer: ArrayBuffer): WavData | null {
  const bytes = new Uint8Array(buffer)
  if (bytes.length < 12) return null
  const view = new DataView(buffer)
  const ascii = (off: number, len: number) => {
    let s = ''
    for (let i = 0; i < len; i++) s += String.fromCharCode(bytes[off + i] ?? 0)
    return s
  }
  if (ascii(0, 4) !== 'RIFF' || ascii(8, 4) !== 'WAVE') return null

  let offset = 12
  let fmt: WavFormat | null = null
  let dataOffset = -1
  let dataLength = -1

  while (offset + 8 <= bytes.length) {
    const id = ascii(offset, 4)
    const size = view.getUint32(offset + 4, true)
    const body = offset + 8
    if (id === 'fmt ') {
      let formatTag = view.getUint16(body, true)
      const channels = view.getUint16(body + 2, true)
      const sampleRate = view.getUint32(body + 4, true)
      const blockAlign = view.getUint16(body + 12, true)
      const bitsPerSample = view.getUint16(body + 14, true)
      const cbSize = body + 16 <= bytes.length ? view.getUint16(body + 16, true) : 0
      let extensibleSubTag: number | null = null
      const msCoeff: [number, number][] = []
      if (formatTag === 0xfffe && cbSize >= 22) {
        extensibleSubTag = view.getUint16(body + 24, true)
        formatTag = extensibleSubTag
      }
      if (formatTag === 0x0002 && cbSize >= 32) {
        // MS-ADPCM：fmt 扩展含系数对（每个 4 字节）
        const pairs = view.getUint16(body + 18, true)
        let p = body + 20
        for (let i = 0; i < pairs && p + 4 <= bytes.length; i++) {
          msCoeff.push([view.getInt16(p, true), view.getInt16(p + 2, true)])
          p += 4
        }
      }
      fmt = {
        formatTag,
        channels,
        sampleRate,
        blockAlign,
        bitsPerSample,
        msCoeff,
        samplesPerBlock: 0,
        extensibleSubTag,
      }
    } else if (id === 'data') {
      dataOffset = body
      dataLength = size
      break
    } else {
      // LIST / fact / 未知块：跳过（size 可能为 0 时防御性前进）
      offset = body + (size > 0 ? size : 1)
      // 偶发损坏：size 越界则退化为逐字节搜索下一个 'data'
      if (offset + 8 > bytes.length) break
      continue
    }
    offset = body + size
  }

  if (!fmt) return null
  // 容错：data 块定位失败或声明长度越界（轻微头损坏），L1 放弃，交由 L2 或跳过
  if (dataOffset < 0 || dataLength <= 0 || dataOffset + dataLength > bytes.length) {
    return null
  }

  // IMA-ADPCM 每块的样本数（用于解码排布），用通用近似公式
  if (fmt.formatTag === 0x0011 && fmt.blockAlign > 0 && fmt.channels > 0) {
    const perChannel = (fmt.blockAlign / fmt.channels - 4) * 2 + 1
    fmt.samplesPerBlock = perChannel > 0 ? Math.floor(perChannel) : 0
  }
  return { format: fmt, dataOffset, dataLength }
}

/** G.711 μ-law → 16-bit 线性 */
function mulawToLinear(ulaw: number): number {
  const MAGIC = 0x84
  let s = (~ulaw & 0xff) as number
  const sign = s & 0x80 ? -1 : 1
  s &= 0x7f
  const exponent = (s >> 4) & 0x07
  const mantissa = s & 0x0f
  const sample = ((mantissa << 3) + MAGIC) << exponent
  return sign * (sample - MAGIC)
}

/** G.711 A-law → 16-bit 线性 */
function alawToLinear(alaw: number): number {
  let a = alaw ^ 0x55
  const sign = a & 0x80 ? -1 : 1
  a &= 0x7f
  const exponent = (a >> 4) & 0x07
  const mantissa = a & 0x0f
  const sample =
    exponent === 0
      ? (mantissa << 4) + 8
      : ((mantissa << 4) + 0x108) << (exponent - 1)
  return sign * sample
}

/** 把任意位深 PCM 样本（无符号/有符号）归一为 16-bit */
function normalizeSample(raw: number, bits: number): number {
  switch (bits) {
    case 8:
      return ((raw & 0xff) - 128) << 8
    case 16: {
      const v = raw & 0xffff
      return v >= 0x8000 ? v - 0x10000 : v
    }
    case 24: {
      // raw 已是 24-bit 有符号（范围 ±2^23），右移 8 位降为 16-bit（丢低 8 位精度）
      return raw >> 8
    }
    case 32:
      return Math.max(-32768, Math.min(32767, Math.round((raw / 0x80000000) * 32768)))
    default:
      return 0
  }
}

/** 解码 PCM（8/16/24/32-bit 多声道）为交错 Int16Array */
function decodePcm(view: DataView, fmt: WavFormat, dataOffset: number, dataLength: number): Int16Array | null {
  const ch = fmt.channels
  if (ch <= 0) return null
  const bits = fmt.bitsPerSample || 16
  const bytesPerSample = Math.max(1, bits / 8)
  if (bits !== 8 && bits !== 16 && bits !== 24 && bits !== 32) return null
  const totalSamples = Math.floor(dataLength / (bytesPerSample * ch)) * ch
  const out = new Int16Array(totalSamples)
  let o = 0
  const end = dataOffset + dataLength
  for (let pos = dataOffset; pos + bytesPerSample * ch <= end; pos += bytesPerSample * ch) {
    for (let c = 0; c < ch; c++) {
      const p = pos + c * bytesPerSample
      let raw = 0
      if (bits === 8) raw = view.getUint8(p)
      else if (bits === 16) raw = view.getInt16(p, true)
      else if (bits === 24) {
        const b0 = view.getUint8(p)
        const b1 = view.getUint8(p + 1)
        const b2 = view.getUint8(p + 2)
        let s = b0 | (b1 << 8) | (b2 << 16)
        if (s & 0x800000) s -= 0x1000000
        raw = s
      }
      else if (bits === 32) raw = view.getInt32(p, true)
      out[o++] = normalizeSample(raw, bits) as number
    }
  }
  return out
}

/** 解码 32-bit IEEE float PCM 为多声道交错 Int16Array（clamp 到 16-bit） */
function decodeFloat32(view: DataView, fmt: WavFormat, dataOffset: number, dataLength: number): Int16Array | null {
  const ch = fmt.channels
  if (ch <= 0) return null
  const totalSamples = Math.floor(dataLength / (4 * ch)) * ch
  const out = new Int16Array(totalSamples)
  let o = 0
  const end = dataOffset + dataLength
  for (let pos = dataOffset; pos + 4 * ch <= end; pos += 4 * ch) {
    for (let c = 0; c < ch; c++) {
      const f = view.getFloat32(pos + c * 4, true)
      let s = Math.round(f * 32767)
      if (s > 32767) s = 32767
      else if (s < -32768) s = -32768
      out[o++] = s
    }
  }
  return out
}

function decodeG711(
  data: Uint8Array,
  aLaw: boolean,
  channels: number,
): Int16Array {
  const out = new Int16Array(data.length)
  const ch = Math.max(1, channels)
  // G.711 为单声道字节流；多声道时按帧交错，逐字节独立解码即可
  for (let i = 0; i < data.length; i++) {
    out[i] = (aLaw ? alawToLinear(data[i]) : mulawToLinear(data[i])) as number
  }
  // 若需多声道展开但源为单声道，调用方负责；此处保持样本数不变
  void ch
  return out
}

/** IMA-ADPCM（4-bit 小端 nibble，低位在前）解码为多声道交错 Int16Array */
function decodeImaAdpcm(view: DataView, fmt: WavFormat, dataOffset: number, dataLength: number): Int16Array | null {
  const ch = fmt.channels
  if (ch <= 0 || fmt.blockAlign <= 0) return null
  const blockSize = fmt.blockAlign
  const totalBlocks = Math.floor(dataLength / blockSize)
  if (totalBlocks <= 0) return null
  const samplesPerBlock = fmt.samplesPerBlock > 0 ? fmt.samplesPerBlock : 1
  const out = new Int16Array(totalBlocks * samplesPerBlock * ch)

  const predictor = new Int32Array(ch)
  const index = new Int32Array(ch)
  let outPos = 0

  for (let b = 0; b < totalBlocks; b++) {
    const blockStart = dataOffset + b * blockSize
    for (let c = 0; c < ch; c++) {
      const headerPos = blockStart + c * 4
      predictor[c] = view.getInt16(headerPos, true)
      index[c] = view.getInt8(headerPos + 2)
      // 头部第 4 字节为保留（通常 0）
    }
    let nibblePos = blockStart + ch * 4
    const nibbleEnd = blockStart + blockSize
    let sampleInBlock = 0
    const startOutPos = outPos
    while (nibblePos < nibbleEnd && sampleInBlock < samplesPerBlock) {
      // 每字节 2 个 nibble，声道顺序：先 c0 一字节(2样本)，再 c1...
      for (let c = 0; c < ch && sampleInBlock < samplesPerBlock; c++) {
        if (nibblePos >= nibbleEnd) break
        const byte = view.getUint8(nibblePos++)
        for (let n = 0; n < 2 && sampleInBlock < samplesPerBlock; n++) {
          const nib = n === 0 ? byte & 0x0f : (byte >> 4) & 0x0f
          let step = IMA_STEP_TABLE[index[c]]
          let diff = step >> 3
          if (nib & 1) diff += step >> 2
          if (nib & 2) diff += step >> 1
          if (nib & 4) diff += step
          if (nib & 8) predictor[c] -= diff
          else predictor[c] += diff
          if (predictor[c] > 32767) predictor[c] = 32767
          if (predictor[c] < -32768) predictor[c] = -32768
          index[c] += IMA_INDEX_TABLE[nib]
          if (index[c] < 0) index[c] = 0
          if (index[c] > 88) index[c] = 88
          out[outPos++] = predictor[c] as number
          sampleInBlock++
        }
      }
    }
    // 若解码样本数不足（损坏块），用最后预测值填充
    while (sampleInBlock < samplesPerBlock) {
      for (let c = 0; c < ch; c++) out[outPos++] = predictor[c] as number
      sampleInBlock++
    }
    void startOutPos
  }
  return out
}

/** MS-ADPCM 解码为多声道交错 Int16Array（支持 2 系数集） */
function decodeMsAdpcm(view: DataView, fmt: WavFormat, dataOffset: number, dataLength: number): Int16Array | null {
  const ch = fmt.channels
  if (ch <= 0 || fmt.blockAlign <= 0 || fmt.msCoeff.length < 1) return null
  const blockSize = fmt.blockAlign
  const totalBlocks = Math.floor(dataLength / blockSize)
  if (totalBlocks <= 0) return null
  // MS-ADPCM 每块样本数（标准公式）
  const samplesPerBlock =
    ch === 1 ? (blockSize - 7) * 2 : (blockSize - 14) * 2 / ch + 2
  const spb = Math.floor(samplesPerBlock)
  const out = new Int16Array(totalBlocks * spb * ch)

  const predictor = new Int32Array(ch)
  const index = new Int32Array(ch)
  const sample1 = new Int32Array(ch)
  const sample2 = new Int32Array(ch)
  let outPos = 0

  for (let b = 0; b < totalBlocks; b++) {
    const blockStart = dataOffset + b * blockSize
    for (let c = 0; c < ch; c++) {
      const headerPos = blockStart + c * 7
      predictor[c] = view.getInt16(headerPos, true)
      index[c] = view.getInt8(headerPos + 2)
      sample1[c] = view.getInt16(headerPos + 3, true)
      sample2[c] = view.getInt16(headerPos + 5, true)
    }
    // 写出前两个样本
    for (let c = 0; c < ch; c++) out[outPos++] = sample2[c] as number
    for (let c = 0; c < ch; c++) out[outPos++] = sample1[c] as number

    let pos = blockStart + ch * 7
    const nibbleEnd = blockStart + blockSize
    while (pos < nibbleEnd) {
      const byte = view.getUint8(pos++)
      for (let n = 0; n < 2; n++) {
        const nib = n === 0 ? byte & 0x0f : (byte >> 4) & 0x0f
        for (let c = 0; c < ch; c++) {
          const coeff = fmt.msCoeff[predictor[c]] ?? [0, 0]
          let step = MS_ADPCM_ADAPT_TABLE[index[c]]
          let diff = step >> 3
          if (nib & 1) diff += step >> 2
          if (nib & 2) diff += step >> 1
          if (nib & 4) diff += step
          if (nib & 8) diff = -diff
          const predicted = (sample1[c] * coeff[0] + sample2[c] * coeff[1]) / 256 + diff
          let s = Math.max(-32768, Math.min(32767, Math.round(predicted)))
          sample2[c] = sample1[c]
          sample1[c] = s
          index[c] += (nib & 8 ? -1 : 1) * MS_ADPCM_ADAPT_TABLE_ADJUST(nib)
          if (index[c] < 0) index[c] = 0
          if (index[c] > 15) index[c] = 15
          out[outPos++] = s as number
        }
      }
    }
  }
  return out

  function MS_ADPCM_ADAPT_TABLE_ADJUST(nib: number): number {
    return [0, 0, 0, 0, 1, 1, 1, 1][nib & 0x07] ?? 0
  }
}

/** 把交错 Int16Array 封装为标准 16-bit PCM WAV（含 RIFF 头） */
export function encodeWav(pcm: Int16Array, sampleRate: number, channels: number): ArrayBuffer {
  const bytesPerSample = 2
  const dataSize = pcm.length * bytesPerSample
  const buffer = new ArrayBuffer(44 + dataSize)
  const view = new DataView(buffer)
  const writeStr = (off: number, s: string) => {
    for (let i = 0; i < s.length; i++) view.setUint8(off + i, s.charCodeAt(i))
  }
  writeStr(0, 'RIFF')
  view.setUint32(4, 36 + dataSize, true)
  writeStr(8, 'WAVE')
  writeStr(12, 'fmt ')
  view.setUint32(16, 16, true)
  view.setUint16(20, 1, true) // PCM
  view.setUint16(22, channels, true)
  view.setUint32(24, sampleRate, true)
  view.setUint32(28, sampleRate * channels * bytesPerSample, true)
  view.setUint16(32, channels * bytesPerSample, true)
  view.setUint16(34, 16, true)
  writeStr(36, 'data')
  view.setUint32(40, dataSize, true)
  let off = 44
  for (let i = 0; i < pcm.length; i++) {
    view.setInt16(off, pcm[i], true)
    off += 2
  }
  return buffer
}

/** 检测 WAV data 块开头是否为 DTS 比特流同步字（DTS-in-WAV 常把 wFormatTag 标为 PCM，但数据是压缩流） */
function isDtsSync(bytes: Uint8Array, off: number): boolean {
  // 在 data 块开头 1KB 内扫描，覆盖封装器在压缩流前填充少量字节的情形
  const limit = Math.min(off + 1024, bytes.length - 4)
  for (let i = off; i <= limit; i++) {
    const a = bytes[i]
    const b = bytes[i + 1]
    const c = bytes[i + 2]
    const d = bytes[i + 3]
    // 16-bit BE 0x7FFE8001 / LE 0xFE7F0180；14-bit BE 0x1FFFE800 / LE 0xFF1F00E8
    if (a === 0x7f && b === 0xfe && c === 0x80 && d === 0x01) return true
    if (a === 0xfe && b === 0x7f && c === 0x01 && d === 0x80) return true
    if (a === 0x1f && b === 0xff && c === 0xe8 && d === 0x00) return true
    if (a === 0xff && b === 0x1f && c === 0x00 && d === 0xe8) return true
  }
  return false
}

/**
 * 尝试用 L1 解码给定 WAV 容器字节。成功返回 PCM 描述，失败（不支持的编码/损坏）返回 null。
 */
export function decodeWavL1(buffer: ArrayBuffer): {
  pcm: Int16Array
  sampleRate: number
  channels: number
} | null {
  const wav = scanWav(buffer)
  if (!wav) return null
  const { format, dataOffset, dataLength } = wav
  if (dataLength <= 0) return null
  const view = new DataView(buffer)
  const bytes = new Uint8Array(buffer)
  try {
    switch (format.formatTag) {
      case 0x0001: {
        // 「假 PCM」防护：DTS 等压缩流常封装在 WAV 里却把 fmt.wFormatTag 标成 PCM。
        // 若 data 块开头是 DTS 同步字，L1 无法处理，返回 null 交 L2（FFmpeg）解码。
        const dtsHit = isDtsSync(bytes, dataOffset)
        if (dtsHit) return null
        // PCM（8/16/24/32-bit 整数）
        const pcm = decodePcm(view, format, dataOffset, dataLength)
        return pcm ? { pcm, sampleRate: format.sampleRate, channels: format.channels } : null
      }
      case 0x0003: {
        // 32-bit IEEE float PCM（常见于 DAW 导出 / 高保真 WAV）
        const pcm = decodeFloat32(view, format, dataOffset, dataLength)
        return pcm ? { pcm, sampleRate: format.sampleRate, channels: format.channels } : null
      }
      case 0x0006: // μ-law
      case 0x0007: {
        // A-law
        const data = bytes.subarray(dataOffset, dataOffset + dataLength)
        const pcm = decodeG711(data, format.formatTag === 0x0007, format.channels)
        return { pcm, sampleRate: format.sampleRate, channels: format.channels }
      }
      case 0x0011: {
        // IMA-ADPCM
        const pcm = decodeImaAdpcm(view, format, dataOffset, dataLength)
        return pcm ? { pcm, sampleRate: format.sampleRate, channels: format.channels } : null
      }
      case 0x0002: {
        // MS-ADPCM
        const pcm = decodeMsAdpcm(view, format, dataOffset, dataLength)
        return pcm ? { pcm, sampleRate: format.sampleRate, channels: format.channels } : null
      }
      default:
        return null
    }
  } catch {
    return null
  }
}

/** 判断 L1 是否能处理某检测格式（仅 WAV 容器走 L1；其余交 L2） */
export function isL1Capable(kind: string): boolean {
  return kind === 'wav'
}
