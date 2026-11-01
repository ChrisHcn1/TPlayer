/**
 * 实验室·音频可视化（前端渲染侧）
 *
 * 频谱数据由后端播放链的分析器实时计算，经 Tauri 事件 `audio-analyser` 推送
 * （后端 `AnalyserSource` 仅做 pass-through，开关由全局原子标志控制，对主程序零影响）。
 * 本模块负责：订阅事件拿到 bins、按用户选择的可视化组合用 Canvas + RAF 渲染。
 *
 * 可配置维度（均 localStorage 持久化、彼此独立可自由组合）：
 *  - vizStyle    波形形态：柱状 / 线条 / 面积填充 / 上下镜像；
 *  - vizDensity  横向采样密度（稀疏/适中/密集）；
 *  - vizGradient 颜色渐变预设（5 套）；
 *  - vizLineWidth 线条粗细（细/中/粗，作用于 line/area 描边）；
 *  - vizSpeed    动效速度（慢/中/快，控制时间平滑强度，即响应快慢）；
 *  - vizCurve    曲线形态（平滑/直线/阶梯，作用于 line/area 路径）；
 *  - vizDock     停靠位置（侧栏底部 / 顶部工具条 / 自由浮动）。
 *
 * 隔离性：全部封在本文件 + `AudioVisualizer.vue`；仅 `enabled` 时挂载画布、订阅事件；
 * 关闭即退订并停止 RAF，无残留监听。与主程序配置完全解耦。
 */
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useLabStore } from '../../stores/lab'

/** 波形形态 */
export type VizStyle = 'bars' | 'line' | 'area' | 'mirror'
/** 横向采样密度 */
export type VizDensity = 'sparse' | 'medium' | 'dense'
/** 颜色渐变预设 */
export type VizGradient = 'aurora' | 'sunset' | 'mono' | 'neon' | 'ice'
/** 线条粗细 */
export type VizLineWidth = 'thin' | 'normal' | 'thick'
/** 动效速度（时间平滑强度） */
export type VizSpeed = 'slow' | 'normal' | 'fast'
/** 曲线形态（line / area 路径） */
export type VizCurve = 'smooth' | 'straight' | 'stepped'
/** 停靠位置 */
export type VizDock = 'sidebar' | 'toolbar' | 'floating'

/** 密度 → 采样柱数 */
const DENSITY_COUNT: Record<VizDensity, number> = {
  sparse: 24,
  medium: 48,
  dense: 96,
}

/** 线条粗细（CSS 像素，绘制时再乘 dpr） */
const LINE_WIDTH: Record<VizLineWidth, number> = {
  thin: 1.5,
  normal: 2.5,
  thick: 4,
}

/**
 * 动效速度 → 时间平滑 alpha：prev = prev*α + cur*(1-α)。
 * α 越大越「慢/平滑/迟滞」，越小越「快/灵敏/抖动」，对应直观的动效速度档。
 */
const SMOOTH_ALPHA: Record<VizSpeed, number> = {
  slow: 0.82,
  normal: 0.6,
  fast: 0.35,
}

/** 渐变预设：line 为横向（沿宽度）多色渐变；area 为竖向（沿高度）透明渐变 */
interface GradientDef {
  line: string[]
  area: string[]
}
const GRADIENTS: Record<VizGradient, GradientDef> = {
  aurora: {
    line: ['#5b8cff', '#22d3ee', '#a78bfa'],
    area: ['rgba(91,140,255,0.55)', 'rgba(34,211,238,0.22)', 'rgba(167,139,250,0.02)'],
  },
  sunset: {
    line: ['#ffb24d', '#ff5e7e', '#b14bff'],
    area: ['rgba(255,94,126,0.55)', 'rgba(255,178,77,0.22)', 'rgba(177,75,255,0.02)'],
  },
  mono: {
    line: ['#22d3ee', '#22d3ee'],
    area: ['rgba(34,211,238,0.55)', 'rgba(34,211,238,0.18)', 'rgba(34,211,238,0.02)'],
  },
  neon: {
    line: ['#00ffa3', '#39ff14', '#22d3ee'],
    area: ['rgba(0,255,163,0.5)', 'rgba(57,255,20,0.2)', 'rgba(34,211,238,0.02)'],
  },
  ice: {
    line: ['#bfe3ff', '#ffffff', '#7aa2ff'],
    area: ['rgba(191,227,255,0.55)', 'rgba(255,255,255,0.2)', 'rgba(122,162,255,0.02)'],
  },
}

