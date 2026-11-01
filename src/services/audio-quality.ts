/**
 * 音频质量分级（行业对齐）
 *
 * 依据曲目的 格式 / 比特率 / 采样率 / SACD-DSD 信息，将音频归入 6 个质量等级，
 * 供播放列表等场景在曲目后展示统一的音质标识。
 *
 * 分级边界对齐用户规范：
 *   1. 标准音质  128–192kbps 有损
 *   2. HQ       320kbps 有损（MP3/AAC）
 *   3. SQ       无损 CD 级 44.1kHz/16bit
 *   4. HR       高清无损 48kHz/24bit
 *   5. Hi-Res   高解析 96kHz/24bit+
 *   6. DSD      母带 / DSD64+
 *
 * 列表型 Track 不含 bitDepth 字段，故无损档用 采样率 推断 HR/Hi-Res（≥48k 视为高清，
 * ≥88.2k 视为高解析），缺失采样率时回落 SQ；有损档用比特率判定 HQ/标准。
 */

export type AudioQualityLevel = 'standard' | 'hq' | 'sq' | 'hr' | 'hires' | 'dsd'

export interface QualityMeta {
  /** i18n 短标识键（如 quality.label.standard） */
  labelKey: string
  /** i18n 说明键（悬停提示） */
  descKey: string
  /** 背景色（实心）或渐变起止色（Hi-Res） */
  bg: string
  /** Hi-Res 金橙渐变：[起, 止]；其余档为 undefined */
  grad?: [string, string]
  /** 文字色 */
  fg: string
  /** 宽（dp，前端按 1dp≈1px） */
  w: number
  /** 高（dp） */
  h: number
  /** 字号（px / sp） */
  font: number
  /** 字重（默认 600，Hi-Res 700） */
  weight?: number
}

export const QUALITY_LEVELS: Record<AudioQualityLevel, QualityMeta> = {
  standard: {
    labelKey: 'quality.label.standard',
    descKey: 'quality.desc.standard',
    bg: '#8C8C8C',
    fg: '#FFFFFF',
    w: 24,
    h: 12,
    font: 10,
  },
  hq: {
    labelKey: 'quality.label.hq',
    descKey: 'quality.desc.hq',
    bg: '#1677FF',
    fg: '#FFFFFF',
    w: 20,
    h: 12,
    font: 10,
  },
  sq: {
    labelKey: 'quality.label.sq',
    descKey: 'quality.desc.sq',
    bg: '#52C41A',
    fg: '#FFFFFF',
    w: 20,
    h: 12,
    font: 10,
  },
  hr: {
    labelKey: 'quality.label.hr',
    descKey: 'quality.desc.hr',
    bg: '#1890FF',
    fg: '#FFFFFF',
    w: 22,
    h: 14,
    font: 11,
  },
  hires: {
    labelKey: 'quality.label.hires',
    descKey: 'quality.desc.hires',
    bg: '#FFD700',
    grad: ['#FFD700', '#FFA500'],
    fg: '#000000',
    w: 28,
    h: 16,
    font: 9,
    weight: 700,
  },
  dsd: {
    labelKey: 'quality.label.dsd',
    descKey: 'quality.desc.dsd',
    bg: '#722ED1',
    fg: '#FFFFFF',
    w: 30,
    h: 16,
    font: 10,
  },
}

/** 常见无损容器（m4a 可能是 AAC 或 ALAC，单独按采样率判定） */
const LOSSLESS_FORMATS = new Set([
  'flac',
  'wav',
  'aiff',
  'aif',
  'ape',
  'wv',
  'tta',
  'tak',
  'w64',
  'caf',
  'dsf',
  'dff',
])

interface QualityTrack {
  format: string
  bitrate: number | null
  sampleRate: number | null
  metadataPending?: boolean
  isoPlaceholder?: boolean
  isSacd?: boolean
  sacdDsdRate?: number | null
  /** ffprobe 编码名（如 dts / flac），用于 DTS 子类型识别 */
  codec?: string | null
  /** ffprobe profile（如 "DTS-HD MA" / "DTS-HD HRA" / "DTS"） */
  codecProfile?: string | null
}

