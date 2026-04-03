export interface LyricWord {
  word: string
  startTime: number
  endTime: number
  romanWord?: string
}

export interface LyricLine {
  words: LyricWord[]
  startTime: number
  endTime: number
  translatedLyric?: string
  romanLyric?: string
  isBG?: boolean
  isDuet?: boolean
}

export interface SongLyric {
  lrcData: LyricLine[]
  yrcData: LyricLine[]
}

export enum LrcFormat {
  Line = "line",
  WordByWord = "word-by-word",
  Enhanced = "enhanced",
  QRC = "qrc",
  TTML = "ttml"
}

const META_TAG_REGEX = /^\[[a-z]+:/i
const TIME_TAG_REGEX = /\[(\d{2}):(\d{2})\.(\d{1,})\]/g
const ENHANCED_TIME_TAG_REGEX = /<(\d{2}):(\d{2})\.(\d{1,})>/
const LINE_TIME_REGEX = /^\[(\d{2}):(\d{2})\.(\d{1,})\]/
const DEFAULT_WORD_DURATION = 1000

function parseTimeToMs(min: string, sec: string, ms: string): number {
  const minutes = parseInt(min, 10)
  const seconds = parseInt(sec, 10)
  const msNormalized = ms.padEnd(3, "0").slice(0, 3)
  const milliseconds = parseInt(msNormalized, 10)
  return minutes * 60 * 1000 + seconds * 1000 + milliseconds
}

function createWord(word: string, startTime: number, endTime: number = startTime): LyricWord {
  return {
    word,
    startTime,
    endTime,
    romanWord: "",
  }
}

function createLine(words: LyricWord[], startTime: number, endTime: number = 0): LyricLine {
  return {
    words,
    startTime,
    endTime,
    translatedLyric: "",
    romanLyric: "",
    isBG: false,
    isDuet: false,
  }
}

export function detectLrcFormat(content: string): LrcFormat {
  if (!content) return LrcFormat.Line
  
  // 检测 QRC 格式
  if (content.trim().startsWith('<') || content.includes('<QrcInfos>')) {
    return LrcFormat.QRC
  }
  
  // 检测 TTML 格式
  if (content.includes('<ttml') || content.includes('<body') || content.includes('<p>')) {
    return LrcFormat.TTML
  }
  
  const lines = content.split(/\r?\n/)
  for (const rawLine of lines) {
    const line = rawLine.trim()
    if (!line || META_TAG_REGEX.test(line)) continue
    if (ENHANCED_TIME_TAG_REGEX.test(line)) {
      return LrcFormat.Enhanced
    }
    const matches = line.match(TIME_TAG_REGEX)
    if (matches && matches.length > 1) {
      return LrcFormat.WordByWord
    }
  }
  return LrcFormat.Line
}

export function parseWordByWordLrc(content: string): LyricLine[] {
  const result: LyricLine[] = []
  let prevLine: LyricLine | null = null
  const WORD_BY_WORD_PATTERN = /\[(\d{2}):(\d{2})\.(\d{1,})\]([^[\\]]*)/g

  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim()
    if (!line || META_TAG_REGEX.test(line)) continue

    const words: LyricWord[] = []
    let lineStartTime = Infinity
    let prevWord: LyricWord | null = null

    const matches = line.matchAll(WORD_BY_WORD_PATTERN)

    for (const match of matches) {
      const startTime = parseTimeToMs(match[1], match[2], match[3])
      const wordText = match[4]

      if (!wordText && words.length === 0) continue

      lineStartTime = Math.min(lineStartTime, startTime)

      if (prevWord) {
        prevWord.endTime = startTime
      }

      if (wordText) {
        const newWord = createWord(wordText, startTime)
        words.push(newWord)
        prevWord = newWord
      }
    }

    if (prevWord) {
      prevWord.endTime = prevWord.startTime + DEFAULT_WORD_DURATION
    }

    if (words.length > 0) {
      const lineObj = createLine(words, lineStartTime === Infinity ? 0 : lineStartTime)
      lineObj.endTime = words[words.length - 1].endTime

      if (prevLine) {
        const prevLastWord = prevLine.words[prevLine.words.length - 1]
        if (lineObj.startTime > prevLastWord.startTime) {
          prevLastWord.endTime = Math.min(prevLastWord.endTime, lineObj.startTime)
          prevLine.endTime = prevLastWord.endTime
        }
      }

      result.push(lineObj)
      prevLine = lineObj
    }
  }

  return result
}

