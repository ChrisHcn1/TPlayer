import { onlineMusicService } from './onlineMusicService'
import { chartLyricsService } from './chartLyricsService'
import { localStorageService } from '../stores/local'
import { findBestMatch } from '../utils/stringSimilarity'
import type { LyricLine } from './lyricParser'

export enum LyricSourcePriority {
  EMBEDDED = 1,
  LOCAL_LRC = 2,
  CACHED_ONLINE = 3,
  NETEASE = 4,
  QQ_MUSIC = 5,
  CHARTLYRICS = 6
}

export enum LyricSourceType {
  EMBEDDED = 'embedded',
  LOCAL_LRC = 'local_lrc',
  CACHED_ONLINE = 'cached_online',
  NETEASE = 'netease',
  QQ_MUSIC = 'qq_music',
  CHARTLYRICS = 'chartlyrics'
}

export interface LyricScore {
  source: LyricSourceType
  totalScore: number
  metrics: {
    completeness: number
    hasTranslation: boolean
    hasRomanization: boolean
    hasWordLevel: boolean
    timestampAccuracy: number
  }
  lyricText?: string
  lyricLines?: LyricLine[]
  yrcData?: LyricLine[]
}

// 多源歌词在 localStorage 中的缓存结构
interface CachedSourceEntry {
  lrcData?: LyricLine[]
  yrcData?: LyricLine[]
  fetchedAt?: number
  score?: LyricScore
}

interface MultiSourceCache {
  songId: string
  sources: Record<string, CachedSourceEntry>
  bestSource: string | null
  updatedAt: number
}

// 将结构化歌词行重新拼成纯文本，用于完整度评分
function lyricLinesToText(lines: LyricLine[]): string {
  return lines
    .map(line => line.words.map(word => word.word).join(''))
    .filter(text => text.trim().length > 0)
    .join('\n')
}

const SCORE_WEIGHTS = {
  completeness: 0.4,
  translation: 20,
  romanization: 15,
  wordLevel: 25,
  timestampAccuracy: 0.2
}

export function calculateLyricScore(
  source: LyricSourceType,
  lyricText?: string,
  lyricLines?: LyricLine[],
  yrcData?: LyricLine[]
): LyricScore {
  let completeness = 0
  let hasTranslation = false
  let hasRomanization = false
  let hasWordLevel = false
  let timestampAccuracy = 0

  if (lyricText && lyricText.trim().length > 50) {
    completeness = Math.min(100, lyricText.trim().length / 5)
  } else if (lyricLines && lyricLines.length > 0) {
    // 没有纯文本时，用结构化歌词行的文本长度估算完整度
    completeness = Math.min(100, lyricLinesToText(lyricLines).trim().length / 5)
  }

  if (lyricLines && lyricLines.length > 0) {
    hasTranslation = lyricLines.some(line => line.translatedLyric?.trim())
    hasRomanization = lyricLines.some(line => line.romanLyric?.trim())

    const linesWithTime = lyricLines.filter(
      line => Number.isFinite(line.startTime) && line.startTime > 0
    )
    timestampAccuracy = linesWithTime.length > 0
      ? (linesWithTime.length / lyricLines.length) * 100
      : 0
  }

  hasWordLevel = !!yrcData && yrcData.length > 0

  const totalScore = 
    completeness * SCORE_WEIGHTS.completeness +
    (hasTranslation ? SCORE_WEIGHTS.translation : 0) +
    (hasRomanization ? SCORE_WEIGHTS.romanization : 0) +
    (hasWordLevel ? SCORE_WEIGHTS.wordLevel : 0) +
    timestampAccuracy * SCORE_WEIGHTS.timestampAccuracy

  return {
    source,
    totalScore,
    metrics: {
      completeness,
      hasTranslation,
      hasRomanization,
      hasWordLevel,
      timestampAccuracy
    },
    lyricText,
    lyricLines,
    yrcData
  }
}

export interface MultiSourceLyricResult {
  success: boolean
  bestSource: LyricSourceType | null
  bestScore: LyricScore | null
  allSources: Map<LyricSourceType, LyricScore>
  error?: string
}

