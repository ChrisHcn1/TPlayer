// 均衡器 10 个频段的显示标签
export const EQ_BAND_LABELS = [
  '31Hz', '62Hz', '125Hz', '250Hz', '500Hz',
  '1kHz', '2kHz', '4kHz', '8kHz', '16kHz'
]

// 均衡器预设（10 段增益，单位 dB）
export const EQ_PRESETS: Record<string, number[]> = {
  flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  rock: [6, 5, 4, 3, 2, -1, -2, -3, -2, 0],
  pop: [-2, -1, 0, 2, 4, 4, 3, 2, 1, 0],
  jazz: [4, 3, 2, 1, -1, -2, -1, 0, 2, 4],
  classical: [7, 5, 3, 1, -1, -2, -1, 1, 3, 5],
  electronic: [4, 3, 2, -1, -3, -2, 0, 2, 3, 4]
}

// 获取频段显示标签
export function getBandLabel(index: number): string | undefined {
  return EQ_BAND_LABELS[index]
}

// 按名称获取预设增益数组（复制一份，避免外部修改污染常量）
export function getEqPreset(name: string): number[] | undefined {
  const preset = EQ_PRESETS[name]
  return preset ? [...preset] : undefined
}