export function parseEnhancedLrc(content: string): LyricLine[] {
  const result: LyricLine[] = []
  let prevLine: LyricLine | null = null
  const ENHANCED_WORD_PATTERN = /<(\d{2}):(\d{2})\.(\d{1,})>([^<]*)/g

  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim()
    if (!line || META_TAG_REGEX.test(line)) continue

    const lineTimeMatch = LINE_TIME_REGEX.exec(line)
    if (!lineTimeMatch) continue

    const lineStartTime = parseTimeToMs(lineTimeMatch[1], lineTimeMatch[2], lineTimeMatch[3])
    const contentAfterTime = line.slice(lineTimeMatch[0].length)

    const words: LyricWord[] = []

    if (ENHANCED_TIME_TAG_REGEX.test(contentAfterTime)) {
      let prevWord: LyricWord | null = null

      const matches = contentAfterTime.matchAll(ENHANCED_WORD_PATTERN)

      for (const match of matches) {
        const startTime = parseTimeToMs(match[1], match[2], match[3])
        const wordText = match[4]

        if (prevWord) {
          prevWord.endTime = startTime
        }

        if (wordText) {
          const newWord = createWord(wordText, startTime)
          words.push(newWord)
          prevWord = newWord
        }
      }

      if (prevWord) {
        prevWord.endTime = prevWord.startTime + DEFAULT_WORD_DURATION
      }
    } else {
      const text = contentAfterTime.trim()
      if (text) {
        words.push(createWord(text, lineStartTime, lineStartTime + DEFAULT_WORD_DURATION))
      }
    }

    if (words.length > 0) {
      const lineObj = createLine(words, lineStartTime)
      lineObj.endTime = words[words.length - 1].endTime

      if (prevLine) {
        const prevLastWord = prevLine.words[prevLine.words.length - 1]
        if (lineObj.startTime > prevLastWord.startTime) {
          prevLastWord.endTime = Math.min(prevLastWord.endTime, lineObj.startTime)
          prevLine.endTime = prevLastWord.endTime
        }
      }

      result.push(lineObj)
      prevLine = lineObj
    }
  }

  return result
}

export function parseLrc(content: string): LyricLine[] {
  const result: LyricLine[] = []
  const LRC_LINE_PATTERN = /\[(\d{2}):(\d{2})\.(\d{1,})\](.*)/

  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim()
    if (!line || META_TAG_REGEX.test(line)) continue

    const match = LRC_LINE_PATTERN.exec(line)
    if (match) {
      const startTime = parseTimeToMs(match[1], match[2], match[3])
      const text = match[4].trim()

      if (text) {
        const words = [createWord(text, startTime, startTime + DEFAULT_WORD_DURATION)]
        const lineObj = createLine(words, startTime, startTime + DEFAULT_WORD_DURATION)
        result.push(lineObj)
      }
    }
  }

  return result
}

/**
 * 解析 QRC 歌词
 */
export function parseQrcLyric(content: string): LyricLine[] {
  try {
    // 简单的 QRC 解析实现
    
    // 尝试解析 XML 格式
    if (content.includes('<QrcInfos>')) {
      // 这里可以实现更复杂的 XML 解析
      // 暂时回退到标准解析
      return parseLrc(content)
    }
    
    // 尝试解析 JSON 格式
    try {
      const data = JSON.parse(content)
      if (data.qrc) {
        return parseLrc(data.qrc)
      }
    } catch {
      // 解析失败，回退到标准解析
    }
    
    return parseLrc(content)
  } catch (error) {
    console.error('QRC 歌词解析失败:', error)
    return parseLrc(content)
  }
}

/**
 * 解析 TTML 歌词
 */
export function parseTTML(_content: string): { lines: LyricLine[] } {
  try {
    // 简单的 TTML 解析实现
    // 这里可以实现更复杂的 TTML 解析
    const result: LyricLine[] = []
    
    return { lines: result }
  } catch (error) {
    console.error('TTML 歌词解析失败:', error)
    return { lines: [] }
  }
}

