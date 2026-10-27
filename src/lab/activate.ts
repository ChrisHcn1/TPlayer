/**
 * 实验室功能激活入口
 *
 * App 启动、lab store `init()` 之后调用一次。只对「生效中」的功能挂载反应式钩子；
 * 关闭的功能完全不加载，确保对现有功能零影响。
 *
 * 新增实验功能时，在此按 `lab.isEnabled('<id>')` 接入其激活逻辑（各功能本体在
 * `src/lab/features/<id>.ts` 中隔离实现）。
 */
import { useLabStore } from '../stores/lab'
import { useEqualizer } from './features/equalizer'
import { useCustomWallpaper, useThemeColors } from './features/theme-colors'

export function activateLabFeatures(): void {
  const lab = useLabStore()

  // 皮肤配色（强调色 + 窗口底纹）：已实现，挂反应式（内部再按开关判断是否应用）
  useThemeColors()

  // 自定义壁纸（含每日 Bing）：独立实验室功能，按开关门控激活（开启即加载图片）
  useCustomWallpaper()

  // 均衡器：已实现（10 段峰值滤波）。开启时读取一次后端已保存的曲线用于面板回显；
  // 关闭时不加载，滑块状态也不再下发。
  if (lab.isEnabled('equalizer')) useEqualizer()

  // 元数据增强：后端扫描期原子开关，在 `lab_set_feature` 中实时同步 ENHANCED_METADATA，
  // 且切换时触发一次强制重扫，使已扫描曲目按新模式重新解析，无需前端钩子。

  // CUE/SACD 分轨转换：本体在「音频转换」页（ConverterView）按
  // `lab.isEnabled('cue-sacd-convert')` 门控暴露分轨 UI，由后端
  // `converter_split` 执行，无需在此挂载反应式钩子。

  // 歌词边车元数据补全：已并入「元数据增强」（enhanced-metadata），无独立开关；
  // 挂载于共享的 TrackRow 组件（useLyricsMetadata 内部按 lab.isEnabled('enhanced-metadata') 自门控），
  // 开启元数据增强后曲目行渲染时从同名 .lrc 边车回填未知艺术家/专辑。
}
