/**
 * 魔数识别真实音频格式 —— 不依赖文件后缀名。
 *
 * 覆盖常见格式：WAV / MP3 / FLAC / AAC(ADIF/ADTS) / AMR / OGG(Vorbis/Opus) /
 * WMA / AIFF / DSF / DFF(DSD) / DTS / APE / WV / TTA / TAK / MIDI。
 * 设计目标：识别「错误后缀 / 冷门编码 / 轻微头部损坏」场景下的真实容器。
 *
 * 冷门格式识别机制（核心）：
 *  - 各格式文件头固定特征码（魔数）快速识别，见下方各分支；
 *  - DTS 位流按 ffmpeg dca_parser 的 5 个同步字全量识别（16/14-bit × 大小端 + DTS-HD）；
 *  - WAV 容器深检：fmt 声称 PCM 但 data 起始实为 DTS/WavPack 位流的「伪装 WAV」
 *    （如 DTS-CD / DTS-ES 封装在 WAV 中）——symphonia 会按 PCM 打开并输出噪声，
 *    必须在路由层识别为冷门格式。
 */
import type { DetectedFormat } from './types'

function ascii(bytes: Uint8Array, offset: number, len: number): string {
  let s = ''
  for (let i = 0; i < len; i++) s += String.fromCharCode(bytes[offset + i] ?? 0)
  return s
}

function readU32be(bytes: Uint8Array, offset: number): number {
  // >>> 0 无符号化：0xFF<<24 等有符号位运算结果为负，无法与无符号字面量表比较
  return (
    (((bytes[offset] ?? 0) << 24) |
      ((bytes[offset + 1] ?? 0) << 16) |
      ((bytes[offset + 2] ?? 0) << 8) |
      (bytes[offset + 3] ?? 0)) >>>
    0
  )
}

/** DTS 位流同步字表（与 ffmpeg dca_parser 一致，按大端 32-bit 读入后比较） */
const DTS_SYNC_WORDS: number[] = [
  0x7ffe8001, // 16-bit big-endian（DTS-CD / 常规裸流最常见）
  0xff1f00e8, // 14-bit big-endian
  0xe8001fff, // 14-bit little-endian
  0x0180fe7f, // 16-bit little-endian
  0x64582025, // DTS-HD（XLL / 扩展子流 "dX %"）
]

/** offset 处 4 字节是否命中 DTS 同步字表 */
function isDtsSync(bytes: Uint8Array, offset: number): boolean {
  const w = readU32be(bytes, offset)
  return DTS_SYNC_WORDS.includes(w)
}

/**
 * 解析 RIFF chunk 表，返回 data chunk 数据起始的绝对偏移。
 * buffer 不足以走到 data chunk 时返回 null（调用方保持原判断）。
 */
function riffDataOffset(bytes: Uint8Array): number | null {
  const n = bytes.length
  if (n < 12 || ascii(bytes, 0, 4) !== 'RIFF') return null
  let off = 12
  while (off + 8 <= n) {
    const id = ascii(bytes, off, 4)
    const size =
      (bytes[off + 4] ?? 0) |
      ((bytes[off + 5] ?? 0) << 8) |
      ((bytes[off + 6] ?? 0) << 16) |
      ((bytes[off + 7] ?? 0) << 24)
    if (id === 'data') return off + 8
    // chunk 数据区 + 2 字节对齐填充
    off += 8 + size + (size % 2)
  }
  return null
}

/** data chunk 起始小窗口内扫描 DTS 同步字（容忍极端帧对齐偏移） */
function dataStartsWithDts(bytes: Uint8Array, dataOffset: number): boolean {
  const end = Math.min(bytes.length, dataOffset + 16)
  for (let i = dataOffset; i + 4 <= end; i++) {
    if (isDtsSync(bytes, i)) return true
  }
  return false
}

/**
 * 识别真实格式。传入的 buffer 至少应含前 16 字节（调用方保证）；
 * 对 WAV 容器深检建议提供前 4KB（覆盖带 LIST/INFO/封面 chunk 的头部）。
 * 对「轻微头损坏」场景：若前若干字节被污染但后续能匹配可放宽起点。
 */
