import { parseSmartLrc } from '../services/lyricParser'
import { logInfo, logError } from './logger'

// 界面歌词滚动使用的简单歌词行格式
export interface SimpleLyricLine {
  time: number // 时间戳（秒）
  text: string // 歌词内容
}

// 将新版结构化歌词行（startTime 毫秒、words 逐字数据）
// 转换为界面歌词滚动使用的 { time(秒), text } 格式
export function toSimpleLyricLines(
  lines: { startTime: number; words: { word: string }[] }[]
): SimpleLyricLine[] {
  return lines.map(line => ({
    time: line.startTime / 1000, // 转换为秒
    text: line.words.map(w => w.word).join('')
  }))
}

// 解析歌词内容为简单歌词行数组
// 主路径用 parseSmartLrc 解析结构化歌词，失败时回退到正则解析
export function parseLyrics(lyricContent: string): SimpleLyricLine[] {
  if (!lyricContent) return []

  try {
    const parsed = parseSmartLrc(lyricContent)
    logInfo('歌词解析完成，格式:', parsed.format, '行数:', parsed.lines.length)

    return toSimpleLyricLines(parsed.lines)
  } catch (error) {
    logError('歌词解析失败，使用简单解析:', error)

    // 回退到简单解析
    const lines: SimpleLyricLine[] = []
    const lyricLines = lyricContent.split('\n')
    const timeRegex = /\[(\d+):(\d+\.\d+)\]/g

    for (const line of lyricLines) {
      const matches = [...line.matchAll(timeRegex)]
      if (matches.length > 0) {
        const text = line.replace(timeRegex, '').trim()
        if (text) {
          for (const match of matches) {
            const minutes = parseInt(match[1])
            const seconds = parseFloat(match[2])
            const time = minutes * 60 + seconds
            lines.push({ time, text })
          }
        }
      }
    }

    lines.sort((a, b) => a.time - b.time)
    return lines
  }
}
