/**
 * 实验室·均衡器（equalizer）
 *
 * 本体隔离于此文件，由 `src/lab/activate.ts` 在功能开启时挂载；关闭时完全不加载，
 * 对现有功能零影响（与实验室其它功能同一约定）。
 *
 * 后端是一组手写双二阶峰值滤波器（`src-tauri/src/player/engine.rs` 的
 * `EqProcessor`），10 段、每段 ±15 dB。这里只负责：
 *  - 维护前端滑块状态（gainDb）与预设；
 *  - 经 `player_get_eq` / `player_set_eq` 与后端同步。
 *
 * 两段式提交：拖动滑块只改本地状态并**防抖**下发（拖动过程中连续下发会让后端
 * 反复重新调谐，虽已做系数插值但仍没必要），松手 / 停止 120ms 后才真正下发。
 *
 * 与 Rust 侧 `EqBand` 的字段对应（后端 serialize 用 camelCase，见 engine.rs）：
 * `{ freq, gainDb, q }`。
 */
import { ref } from 'vue'

import { callCommand, isTauriRuntime } from '../../services/ipc'

/** 单个频段（与 Rust `EqBand` 对齐） */
export interface EqBand {
  /** 中心频率（Hz） */
  freq: number
  /** 增益（dB，-15 ~ +15） */
  gainDb: number
  /** Q 值（0.1 ~ 10） */
  q: number
}

/** 均衡器配置（与 Rust `EqConfig` 对齐） */
export interface EqConfig {
  enabled: boolean
  bands: EqBand[]
}

/** 预设名 */
export type EqPresetId = 'flat' | 'rock' | 'pop' | 'jazz' | 'classical' | 'vocal' | 'bass'

/** 10 段默认中心频率（与后端 `EqConfig::default()` 一致，勿轻易改动） */
export const EQ_FREQUENCIES = [60, 170, 310, 600, 1000, 3000, 6000, 12000, 14000, 16000]

/**
 * 预设曲线：索引与 `EQ_FREQUENCIES` 一一对应的增益（dB）。
 *
 * 数值按常见听感曲线给定，均控制在 ±15 dB 内（后端会 clamp，超范围无意义）。
 */
export const EQ_PRESETS: Record<EqPresetId, { name: string; gains: number[] }> = {
  flat: { name: '原声', gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
  rock: { name: '摇滚', gains: [5, 3, -1, -2, -1, 1, 3, 4, 4, 3] },
  pop: { name: '流行', gains: [-1, 1, 3, 4, 3, 0, -1, -1, -1, -1] },
  jazz: { name: '爵士', gains: [3, 2, 1, 2, -1, -1, 0, 1, 2, 3] },
  classical: { name: '古典', gains: [2, 1, 0, 0, 0, 0, -1, -2, -2, -3] },
  vocal: { name: '人声', gains: [-2, -1, 0, 2, 4, 4, 3, 1, 0, -1] },
  bass: { name: '重低音', gains: [8, 6, 4, 2, 0, 0, 0, 0, 0, 0] },
}

/** 默认 Q 值：峰值滤波器带宽，1.0 为适中（与后端默认一致） */
const DEFAULT_Q = 1.0

/** 防抖时长：拖动停止后再下发，避免后端被密集重调 */
const COMMIT_DEBOUNCE_MS = 120

function defaultBands(): EqBand[] {
  return EQ_FREQUENCIES.map((freq) => ({ freq, gainDb: 0, q: DEFAULT_Q }))
}

function defaultConfig(): EqConfig {
  return { enabled: false, bands: defaultBands() }
}

/* ---------- 模块级单例状态：面板与 activate 共用同一份 ---------- */

const config = ref<EqConfig>(defaultConfig())
const loaded = ref(false)
/** 当前预设名；与滑块值不一致时为 null（用户手动改过） */
const preset = ref<EqPresetId | null>('flat')
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
    config.value = { enabled: Boolean(cfg.enabled), bands }
    preset.value = matchPreset(bands.map((b) => b.gainDb))
    error.value = null
  } catch {
    // 读取失败保持本地默认，不阻塞面板渲染
  }
  loaded.value = true
}

function clampGain(db: number): number {
  if (!Number.isFinite(db)) return 0
  return Math.max(-15, Math.min(15, db))
}

/** 判断当前曲线是否命中某预设；无命中返回 null */
function matchPreset(gains: number[]): EqPresetId | null {
  for (const [id, p] of Object.entries(EQ_PRESETS) as [EqPresetId, { gains: number[] }][]) {
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

/** 设置总开关（立即下发——开关需要即时听感反馈） */
export async function setEqEnabled(enabled: boolean): Promise<void> {
  config.value.enabled = enabled
  await commit()
}

/** 应用预设（立即下发）。选任何预设即开启均衡器（含「原声」）。 */
export async function applyPreset(id: EqPresetId): Promise<void> {
  const p = EQ_PRESETS[id]
  if (!p) return
  config.value.bands = EQ_FREQUENCIES.map((freq, i) => ({
    freq,
    gainDb: clampGain(p.gains[i] ?? 0),
    q: DEFAULT_Q,
  }))
  preset.value = id
  config.value.enabled = true
  await commit()
}

/** 复位为原声（全部 0 dB）并关闭均衡器 */
export async function resetEqualizer(): Promise<void> {
  config.value.bands = defaultBands()
  preset.value = 'flat'
  config.value.enabled = false
  await commit()
}

/**
 * 挂载均衡器（由 `activate.ts` 在功能开启时调用一次）。
 *
 * 只做「读取一次后端配置」，滑块状态由面板直接读写本模块的单例 ref，
 * 故无需在此注册任何全局副作用。
 */
export function useEqualizer(): void {
  if (!loaded.value) {
    void loadEqualizer()
  }
}

export function useEqualizerState() {
  return { config, loaded, preset, error, EQ_FREQUENCIES, EQ_PRESETS }
}
