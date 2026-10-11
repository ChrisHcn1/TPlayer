/**
 * 分享卡片服务（收听报告 → 精美卡片 → 导出 PNG）
 *
 * 设计要点：
 *  - 卡片以 SVG 模板渲染（原生 SVG 元素，避免 foreignObject 在外部图片加载时失效）；
 *  - 颜色不从字面量写死：运行时经 getComputedStyle 读取 theme.css 语义 token
 *    （--color-bg-* / --color-text-* / --color-brand-*），深 / 浅主题自动适配；
 *  - 导出走 SVG → <img> → canvas → PNG，Tauri 环境经 plugin-dialog + plugin-fs
 *    保存到用户指定位置，浏览器环境降级为 a[download]；
 *  - 数据与视图解耦：调用方传 ShareCardData（由 stats store 构建）。
 */
import type { ShareCardData, TopArtistItem, TopTrackItem } from '../types/stats'
import { isTauriRuntime } from './ipc'

/** 卡片画布尺寸（竖版 3:4 分享卡） */
export const CARD_WIDTH = 1080
export const CARD_HEIGHT = 1440

/* ------------------------------------------------------------------ *
 * CSS 语义 token 解析（唯一取色入口，禁止字面量色值）
 * ------------------------------------------------------------------ */

/** 读取 theme.css 语义 token 的实际值；var(...) 引用递归解引用；失败回落黑色 */
function cssVar(name: string, depth = 0): string {
  if (depth > 8) return '#000000'
  try {
    const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
    if (!value) return '#000000'
    if (value.startsWith('var(')) {
      const inner = value.slice(4, -1).trim()
      return cssVar(inner, depth + 1)
    }
    return value
  } catch {
    return '#000000'
  }
}

/* ------------------------------------------------------------------ *
 * 文本工具
 * ------------------------------------------------------------------ */