const STORAGE_KEY = 'tplayer.lab.audioVisualizer'

/** 波形形态（持久化） */
export const vizStyle = ref<VizStyle>('bars')
/** 显示密度（持久化） */
export const vizDensity = ref<VizDensity>('medium')
/** 颜色渐变（持久化） */
export const vizGradient = ref<VizGradient>('aurora')
/** 线条粗细（持久化） */
export const vizLineWidth = ref<VizLineWidth>('normal')
/** 动效速度（持久化） */
export const vizSpeed = ref<VizSpeed>('normal')
/** 曲线形态（持久化） */
export const vizCurve = ref<VizCurve>('smooth')
/** 停靠位置（持久化） */
export const vizDock = ref<VizDock>('sidebar')
/** 自由浮动坐标（拖动产生；null 表示使用默认右下角位置） */
export const vizPos = ref<{ x: number; y: number } | null>(null)

interface Persisted {
  style?: VizStyle
  density?: VizDensity
  gradient?: VizGradient
  lineWidth?: VizLineWidth
  speed?: VizSpeed
  curve?: VizCurve
  dock?: VizDock
  pos?: { x: number; y: number } | null
}

function loadSettings(): void {
  try {
    if (typeof localStorage === 'undefined') return
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return
    const s = JSON.parse(raw) as Persisted
    if (s.style) vizStyle.value = s.style
    if (s.density) vizDensity.value = s.density
    if (s.gradient) vizGradient.value = s.gradient
    if (s.lineWidth) vizLineWidth.value = s.lineWidth
    if (s.speed) vizSpeed.value = s.speed
    if (s.curve) vizCurve.value = s.curve
    if (s.dock) vizDock.value = s.dock
    if (s.pos !== undefined) vizPos.value = s.pos
  } catch {
    /* 读取/解析失败：回退默认值 */
  }
}

function saveSettings(): void {
  try {
    if (typeof localStorage === 'undefined') return
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        style: vizStyle.value,
        density: vizDensity.value,
        gradient: vizGradient.value,
        lineWidth: vizLineWidth.value,
        speed: vizSpeed.value,
        curve: vizCurve.value,
        dock: vizDock.value,
        pos: vizPos.value,
      }),
    )
  } catch {
    /* 存储不可用则静默忽略 */
  }
}

/** 配置高频变化，做 200ms 去抖再落盘，避免每帧写 localStorage */
let saveTimer: ReturnType<typeof setTimeout> | null = null
function scheduleSave(): void {
  if (saveTimer !== null) clearTimeout(saveTimer)
  saveTimer = setTimeout(saveSettings, 200)
}

loadSettings()
watch(
  [vizStyle, vizDensity, vizGradient, vizLineWidth, vizSpeed, vizCurve, vizDock, vizPos],
  scheduleSave,
  { deep: true },
)

/* ---------------- 风格预设 ----------------
 * 面板只暴露「预设 + 位置」两层；预设把 6 个维度一次性套成一组协调的组合，
 * 免去逐项挑选。需要微调时在「高级设置」里改单项，预设标记自动转为「自定义」。
 */
export interface VizPresetConfig {
  style: VizStyle
  density: VizDensity
  gradient: VizGradient
  lineWidth: VizLineWidth
  speed: VizSpeed
  curve: VizCurve
}

/** 渲染配置（6 个可组合维度）；预设与主视图共用同一形状 */
export type VizConfig = VizPresetConfig

export interface VizPreset {
  id: string
  /** i18n 键（名称） */
  key: string
  config: VizPresetConfig
}

