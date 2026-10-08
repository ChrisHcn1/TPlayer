// 秒数格式化为 mm:ss
export function formatTime(seconds: number): string {
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

// 毫秒时间戳格式化为 LRC 的时间标签格式 mm:ss.cs（百分之一秒）
export function formatTimeForLrc(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  const centiseconds = Math.floor((ms % 1000) / 10)
  return `${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}.${centiseconds.toString().padStart(2, '0')}`
}

// 秒数格式化为 h:mm:ss（小时数 > 0 时）或 m:ss
// 用于累计总时长等可能跨小时的场景
export function formatTimeWithHours(seconds: number): string {
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  const secs = Math.floor(seconds % 60)
  if (hours > 0) {
    return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`
  }
  return `${minutes}:${secs.toString().padStart(2, '0')}`
}

// 解析 "mm:ss" 形式的时长字符串为秒数
// 非法输入（空串、"未知"、非两段格式）返回 0，与原内联实现保持一致
export function parseDurationToSeconds(duration: string): number {
  if (!duration || duration === '未知') return 0
  const parts = duration.split(':')
  if (parts.length !== 2) return 0
  const minutes = parseInt(parts[0])
  const seconds = parseInt(parts[1])
  return minutes * 60 + seconds
}