export function detectFormat(buffer: ArrayBuffer): DetectedFormat {
  const bytes = new Uint8Array(buffer)
  const n = bytes.length

  // WAV / AVI / 通用 RIFF：'RIFF' + size + 'WAVE'|'AVI '|...
  if (n >= 12 && ascii(bytes, 0, 4) === 'RIFF') {
    const form = ascii(bytes, 8, 4)
    if (form === 'AVI ') return { kind: 'unknown', description: 'AVI (非音频)' }
    // 容器深检：fmt 声称 PCM 但数据实为 DTS/WavPack 位流的伪装 WAV
    // （DTS-ES in WAV：symphonia 按 PCM 播会输出噪声，必须识别为冷门格式）
    const dataOff = riffDataOffset(bytes)
    if (dataOff !== null) {
      if (dataStartsWithDts(bytes, dataOff)) {
        return { kind: 'dts', description: 'WAV 容器封装 DTS (DTS-in-WAV)' }
      }
      if (ascii(bytes, dataOff, 4) === 'wvpk') {
        return { kind: 'wv', description: 'WAV 容器封装 WavPack' }
      }
    }
    if (form === 'WAVE') return { kind: 'wav', description: 'WAV (RIFF/WAVE)' }
    return { kind: 'wav', description: 'RIFF 容器' }
  }

  // AIFF / AIFF-C：'FORM' + size + 'AIFF'|'AIFC'
  if (n >= 12 && ascii(bytes, 0, 4) === 'FORM') {
    const form = ascii(bytes, 8, 4)
    if (form === 'AIFF' || form === 'AIFC') return { kind: 'aiff', description: 'AIFF' }
  }

  // FLAC：'fLaC'
  if (n >= 4 && ascii(bytes, 0, 4) === 'fLaC') return { kind: 'flac', description: 'FLAC' }

  // OGG：'OggS'
  if (n >= 4 && ascii(bytes, 0, 4) === 'OggS') return { kind: 'ogg', description: 'OGG (Vorbis/Opus)' }

  // AMR：'#!AMR'（AMR-NB）或 '#!AMR-WB'
  if (n >= 5 && ascii(bytes, 0, 5) === '#!AMR') return { kind: 'amr', description: 'AMR' }

  // AAC-ADIF：'ADIF'
  if (n >= 4 && ascii(bytes, 0, 4) === 'ADIF') return { kind: 'aac', description: 'AAC (ADIF)' }

  // AAC-ADTS：12 位同步字 0xFFF（或带 MPEG 版本的 0xFFF）
  if (n >= 2 && bytes[0] === 0xff && (bytes[1] & 0xf6) === 0xf0) {
    return { kind: 'aac', description: 'AAC (ADTS)' }
  }

  // WMA / ASF：GUID 头 0x30 0x26 0xB2 0x75 ...（ASF 对象头）
  if (n >= 4 && bytes[0] === 0x30 && bytes[1] === 0x26 && bytes[2] === 0xb2 && bytes[3] === 0x75) {
    return { kind: 'wma', description: 'WMA (ASF)' }
  }

  // MP4 / M4A / MOV / 3GP：在偏移 4 处出现 'ftyp'
  if (n >= 12) {
    const ftyp = ascii(bytes, 4, 4)
    if (ftyp === 'ftyp') {
      const major = ascii(bytes, 8, 3)
      if (major === 'M4A' || major === 'MP4' || major === 'isom' || major === '3gp') {
        return { kind: 'aac', description: 'MP4/M4A 容器(AAC)' }
      }
      return { kind: 'aac', description: 'ISO-MP4 容器' }
    }
  }

  // DSF（DSD）：'DSD '（后跟 8 字节 DSD chunk size + 'fmt '）
  if (n >= 4 && ascii(bytes, 0, 4) === 'DSD ') return { kind: 'dsf', description: 'DSF (DSD)' }

  // DFF（DSD）：'FRM8'
  if (n >= 4 && ascii(bytes, 0, 4) === 'FRM8') return { kind: 'dff', description: 'DFF (DSD)' }

  // DTS 裸流：ASCII 头 'DTS ' / 'dtsh'（含偏移 1 处 'tsh' 容错），或标准二进制同步字
  if (n >= 4) {
    if (
      ascii(bytes, 0, 4) === 'DTS ' ||
      ascii(bytes, 0, 4) === 'dtsh' ||
      ascii(bytes, 1, 3) === 'tsh'
    ) {
      return { kind: 'dts', description: 'DTS' }
    }
    if (isDtsSync(bytes, 0)) return { kind: 'dts', description: 'DTS 位流' }
  }

  // Monkey's Audio：'MAC '（APE）
  if (n >= 4 && ascii(bytes, 0, 4) === 'MAC ') return { kind: 'ape', description: 'APE (Monkey)' }

  // WavPack：'wvpk'
  if (n >= 4 && ascii(bytes, 0, 4) === 'wvpk') return { kind: 'wv', description: 'WavPack' }

  // TAK：'tBaK'（注意大小写：0x74 0x42 0x61 0x4B）
  if (n >= 4 && ascii(bytes, 0, 4) === 'tBaK') return { kind: 'tak', description: 'TAK' }

  // TTA：'TTA1' / 'TTA2'
  if (n >= 4 && (ascii(bytes, 0, 3) === 'TTA')) return { kind: 'tta', description: 'True Audio' }

  // MIDI：'MThd'
  if (n >= 4 && ascii(bytes, 0, 4) === 'MThd') return { kind: 'midi', description: 'MIDI' }

  // MP3：ID3v2 标签，或 MPEG 帧同步字 0xFF 后跟 0xE0~0xFB 的帧头
  if (n >= 3) {
    if (ascii(bytes, 0, 3) === 'ID3') return { kind: 'mp3', description: 'MP3 (ID3)' }
    const b1 = bytes[0]
    const b2 = bytes[1]
    if (b1 === 0xff && (b2 & 0xe0) === 0xe0 && (b2 & 0x18) !== 0x08) {
      return { kind: 'mp3', description: 'MP3 (MPEG Audio)' }
    }
  }

  // 轻微头损坏的容错：在偏移 0 处未发现魔数，但前 64 字节内出现 OggS/fLaC/RIFF 等，
  // 视为对应格式（常见于文件头被少量零字节污染）。
  const probe = Math.min(n, 64)
  for (let i = 1; i < probe; i++) {
    const window = bytes.subarray(i, i + 4)
    const tag = ascii(window, 0, 4)
    if (tag === 'OggS') return { kind: 'ogg', description: 'OGG (偏移容错)' }
    if (tag === 'fLaC') return { kind: 'flac', description: 'FLAC (偏移容错)' }
    if (tag === 'RIFF') return { kind: 'wav', description: 'WAV (偏移容错)' }
    if (tag === 'wvpk') return { kind: 'wv', description: 'WavPack (偏移容错)' }
    if (tag === 'MAC ') return { kind: 'ape', description: 'APE (偏移容错)' }
  }

  return { kind: 'unknown', description: '未知格式' }
}

/** 该格式是否大概率能被原生引擎（rodio/symphonia）直接解码 */
export function isNativePlayable(kind: DetectedFormat['kind']): boolean {
  return kind === 'mp3' || kind === 'flac' || kind === 'wav' || kind === 'ogg'
}
