/**
 * 均衡器（Equalizer）—— 正式版特性
 *
 * 已从「实验室」毕业：代码迁至主项目（src/features），不再受 lab 开关门控，
 * 由播放器主界面（PlayerBar → EqPanel 抽屉）直接暴露。
 *
 * 后端是一组手写双二阶峰值滤波器（`src-tauri/src/player/engine.rs` 的
 * `EqProcessor`），10 段、每段 ±15 dB。这里只负责：
 *  - 维护前端滑块状态（gainDb / q / 输出增益）与预设场景；
 *  - 经 `player_get_eq` / `player_set_eq` 与后端同步；
 *  - 实时计算合成幅频响应曲线，供 UI 绘制（纯前端，无需后端额外数据）。
 *
 * 两段式提交：拖动滑块只改本地状态并**防抖**下发（拖动过程中连续下发会让后端
 * 反复重新调谐，虽已做系数插值但仍没必要），松手 / 停止 120ms 后才真正下发。
 *
 * 与 Rust 侧 `EqBand` / `EqConfig` 字段对应（后端 serialize 用 camelCase）：
 * `{ freq, gainDb, q }` 与 `{ enabled, bands, outputGainDb }`。
 */
import { ref } from 'vue'

import { callCommand, isTauriRuntime } from '../services/ipc'

/** 单个频段（与 Rust `EqBand` 对齐） */
export interface EqBand {
  /** 中心频率（Hz） */
  freq: number
  /** 增益（dB，-15 ~ +15） */
  gainDb: number
  /** Q 值（带宽，0.1 ~ 10） */
  q: number
}

/** 均衡器配置（与 Rust `EqConfig` 对齐） */
export interface EqConfig {
  enabled: boolean
  bands: EqBand[]
  /** 输出增益（dB，-12 ~ +12）：提升多段时为防削波可由用户压低，默认 0 */
  outputGainDb: number
}

/** 预设场景 */
export interface EqPreset {
  /** 显示名对应的 i18n 键（如 player.eqPreset.flat） */
  nameKey: string
  /** 与 EQ_FREQUENCIES 一一对应的增益（dB） */
  gains: number[]
  /** Q 值：单值作用于全部频段，或按频段数组；缺省 1.0 */
  q?: number | number[]
}

/** 10 段默认中心频率（与后端 `EqConfig::default()` 一致，勿轻易改动） */
export const EQ_FREQUENCIES = [60, 170, 310, 600, 1000, 3000, 6000, 12000, 14000, 16000]

/**
 * 预设场景曲线。索引与 `EQ_FREQUENCIES` 一一对应。
 *
 * 覆盖不同使用需求：原声 / 摇滚 / 流行 / 爵士 / 古典 / 人声 / 重低音 / 电子 /
 * 金属 / 钢琴 / 原声乐器 / 高音增强 / 语音播客 / 游戏 / 影院 / 轻音乐 / 车载。
 * 数值控制在 ±8 dB 内（个别重低音 8dB），避免过度提升导致削波；
 * 部分场景用差异化的 Q 增强音色特征（默认 1.0）。
 */