export const VIZ_PRESETS: VizPreset[] = [
  {
    id: 'rhythm',
    key: 'lab.audioVisualizer.presetRhythm',
    config: {
      style: 'bars',
      density: 'medium',
      gradient: 'aurora',
      lineWidth: 'normal',
      speed: 'normal',
      curve: 'smooth',
    },
  },
  {
    id: 'minimal',
    key: 'lab.audioVisualizer.presetMinimal',
    config: {
      style: 'line',
      density: 'medium',
      gradient: 'mono',
      lineWidth: 'thin',
      speed: 'normal',
      curve: 'smooth',
    },
  },
  {
    id: 'neon',
    key: 'lab.audioVisualizer.presetNeon',
    config: {
      style: 'line',
      density: 'dense',
      gradient: 'neon',
      lineWidth: 'thick',
      speed: 'fast',
      curve: 'smooth',
    },
  },
  {
    id: 'ambient',
    key: 'lab.audioVisualizer.presetAmbient',
    config: {
      style: 'area',
      density: 'medium',
      gradient: 'sunset',
      lineWidth: 'normal',
      speed: 'slow',
      curve: 'smooth',
    },
  },
  {
    id: 'glacier',
    key: 'lab.audioVisualizer.presetGlacier',
    config: {
      style: 'area',
      density: 'medium',
      gradient: 'ice',
      lineWidth: 'thick',
      speed: 'normal',
      curve: 'stepped',
    },
  },
]

/** 一键套用预设（6 个维度同时写入，下一帧即生效） */
export function applyVizPreset(id: string): void {
  const preset = VIZ_PRESETS.find((p) => p.id === id)
  if (!preset) return
  vizStyle.value = preset.config.style
  vizDensity.value = preset.config.density
  vizGradient.value = preset.config.gradient
  vizLineWidth.value = preset.config.lineWidth
  vizSpeed.value = preset.config.speed
  vizCurve.value = preset.config.curve
}

/** 当前组合与哪个预设完全一致；不一致返回 null（即「自定义」） */
export function matchVizPreset(): string | null {
  const cur: VizPresetConfig = {
    style: vizStyle.value,
    density: vizDensity.value,
    gradient: vizGradient.value,
    lineWidth: vizLineWidth.value,
    speed: vizSpeed.value,
    curve: vizCurve.value,
  }
  const keys = Object.keys(cur) as (keyof VizPresetConfig)[]
  const hit = VIZ_PRESETS.find((p) => keys.every((k) => p.config[k] === cur[k]))
  return hit ? hit.id : null
}

/** 最新一帧频谱 bins（0..255），由 audio-analyser 事件更新 */
const bins = ref<number[]>([])
let unlisten: UnlistenFn | null = null
let rafId: number | null = null
/** 时间维度平滑缓存：按 canvas 分别维护，避免主视图与多个缩略图因密度不同而互相重置 */
const prevBarsByCanvas = new Map<HTMLCanvasElement, number[]>()

/** 把后端 512 段原始 bins 重采样为 count 段（分桶取均值） */
function resample(data: number[], count: number): number[] {
  const n = data.length
  const out = new Array<number>(count).fill(0)
  if (n === 0) return out
  for (let i = 0; i < count; i++) {
    const start = Math.floor((i * n) / count)
    const end = Math.max(start + 1, Math.floor(((i + 1) * n) / count))
    let sum = 0
    let c = 0
    for (let j = start; j < end && j < n; j++) {
      sum += data[j]
      c++
    }
    out[i] = c ? sum / c : 0
  }
  return out
}

/** 指数平滑：压住频谱抖动，让柱/线更顺滑；平滑强度由动效速度档决定（按 canvas 分别缓存） */
function smooth(out: number[], canvas: HTMLCanvasElement, alpha: number): number[] {
  let prev = prevBarsByCanvas.get(canvas)
  if (!prev || prev.length !== out.length) {
    prev = out.slice()
    prevBarsByCanvas.set(canvas, prev)
    return prev
  }
  for (let i = 0; i < out.length; i++) {
    prev[i] = prev[i] * alpha + out[i] * (1 - alpha)
  }
  return prev
}

/** 横向渐变（沿宽度，作用于柱填充 / 线描边） */
function lineGradient(ctx: CanvasRenderingContext2D, w: number, g: GradientDef): CanvasGradient {
  const lg = ctx.createLinearGradient(0, 0, w, 0)
  g.line.forEach((c, i) => lg.addColorStop(i / (g.line.length - 1), c))
  return lg
}