/** 将曲目归类为质量等级；无法判定（占位 / 缺元数据）返回 null */
export function classifyQuality(track: QualityTrack): AudioQualityLevel | null {
  if (!track || track.metadataPending || track.isoPlaceholder) return null

  const fmt = (track.format || '').toLowerCase()
  const sr = track.sampleRate ?? 0
  const kbps = track.bitrate ? Math.round(track.bitrate / 1000) : null

  // DSD：原生 DSD 容器，或 SACD 镜像经虚拟分轨且带原始 DSD 采样率
  const isDsd =
    fmt === 'dsf' ||
    fmt === 'dff' ||
    (Boolean(track.isSacd) && (track.sacdDsdRate ?? 0) > 0)
  if (isDsd) return 'dsd'

  // DTS 子类型识别（需后端 ffprobe 提供 codec / profile）：
  // 标准 DTS 核心为有损；DTS-HD MA 为无损母带；DTS-HD HRA 为有损高解析。
  const codec = (track.codec || '').toLowerCase()
  const profile = (track.codecProfile || '').toLowerCase()
  const isDts = codec === 'dts' || fmt === 'dts' || fmt === 'dtshd'
  if (isDts) {
    // 无损母带 / 高解析：按采样率归入 hr / hires
    if (profile.includes('hd ma') || profile.includes('hd hra')) {
      if (sr >= 88200) return 'hires'
      if (sr >= 48000) return 'hr'
      return 'sq'
    }
    // DTS 核心：有损，按比特率归入 hq / 标准
    if (kbps != null) return kbps >= 256 ? 'hq' : 'standard'
    return null
  }

  const isLossless = LOSSLESS_FORMATS.has(fmt)
  // m4a 高采样率视为 ALAC 无损
  const maybeAlac = fmt === 'm4a' && sr > 48000

  if (isLossless || maybeAlac) {
    if (sr >= 88200) return 'hires'
    if (sr >= 48000) return 'hr'
    return 'sq'
  }

  // 有损：按比特率分 HQ / 标准
  if (kbps != null) {
    return kbps >= 256 ? 'hq' : 'standard'
  }

  // 无码率且非明确无损：无法可靠判定
  return null
}

/**
 * 各等级可直接复用的矢量 SVG（行业标注样式，单文件 <1KB，可导入 Android/iOS/前端）。
 * 内联于项目时建议改用 <AudioQualityBadge>，本导出仅供复制复用参考。
 */
export const QUALITY_SVG: Record<AudioQualityLevel, string> = {
  standard:
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 12" width="24" height="12"><rect width="24" height="12" rx="2" fill="#8C8C8C"/><text x="12" y="9" font-size="10" font-weight="600" fill="#FFFFFF" text-anchor="middle" font-family="sans-serif">标准</text></svg>',
  hq: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 12" width="20" height="12"><rect width="20" height="12" rx="2" fill="#1677FF"/><text x="10" y="9" font-size="10" font-weight="600" fill="#FFFFFF" text-anchor="middle" font-family="sans-serif">HQ</text></svg>',
  sq: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 12" width="20" height="12"><rect width="20" height="12" rx="2" fill="#52C41A"/><text x="10" y="9" font-size="10" font-weight="600" fill="#FFFFFF" text-anchor="middle" font-family="sans-serif">SQ</text></svg>',
  hr: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 22 14" width="22" height="14"><rect width="22" height="14" rx="2" fill="#1890FF"/><text x="11" y="10.5" font-size="11" font-weight="600" fill="#FFFFFF" text-anchor="middle" font-family="sans-serif">HR</text></svg>',
  hires:
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 28 16" width="28" height="16"><defs><linearGradient id="goldGrad" x1="0%" y1="0%" x2="100%" y2="100%"><stop offset="0%" stop-color="#FFD700"/><stop offset="100%" stop-color="#FFA500"/></linearGradient></defs><rect width="28" height="16" rx="2" fill="url(#goldGrad)"/><text x="14" y="11.5" font-size="9" font-weight="700" fill="#000000" text-anchor="middle" font-family="sans-serif">Hi-Res</text></svg>',
  dsd: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30 16" width="30" height="16"><rect width="30" height="16" rx="2" fill="#722ED1"/><text x="15" y="11.5" font-size="10" font-weight="600" fill="#FFFFFF" text-anchor="middle" font-family="sans-serif">DSD</text></svg>',
}
