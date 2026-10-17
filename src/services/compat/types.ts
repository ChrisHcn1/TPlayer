/**
 * 音频格式兼容补丁模块 —— 内部类型
 *
 * 本模块是一个对现有播放器「零侵入」的包裹层，不依赖任何播放核心代码，
 * 只在原 play 失败时介入。所有类型仅在本模块内部流转。
 */

/** 兜底时需要的最小曲目元数据（由接入层注入或从曲库获取） */
export interface TrackMeta {
  id: string
  /** 文件绝对路径（Tauri），或浏览器预览环境的占位路径 */
  filePath: string
  /** 文件大小（字节） */
  fileSize: number
  /** 文件修改时间（Unix 秒） */
  modifiedAt: number
  /** 声明格式（小写扩展名，仅作参考，真实格式以魔数为准） */
  format: string
}

/** 魔数识别出的真实格式 */
export interface DetectedFormat {
  /** 标准化格式代号 */
  kind:
    | 'wav'
    | 'mp3'
    | 'flac'
    | 'aac'
    | 'amr'
    | 'ogg'
    | 'wma'
    | 'aiff'
    | 'dsf'
    | 'dff'
    | 'dts'
    | 'ape'
    | 'wv'
    | 'tta'
    | 'tak'
    | 'midi'
    | 'unknown'
  /** 人类可读描述 */
  description: string
}

/** 主线程 → Worker 的解码请求 */
export interface DecodeRequest {
  trackId: string
  /** 完整文件字节 */
  buffer: ArrayBuffer
  size: number
  modifiedAt: number
  /** 接入层已知声明格式（小写扩展名） */
  format: string
  /** 是否允许 L2（FFmpeg.wasm）兜底 */
  allowFfmpeg: boolean
  /** 预热请求：仅让 worker 提前加载 FFmpeg core，不解码具体文件 */
  preload?: boolean
}

/** Worker → 主线程的解码结果 */
export interface DecodeResponse {
  trackId: string
  ok: boolean
  /** 解码级别：l1=轻量 JS，l2=FFmpeg.wasm，none=无法处理 */
  level: 'l1' | 'l2' | 'none'
  /** 标准 16-bit PCM WAV（含 RIFF 头），解码成功时存在 */
  wav?: ArrayBuffer
  sampleRate?: number
  channels?: number
  durationMs?: number
  error?: string
  /** 预热完成回执（request.preload 触发） */
  preloaded?: boolean
}

/** 远程开关 / 灰度 / 回滚配置 */
export interface CompatConfig {
  /** 总开关 */
  enabled: boolean
  /** 灰度比例 0~1（按 trackId 哈希分流；0=不启用，1=全量） */
  grayRatio: number
  /** 是否允许 L2（FFmpeg.wasm）极端兜底 */
  ffmpegWasmEnabled: boolean
  /** 一键回滚：为 true 时整体禁用补丁（含已进行中的兜底） */
  killSwitch: boolean
  /** 埋点上报端点（null 表示仅本地统计） */
  reportEndpoint: string | null
  /** 配置版本，用于区分灰度批次 */
  version: string
}

/** 埋点样本 */
export interface TelemetrySample {
  trackId: string
  /** 是否触发了兜底流程 */
  triggered: boolean
  /** 兜底是否成功产出可播放 WAV */
  success: boolean
  level: 'l1' | 'l2' | 'none'
  /** 端到端耗时（毫秒，从触发兜底到产出/失败） */
  durationMs: number
  error?: string
  /** 样本产生时间（Unix 毫秒） */
  ts: number
}

/** 内置默认配置（远程拉取失败或首次运行时的保守默认值） */
export const DEFAULT_COMPAT_CONFIG: CompatConfig = {
  enabled: true,
  grayRatio: 1,
  ffmpegWasmEnabled: true,
  killSwitch: false,
  reportEndpoint: null,
  version: 'builtin-1',
}