export const EQ_PRESETS: Record<string, EqPreset> = {
  flat: { nameKey: 'player.eqPreset.flat', gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
  rock: { nameKey: 'player.eqPreset.rock', gains: [5, 3, -1, -2, -1, 1, 3, 4, 4, 3] },
  pop: { nameKey: 'player.eqPreset.pop', gains: [-1, 1, 3, 4, 3, 0, -1, -1, -1, -1] },
  jazz: { nameKey: 'player.eqPreset.jazz', gains: [3, 2, 1, 2, -1, -1, 0, 1, 2, 3] },
  classical: { nameKey: 'player.eqPreset.classical', gains: [2, 1, 0, 0, 0, 0, -1, -2, -2, -3] },
  vocal: { nameKey: 'player.eqPreset.vocal', gains: [-2, -1, 0, 2, 4, 4, 3, 1, 0, -1] },
  bass: { nameKey: 'player.eqPreset.bass', gains: [8, 6, 4, 2, 0, 0, 0, 0, 0, 0], q: 1.2 },
  electronic: { nameKey: 'player.eqPreset.electronic', gains: [4, 2, 0, -2, 1, 2, 2, 3, 4, 4] },
  metal: { nameKey: 'player.eqPreset.metal', gains: [6, 4, 2, 1, -1, 0, 2, 3, 3, 3], q: 0.9 },
  piano: { nameKey: 'player.eqPreset.piano', gains: [-1, 0, 1, 2, 2, -1, -2, -3, -3, -3] },
  acoustic: {
    nameKey: 'player.eqPreset.acoustic',
    gains: [-2, 0, 1, 2, 1, 0, -1, -2, -2, -1],
    q: 1.1,
  },
  treble: { nameKey: 'player.eqPreset.treble', gains: [-3, -2, -1, 0, 1, 3, 5, 6, 6, 6] },
  speech: { nameKey: 'player.eqPreset.speech', gains: [-4, -2, 0, 3, 5, 5, 4, 2, 0, -2], q: 1.3 },
  game: { nameKey: 'player.eqPreset.game', gains: [3, 2, 0, 0, 0, 1, 2, 3, 3, 2] },
  movie: { nameKey: 'player.eqPreset.movie', gains: [4, 3, 1, 0, 0, 0, 1, 2, 4, 5], q: 1.1 },
  relax: { nameKey: 'player.eqPreset.relax', gains: [2, 1, 0, 0, -1, -1, -2, -2, -1, 0] },
  car: { nameKey: 'player.eqPreset.car', gains: [6, 4, 2, 0, 0, 0, 1, 2, 3, 3], q: 1.2 },
}

/** 默认 Q 值：峰值滤波器带宽，1.0 为适中 */
const DEFAULT_Q = 1.0
/** 输出增益范围 */
const OUTPUT_GAIN_MIN = -12
const OUTPUT_GAIN_MAX = 12
/** 防抖时长：拖动停止后再下发，避免后端被密集重调 */
const COMMIT_DEBOUNCE_MS = 120
/** 预设切换动画时长（ms） */
const PRESET_ANIM_MS = 220

function defaultBands(): EqBand[] {
  return EQ_FREQUENCIES.map((freq) => ({ freq, gainDb: 0, q: DEFAULT_Q }))
}

function defaultConfig(): EqConfig {
  return { enabled: false, outputGainDb: 0, bands: defaultBands() }
}

function clampGain(db: number): number {
  if (!Number.isFinite(db)) return 0
  return Math.max(-15, Math.min(15, db))
}

/** 解析预设的 Q 为「单值或按频段数组」 */
function resolveQ(p: EqPreset): number | number[] {
  return p.q ?? DEFAULT_Q
}

/** 取第 i 段的 Q（数组按索引，单值统一） */
function qFor(q: number | number[], i: number): number {
  if (Array.isArray(q)) return q[i] ?? DEFAULT_Q
  return q
}

/* ---------- 模块级单例状态：面板与 activate 共用同一份 ---------- */

const config = ref<EqConfig>(defaultConfig())
const loaded = ref(false)
/** 当前预设名；与滑块值不一致时为 null（用户手动改过） */
const preset = ref<string | null>('flat')
const error = ref<string | null>(null)

let commitTimer: ReturnType<typeof setTimeout> | null = null

/** 读取后端已保存的 EQ 配置（浏览器预览回落默认值） */
export async function loadEqualizer(): Promise<void> {
  if (!isTauriRuntime()) {
    loaded.value = true
    return
  }
  try {
    const cfg = await callCommand<EqConfig>('player_get_eq')
    // 后端频段数可能因版本变化，以本地频率表为准对齐，避免长度不一致
    const bands = EQ_FREQUENCIES.map((freq, i) => {
      const src = cfg.bands?.[i]
      return {
        freq,
        gainDb: typeof src?.gainDb === 'number' ? clampGain(src.gainDb) : 0,
        q: typeof src?.q === 'number' ? src.q : DEFAULT_Q,
      }
    })
    config.value = {
      enabled: Boolean(cfg.enabled),
      outputGainDb:
        typeof cfg.outputGainDb === 'number'
          ? Math.max(OUTPUT_GAIN_MIN, Math.min(OUTPUT_GAIN_MAX, cfg.outputGainDb))
          : 0,
      bands,
    }
    preset.value = matchPreset(bands.map((b) => b.gainDb))
    error.value = null
  } catch {
    // 读取失败保持本地默认，不阻塞面板渲染
  }
  loaded.value = true
}

/** 判断当前曲线是否命中某预设；无命中返回 null */
function matchPreset(gains: number[]): string | null {
  for (const [id, p] of Object.entries(EQ_PRESETS)) {
    if (p.gains.length === gains.length && p.gains.every((g, i) => Math.abs(g - gains[i]) < 0.01)) {
      return id
    }
  }
  return null
}

/** 下发当前配置到后端（立即，清除防抖） */
async function commit(): Promise<void> {
  if (commitTimer) {
    clearTimeout(commitTimer)
    commitTimer = null
  }
  if (!isTauriRuntime()) return
  try {
    await callCommand('player_set_eq', {
      enabled: config.value.enabled,
      bands: config.value.bands.map((b) => ({
        freq: b.freq,
        gainDb: clampGain(b.gainDb),
        q: b.q,
      })),
      outputGainDb: config.value.outputGainDb,
    })
    error.value = null
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : String(cause)
  }
}

/** 防抖下发：拖动滑块时用 */
function commitDebounced(): void {
  if (commitTimer) clearTimeout(commitTimer)
  commitTimer = setTimeout(() => {
    commitTimer = null
    void commit()
  }, COMMIT_DEBOUNCE_MS)
}

/** 设置某段增益（dB），触发防抖下发。拖动任意滑块即视为开启均衡器。 */
export function setBandGain(index: number, gainDb: number): void {
  const band = config.value.bands[index]
  if (!band) return
  band.gainDb = clampGain(gainDb)
  preset.value = matchPreset(config.value.bands.map((b) => b.gainDb))
  config.value.enabled = true
  commitDebounced()
}

/** 设置某段 Q 值，触发防抖下发 */
export function setBandQ(index: number, q: number): void {
  const band = config.value.bands[index]
  if (!band) return
  band.q = Math.max(0.1, Math.min(10, q))
  commitDebounced()
}

/** 设置输出增益（dB），触发防抖下发（用于防削波 / 整体提亮） */
export function setOutputGain(db: number): void {
  config.value.outputGainDb = Math.max(OUTPUT_GAIN_MIN, Math.min(OUTPUT_GAIN_MAX, db))
  commitDebounced()
}

/** 设置总开关（立即下发——开关需要即时听感反馈） */
export async function setEqEnabled(enabled: boolean): Promise<void> {
  config.value.enabled = enabled
  await commit()
}

/** 应用预设（带滑块平滑过渡 + 立即下发）。选任何预设即开启均衡器（含「原声」）。 */
export async function applyPreset(id: string): Promise<void> {
  const p = EQ_PRESETS[id]
  if (!p) return
  const start = config.value.bands.map((b) => b.gainDb)
  const end = EQ_FREQUENCIES.map((_, i) => clampGain(p.gains[i] ?? 0))
  const q = resolveQ(p)
  preset.value = id
  config.value.enabled = true
  await tweenGains(start, end, q)
  await commit()
}

/** 滑块平滑过渡：从 start 渐变到 end，期间逐帧更新本地状态（曲线与滑块同步动画） */
function tweenGains(start: number[], end: number[], q: number | number[]): Promise<void> {
  return new Promise((resolve) => {
    const t0 = performance.now()
    const step = () => {
      const t = Math.min(1, (performance.now() - t0) / PRESET_ANIM_MS)
      const e = t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) * (-2 * t + 2) / 2 // easeInOutQuad
      config.value.bands = EQ_FREQUENCIES.map((freq, i) => ({
        freq,
        gainDb: clampGain(start[i] + (end[i] - start[i]) * e),
        q: qFor(q, i),
      }))
      if (t < 1) requestAnimationFrame(step)
      else resolve()
    }
    requestAnimationFrame(step)
  })
}

