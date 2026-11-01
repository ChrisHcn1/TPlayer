/**
 * 实验室功能注册表
 *
 * 所有实验功能在此登记元数据（id / 名称 / 说明 / 默认开关）。**新增或废除某个
 * 实验功能只需改这张表**——开关状态存于 config.json 的 lab 段（见 services/lab），
 * 与功能本体解耦；废除时把条目从此表移除，其余代码（src/lab/features/<id>）即可
 * 安全删除，不影响主项目。
 *
 * 约束：默认一律 `defaultEnabled: false`（实验室功能默认关闭，绝不影响现有功能）。
 * 每个 id 对应 `src/lab/features/<id>.ts` 的隔离实现。
 */

export interface LabFeatureDef {
  /** 唯一标识（同时作为 config.lab.features / statuses 的键） */
  id: string
  /** 显示名的 i18n 键 */
  nameKey: string
  /** 一句话说明的 i18n 键 */
  descriptionKey: string
  /** 默认开关：实验室功能一律默认关闭 */
  defaultEnabled: false
}

export const LAB_FEATURES: LabFeatureDef[] = [
  {
    id: 'theme-colors',
    nameKey: 'lab.feature.themeColors.name',
    descriptionKey: 'lab.feature.themeColors.description',
    defaultEnabled: false,
  },
  {
    id: 'custom-wallpaper',
    nameKey: 'lab.feature.customWallpaper.name',
    descriptionKey: 'lab.feature.customWallpaper.description',
    defaultEnabled: false,
  },
  {
    id: 'enhanced-metadata',
    nameKey: 'lab.feature.enhancedMetadata.name',
    descriptionKey: 'lab.feature.enhancedMetadata.description',
    defaultEnabled: false,
  },
  {
    id: 'cue-sacd-convert',
    nameKey: 'lab.feature.cueSacdConvert.name',
    descriptionKey: 'lab.feature.cueSacdConvert.description',
    defaultEnabled: false,
  },
  {
    id: 'audio-visualizer',
    nameKey: 'lab.feature.audioVisualizer.name',
    descriptionKey: 'lab.feature.audioVisualizer.description',
    defaultEnabled: false,
  },
  {
    id: 'floating-lyrics',
    nameKey: 'lab.feature.floatingLyrics.name',
    descriptionKey: 'lab.feature.floatingLyrics.description',
    defaultEnabled: false,
  },
]

/** 取某个功能定义（不存在返回 undefined） */
export function getLabFeature(id: string): LabFeatureDef | undefined {
  return LAB_FEATURES.find((f) => f.id === id)
}