/** 竖向渐变（沿高度，作用于面积填充，顶部不透明 → 底部透明） */
function areaGradient(ctx: CanvasRenderingContext2D, h: number, g: GradientDef): CanvasGradient {
  const ag = ctx.createLinearGradient(0, 0, 0, h)
  g.area.forEach((c, i) => ag.addColorStop(i / (g.area.length - 1), c))
  return ag
}

/** 圆角矩形（兼容无 roundRect 的旧 WebView2） */
function roundRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
): void {
  const rad = Math.max(0, Math.min(r, w / 2, h / 2))
  ctx.beginPath()
  ctx.moveTo(x + rad, y)
  ctx.arcTo(x + w, y, x + w, y + h, rad)
  ctx.arcTo(x + w, y + h, x, y + h, rad)
  ctx.arcTo(x, y + h, x, y, rad)
  ctx.arcTo(x, y, x + w, y, rad)
  ctx.closePath()
}

/** 按曲线形态生成 line/area 的折线路径（已 beginPath，调用方负责描边/填充收尾） */
function tracePath(ctx: CanvasRenderingContext2D, pts: { x: number; y: number }[], curve: VizCurve): void {
  ctx.beginPath()
  ctx.moveTo(pts[0].x, pts[0].y)
  if (curve === 'straight') {
    for (let i = 1; i < pts.length; i++) ctx.lineTo(pts[i].x, pts[i].y)
    return
  }
  if (curve === 'stepped') {
    for (let i = 1; i < pts.length; i++) {
      ctx.lineTo(pts[i].x, pts[i - 1].y)
      ctx.lineTo(pts[i].x, pts[i].y)
    }
    return
  }
  // smooth：相邻点中点作控制点的二次贝塞尔，曲线更柔和
  for (let i = 1; i < pts.length; i++) {
    const xc = (pts[i - 1].x + pts[i].x) / 2
    const yc = (pts[i - 1].y + pts[i].y) / 2
    ctx.quadraticCurveTo(pts[i - 1].x, pts[i - 1].y, xc, yc)
  }
  const last = pts[pts.length - 1]
  ctx.quadraticCurveTo(last.x, last.y, last.x, last.y)
}

/**
 * 按给定配置渲染当前 bins 到指定 canvas。
 *
 * 坐标系为 canvas 的**设备像素**（backing store 由调用方按 CSS 尺寸 × dpr 设置），
 * 用 `ratio` 反推 CSS 像素比，保证线宽/圆角/留白在不同 dpr 下观感一致。
 * 无数据时画一条柔和基线，保持观感而非空白。
 */
