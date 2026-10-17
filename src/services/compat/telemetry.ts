/**
 * 埋点统计 —— 兜底触发率、成功率、平均/最大耗时。
 *
 * 所有记录均 try/catch 包裹，绝不向主播放器抛出异常。
 * 默认仅本地统计；若配置 `reportEndpoint` 非空，则周期性上报并清理已上报样本。
 */
import type { CompatConfig, TelemetrySample } from './types'

interface TelemetryState {
  triggered: number
  success: number
  failed: number
  totalDecodeMs: number
  maxDecodeMs: number
  samples: TelemetrySample[]
}

function emptyState(): TelemetryState {
  return { triggered: 0, success: 0, failed: 0, totalDecodeMs: 0, maxDecodeMs: 0, samples: [] }
}

export class Telemetry {
  private state: TelemetryState = emptyState()
  private config: CompatConfig | null = null

  setConfig(config: CompatConfig): void {
    this.config = config
  }

  record(sample: TelemetrySample): void {
    try {
      this.state.triggered += 1
      if (sample.success) this.state.success += 1
      else this.state.failed += 1
      this.state.totalDecodeMs += sample.durationMs
      if (sample.durationMs > this.state.maxDecodeMs) this.state.maxDecodeMs = sample.durationMs
      this.state.samples.push(sample)
      if (this.state.samples.length > 200) this.state.samples.shift()
    } catch {
      /* 忽略 */
    }
  }

  /** 聚合快照（供调试/上报） */
  snapshot(): {
    triggered: number
    success: number
    failed: number
    successRate: number
    avgMs: number
    maxMs: number
  } {
    const s = this.state
    return {
      triggered: s.triggered,
      success: s.success,
      failed: s.failed,
      successRate: s.triggered > 0 ? s.success / s.triggered : 0,
      avgMs: s.triggered > 0 ? s.totalDecodeMs / s.triggered : 0,
      maxMs: s.maxDecodeMs,
    }
  }

  /** 异步上报（若配置了端点）；异常被吞掉 */
  async report(): Promise<void> {
    const cfg = this.config
    if (!cfg || !cfg.reportEndpoint || this.state.samples.length === 0) return
    try {
      const payload = { ...this.snapshot(), samples: this.state.samples }
      const ctrl = new AbortController()
      const timer = setTimeout(() => ctrl.abort(), 4000)
      await fetch(cfg.reportEndpoint, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
        signal: ctrl.signal,
      })
      clearTimeout(timer)
      this.state.samples = []
    } catch {
      /* 上报失败不影响播放 */
    }
  }
}
