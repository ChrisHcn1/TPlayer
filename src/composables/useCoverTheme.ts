/**
 * useCoverTheme —— 皮肤跟随当前曲目封面
 *
 * 从封面 data URL 提取主色（小尺寸 canvas 采样 + HSL 色相桶量化），
 * 派生出整套品牌色阶并临时接管全局品牌 token，于是侧栏选中态、主按钮、
 * 开关、进度滑块、当前歌词高亮、播放行指示条等"皮肤色"全部随封面变化：
 *
 *   覆盖的主题 token（关闭/切歌失败时移除，自动回落 theme.css 定义）：
 *     --color-brand          文字/选中态色（按深/浅主题给不同亮度，保证对比度）
 *     --color-brand-solid    实色按钮
 *     --color-brand-hover
 *     --color-brand-active
 *     --color-brand-subtle   低透明染色背景
 *
 *   封面氛围专用变量：
 *     --color-cover-accent       主强调色
 *     --color-cover-accent-glow  45% 透明，光晕
 *     --color-cover-accent-soft  18% 透明，浅染色
 *     --color-cover-wash         7% 透明，整窗极淡背景染
 *     --color-cover-lyrics       歌词当前行色（近白/近墨的高亮文本色，与彩色 UI 区分）
 *     --color-cover-lyrics-overlay  封面页歌词当前行色（沉浸大图场景：始终高亮、随封面明暗自适应）
 *     --color-cover-lyrics-stroke   封面页歌词当前行描边色（亮封面下保证对比度）
 *     --color-cover-progress     进度条已播放色（饱和实色中调）
 *
 * 特性：
 *  - 取色结果按封面 URL 缓存（切回旧曲目瞬时还原）；
 *  - 切换深/浅主题时按同一色相重新派生，无需重新采样；
 *  - 关闭开关 / 无封面 / 提取失败时清除全部覆盖；
 *  - SVG 默认封面不参与取色（矢量占位图无代表色）。
 */
import { watch } from 'vue'

import { useLayout } from './useLayout'
import { useTheme } from './useTheme'

// ---- 封面氛围变量 ----
const ACCENT_VAR = '--color-cover-accent'
const GLOW_VAR = '--color-cover-accent-glow'
const SOFT_VAR = '--color-cover-accent-soft'
const WASH_VAR = '--color-cover-wash'
/** 歌词当前行色：比皮肤色（brand）更亮一档，与进度/按钮区分 */
const LYRICS_VAR = '--color-cover-lyrics'
/** 封面页歌词当前行色：沉浸式大图场景专用，始终高亮、随封面明暗自适应 */
const LYRICS_OVERLAY_VAR = '--color-cover-lyrics-overlay'
/** 封面页歌词当前行描边色（text-shadow 用），保证亮色封面下仍有对比 */
const LYRICS_STROKE_VAR = '--color-cover-lyrics-stroke'
/** 播放进度条已播放色：介于皮肤色与歌词色之间的中间调 */
const PROGRESS_VAR = '--color-cover-progress'

// ---- 被接管的主题品牌 token ----
const BRAND_VARS = [
  '--color-brand',
  '--color-brand-solid',
  '--color-brand-hover',
  '--color-brand-active',
  '--color-brand-subtle',
] as const

/** 提取出的封面主色（色相 0-1 / 饱和度 0-1，亮度在派生时按用途给定） */
interface CoverHue {
  h: number
  s: number
  /** 封面整体感知亮度（相对亮度加权均值 0-1，可选：灰度/失败分支由调用处兜底） */
  luma?: number
}

const colorCache = new Map<string, CoverHue>()

interface RGB {
  r: number
  g: number
  b: number
}

function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
  r /= 255
  g /= 255
  b /= 255
  const max = Math.max(r, g, b)
  const min = Math.min(r, g, b)
  const l = (max + min) / 2
  if (max === min) return [0, 0, l]
  const d = max - min
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min)
  let h: number
  if (max === r) h = ((g - b) / d + (g < b ? 6 : 0)) / 6
  else if (max === g) h = ((b - r) / d + 2) / 6
  else h = ((r - g) / d + 4) / 6
  return [h, s, l]
}

function hslToRgb(h: number, s: number, l: number): RGB {
  if (s === 0) {
    const v = Math.round(l * 255)
    return { r: v, g: v, b: v }
  }
  const hue2rgb = (p: number, q: number, t: number): number => {
    if (t < 0) t += 1
    if (t > 1) t -= 1
    if (t < 1 / 6) return p + (q - p) * 6 * t
    if (t < 1 / 2) return q
    if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6
    return p
  }
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s
  const p = 2 * l - q
  return {
    r: Math.round(hue2rgb(p, q, h + 1 / 3) * 255),
    g: Math.round(hue2rgb(p, q, h) * 255),
    b: Math.round(hue2rgb(p, q, h - 1 / 3) * 255),
  }
}

