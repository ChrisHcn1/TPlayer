import { cacheService } from './cacheService'
import { findBestMatch } from '../utils/stringSimilarity'
import { parseLrc, parseSmartLrc, parseQrcLyric, parseTTML, type LyricLine } from './lyricParser'

export interface OnlineSong {
  id: string
  title: string
  artist: string
  album: string
  duration: number
  coverUrl: string
  source: 'qq' | 'netease' | 'amll'
}

export interface LyricResult {
  lrcData: LyricLine[]
  yrcData: LyricLine[]
  source: 'qq' | 'netease' | 'amll'
  hasTranslation: boolean
  hasRomanization: boolean
}

class OnlineMusicService {
  private readonly QQ_MUSIC_API = 'https://api.lxmusic.top/qq'
  private readonly NETEASE_API = 'https://music.163.com/api'
  private readonly AMLL_API = 'https://amll.nyakku.moe/ttml'

  /**
   * 搜索歌曲
   */
  async searchSong(keyword: string): Promise<OnlineSong[]> {
    try {
      const [qqResults, neteaseResults] = await Promise.all([
        this.searchQQMusic(keyword),
        this.searchNetease(keyword)
      ])

      return this.mergeAndDeduplicate([...qqResults, ...neteaseResults])
    } catch (error) {
      console.error('搜索失败:', error)
      return []
    }
  }

  private async searchQQMusic(keyword: string): Promise<OnlineSong[]> {
    try {
      const response = await fetch(`${this.QQ_MUSIC_API}/search/${encodeURIComponent(keyword)}?page=1&limit=20`)
      if (!response.ok) throw new Error('QQ 音乐搜索失败')

      const data = await response.json()
      return (data.data?.list || []).map((item: any) => ({
        id: item.id,
        title: item.title,
        artist: item.singer?.[0]?.name || '未知艺术家',
        album: item.album?.name || '',
        duration: item.interval || 0,
        coverUrl: item.album?.mid ? `https://y.gtimg.cn/music/photo_new/T002R300x300M000${item.album.mid}.jpg` : '',
        source: 'qq' as const
      }))
    } catch {
      return []
    }
  }

  private async searchNetease(keyword: string): Promise<OnlineSong[]> {
    try {
      const response = await fetch(`${this.NETEASE_API}/search/get/web`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Referer': 'https://music.163.com'
        },
        body: JSON.stringify({ s: keyword, type: 1, page: 1, limit: 20 })
      })

      if (!response.ok) throw new Error('网易云搜索失败')

