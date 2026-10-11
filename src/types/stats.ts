/**
 * 播放统计领域模型（收听报告数据源）
 *
 * 与曲库 / 播放器解耦：仅记录"播放行为"的轻量聚合，数据本地持久化（localStorage），
 * 不上传任何服务端（privacy-first，对齐收藏 / 最近播放的用户数据策略）。
 * 单曲元数据（标题 / 艺术家 / 时长）不冗余存储，展示时经 library store 按 trackId join。
 */

/** 单曲播放统计 */
export interface TrackStat {
  /** 应用内稳定 ID（对齐 Track.id） */
  trackId: string
  /** 累计播放次数（每次进入 playing 状态计 1 次，暂停恢复不重复计数） */
  playCount: number
  /** 累计播放时长（毫秒，仅统计实际处于播放状态的时间） */
  totalMs: number
  /** 最后播放时间（Unix 秒） */
  lastPlayedAt: number
}

/** 播放统计快照（localStorage 持久化结构） */
export interface StatsSnapshot {
  /** 结构版本（未来迁移用） */
  version: 1
  /** 各曲目统计，key 为 trackId */
  tracks: Record<string, TrackStat>
  /** 播放次数按小时分布（index 0-23，用于"最常听时段"） */
  hourCounts: number[]
  /** 最近一次写盘时间（Unix 秒） */
  updatedAt: number
}

/** 收听报告 - TOP 曲目条目（已 join 曲库元数据） */
export interface TopTrackItem {
  trackId: string
  title: string
  artists: string[]
  playCount: number
  totalMs: number
}

/** 收听报告 - TOP 艺术家条目（按曲目播放量聚合） */
export interface TopArtistItem {
  name: string
  playCount: number
  totalMs: number
}

/** 分享卡片渲染数据（与视图解耦，供 SVG 模板直接消费） */
export interface ShareCardData {
  /** 报告标题（如"我的收听报告"） */
  title: string
  /** 统计区间说明（如"自启用统计以来"） */
  periodText: string
  /** 累计播放时长（毫秒） */
  totalMs: number
  /** 播放曲目数 */
  trackCount: number
  /** 累计播放次数 */
  playCount: number
  /** TOP 曲目（最多 5） */
  topTracks: TopTrackItem[]
  /** TOP 艺术家（最多 5） */
  topArtists: TopArtistItem[]
  /** 播放次数按小时分布（index 0-23，用于卡片迷你柱状） */
  hourCounts: number[]
  /** 最常听时段（0-23，播放次数最高的小时） */
  peakHour: number
  /** 生成时间（Unix 秒） */
  generatedAt: number
}
