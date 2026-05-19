import { parseLrc } from './lyricParser'

const CHARTS_LYRICS_URL = 'https://music-api.vercel.app/chartlyrics'

export interface ChartLyricsResult {
  lyric: string
}

export interface ChartLyricsSearchResult {
  success: boolean
  lyric?: string
  lyricLines?: any[]
  error?: string
}

export const chartLyricsService = {
  async searchLyric(artist: string, title: string): Promise<ChartLyricsSearchResult> {
    try {
      const params = new URLSearchParams({
        artist: artist,
        title: title
      })

      const response = await fetch(`${CHARTS_LYRICS_URL}?${params.toString()}`, {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json'
        }
      })

      if (!response.ok) {
        throw new Error(`ChartLyrics API 错误: ${response.status}`)
      }

      const data: ChartLyricsResult = await response.json()

      if (!data.lyric || data.lyric.trim() === '') {
        return {
          success: false,
          error: '未找到歌词'
        }
      }

      const lyricLines = parseLrc(data.lyric)

      return {
        success: true,
        lyric: data.lyric,
        lyricLines: lyricLines
      }
    } catch (error) {
      console.error('[ChartLyrics] 查询失败:', error)
      return {
        success: false,
        error: error instanceof Error ? error.message : '未知错误'
      }
    }
  }
}