export interface SongInfo {
  id?: string
  title: string
  artist: string
  album?: string
  filePath?: string
}

export class MultiSourceLyricService {
  constructor() {}

  async getLyric(
    song: SongInfo,
    mode: 'auto' | 'manual' = 'auto',
    _skipLocalLRC: boolean = false
  ): Promise<MultiSourceLyricResult> {
    const allSources = new Map<LyricSourceType, LyricScore>()
    const songId = song.id || `${song.artist}-${song.title}`.toLowerCase()

    try {
      const cachedLyricText = await localStorageService.getCachedLyric(songId)
      if (cachedLyricText && cachedLyricText.trim()) {
        const multiCache = this.getMultiSourceCache(songId)
        if (multiCache) {
          for (const [sourceName, sourceData] of Object.entries(multiCache.sources)) {
            const sourceType = this.sourceNameToType(sourceName)
            if (sourceType && sourceData.lrcData && sourceData.lrcData.length > 0) {
              const score = calculateLyricScore(
                sourceType,
                lyricLinesToText(sourceData.lrcData),
                sourceData.lrcData,
                sourceData.yrcData
              )
              allSources.set(sourceType, score)
            }
          }
        } else {
          const { parseLrc } = await import('./lyricParser')
          const parsedLyrics = parseLrc(cachedLyricText)
          const score = calculateLyricScore(
            LyricSourceType.CACHED_ONLINE,
            cachedLyricText,
            parsedLyrics
          )
          allSources.set(LyricSourceType.CACHED_ONLINE, score)
        }

        if (mode === 'auto' && allSources.size > 0) {
          const bestScore = this.selectBestSource(allSources)
          return {
            success: true,
            bestSource: bestScore.source,
            bestScore: bestScore,
            allSources
          }
        }
      }

      const [neteaseResult, qqResult, chartLyricsResult] = await Promise.allSettled([
        this.queryNetease(song),
        this.queryQQMusic(song),
        this.queryChartLyrics(song)
      ])

      // 只有真正拿到歌词行的源才计入候选，0 分空结果不得参与“选优”
      if (neteaseResult.status === 'fulfilled' && neteaseResult.value) {
        allSources.set(LyricSourceType.NETEASE, neteaseResult.value)
      }
      if (qqResult.status === 'fulfilled' && qqResult.value) {
        allSources.set(LyricSourceType.QQ_MUSIC, qqResult.value)
      }
      if (chartLyricsResult.status === 'fulfilled' && chartLyricsResult.value) {
        allSources.set(LyricSourceType.CHARTLYRICS, chartLyricsResult.value)
      }

      if (allSources.size > 0) {
        const sourcesToCache: Record<string, CachedSourceEntry> = {}
        allSources.forEach((score, sourceType) => {
          if (score.lyricLines && score.lyricLines.length > 0) {
            sourcesToCache[this.sourceTypeToName(sourceType)] = {
              lrcData: score.lyricLines,
              yrcData: score.yrcData,
              fetchedAt: Date.now(),
              score: score
            }
          }
        })

        const bestScore = this.selectBestSource(allSources)
        if (bestScore.lyricLines && bestScore.lyricText) {
          await localStorageService.saveCachedLyric(songId, bestScore.lyricText)
        }
        
        this.saveMultiSourceCache(songId, sourcesToCache)
      }

      if (allSources.size === 0) {
        return {
          success: false,
          bestSource: null,
          bestScore: null,
          allSources,
          error: '所有歌词源都没有找到歌词'
        }
      }

      const bestScore = this.selectBestSource(allSources)

      return {
        success: true,
        bestSource: bestScore.source,
        bestScore: bestScore,
        allSources
      }

    } catch (error) {
      console.error('[MultiSourceLyricService] 查询失败:', error)
      return {
        success: false,
        bestSource: null,
        bestScore: null,
        allSources,
        error: error instanceof Error ? error.message : '未知错误'
      }
    }
  }

