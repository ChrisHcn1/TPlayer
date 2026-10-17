/**
 * 远程开关 / 灰度 / 一键回滚配置。
 *
 * 优先级（从高到低）：
 *  1. 本地覆盖（localStorage `compat-config-override`）—— 用于调试与紧急一键回滚；
 *  2. 远程配置（若设置了 `VITE_COMPAT_CONFIG_URL` 或 localStorage `compat-config-url`）；
 *  3. 内置默认配置（DEFAULT_COMPAT_CONFIG）。
 *
 * 远程拉取结果会缓存到 localStorage（带 TTL），拉取失败不影响主播放器。
 */
import { DEFAULT_COMPAT_CONFIG, type CompatConfig } from './types'

const LOCAL_OVERRIDE_KEY = 'tplayer-next.compat.config.override'
const LOCAL_URL_KEY = 'tplayer-next.compat.config.url'
const LOCAL_CACHE_KEY = 'tplayer-next.compat.config.cache'
const TTL_MS = 10 * 60 * 1000

function remoteUrl(): string | null {
  const fromEnv = (import.meta as any).env?.VITE_COMPAT_CONFIG_URL as string | undefined
  if (fromEnv) return fromEnv
  try {
    const custom = localStorage.getItem(LOCAL_URL_KEY)
    return custom || null
  } catch {
    return null
  }
}

function readJson<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key)
    return raw ? (JSON.parse(raw) as T) : null
  } catch {
    return null
  }
}

function writeJson(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch {
    /* 忽略 */
  }
}

/** 合并一层字段，缺失则用默认 */
function mergeConfig(partial: Partial<CompatConfig> | null): CompatConfig {
  const d = DEFAULT_COMPAT_CONFIG
  if (!partial) return { ...d }
  return {
    enabled: partial.enabled ?? d.enabled,
    grayRatio: typeof partial.grayRatio === 'number' ? partial.grayRatio : d.grayRatio,
    ffmpegWasmEnabled: partial.ffmpegWasmEnabled ?? d.ffmpegWasmEnabled,
    killSwitch: partial.killSwitch ?? d.killSwitch,
    reportEndpoint: partial.reportEndpoint ?? d.reportEndpoint,
    version: partial.version ?? d.version,
  }
}

/** 拉取并解析最终生效配置（绝不在本函数内抛出） */
export async function loadCompatConfig(): Promise<CompatConfig> {
  const override = readJson<Partial<CompatConfig>>(LOCAL_OVERRIDE_KEY)
  if (override) {
    // 一键回滚：override 中 killSwitch=true 立即整体禁用
    return mergeConfig(override)
  }

  const url = remoteUrl()
  if (url) {
    try {
      const ctrl = new AbortController()
      const timer = setTimeout(() => ctrl.abort(), 4000)
      const res = await fetch(url, { signal: ctrl.signal })
      clearTimeout(timer)
      if (res.ok) {
        const json = (await res.json()) as Partial<CompatConfig>
        const merged = mergeConfig(json)
        writeJson(LOCAL_CACHE_KEY, { ts: Date.now(), config: merged })
        return merged
      }
    } catch {
      /* 远程不可达：退回缓存或默认 */
    }
  }

  const cached = readJson<{ ts: number; config: Partial<CompatConfig> }>(LOCAL_CACHE_KEY)
  if (cached && Date.now() - cached.ts < TTL_MS) return mergeConfig(cached.config)

  return { ...DEFAULT_COMPAT_CONFIG }
}

/** 是否允许对某曲目启用兜底（综合总开关 / 回滚 / 灰度分流） */
export function isFallbackAllowed(trackId: string, config: CompatConfig): boolean {
  if (!config.enabled || config.killSwitch) return false
  if (config.grayRatio >= 1) return true
  if (config.grayRatio <= 0) return false
  // 稳定哈希分流
  let h = 2166136261
  for (let i = 0; i < trackId.length; i++) {
    h ^= trackId.charCodeAt(i)
    h = Math.imul(h, 16777619)
  }
  const ratio = ((h >>> 0) % 1000) / 1000
  return ratio < config.grayRatio
}
