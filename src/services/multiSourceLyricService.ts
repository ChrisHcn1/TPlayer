import { onlineMusicService, MusicSearchResult } from './onlineMusicService'
import { chartLyricsService, ChartLyricsSearchResult } from './chartLyricsService'
import { localStorageService } from '../stores/local'

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
  lyricLines?: any[]
  yrcData?: any[]
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
  lyricLines?: any[],
  yrcData?: any[]
): LyricScore {
  let completeness = 0
  let hasTranslation = false
  let hasRomanization = false
  let hasWordLevel = false
  let timestampAccuracy = 0

  if (lyricText && lyricText.trim().length > 50) {
    completeness = Math.min(100, lyricText.trim().length / 5)
  }

  if (lyricLines && lyricLines.length > 0) {
    hasTranslation = lyricLines.some(line => line.translation?.trim())
    hasRomanization = lyricLines.some(line => line.romanization?.trim())
    
    const linesWithTime = lyricLines.filter(line => line.time !== null && line.time !== undefined)
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
    skipLocalLRC: boolean = false
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
            if (sourceType) {
              const score = calculateLyricScore(
                sourceType,
                sourceData.lrcData ? JSON.stringify(sourceData.lrcData) : undefined,
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

      if (neteaseResult.status === 'fulfilled') {
        allSources.set(LyricSourceType.NETEASE, neteaseResult.value)
      }
      if (qqResult.status === 'fulfilled') {
        allSources.set(LyricSourceType.QQ_MUSIC, qqResult.value)
      }
      if (chartLyricsResult.status === 'fulfilled') {
        allSources.set(LyricSourceType.CHARTLYRICS, chartLyricsResult.value)
      }

      if (allSources.size > 0) {
        const sourcesToCache: Record<string, any> = {}
        allSources.forEach((score, sourceType) => {
          if (score.lyricLines) {
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

  private async queryNetease(song: SongInfo): Promise<LyricScore> {
    const result = await onlineMusicService.searchSong(
      song.title,
      song.artist,
      'netease'
    )

    if (!result.success) {
      return calculateLyricScore(LyricSourceType.NETEASE)
    }

    const lyricResult = await onlineMusicService.getLyric(
      result.songs[0].id,
      'netease'
    )

    return calculateLyricScore(
      LyricSourceType.NETEASE,
      lyricResult.lrc?.lyric,
      lyricResult.parsedLrc,
      lyricResult.yrc
    )
  }

  private async queryQQMusic(song: SongInfo): Promise<LyricScore> {
    const result = await onlineMusicService.searchSong(
      song.title,
      song.artist,
      'qq'
    )

    if (!result.success) {
      return calculateLyricScore(LyricSourceType.QQ_MUSIC)
    }

    const lyricResult = await onlineMusicService.getLyric(
      result.songs[0].id,
      'qq'
    )

    return calculateLyricScore(
      LyricSourceType.QQ_MUSIC,
      lyricResult.lrc?.lyric,
      lyricResult.parsedLrc,
      lyricResult.yrc
    )
  }

  private async queryChartLyrics(song: SongInfo): Promise<LyricScore> {
    const result = await chartLyricsService.searchLyric(
      song.artist,
      song.title
    )

    if (!result.success || !result.lyricLines) {
      return calculateLyricScore(LyricSourceType.CHARTLYRICS)
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

  private getMultiSourceCache(songId: string): any {
    try {
      const cacheKey = `multi_lyric_${songId}`
      const data = localStorage.getItem(cacheKey)
      return data ? JSON.parse(data) : null
    } catch (e) {
      return null
    }
  }

  private saveMultiSourceCache(songId: string, sources: Record<string, any>): void {
    try {
      const cacheKey = `multi_lyric_${songId}`
      const cache = {
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
