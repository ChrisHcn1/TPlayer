/**
 * 实验室功能激活入口
 *
 * App 启动、lab store `init()` 之后调用一次。只对「生效中」的功能挂载反应式钩子；
 * 关闭的功能完全不加载，确保对现有功能零影响。
 *
 * 新增实验功能时，在此按 `lab.isEnabled('<id>')` 接入其激活逻辑（各功能本体在
 * `src/lab/features/<id>.ts` 中隔离实现）。
 */
import { useCustomWallpaper, useThemeColors } from './features/theme-colors'
import { useFloatingLyrics } from './features/floating-lyrics'

export function activateLabFeatures(): void {
  // 皮肤配色（强调色 + 窗口底纹）：已实现，挂反应式（内部再按开关判断是否应用）
  useThemeColors()

  // 自定义壁纸（含每日 Bing）：独立实验室功能，按开关门控激活（开启即加载图片）
  useCustomWallpaper()

  // 音频可视化：纯 UI 驱动——画布组件（AudioVisualizer.vue）按 lab.isEnabled 挂载于
  // 播放条，订阅后端 audio-analyser 事件渲染；后端分析器由 lab_set_feature 的原子开关
  // 即时启停，无需在此挂载反应式钩子。

  // 悬浮歌词：仅在功能开启时注册；自动弹窗由 App.vue 在数据/播放初始化完成后再触发
  // （避免启动阶段与曲库/歌单/引擎初始化抢占，且窗口创建为异步命令，不阻塞主线程）。
  useFloatingLyrics()

  // 元数据增强：后端扫描期原子开关，在 `lab_set_feature` 中实时同步 ENHANCED_METADATA，
  // 且切换时触发一次强制重扫，使已扫描曲目按新模式重新解析，无需前端钩子。

  // CUE/SACD 分轨转换：本体在「音频转换」页（ConverterView）按
  // `lab.isEnabled('cue-sacd-convert')` 门控暴露分轨 UI，由后端
  // `converter_split` 执行，无需在此挂载反应式钩子。

  // 歌词边车元数据补全：已并入「元数据增强」（enhanced-metadata），无独立开关；
  // 挂载于共享的 TrackRow 组件（useLyricsMetadata 内部按 lab.isEnabled('enhanced-metadata') 自门控），
  // 开启元数据增强后曲目行渲染时从同名 .lrc 边车回填未知艺术家/专辑。
}