function hslCss(h: number, s: number, l: number): string {
  return `hsl(${Math.round(h * 360)}, ${Math.round(s * 100)}%, ${Math.round(l * 100)}%)`
}

function hslaCss(h: number, s: number, l: number, a: number): string {
  return `hsla(${Math.round(h * 360)}, ${Math.round(s * 100)}%, ${Math.round(l * 100)}%, ${a})`
}

function rgbaCss(h: number, s: number, l: number, a: number): string {
  const { r, g, b } = hslToRgb(h, s, l)
  return `rgba(${r}, ${g}, ${b}, ${a})`
}

/**
 * 提取封面主色：
 * 1. 绘制到 24x24 小 canvas；
 * 2. 过滤近黑 / 近白 / 低饱和像素，剩余按 12 个色相桶聚合（权重=饱和度）；
 * 3. 权重最高桶取加权色相与平均饱和度；
 * 4. 没有合格彩色像素（纯灰度封面）时给一个中性蓝相，保证仍有皮肤变化。
 */
async function extractHue(dataUrl: string): Promise<CoverHue | null> {
  const cached = colorCache.get(dataUrl)
  if (cached) return cached

  try {
    const image = new Image()
    image.decoding = 'async'
    image.src = dataUrl
    await image.decode()
    if (!image.naturalWidth || !image.naturalHeight) return null

    const size = 24
    const canvas = document.createElement('canvas')
    canvas.width = size
    canvas.height = size
    const ctx = canvas.getContext('2d', { willReadFrequently: true })
    if (!ctx) return null
    ctx.drawImage(image, 0, 0, size, size)
    const { data } = ctx.getImageData(0, 0, size, size)

    interface Bucket {
      /** 色相按饱和度加权累加（主色像素更有话语权） */
      hWeighted: number
      sSum: number
      weight: number
      n: number
    }
    const buckets = new Map<number, Bucket>()
    let grayN = 0
    // 全像素相对亮度累计（含近黑/近白），衡量封面整体明暗而非仅主色
    let lumaSum = 0
    let lumaN = 0

    for (let i = 0; i < data.length; i += 4) {
      const r = data[i]
      const g = data[i + 1]
      const b = data[i + 2]
      // 相对亮度 Y = 0.2126R + 0.7152G + 0.0722B（sRGB 近似）
      lumaSum += 0.2126 * r + 0.7152 * g + 0.0722 * b
      lumaN += 1
      const [h, s, l] = rgbToHsl(r, g, b)
      if (l < 0.16 || l > 0.92 || s < 0.18) {
        grayN += 1
        continue
      }
      const key = Math.min(11, Math.floor(h * 12))
      const bucket = buckets.get(key) ?? { hWeighted: 0, sSum: 0, weight: 0, n: 0 }
      bucket.hWeighted += h * s
      bucket.sSum += s
      bucket.weight += s
      bucket.n += 1
      buckets.set(key, bucket)
    }

    let hue: CoverHue | null = null
    let best: Bucket | null = null
    for (const bucket of buckets.values()) {
      if (!best || bucket.weight > best.weight) best = bucket
    }
    if (best && best.weight > 0 && best.n > 0) {
      hue = {
        h: best.hWeighted / best.weight,
        // 平均饱和度收敛到 [0.5, 0.82]：过淡则提、过艳则压，皮肤色稳定
        s: Math.max(0.5, Math.min(0.82, best.sSum / best.n)),
      }
    } else if (grayN > 0) {
      // 纯灰度封面：中性蓝相，皮肤仍有微妙冷色调而不发灰
      hue = { h: 0.6, s: 0.32 }
    }

    if (hue) {
      // 相对亮度归一化到 0-1（RGB 255 尺度）；失败/无采样像素时给中性值 0.5
      hue.luma = lumaN > 0 ? lumaSum / lumaN / 255 : 0.5
      colorCache.set(dataUrl, hue)
    }
    return hue
  } catch {
    // decode 失败 / canvas 被污染（非 data: URL 等）：不染色
    return null
  }
}

/**
 * 按当前主题把封面色相派生为完整色阶，写入 :root 内联样式（优先级高于样式表，
 * removeProperty 后自动回落 theme.css 的主题定义）。
 */
