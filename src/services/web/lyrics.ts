/**
 * Web 预览歌词服务 —— services/lyrics 在非 Tauri 环境下的实现
 *
 * 依据曲目 id 确定性生成一份带时间轴的歌词（刷新后一致），用于验证
 * 逐行高亮 / 自动滚动 / 偏移校正等界面交互。少数曲目返回 null（无词）
 * 或纯音乐行，以覆盖空状态分支。真实歌词优先级链（内嵌 > 边车 > 在线）在 M3 落地。
 */
import type { Lyrics } from '../../types'
import { getTrackById } from './dataset'

const PHRASES = [
  '夜色把城市慢慢折叠',
  '风从很远的地方带来消息',
  '我们在港口交换了名字',
  '灯光像浮在水面的星',
  '时间在这里放轻了脚步',
  '把没说完的话交给潮汐',
  '山脊线托起最后的光',
  '一整列火车驶过梦境',
  '玻璃窗映出两个黄昏',
  '我把思念折成纸飞机',
  '雨落在旧屋顶的音阶上',
  '远方的雪正悄悄落下',
  'Every echo calls your name',
  'We were brighter than the morning',
  'Hold the light a little longer',
  '沿着溪流一直走回家',
  '雾散之后群山重新呼吸',
  '把今天轻轻放进抽屉',
]

/** 基于字符串的确定性伪随机（mulberry32 思路的简化版） */
function seededRandom(seed: number): () => number {
  let a = seed >>> 0
  return () => {
    a |= 0
    a = (a + 0x6d2b79f5) | 0
    let t = Math.imul(a ^ (a >>> 15), 1 | a)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

function seedOf(trackId: string): number {
  let seed = 0
  for (let i = 0; i < trackId.length; i += 1) seed = (seed * 31 + trackId.charCodeAt(i)) >>> 0
  return seed
}

const offsets = new Map<string, number>()

export const webLyrics = {
  async load(trackId: string): Promise<Lyrics | null> {
    const track = getTrackById(trackId)
    if (!track) return null
    const numeric = Number(trackId.replace(/\D/g, '')) || 0
    // 每 9 首中有 1 首无歌词，覆盖“未找到歌词”空态
    if (numeric % 9 === 0) return null

    const random = seededRandom(seedOf(trackId))
    const durationMs = track.duration * 1000
    const lineCount = Math.min(26, Math.max(8, Math.round(track.duration / 9)))
    const introMs = 9000
    const span = Math.max(1, durationMs - introMs - 6000)
    const lines = Array.from({ length: lineCount }, (_, index) => {
      const timeMs = Math.round(introMs + (span * index) / (lineCount - 1))
      const phrase = PHRASES[Math.floor(random() * PHRASES.length)]
      // 每 7 行安排一行“纯音乐”间奏
      const text = index > 0 && index % 7 === 0 ? '♪' : phrase
      return { timeMs, text, translation: null }
    })

    return {
      trackId,
      source: numeric % 3 === 0 ? 'sidecar' : 'embedded',
      synced: true,
      lines,
      offsetMs: offsets.get(trackId) ?? 0,
    }
  },

  async saveOffset(trackId: string, offsetMs: number): Promise<void> {
    offsets.set(trackId, offsetMs)
  },

  async clearCache(): Promise<void> {
    offsets.clear()
  },
}