/** 复位为原声（全部 0 dB）并关闭均衡器 */
export async function resetEqualizer(): Promise<void> {
  config.value.bands = defaultBands()
  preset.value = 'flat'
  config.value.enabled = false
  await commit()
}

/**
 * 挂载均衡器（App 启动 / 抽屉首次打开时调用一次）。
 * 只做「读取一次后端配置」，滑块状态由面板直接读写本模块的单例 ref。
 */
export function useEqualizer(): void {
  if (!loaded.value) {
    void loadEqualizer()
  }
}

export function useEqualizerState() {
  return {
    config,
    loaded,
    preset,
    error,
    EQ_FREQUENCIES,
    EQ_PRESETS,
    OUTPUT_GAIN_MIN,
    OUTPUT_GAIN_MAX,
  }
}

/**
 * 计算合成幅频响应曲线（dB），供 UI 绘制响应图。纯前端，与后端 biquad 公式一致。
 * 在 20Hz~20kHz 对数取点，逐频段相乘得到复合响应，叠加输出增益。
 *
 * @param bands 当前频段（含 gainDb / q）
 * @param sampleRate 采样率（默认 44100）
 * @param points 采样点数（默认 160）
 * @returns { freqs, db } 对数频率数组与对应的 dB 值
 */
export function computeResponseCurve(
  bands: EqBand[],
  sampleRate = 44100,
  points = 160,
): { freqs: number[]; db: number[] } {
  const freqs: number[] = []
  const db: number[] = []
  for (let i = 0; i <= points; i++) {
    const freq = 20 * Math.pow(1000, i / points) // 20Hz .. 20kHz 对数
    let mag = 1.0
    for (const band of bands) {
      mag *= biquadMagnitude(sampleRate, band.gainDb, band.q, freq)
    }
    freqs.push(freq)
    db.push(20 * Math.log10(mag) + (config.value?.outputGainDb ?? 0))
  }
  return { freqs, db }
}