export function parseSmartLrc(content: string): { format: LrcFormat; lines: LyricLine[] } {
  const format = detectLrcFormat(content)

  let lines: LyricLine[]
  switch (format) {
    case LrcFormat.QRC:
      lines = parseQrcLyric(content)
      break
    case LrcFormat.TTML:
      const ttmlResult = parseTTML(content)
      lines = ttmlResult.lines
      break
    case LrcFormat.WordByWord:
      lines = parseWordByWordLrc(content)
      break
    case LrcFormat.Enhanced:
      lines = parseEnhancedLrc(content)
      break
    default:
      lines = parseLrc(content)
  }

  return { format, lines }
}

export function isWordLevelFormat(format: LrcFormat): boolean {
  return format === LrcFormat.WordByWord || format === LrcFormat.Enhanced || format === LrcFormat.QRC
}

export function formatTime(ms: number): string {
  const totalSeconds = ms / 1000
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes.toString().padStart(2, "0")}:${seconds.toFixed(2).padStart(5, "0")}`
}

export function getCurrentLyricLine(lines: LyricLine[], currentTime: number): number {
  if (!lines.length) return -1

  for (let i = lines.length - 1; i >= 0; i--) {
    if (currentTime >= lines[i].startTime) {
      return i
    }
  }

  return -1
}

export function getLyricText(lines: LyricLine[], index: number): string {
  if (index < 0 || index >= lines.length) return ""
  return lines[index].words.map(w => w.word).join("")
}

/**
 * 对齐歌词（用于翻译和音译）
 */
export function alignLyrics(
  mainLines: LyricLine[],
  subLines: LyricLine[],
  type: 'translatedLyric' | 'romanLyric'
): LyricLine[] {
  const result = [...mainLines]
  
  for (let i = 0; i < result.length; i++) {
    const mainLine = result[i]
    const subLine = subLines.find(line => 
      Math.abs(line.startTime - mainLine.startTime) < 1000
    )
    
    if (subLine) {
      mainLine[type] = subLine.words.map(w => w.word).join("")
    }
  }
  
  return result
}

/**
 * 清洗 TTML 中不需要的翻译
 */
export function cleanTTMLTranslations(ttmlContent: string): string {
  const lang_counter = (ttml_text: string) => {
    const langRegex = /(?<=<(span|translation)[^<>]+)xml:lang="([^"]+)"/g
    const matches = ttml_text.matchAll(langRegex)
    
    const langSet = new Set<string>()
    for (const match of matches) {
      if (match[2]) langSet.add(match[2])
    }
    
    return Array.from(langSet)
  }
  
  const lang_filter = (langs: string[]): string | null => {
    if (langs.length <= 1) return null
    
    const lang_matcher = (target: string) => {
      return langs.find((lang) => {
        try {
          return new Intl.Locale(lang).maximize().script === target
        } catch {
          return false
        }
      })
    }
    
    const hans_matched = lang_matcher('Hans')
    if (hans_matched) return hans_matched
    
    const hant_matched = lang_matcher('Hant')
    if (hant_matched) return hant_matched
    
    const major = langs.find((key) => key.startsWith('zh'))
    if (major) return major
    
    return langs[0]
  }
  
  const ttml_cleaner = (ttml_text: string, major_lang: string | null): string => {
    if (major_lang === null) return ttml_text
    
    const replacer = (match: string, lang: string) => (lang === major_lang ? match : '')
    const translationRegex = /<translation[^>]+xml:lang="([^"]+)"[^>]*>[\s\S]*?<\/translation>/g
    const spanRegex = /<span[^>]+xml:lang="([^" ]+)"[^>]*>[\s\S]*?<\/span>/g
    return ttml_text.replace(translationRegex, replacer).replace(spanRegex, replacer)
  }
  
  const context_lang = lang_counter(ttmlContent)
  const major = lang_filter(context_lang)
  const cleaned_ttml = ttml_cleaner(ttmlContent, major)
  
  return cleaned_ttml.replace(/\n\s*/g, '')
}