/** SVG 文本转义（防止曲目 / 艺术家名中的 & < > 破坏 XML） */
function xmlEscape(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

/** 超长文本截断（SVG 无自动换行，按字符数保守截断） */
function clip(value: string, maxChars: number): string {
  return value.length > maxChars ? `${value.slice(0, maxChars - 1)}…` : value
}

/** 时长格式化：毫秒 → "X 小时 Y 分钟"（不足 1 小时显示分钟） */
export function formatDuration(ms: number): string {
  const totalMinutes = Math.floor(ms / 60_000)
  if (totalMinutes <= 0) {
    const seconds = Math.floor(ms / 1000)
    return seconds > 0 ? `${seconds} 秒` : '0 分钟'
  }
  const hours = Math.floor(totalMinutes / 60)
  const minutes = totalMinutes % 60
  if (hours > 0) {
    return minutes > 0 ? `${hours} 小时 ${minutes} 分钟` : `${hours} 小时`
  }
  return `${minutes} 分钟`
}

/** 时段名（按小时归类，用于"最常听时段"文案） */
function hourLabel(hour: number): string {
  if (hour >= 5 && hour < 9) return '清晨'
  if (hour >= 9 && hour < 12) return '上午'
  if (hour >= 12 && hour < 14) return '中午'
  if (hour >= 14 && hour < 18) return '下午'
  if (hour >= 18 && hour < 23) return '晚上'
  return '深夜'
}

/** 格式化 Unix 秒 → "2026-10-10" */
function formatDate(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000)
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(
    d.getDate(),
  ).padStart(2, '0')}`
}

/* ------------------------------------------------------------------ *
 * SVG 模板
 * ------------------------------------------------------------------ */

/** 排名徽标（圆 + 序号） */
function rankBadge(x: number, y: number, rank: number, brand: string, onBrand: string): string {
  return `
    <circle cx="${x}" cy="${y}" r="26" fill="${brand}"/>
    <text x="${x}" y="${y + 12}" text-anchor="middle" font-size="28" font-weight="700" fill="${onBrand}">${rank}</text>`
}

/** TOP 曲目行 */
function topTrackRow(
  y: number,
  rank: number,
  item: TopTrackItem,
  primary: string,
  secondary: string,
  tertiary: string,
  brand: string,
  onBrand: string,
  countText: string,
): string {
  const title = clip(item.title, 18)
  const artists = item.artists.length > 0 ? clip(item.artists.join(' / '), 26) : '未知艺术家'
  return `
    ${rankBadge(150, y, rank, brand, onBrand)}
    <text x="200" y="${y - 6}" font-size="32" font-weight="600" fill="${primary}">${xmlEscape(title)}</text>
    <text x="200" y="${y + 26}" font-size="26" fill="${tertiary}">${xmlEscape(artists)}</text>
    <text x="930" y="${y + 12}" text-anchor="end" font-size="30" font-weight="600" fill="${secondary}">${xmlEscape(countText)}</text>`
}

/** TOP 艺术家行 */
function topArtistRow(
  y: number,
  rank: number,
  item: TopArtistItem,
  primary: string,
  secondary: string,
  brand: string,
  onBrand: string,
  countText: string,
): string {
  const name = clip(item.name, 22)
  return `
    ${rankBadge(150, y, rank, brand, onBrand)}
    <text x="200" y="${y + 9}" font-size="30" font-weight="600" fill="${primary}">${xmlEscape(name)}</text>
    <text x="930" y="${y + 9}" text-anchor="end" font-size="28" font-weight="600" fill="${secondary}">${xmlEscape(countText)}</text>`
}

/** 构建分享卡片 SVG 字符串 */
export function buildShareCardSvg(data: ShareCardData): string {
  const bgBase = cssVar('--color-bg-base')
  const bgElevated = cssVar('--color-bg-elevated')
  const brand = cssVar('--color-brand')
  const brandSolid = cssVar('--color-brand-solid')
  const brandSubtle = cssVar('--color-brand-subtle')
  const primary = cssVar('--color-text-primary')
  const secondary = cssVar('--color-text-secondary')
  const tertiary = cssVar('--color-text-tertiary')
  const onBrand = cssVar('--color-on-brand')
  const fontFamily = cssVar('--font-family-base')

  const totalText = formatDuration(data.totalMs)
  const topTracks = data.topTracks.slice(0, 5)
  const topArtists = data.topArtists.slice(0, 3)
  const peak = data.peakHour

  // 最常听时段 24h 迷你柱状：max 归一化，peak 小时高亮
  const hourCounts = data.hourCounts ?? []
  const maxHour = Math.max(1, ...hourCounts)
  const barCount = 24
  const barAreaX = 180
  const barAreaY = 1325
  const barWidth = 26
  const barGap = 4
  const barMaxHeight = 60
  const bars = hourCounts
    .slice(0, barCount)
    .map((count, h) => {
      const height = Math.max(4, Math.round((count / maxHour) * barMaxHeight))
      const x = barAreaX + h * (barWidth + barGap)
      const y = barAreaY - height
      const fill = h === peak ? brandSolid : brandSubtle
      return `<rect x="${x}" y="${y}" width="${barWidth}" height="${height}" rx="4" fill="${fill}"/>`
    })
    .join('')

  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${CARD_WIDTH}" height="${CARD_HEIGHT}" viewBox="0 0 ${CARD_WIDTH} ${CARD_HEIGHT}" font-family="${xmlEscape(fontFamily)}">
  <defs>
    <linearGradient id="card-bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="${bgElevated}"/>
      <stop offset="1" stop-color="${bgBase}"/>
    </linearGradient>
    <linearGradient id="card-accent" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="${brand}"/>
      <stop offset="1" stop-color="${brandSolid}"/>
    </linearGradient>
  </defs>
  <rect width="${CARD_WIDTH}" height="${CARD_HEIGHT}" fill="url(#card-bg)"/>
  <circle cx="${CARD_WIDTH - 120}" cy="120" r="260" fill="${brandSubtle}" opacity="0.55"/>
  <circle cx="${CARD_WIDTH - 60}" cy="180" r="90" fill="url(#card-accent)" opacity="0.5"/>

  <text x="90" y="150" font-size="64" font-weight="700" fill="${primary}">${xmlEscape(data.title)}</text>
  <text x="90" y="216" font-size="34" font-weight="400" fill="${tertiary}">${xmlEscape(data.periodText)}</text>

  <g font-family="${xmlEscape(fontFamily)}">
    <text x="90" y="410" font-size="72" font-weight="700" fill="url(#card-accent)">${xmlEscape(totalText)}</text>
    <text x="90" y="462" font-size="30" fill="${tertiary}">累计播放时长</text>

    <text x="450" y="410" font-size="72" font-weight="700" fill="${primary}">${data.trackCount}</text>
    <text x="450" y="462" font-size="30" fill="${tertiary}">播放曲目数</text>

    <text x="810" y="410" font-size="72" font-weight="700" fill="${primary}">${data.playCount}</text>
    <text x="810" y="462" font-size="30" fill="${tertiary}">累计播放次数</text>
  </g>

  <line x1="90" y1="540" x2="990" y2="540" stroke="${brandSubtle}" stroke-width="2"/>

  <text x="90" y="640" font-size="34" font-weight="600" fill="${secondary}">TOP 曲目</text>
  ${topTracks
    .map((item, index) =>
      topTrackRow(
        750 + index * 84,
        index + 1,
        item,
        primary,
        secondary,
        tertiary,
        brand,
        onBrand,
        `${item.playCount} 次`,
      ),
    )
    .join('')}

  <text x="90" y="1130" font-size="34" font-weight="600" fill="${secondary}">TOP 艺术家</text>
  ${topArtists
    .map((item, index) =>
      topArtistRow(
        1210 + index * 62,
        index + 1,
        item,
        primary,
        secondary,
        brand,
        onBrand,
        `${item.playCount} 次`,
      ),
    )
    .join('')}

  <text x="90" y="1316" font-size="34" font-weight="600" fill="${secondary}">最常听时段</text>
  <text x="990" y="1316" text-anchor="end" font-size="30" font-weight="600" fill="${brand}">${xmlEscape(hourLabel(peak))} ${peak} 点</text>
  ${bars}

  <text x="90" y="${CARD_HEIGHT - 40}" font-size="26" fill="${tertiary}">生成于 ${formatDate(data.generatedAt)} · TPlayer Next</text>
</svg>`
}