/** 峰值滤波器在 freq(Hz) 处的稳态幅值（线性），与后端 peaking_coeffs 一致 */
function biquadMagnitude(
  sampleRate: number,
  gainDb: number,
  q: number,
  freq: number,
): number {
  const w = (2 * Math.PI * freq) / sampleRate
  const a = Math.pow(10, gainDb / 40)
  const alpha = Math.sin(w) / (2 * Math.max(0.1, Math.min(10, q)))
  const cosw = Math.cos(w)
  const sinw = Math.sin(w)
  const b0 = 1 + alpha * a
  const b1 = -2 * cosw
  const b2 = 1 - alpha * a
  const a0 = 1 + alpha / a
  const a1 = -2 * cosw
  const a2 = 1 - alpha / a
  const c2 = 2 * cosw * cosw - 1 // cos(2w)
  const s2 = 2 * sinw * cosw // sin(2w)
  const numRe = b0 + b1 * cosw + b2 * c2
  const numIm = -(b1 * sinw + b2 * s2)
  const denRe = a0 + a1 * cosw + a2 * c2
  const denIm = -(a1 * sinw + a2 * s2)
  const num2 = numRe * numRe + numIm * numIm
  const den2 = denRe * denRe + denIm * denIm
  return Math.sqrt(num2 / den2)
}
