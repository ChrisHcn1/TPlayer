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
