/**
 * 时间 / 进度等通用纯函数
 *
 * 规则：
 * - 所有函数无副作用，可独立单元测试；
 * - 边界值（0、负数、NaN、Infinity）均有明确定义行为，不抛异常；
 * - 供组件（如 ProgressBar）与 store 共用，避免重复实现。
 */

/**
 * 毫秒 → `m:ss` 时间码（不足一分钟只显示 mm:ss；零值返回 "0:00"）。
 * 对负数、NaN、Infinity 一律当作 0 处理，保证 UI 永不崩溃。
 */
export function formatMs(ms: number): string {
  const finite = Number.isFinite(ms) ? ms : 0
  const total = Math.max(0, Math.round(finite / 1000))
  const minutes = Math.floor(total / 60)
  const seconds = total % 60
  return `${minutes}:${seconds.toString().padStart(2, '0')}`
}

/**
 * 进度比例：clamp(0, position / duration, 1)。
 *
 * 边界：
 * - duration ≤ 0 → 0（避免除零，也符合"无时长无进度"语义）；
 * - position / duration 负数 → 0；
 * - position > duration → 1（进度条不溢出）。
 */
export function progressRatio(positionMs: number, durationMs: number): number {
  if (!Number.isFinite(positionMs) || !Number.isFinite(durationMs)) return 0
  if (durationMs <= 0) return 0
  return Math.min(1, Math.max(0, positionMs / durationMs))
}
