// 播放器内部使用的歌曲模型（比 stores/local 中持久化用的 Song 字段更宽，
// 额外承载音频元数据、浏览器 File 对象与 CUE 起止时间）。
// 与 stores/local 的 Song 结构兼容：持久化前会把 startTime/endTime 转为字符串。
export interface Song {
  id: string
  title: string
  artist: string
  album: string
  path: string
  duration: string
  cover: string
  year: string
  genre: string
  lyric?: string
  isFavorite?: boolean
  isCueTrack?: boolean
  startTime?: string | number
  endTime?: string | number
  parentFile?: string
  trackNumber?: string
  cueInfo?: string
  dynamicCoverUrl?: string
  // 转码相关
  needs_transcode: boolean
  // 浏览器环境下的原始文件对象（仅浏览器环境使用）
  file?: File
  // 音频元数据
  format?: string
  sample_rate?: number
  channels?: number
  bit_rate?: number
  bit_depth?: number
}
