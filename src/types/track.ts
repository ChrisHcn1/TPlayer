/**
 * 音乐库领域模型（前端）
 * 与 Rust 侧 `library` 模块序列化结构一一对应（snake_case 由后端负责，前端保持 camelCase）。
 */

/** 单曲 */
export interface Track {
  /** 应用内稳定 ID（由后端生成，不使用路径作主键） */
  id: string
  /** 标题 */
  title: string
  /** 歌手名（多歌手以数组表达，避免旧版字符串切割歧义） */
  artists: string[]
  /** 专辑名 */
  album: string
  /** 专辑 ID */
  albumId: string | null
  /** 时长（秒） */
  duration: number
  /** 音轨序号 */
  trackNumber: number | null
  /** 发行年份 */
  year: number | null
  /** 文件绝对路径 */
  filePath: string
  /** 文件大小（字节） */
  fileSize: number
  /** 音频格式（小写扩展名） */
  format: string
  /** 比特率（bps） */
  bitrate: number | null
  /** 采样率（Hz） */
  sampleRate: number | null
  /** 内嵌封面是否可用 */
  hasCover: boolean
  /** 入库时间（Unix 秒） */
  addedAt: number
  /** 文件修改时间（Unix 秒），用于增量扫描 */
  modifiedAt: number
}

/** 专辑（聚合视图，不落盘为独立实体） */
export interface Album {
  id: string
  name: string
  artists: string[]
  year: number | null
  trackCount: number
  /** 封面是否可用 */
  hasCover: boolean
}

/** 歌手（聚合视图） */
export interface Artist {
  id: string
  name: string
  albumCount: number
  trackCount: number
}

/** 歌单 */
export interface Playlist {
  id: string
  name: string
  description: string
  trackIds: string[]
  createdAt: number
  updatedAt: number
  /** 系统歌单（如"最近播放"）不可编辑 */
  isSystem: boolean
}

/** 扫描进度 */
export interface ScanProgress {
  /** 当前阶段 */
  stage: 'idle' | 'discovering' | 'parsing' | 'writing' | 'done' | 'failed'
  /** 已处理文件数 */
  processed: number
  /** 发现的文件总数 */
  total: number
  /** 当前处理的文件路径 */
  currentPath: string | null
  /** 失败原因（stage 为 failed 时有效） */
  error: string | null
}

/** 曲库统计 */
export interface LibraryStats {
  trackCount: number
  albumCount: number
  artistCount: number
  /** 曲库总时长（秒） */
  totalDuration: number
  lastScanAt: number | null
}

/** 最近播放条目（系统记录，非用户歌单） */
export interface RecentEntry {
  trackId: string
  /** 最近一次播放时间（Unix 秒） */
  playedAt: number
}
