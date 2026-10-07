/**
 * 导航文案映射模块（titleKey → 展示文案）
 *
 * 职责：侧栏 / 导航相关文案的唯一出口。路由表只声明 i18n 键（meta.titleKey），
 * 组件只调用本模块解析；组件与路由表内不得再出现导航文案字面量
 * （旧版把导航文案硬编码散落在视图与侧栏两处，是重复逻辑的来源之一）。
 *
 * DESIGN 依据：§1.1 主导航项清单、§1.4「侧栏导航由 meta.nav + meta.titleKey 驱动生成」、
 * §4.2 SideBar / NavItem 规格（分组标题、底部固定组的主题切换与折叠按钮文案同样纳入本模块）。
 *
 * M0 形态与演进：
 *  1. 当前为同步静态词条表（默认语言 zh-CN），零依赖、零异步，首屏不出现文案闪烁；
 *     "当前语言"的唯一来源是 settings store 的 locale（constants.LOCALE_STORAGE_KEY）。
 *  2. public/locales/*.json 是最终资源位：M3 接入 i18n 运行时后改由其驱动，
 *     本模块收敛为"键表 + 缺键兜底"，届时词条值不再在此维护（避免两处事实源）。
 */

/** 侧栏分组键与顺序（DESIGN §1.1：曲库组 → 工具组 → 底部固定组） */
export const NAV_GROUPS = ['library', 'tools', 'bottom'] as const

export type NavGroup = (typeof NAV_GROUPS)[number]

/** 分组标题（§4.2：11px / 500 / tertiary 的不可点击文本；bottom 为底部固定区，按规范不显示标题） */
const NAV_GROUP_LABELS: Record<NavGroup, string> = {
  library: '曲库',
  tools: '工具',
  bottom: '',
}

/** 词条表：键与路由 meta.titleKey 同名（§3.7：图标语义名与 i18n key 同源，便于整体替换） */
const NAV_LABELS: Record<string, string> = {
  /* 曲库组（§1.1 第 1~7 项） */
  'nav.songs': '全部歌曲',
  'nav.recent': '最近播放',
  'nav.favorites': '收藏',
  'nav.artists': '艺术家',
  'nav.albums': '专辑',
  'nav.cue': '分轨专辑',
  'nav.playlists': '歌单',
  /* 由标题栏搜索框承载、不占侧栏项（§1.1），词条仍在此登记以备标题栏复用 */
  'nav.search': '搜索',
  /* 工具组（§1.1 第 8 项） */
  'nav.converter': '转换器',
  /* 底部固定组（§1.1 第 9 项 + §4.2 的主题切换 / 折叠按钮） */
  'nav.settings': '设置',
  'nav.theme': '切换主题',
  'nav.theme-to-light': '浅色',
  'nav.theme-to-dark': '深色',
  'nav.collapse': '折叠',
  'nav.expand': '展开',
  'nav.collapse-hint': '收起侧栏（Ctrl+B）',
  'nav.expand-hint': '展开侧栏（Ctrl+B）',
  /* 右面板（§2.6 / §4.10）：地标名、页签、关闭按钮与骨架态说明文案 */
  'nav.panel-label': '歌词与队列',
  'nav.panel-lyrics': '歌词',
  'nav.panel-queue': '队列',
  'nav.panel-close': '关闭面板',
  'nav.lyrics-loading': '歌词加载中',
  'nav.queue-loading': '播放队列加载中',
}

/**
 * 解析 titleKey → 展示文案。
 * 缺键时回落键尾段（如 nav.unknown → unknown），保证界面不直接暴露 nav.xxx 原始键（§6.2）。
 */
export function resolveNavLabel(titleKey: string): string {
  const label = NAV_LABELS[titleKey]
  if (label !== undefined && label.length > 0) {
    return label
  }
  const segments = titleKey.split('.')
  return segments[segments.length - 1] ?? titleKey
}

/** 解析分组 → 分组标题；bottom 组返回空串，由组件据此跳过标题渲染（§4.2） */
export function resolveGroupLabel(group: string): string {
  return isNavGroup(group) ? NAV_GROUP_LABELS[group] : ''
}

/** 分组键守卫：避免组件内散写 'library' / 'tools' / 'bottom' 魔法字符串（脏 meta 一律视为非法） */
export function isNavGroup(value: unknown): value is NavGroup {
  return typeof value === 'string' && (NAV_GROUPS as readonly string[]).includes(value)
}