      const data = await response.json()
      return (data.result?.songs || []).map((item: any) => ({
        id: item.id.toString(),
        title: item.name,
        artist: item.artists?.[0]?.name || '未知艺术家',
        album: item.album?.name || '',
        duration: item.duration || 0,
        coverUrl: item.album?.picUrl || '',
        source: 'netease' as const
      }))
    } catch {
      return []
    }
  }

  private mergeAndDeduplicate(results: OnlineSong[]): OnlineSong[] {
    const map = new Map<string, OnlineSong>()
    
    for (const result of results) {
      const key = `${result.title}-${result.artist}`.toLowerCase()
      if (!map.has(key)) {
        map.set(key, result)
      }
    }

    return Array.from(map.values())
  }

  /**
   * 获取歌词
   */
  async getLyric(songId: string, source: 'qq' | 'netease'): Promise<LyricResult> {
    const cacheKey = `${source}_${songId}`
    
    // 检查缓存
    const cached = await cacheService.getLyric(cacheKey)
    if (cached) {
      return cached
    }

    // 请求 API
    const result = source === 'qq'
      ? await this.getQQMusicLyric(songId)
      : await this.getNeteaseLyric(songId)

    // 保存到缓存
    await cacheService.saveLyric(cacheKey, source, result)

    return result
  }

  private async getQQMusicLyric(songId: string): Promise<LyricResult> {
    try {
      const response = await fetch(`${this.QQ_MUSIC_API}/lyric/${songId}`)
      if (!response.ok) throw new Error('QQ 音乐歌词获取失败')

      const data = await response.json()
      
      const result: LyricResult = {
        lrcData: [],
        yrcData: [],
        source: 'qq',
        hasTranslation: false,
        hasRomanization: false
      }

      // 解析 QRC 逐字歌词
      if (data.qrc) {
        result.yrcData = parseQrcLyric(data.qrc)
      }

      // 解析 LRC 歌词
      if (data.lrc) {
        result.lrcData = parseSmartLrc(data.lrc).lines
        
        // 翻译
        if (data.trans) {
          const transLines = parseSmartLrc(data.trans).lines
          result.lrcData = this.alignLyrics(result.lrcData, transLines, 'translatedLyric')
          result.hasTranslation = true
        }

        // 音译
        if (data.roma) {
          const romaLines = parseSmartLrc(data.roma).lines
          result.lrcData = this.alignLyrics(result.lrcData, romaLines, 'romanLyric')
          result.hasRomanization = true
        }
      }

      return result
    } catch (error) {
      console.error('QQ 音乐歌词获取失败:', error)
      return { lrcData: [], yrcData: [], source: 'qq', hasTranslation: false, hasRomanization: false }
    }
  }

  private async getNeteaseLyric(songId: string): Promise<LyricResult> {
    try {
      const response = await fetch(`${this.NETEASE_API}/song/lyric/v1?id=${songId}`, {
        headers: { 'Referer': 'https://music.163.com' }
      })

      if (!response.ok) throw new Error('网易云歌词获取失败')

      const data = await response.json()
      
      const result: LyricResult = {
        lrcData: [],
        yrcData: [],
        source: 'netease',
        hasTranslation: false,
        hasRomanization: false
      }

      // 逐字歌词
      if (data.yrc?.lyric) {
        result.yrcData = parseSmartLrc(data.yrc.lyric).lines
        
        if (data.ytlrc?.lyric) {
          const transLines = parseSmartLrc(data.ytlrc.lyric).lines
          result.yrcData = this.alignLyrics(result.yrcData, transLines, 'translatedLyric')
          result.hasTranslation = true
        }
      }

      // 普通歌词
      if (data.lrc?.lyric && result.yrcData.length === 0) {
        result.lrcData = parseSmartLrc(data.lrc.lyric).lines
        
        if (data.tlyric?.lyric) {
          const transLines = parseSmartLrc(data.tlyric.lyric).lines
          result.lrcData = this.alignLyrics(result.lrcData, transLines, 'translatedLyric')
          result.hasTranslation = true
        }
      }

      return result
    } catch (error) {
      console.error('网易云歌词获取失败:', error)
      return { lrcData: [], yrcData: [], source: 'netease', hasTranslation: false, hasRomanization: false }
    }
  }

  private alignLyrics(
    base: LyricLine[],
    align: LyricLine[],
    field: 'translatedLyric' | 'romanLyric'
  ): LyricLine[] {
    // 简单的对齐逻辑：按索引匹配
    return base.map((line, i) => ({
      ...line,
      [field]: align[i]?.words?.[0]?.word || ''
    }))
  }

  /**
   * 获取封面
   */
  async getCover(songId: string, source: 'qq' | 'netease', size: 's' | 'm' | 'l' | 'xl' = 'l'): Promise<string | null> {
    const cacheKey = `${source}_${songId}_${size}`
    
    // 检查缓存
    const cached = await cacheService.getCover(cacheKey)
    if (cached) return cached

    // 获取封面 URL
    const coverUrl = source === 'qq'
      ? await this.getQQMusicCover(songId, size)
      : await this.getNeteaseCover(songId, size)

    if (!coverUrl) return null

    // 下载并保存
    try {
      const response = await fetch(coverUrl)
      const blob = await response.blob()
      const arrayBuffer = await blob.arrayBuffer()
      const uint8Array = new Uint8Array(arrayBuffer)

      await cacheService.saveCover(cacheKey, source, uint8Array)

      return URL.createObjectURL(blob)
    } catch (error) {
      console.error('下载封面失败:', error)
      return coverUrl // 返回原 URL
    }
  }

  private async getQQMusicCover(songId: string, size: 's' | 'm' | 'l' | 'xl'): Promise<string | null> {
    const sizeMap = { s: 150, m: 300, l: 500, xl: 1000 }
    try {
      const response = await fetch(`${this.QQ_MUSIC_API}/cover/${songId}`)
      if (!response.ok) throw new Error('QQ 音乐封面获取失败')

      const data = await response.json()
      const baseUrl = data.coverUrl || ''
      
      // QQ 音乐封面 URL 格式：https://y.gtimg.cn/music/photo_new/T002R{size}x{size}M000{mid}.jpg
      return baseUrl.replace('R300x300', `R${sizeMap[size]}x${sizeMap[size]}`)
    } catch {
      return null
    }
  }

  private async getNeteaseCover(songId: string, size: 's' | 'm' | 'l' | 'xl'): Promise<string | null> {
    const sizeMap = { s: 150, m: 300, l: 500, xl: 1000 }
    try {
      const response = await fetch(`${this.NETEASE_API}/song/detail?ids=${songId}`, {
        headers: { 'Referer': 'https://music.163.com' }
      })

      if (!response.ok) throw new Error('网易云封面获取失败')

      const data = await response.json()
      const song = data.songs?.[0]
      const picUrl = song?.album?.picUrl || ''

      // 网易云封面尺寸参数
      return `${picUrl}?param=${sizeMap[size]}y${sizeMap[size]}`
    } catch {
      return null
    }
  }

  /**
   * 智能匹配本地歌曲
   */
  async matchLocalSong(title: string, artist: string): Promise<OnlineSong | null> {
    const keyword = `${title} ${artist}`.trim()
    const searchResults = await this.searchSong(keyword)
    
    if (searchResults.length === 0) return null

    // 使用相似度算法找到最佳匹配
    const target = { title, artist }
    const bestMatch = findBestMatch(target, searchResults)

    return bestMatch
  }

  /**
   * 获取动态封面
   */
  async getDynamicCover(songId: string): Promise<string | null> {
    try {
      const response = await fetch(`${this.NETEASE_API}/song/dynamic/cover?id=${songId}`, {
        headers: { 'Referer': 'https://music.163.com' }
      })

      if (!response.ok) throw new Error('动态封面获取失败')

      const data = await response.json()
      return data.data?.videoPlayUrl || null
    } catch {
      return null
    }
  }
}

export const onlineMusicService = new OnlineMusicService()