function applyHue(hue: CoverHue, theme: 'dark' | 'light'): void {
  const root = document.documentElement
  const { h, s } = hue

  // 实色按钮基准亮度；hover 提亮、active 压暗
  const solidL = 0.5
  const solid = hslCss(h, s, solidL)
  const hover = hslCss(h, s, solidL + 0.06)
  const active = hslCss(h, s, solidL - 0.07)
  // 文字/选中态色：深主题需要更亮，浅主题需要更深，保证在各自底色上的对比度
  const brand = theme === 'dark' ? hslCss(h, Math.min(0.9, s + 0.08), 0.72) : hslCss(h, s, 0.45)
  const subtle = hslaCss(h, s, solidL, 0.14)

  // 三者按"色相 × 明度 × 彩度"多维拉开，仅靠明度微调在近色相下仍难分辨：
  //   歌词当前行 —— 去饱和的近白（深主题）/ 近墨（浅主题）+ 供辉光的强调色，
  //                 读作"被点亮的高亮文本"，与任何彩色 UI 元素一眼区分；
  //   进度条已播放 —— 实色中调（L50%，与主按钮同档），饱和实色块；
  //   皮肤色（brand）—— 亮色调（L72%/45%），用于文字与选中态。
  const lyrics =
    theme === 'dark'
      ? hslCss(h, 0.18, 0.94)
      : hslCss(h, 0.3, 0.14)
  const progress = hslCss(h, s, 0.5)

  // 封面页（沉浸式大图场景）专用歌词色：不受主题影响、始终高亮，
  // 并按封面整体明暗微调——封面越亮文字越白，配合描边保证任意封面可读
  const luma = hue.luma ?? 0.5
  const lyricsOverlay =
    luma >= 0.55
      ? hslCss(h, 0.08, 0.985)
      : luma >= 0.25
        ? hslCss(h, 0.12, 0.96)
        : hslCss(h, 0.15, 0.92)
  // 歌词描边色：封面越亮描边越浓，抵消亮部透出
  const lyricsStroke = luma >= 0.55 ? 'rgba(6, 8, 12, 0.9)' : 'rgba(8, 10, 14, 0.72)'

  root.style.setProperty('--color-brand-solid', solid)
  root.style.setProperty('--color-brand-hover', hover)
  root.style.setProperty('--color-brand-active', active)
  root.style.setProperty('--color-brand', brand)
  root.style.setProperty('--color-brand-subtle', subtle)

  // 封面氛围变量
  root.style.setProperty(ACCENT_VAR, solid)
  root.style.setProperty(GLOW_VAR, rgbaCss(h, s, solidL, 0.45))
  root.style.setProperty(SOFT_VAR, rgbaCss(h, s, solidL, 0.18))
  root.style.setProperty(WASH_VAR, rgbaCss(h, s, solidL, 0.07))
  root.style.setProperty(LYRICS_VAR, lyrics)
  root.style.setProperty(PROGRESS_VAR, progress)
  root.style.setProperty(LYRICS_OVERLAY_VAR, lyricsOverlay)
  root.style.setProperty(LYRICS_STROKE_VAR, lyricsStroke)
}

function clearAccent(): void {
  const root = document.documentElement
  for (const name of BRAND_VARS) root.style.removeProperty(name)
  root.style.removeProperty(ACCENT_VAR)
  root.style.removeProperty(GLOW_VAR)
  root.style.removeProperty(SOFT_VAR)
  root.style.removeProperty(WASH_VAR)
  root.style.removeProperty(LYRICS_VAR)
  root.style.removeProperty(PROGRESS_VAR)
  root.style.removeProperty(LYRICS_OVERLAY_VAR)
  root.style.removeProperty(LYRICS_STROKE_VAR)
}

/**
 * 在应用外壳挂载一次：跟随当前曲目封面、取色开关与主题维护全局皮肤色。
 * @param coverUrlGetter 当前曲目封面（data URL 或 null）
 */
export function useCoverTheme(coverUrlGetter: () => string | null): void {
  const { isCoverAccentEnabled } = useLayout()
  const { theme } = useTheme()

  /** 最近一次提取成功的封面色相（切主题时据此重新派生，无需重新采样） */
  let currentHue: CoverHue | null = null
  /** 当前封面 URL：主题切换时判断是否需要重新应用 */
  let currentUrl: string | null = null
  let extractToken = 0

  function reapply(): void {
    if (isCoverAccentEnabled.value && currentHue) {
      applyHue(currentHue, theme.value)
    } else {
      clearAccent()
    }
  }

  watch(
    () => isCoverAccentEnabled.value,
    () => reapply(),
  )

  // 深/浅主题切换：同色相按新主题亮度重新派生
  watch(
    () => theme.value,
    () => reapply(),
  )

  watch(
    coverUrlGetter,
    (url) => {
      currentUrl = url
      currentHue = null
      const token = ++extractToken
      if (!isCoverAccentEnabled.value || !url || url.startsWith('data:image/svg')) {
        clearAccent()
        return
      }
      void extractHue(url).then((hue) => {
        // 切歌 / 开关关闭期间的过期响应一律丢弃
        if (token !== extractToken || currentUrl !== url) return
        currentHue = hue
        reapply()
      })
    },
    { immediate: true },
  )
}