export function drawWith(canvas: HTMLCanvasElement | null, config: VizConfig): void {
  if (!canvas) return
  const ctx = canvas.getContext('2d')
  if (!ctx) return
  const w = canvas.width
  const h = canvas.height
  if (w <= 1 || h <= 1) return
  ctx.clearRect(0, 0, w, h)

  const ratio = canvas.clientWidth > 0 ? Math.max(1, w / canvas.clientWidth) : 1
  const raw = bins.value
  const count = DENSITY_COUNT[config.density]
  const grad = GRADIENTS[config.gradient]
  const alpha = SMOOTH_ALPHA[config.speed]

  // 无数据（未播放 / 已停止）：画一条柔和基线，保持观感而非空白
  if (!raw.length) {
    ctx.strokeStyle = 'rgba(148, 163, 184, 0.35)'
    ctx.lineWidth = LINE_WIDTH[config.lineWidth] * ratio
    ctx.beginPath()
    ctx.moveTo(0, h / 2)
    ctx.lineTo(w, h / 2)
    ctx.stroke()
    return
  }

  const bars = smooth(resample(raw, count), canvas, alpha)
  const style = config.style

  // 柱状 / 镜像：逐柱绘制（横向渐变填充）
  if (style === 'bars' || style === 'mirror') {
    const slot = w / count
    const gap = Math.min(slot * 0.32, 3 * ratio)
    const bw = Math.max(1, slot - gap)
    const radius = Math.min(bw / 2, 3 * ratio)
    ctx.fillStyle = lineGradient(ctx, w, grad)
    for (let i = 0; i < count; i++) {
      const x = i * slot + gap / 2
      if (style === 'bars') {
        const bh = Math.max(1, (bars[i] / 255) * (h - 2 * ratio))
        roundRect(ctx, x, h - bh, bw, bh, radius)
      } else {
        const mid = h / 2
        const bh = Math.max(1, (bars[i] / 255) * (mid - ratio))
        roundRect(ctx, x, mid - bh, bw, bh * 2, radius)
      }
      ctx.fill()
    }
    return
  }

  // 线条 / 面积：按密度采样点，再按曲线形态生成路径
  const pad = 2 * ratio
  const pts: { x: number; y: number }[] = []
  for (let i = 0; i < count; i++) {
    const x = (i + 0.5) * (w / count)
    const y = h - (bars[i] / 255) * (h - pad * 2) - pad
    pts.push({ x, y })
  }
  const curve = config.curve

  if (style === 'area') {
    tracePath(ctx, pts, curve)
    ctx.lineTo(w, h)
    ctx.lineTo(0, h)
    ctx.closePath()
    ctx.fillStyle = areaGradient(ctx, h, grad)
    ctx.fill()
  }

  // 顶部描边（line 与 area 共用同一根曲线轮廓）
  tracePath(ctx, pts, curve)
  ctx.lineWidth = LINE_WIDTH[config.lineWidth] * ratio
  ctx.lineJoin = 'round'
  ctx.lineCap = 'round'
  ctx.strokeStyle = lineGradient(ctx, w, grad)
  ctx.shadowColor = 'rgba(99, 102, 241, 0.5)'
  ctx.shadowBlur = 6 * ratio
  ctx.stroke()
  ctx.shadowBlur = 0
}

/** 主视图绘制：用当前全局配置渲染 */
export function draw(canvas: HTMLCanvasElement | null): void {
  drawWith(canvas, {
    style: vizStyle.value,
    density: vizDensity.value,
    gradient: vizGradient.value,
    lineWidth: vizLineWidth.value,
    speed: vizSpeed.value,
    curve: vizCurve.value,
  })
}

/**
 * 缩略图渲染循环：以指定配置持续渲染当前音频到小 canvas。
 * 与主视图共用模块级 bins（由主视图的 audio-analyser 订阅驱动），因此无需各自订阅事件；
 * 返回停止函数。
 */
export function startThumbLoop(canvas: HTMLCanvasElement | null, config: VizConfig): () => void {
  let stopped = false
  let id = 0
  const loop = () => {
    if (stopped) return
    drawWith(canvas, config)
    id = requestAnimationFrame(loop)
  }
  id = requestAnimationFrame(loop)
  return () => {
    stopped = true
    cancelAnimationFrame(id)
  }
}

export function useAudioVisualizer() {
  const lab = useLabStore()

  async function ensureListen(): Promise<void> {
    if (unlisten) return
    unlisten = await listen<number[]>('audio-analyser', (e) => {
      bins.value = e.payload
    })
  }

  function stopListen(): void {
    unlisten?.()
    unlisten = null
    bins.value = []
    prevBarsByCanvas.clear()
  }

  /** 启动渲染循环（绑定到指定 canvas）；返回停止函数 */
  function startLoop(canvas: HTMLCanvasElement | null): () => void {
    let stopped = false
    const loop = () => {
      if (stopped) return
      draw(canvas)
      rafId = requestAnimationFrame(loop)
    }
    if (canvas) void ensureListen().then(() => loop())
    return () => {
      stopped = true
      if (rafId !== null) {
        cancelAnimationFrame(rafId)
        rafId = null
      }
    }
  }

  onMounted(() => {
    if (lab.isEnabled('audio-visualizer')) void ensureListen()
  })
  onBeforeUnmount(() => {
    stopListen()
    if (rafId !== null) {
      cancelAnimationFrame(rafId)
      rafId = null
    }
  })

  return { bins, draw, startLoop }
}
