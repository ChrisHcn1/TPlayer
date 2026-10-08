// 播放模式：顺序 / 随机 / 单曲循环
// 全项目唯一的播放模式事实来源，App.vue 的 ref 类型、模式切换数组、
// 图标映射均复用此处的类型与常量
export type PlaybackMode = 'order' | 'random' | 'repeat'

// 切换循环使用的模式顺序（changePlaybackMode 按此数组轮转）
export const PLAYBACK_MODES: PlaybackMode[] = ['order', 'random', 'repeat']

// 播放模式对应的图标资源路径（public 目录下）
// 与 App.vue 原 playbackModeImage computed 内的 switch 一致
export function getPlaybackModeImage(mode: PlaybackMode): string {
  switch (mode) {
    case 'order':
      return '/play-button_25b6-fe0f.png'
    case 'random':
      return '/shuffle-tracks-button_1f500.png'
    case 'repeat':
      return '/repeat-button_1f501.png'
    default:
      return '/play-button_25b6-fe0f.png'
  }
}