  // 在合并搜索结果中筛选指定来源，并用标题/艺术家相似度挑出最匹配的歌曲
  private async findBestSong(
    song: SongInfo,
    source: 'qq' | 'netease'
  ) {
    const keyword = `${song.title} ${song.artist}`.trim()
    const candidates = (await onlineMusicService.searchSong(keyword))
      .filter(item => item.source === source)
    return findBestMatch({ title: song.title, artist: song.artist }, candidates)
  }

  private async queryNetease(song: SongInfo): Promise<LyricScore | null> {
    const matched = await this.findBestSong(song, 'netease')
    if (!matched) {
      return null
    }

    const lyricResult = await onlineMusicService.getLyric(matched.id, 'netease')
    if (lyricResult.lrcData.length === 0 && lyricResult.yrcData.length === 0) {
      return null
    }

    return calculateLyricScore(
      LyricSourceType.NETEASE,
      undefined,
      lyricResult.lrcData,
      lyricResult.yrcData
    )
  }

  private async queryQQMusic(song: SongInfo): Promise<LyricScore | null> {
    const matched = await this.findBestSong(song, 'qq')
    if (!matched) {
      return null
    }

    const lyricResult = await onlineMusicService.getLyric(matched.id, 'qq')
    if (lyricResult.lrcData.length === 0 && lyricResult.yrcData.length === 0) {
      return null
    }

    return calculateLyricScore(
      LyricSourceType.QQ_MUSIC,
      undefined,
      lyricResult.lrcData,
      lyricResult.yrcData
    )
  }

  private async queryChartLyrics(song: SongInfo): Promise<LyricScore | null> {
    const result = await chartLyricsService.searchLyric(
      song.artist,
      song.title
    )

    if (!result.success || !result.lyricLines || result.lyricLines.length === 0) {
      return null
    }

    return calculateLyricScore(
      LyricSourceType.CHARTLYRICS,
      result.lyric,
      result.lyricLines
    )
  }

  private selectBestSource(sources: Map<LyricSourceType, LyricScore>): LyricScore {
    let best: LyricScore | null = null
    for (const score of sources.values()) {
      if (!best || score.totalScore > best.totalScore) {
        best = score
      }
    }
    return best!
  }

  private sourceNameToType(name: string): LyricSourceType | null {
    const mapping: Record<string, LyricSourceType> = {
      'netease': LyricSourceType.NETEASE,
      'qq_music': LyricSourceType.QQ_MUSIC,
      'chartlyrics': LyricSourceType.CHARTLYRICS,
      'embedded': LyricSourceType.EMBEDDED,
      'local_lrc': LyricSourceType.LOCAL_LRC
    }
    return mapping[name] || null
  }

  private sourceTypeToName(type: LyricSourceType): string {
    const mapping: Record<LyricSourceType, string> = {
      [LyricSourceType.NETEASE]: 'netease',
      [LyricSourceType.QQ_MUSIC]: 'qq_music',
      [LyricSourceType.CHARTLYRICS]: 'chartlyrics',
      [LyricSourceType.EMBEDDED]: 'embedded',
      [LyricSourceType.LOCAL_LRC]: 'local_lrc',
      [LyricSourceType.CACHED_ONLINE]: 'cached_online'
    }
    return mapping[type]
  }

  private getMultiSourceCache(songId: string): MultiSourceCache | null {
    try {
      const cacheKey = `multi_lyric_${songId}`
      const data = localStorage.getItem(cacheKey)
      return data ? (JSON.parse(data) as MultiSourceCache) : null
    } catch (e) {
      return null
    }
  }

  private saveMultiSourceCache(songId: string, sources: Record<string, CachedSourceEntry>): void {
    try {
      const cacheKey = `multi_lyric_${songId}`
      const cache: MultiSourceCache = {
        songId,
        sources,
        bestSource: null,
        updatedAt: Date.now()
      }
      localStorage.setItem(cacheKey, JSON.stringify(cache))
    } catch (e) {
      console.error('[MultiSourceLyricService] 保存缓存失败:', e)
    }
  }
}

export const multiSourceLyricService = new MultiSourceLyricService()