/* ------------------------------------------------------------------ *
 * 导出 PNG
 * ------------------------------------------------------------------ */

export interface ExportResult {
  ok: boolean
  /** Tauri 环境为保存路径；浏览器降级为空串（已触发下载） */
  path?: string
  /** 用户取消保存 */
  cancelled?: boolean
}

/** 导出分享卡片 PNG：Tauri 走系统保存对话框 + plugin-fs；浏览器降级 a[download] */
export async function exportShareCardPng(data: ShareCardData): Promise<ExportResult> {
  const svg = buildShareCardSvg(data)
  const blob = new Blob([svg], { type: 'image/svg+xml;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  try {
    const img = new Image()
    img.decoding = 'async'
    await new Promise<void>((resolve, reject) => {
      img.onload = () => resolve()
      img.onerror = () => reject(new Error('卡片渲染失败'))
      img.src = url
    })

    const canvas = document.createElement('canvas')
    canvas.width = CARD_WIDTH
    canvas.height = CARD_HEIGHT
    const ctx = canvas.getContext('2d')
    if (!ctx) throw new Error('无法创建画布')
    ctx.drawImage(img, 0, 0, CARD_WIDTH, CARD_HEIGHT)

    const png = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'))
    if (!png) throw new Error('PNG 编码失败')

    if (isTauriRuntime()) {
      return await saveViaDialog(png)
    }
    return await downloadViaAnchor(png)
  } finally {
    URL.revokeObjectURL(url)
  }
}

/** Tauri：系统保存对话框 + plugin-fs 写盘 */
async function saveViaDialog(png: Blob): Promise<ExportResult> {
  try {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const { writeFile } = await import('@tauri-apps/plugin-fs')
    const defaultName = `收听报告-${formatDate(Math.floor(Date.now() / 1000))}.png`
    const path = await save({
      defaultPath: defaultName,
      filters: [{ name: 'PNG 图片', extensions: ['png'] }],
    })
    if (!path) return { ok: false, cancelled: true }
    const buffer = new Uint8Array(await png.arrayBuffer())
    await writeFile(path, buffer)
    return { ok: true, path }
  } catch (cause) {
    return { ok: false, path: cause instanceof Error ? cause.message : String(cause) }
  }
}

/** 浏览器环境：触发下载（web 预览调试用） */
async function downloadViaAnchor(png: Blob): Promise<ExportResult> {
  const url = URL.createObjectURL(png)
  try {
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = `收听报告-${formatDate(Math.floor(Date.now() / 1000))}.png`
    document.body.appendChild(anchor)
    anchor.click()
    anchor.remove()
    return { ok: true, path: '' }
  } finally {
    window.setTimeout(() => URL.revokeObjectURL(url), 30_000)
  }
}
