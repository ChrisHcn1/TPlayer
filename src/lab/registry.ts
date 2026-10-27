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
  /** 显示名 */
  name: string
  /** 一句话说明 */
  description: string
  /** 默认开关：实验室功能一律默认关闭 */
  defaultEnabled: false
}

export const LAB_FEATURES: LabFeatureDef[] = [
  {
    id: 'equalizer',
    name: '均衡器',
    description: '调整音乐音色，让低音更浑厚或人声更清晰',
    defaultEnabled: false,
  },
  {
    id: 'theme-colors',
    name: '皮肤配色',
    description: '自定义窗口颜色与风格，让播放器更符合你的喜好',
    defaultEnabled: false,
  },
  {
    id: 'custom-wallpaper',
    name: '自定义壁纸',
    description: '设置窗口背景图片，支持本地图片或每日自动更换的壁纸',
    defaultEnabled: false,
  },
  {
    id: 'enhanced-metadata',
    name: '元数据增强',
    description: '自动补全歌曲的歌手、专辑等信息，让曲库显示更整齐',
    defaultEnabled: false,
  },
  {
    id: 'cue-sacd-convert',
    name: 'CUE/SACD 分轨转换',
    description: '把整张专辑或光盘镜像拆分成单曲，方便单独播放和管理',
    defaultEnabled: false,
  },
]

/** 取某个功能定义（不存在返回 undefined） */
export function getLabFeature(id: string): LabFeatureDef | undefined {
  return LAB_FEATURES.find((f) => f.id === id)
}
