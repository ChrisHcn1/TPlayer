/**
 * 全局常量（前端）
 * 主题键在此定义，index.html 的首屏预加载脚本以字面量引用同一取值，改动需同步两处。
 */

/** 主题本地存储键（index.html 内联脚本读取同一键） */
export const THEME_STORAGE_KEY = 'tplayer-next.theme'

/** 语言本地存储键 */
export const LOCALE_STORAGE_KEY = 'tplayer-next.locale'

/** 用户设置持久化键（Pinia settings store 使用） */
export const SETTINGS_STORAGE_KEY = 'tplayer-next.settings'

/** 播放位置持久化键 */
export const PLAYBACK_PROGRESS_STORAGE_KEY = 'tplayer-next.playback-progress'

/* ------------------------------------------------------------------ *
 * Web 预览数据源（services/web）使用的 localStorage 键。
 * 仅在非 Tauri（浏览器）运行时使用；Tauri 下这些数据由 Rust 侧落盘，
 * 因此与正式持久化键隔离、互不污染。
 * ------------------------------------------------------------------ */
export const WEB_FAVORITES_STORAGE_KEY = 'tplayer-next.web.favorites'
export const WEB_RECENT_STORAGE_KEY = 'tplayer-next.web.recent'
export const WEB_PLAYLISTS_STORAGE_KEY = 'tplayer-next.web.playlists'
export const WEB_REMOVED_STORAGE_KEY = 'tplayer-next.web.removed'

/** 默认主题 */
export const DEFAULT_THEME = 'dark' as const

/** 默认语言 */
export const DEFAULT_LOCALE = 'zh-CN'

/**
 * 布局状态持久化键（DESIGN §5.10）
 * 唯一持有者为 src/composables/useLayout.ts（侧栏折叠 / 右面板开合与页签）；
 * 值为 JSON 对象，字段缺失或类型非法时整体回落到 LAYOUT_DEFAULTS。
 */
export const LAYOUT_STORAGE_KEY = 'tplayer-next.layout'

/**
 * 布局默认值（§2.2 默认列 + §5.10「恢复失败回落标准模式」的基准）
 * 说明：像素数值与 theme.css 的 --size-sidebar-w / --size-panel-w 同值，
 * 但渲染宽度一律取 CSS token；此处仅用于字段合法性判断，不参与布局计算。
 */
export const LAYOUT_DEFAULTS = {
  sidebarCollapsed: false,
  panelOpen: false,
  panelTab: 'lyrics',
  // 播放列表上方的实时歌词条默认展开，用户可手动开合（状态记忆）
  desktopLyrics: true,
  // 皮肤色跟随当前曲目封面（播放条氛围光 + 放大层强调色）
  coverAccent: true,
} as const

/**
 * 支持的音频扩展名（曲库扫描索引范围，与 Rust 侧 scan.rs SUPPORTED_EXT 保持一致）。
 * 其中 dsd(dsf/dff)/dts/ape/wv/tta/tak/wma/aiff 可入库展示，
 * 但当前播放引擎（rodio/symphonia）不支持解码，播放时会收到明确错误提示。
 */
export const SUPPORTED_AUDIO_EXTENSIONS = [
  'mp3',
  'flac',
  'wav',
  'aac',
  'ogg',
  'oga',
  'opus',
  'm4a',
  'wma',
  'aif',
  'aiff',
  'ape',
  'wv',
  'tta',
  'tak',
  'dsf',
  'dff',
  'dts',
] as const

/** 应用数据目录名（与旧版 %APPDATA%\TPlayer 隔离） */
export const APP_DATA_DIR_NAME = 'TPlayer Next'
