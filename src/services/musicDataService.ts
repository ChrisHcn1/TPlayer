import { parseSmartLrc, alignLyrics, parseQrcLyric, parseTTML, type LyricLine, type SongLyric } from './lyricParser'

export interface SongInfo {
  id?: string
  title: string
  artist: string
  album?: string
  duration?: number
  coverUrl?: string
  lyricUrl?: string
  lyric?: string
  lyricLines?: LyricLine[]
  dynamicCoverUrl?: string
}

export interface SearchSongResult {
  id: string
  title: string
  artist: string
  album: string
  duration: number
  coverUrl: string
  lyricUrl?: string
}

export interface LyricData {
  lyric?: string
  tlyric?: string
  romalyric?: string
  yrc?: string
  ytlrc?: string
  yromalyric?: string
}

export interface QQMusicLyricData {
  qrc?: string
  lrc?: string
  trans?: string
  roma?: string
  song?: {
    duration: number
  }
  code: number
}

export interface DynamicCoverData {
  data?: {
    videoPlayUrl?: string
  }
}

class MusicDataService {
  private readonly API_BASE = 'https://music.163.com/api'
  private readonly SEARCH_API = 'https://music.163.com/api/search/get/web'
  private readonly LYRIC_API = 'https://music.163.com/api/song/lyric'
  private readonly QQ_MUSIC_API = 'https://api.lxmusic.top/qq'
  private readonly AMLL_DB_SERVER = 'https://amll.nyakku.moe/ttml/%s.ttml'
  private readonly DYNAMIC_COVER_API = 'https://music.163.com/api/song/dynamic/cover'

  async searchSong(keyword: string): Promise<SearchSongResult[]> {
    try {
      // 使用 POST 请求并添加必要的请求头
      const response = await fetch(this.SEARCH_API, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/x-www-form-urlencoded',
          'Referer': 'https://music.163.com',
        },
        body: new URLSearchParams({
          s: keyword,
          type: '1',
          limit: '10',
          offset: '0'
        })
      })
      
      if (!response.ok) {
        console.error('搜索请求失败:', response.status, response.statusText)
        return []
      }
      
      const data = await response.json()
      
      if (data.code === 200 && data.result?.songs) {
        return data.result.songs.map((song: any) => ({
          id: song.id.toString(),
          title: song.name,
          artist: song.artists?.map((a: any) => a.name).join(', ') || '未知艺术家',
          album: song.album?.name || '',
          duration: song.duration || 0,
          coverUrl: song.album?.picUrl || '',
          lyricUrl: song.id ? `${this.LYRIC_API}?id=${song.id}` : undefined,
        }))
      }
      
