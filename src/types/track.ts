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
  /** 是否为 CUE 分轨 */
  isCue?: boolean
  /** CUE 分轨在镜像文件中的起始秒数 */
  cueOffset?: number | null
  /** 是否为 SACD ISO 虚拟分轨 */
  isSacd?: boolean
  /** SACD 分轨在 ISO 内的 0-based 索引 */
  sacdTrackIndex?: number
  /** SACD 区域（后端 SacdArea 枚举序列化值） */
  sacdArea?: 'Stereo2ch' | 'MultiCh'
  /** 原始 DSD 采样率（DSD64=2822400） */
  sacdDsdRate?: number
  /** 两阶段懒加载：元数据尚未解析（占位条目，仅文件名/路径/大小已入库） */
  metadataPending?: boolean
  /** 两阶段懒加载：SACD ISO 占位条目（尚未解析出虚拟分轨） */
  isoPlaceholder?: boolean
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
  /** 是否为 CUE 分轨专辑（整盘镜像拆出的虚拟专辑） */
  isCue?: boolean
  /** 是否为 SACD ISO 虚拟专辑 */
  isSacd?: boolean
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
  /**
   * 当前阶段
   *
   * - `discovering` / `parsing` / `writing`：扫描阶段（快速占位入库）
   * - `backfilling`：**后台补解析阶段**——扫描已结束，后台线程继续解析
   *   占位条目的元数据 / CUE 分轨 / SACD 分轨；本地深度解析、网络仅读文件头。
   *   该阶段的进度即「后台解析中 x/y」。
   */
  stage: 'idle' | 'discovering' | 'parsing' | 'writing' | 'backfilling' | 'done' | 'failed'
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

/** 曲目详情（完整元数据）—— Rust track_info 命令返回 */
export interface TrackInfo {
  title: string
  artists: string[]
  album: string
  trackNumber: number | null
  year: number | null
  /** 时长（秒） */
  duration: number

  /** 音频格式（小写扩展名） */
  format: string
  /** 比特率（bps） */
  bitrate: number | null
  /** 采样率（Hz） */
  sampleRate: number | null
  /** 位深（bit） */
  bitDepth: number | null
  /** 声道数 */
  channels: number | null

  filePath: string
  fileSize: number
  modifiedAt: number
  addedAt: number

  /** 封面 data URL（无封面为 null） */
  cover: string | null
  /** 内嵌歌词纯文本（无歌词为 null） */
  embeddedLyrics: string | null

  replaygainDb: number | null
  replaygainPeak: number | null
}