      return []
    } catch (error) {
      console.error('搜索歌曲失败:', error)
      return []
    }
  }

  async getLyric(songId: string): Promise<LyricData | null> {
    try {
      const response = await fetch(`${this.LYRIC_API}?id=${songId}&lv=-1&kv=-1&tv=-1`, {
        headers: {
          'Referer': 'https://music.163.com',
        }
      })
      
      if (!response.ok) {
        console.error('歌词请求失败:', response.status, response.statusText)
        return null
      }
      
      const data = await response.json()
      
      if (data.code === 200) {
        return {
          lyric: data.lrc?.lyric || '',
          tlyric: data.tlyric?.lyric || '',
          romalyric: data.romalyric?.lyric || '',
          yrc: data.yrc?.lyric || '',
          ytlrc: data.ytlrc?.lyric || '',
          yromalyric: data.yromalyric?.lyric || '',
        }
      }
      
      return null
    } catch (error) {
      console.error('获取歌词失败:', error)
      return null
    }
  }

  async getTTMLLyric(songId: string): Promise<string | null> {
    try {
      const url = this.AMLL_DB_SERVER.replace('%s', songId)
      const response = await fetch(url)
      
      if (!response.ok) {
        console.error('TTML歌词请求失败:', response.status, response.statusText)
        return null
      }
      
      const data = await response.text()
      return data
    } catch (error) {
      console.error('获取 TTML 歌词失败:', error)
      return null
    }
  }

  async getQQMusicLyric(keyword: string): Promise<QQMusicLyricData | null> {
    try {
      const response = await fetch(`${this.QQ_MUSIC_API}/search?keyword=${encodeURIComponent(keyword)}`)
      
      if (!response.ok) {
        console.error('QQ音乐歌词请求失败:', response.status, response.statusText)
        return null
      }
      
      const data = await response.json()
      
      if (data.code === 200) {
        return data
      }
      
      return null
    } catch (error) {
      console.error('获取 QQ 音乐歌词失败:', error)
      return null
    }
  }

  async getDynamicCover(songId: string): Promise<string | null> {
    try {
      const response = await fetch(`${this.DYNAMIC_COVER_API}?id=${songId}`, {
        headers: {
          'Referer': 'https://music.163.com',
        }
      })
      
      if (!response.ok) {
        console.error('动态封面请求失败:', response.status, response.statusText)
        return null
      }
      
      const data = await response.json()
      
      if (data.code === 200 && data.data && data.data.videoPlayUrl) {
        return data.data.videoPlayUrl
      }
      
      return null
    } catch (error) {
      console.error('获取动态封面失败:', error)
      return null
    }
  }

  async getSongDetail(songId: string): Promise<SongInfo | null> {
    try {
      const response = await fetch(`${this.API_BASE}/song/detail?ids=${songId}`, {
        headers: {
          'Referer': 'https://music.163.com',
        }
      })
      
      if (!response.ok) {
        console.error('歌曲详情请求失败:', response.status, response.statusText)
        return null
      }
      
      const data = await response.json()
      
      if (data.code === 200 && data.songs?.length > 0) {
        const song = data.songs[0]
        return {
          id: song.id.toString(),
          title: song.name,
          artist: song.ar?.map((a: any) => a.name).join(', ') || '未知艺术家',
          album: song.al?.name || '',
          duration: song.dt || 0,
          coverUrl: song.al?.picUrl || '',
        }
      }
      
      return null
    } catch (error) {
      console.error('获取歌曲详情失败:', error)
      return null
    }
  }

  async getSongInfoWithLyric(keyword: string): Promise<{
    song: SongInfo | null
    lyricData: SongLyric
    hasTTML: boolean
    hasQRC: boolean
  }> {
    try {
      const searchResults = await this.searchSong(keyword)
      if (searchResults.length === 0) {
        return { song: null, lyricData: { lrcData: [], yrcData: [] }, hasTTML: false, hasQRC: false }
      }

      const bestMatch = searchResults[0]
      const lyricData = await this.getLyric(bestMatch.id)
      const ttmlData = await this.getTTMLLyric(bestMatch.id)
      const qqMusicData = await this.getQQMusicLyric(keyword)
      const dynamicCoverUrl = await this.getDynamicCover(bestMatch.id)

      let lrcData: LyricLine[] = []
      let yrcData: LyricLine[] = []
      let hasTTML = false
      let hasQRC = false

      // 处理标准歌词
      if (lyricData?.lyric) {
        const parsed = parseSmartLrc(lyricData.lyric)
        lrcData = parsed.lines
        
        // 处理翻译
        if (lyricData.tlyric) {
          const tlyricParsed = parseSmartLrc(lyricData.tlyric)
          lrcData = alignLyrics(lrcData, tlyricParsed.lines, 'translatedLyric')
        }
        
        // 处理音译
        if (lyricData.romalyric) {
          const romalrcParsed = parseSmartLrc(lyricData.romalyric)
          lrcData = alignLyrics(lrcData, romalrcParsed.lines, 'romanLyric')
        }
      }

      // 处理逐字歌词
      if (lyricData?.yrc) {
        const parsed = parseSmartLrc(lyricData.yrc)
        yrcData = parsed.lines
        
        // 处理逐字翻译
        if (lyricData.ytlrc) {
          const ytlrcParsed = parseSmartLrc(lyricData.ytlrc)
          yrcData = alignLyrics(yrcData, ytlrcParsed.lines, 'translatedLyric')
        }
        
        // 处理逐字音译
        if (lyricData.yromalyric) {
          const yromalrcParsed = parseSmartLrc(lyricData.yromalyric)
          yrcData = alignLyrics(yrcData, yromalrcParsed.lines, 'romanLyric')
        }
      }

      // 处理 TTML 歌词
      if (ttmlData) {
        const ttmlParsed = parseTTML(ttmlData)
        if (ttmlParsed.lines.length > 0) {
          yrcData = ttmlParsed.lines
          hasTTML = true
        }
      }

      // 处理 QQ 音乐歌词
    if (qqMusicData?.qrc) {
      const qrcParsed = parseQrcLyric(qqMusicData.qrc)
      if (qrcParsed.length > 0) {
        yrcData = qrcParsed
        hasQRC = true
      }
    } else if (qqMusicData?.lrc) {
      const lrcParsed = parseSmartLrc(qqMusicData.lrc)
      if (lrcParsed.lines.length > 0) {
        lrcData = lrcParsed.lines
        
        // 处理翻译
        if (qqMusicData.trans) {
          const transParsed = parseSmartLrc(qqMusicData.trans)
          lrcData = alignLyrics(lrcData, transParsed.lines, 'translatedLyric')
        }
        
        // 处理音译
        if (qqMusicData.roma) {
          const romaParsed = parseSmartLrc(qqMusicData.roma)
          lrcData = alignLyrics(lrcData, romaParsed.lines, 'romanLyric')
        }
      }
    }

      const songInfo: SongInfo = {
        id: bestMatch.id,
        title: bestMatch.title,
        artist: bestMatch.artist,
        album: bestMatch.album,
        duration: bestMatch.duration,
        coverUrl: bestMatch.coverUrl,
        dynamicCoverUrl: dynamicCoverUrl || undefined,
        lyric: lyricData?.lyric,
        lyricLines: lrcData.length > 0 ? lrcData : yrcData
      }

      return {
        song: songInfo,
        lyricData: { lrcData, yrcData },
        hasTTML,
        hasQRC
      }
    } catch (error) {
      console.error('获取歌曲信息失败:', error)
      return { song: null, lyricData: { lrcData: [], yrcData: [] }, hasTTML: false, hasQRC: false }
    }
  }

  async matchLocalSong(title: string, artist: string): Promise<SongInfo | null> {
    const keyword = `${title} ${artist}`.trim()
    const result = await this.getSongInfoWithLyric(keyword)
    return result.song
  }

  async downloadCover(coverUrl: string): Promise<Blob | null> {
    try {
      const response = await fetch(coverUrl)
      if (response.ok) {
        return await response.blob()
      }
      return null
    } catch (error) {
      console.error('下载封面失败:', error)
      return null
    }
  }

  convertBlobToBase64(blob: Blob): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader()
      reader.onloadend = () => resolve(reader.result as string)
      reader.onerror = reject
      reader.readAsDataURL(blob)
    })
  }

  async getCoverAsBase64(coverUrl: string): Promise<string | null> {
    const blob = await this.downloadCover(coverUrl)
    if (blob) {
      return await this.convertBlobToBase64(blob)
    }
    return null
  }

  async batchGetSongInfo(songIds: string[]): Promise<SongInfo[]> {
    const results: SongInfo[] = []
    
    for (const id of songIds) {
      const song = await this.getSongDetail(id)
      if (song) {
        results.push(song)
      }
    }
    
    return results
  }
}

export const musicDataService = new MusicDataService()
