/**
 * 内联 SVG 图标集（DESIGN §3.7）
 *
 * 规范：24×24 网格；默认描边 1.6、round 端点/转角、fill=none、颜色 currentColor，
 * 由 <Icon> 在 <svg> 上统一声明；需要填充态的元素（播放三角 / 实心心形 / 圆点）
 * 在各自节点上写 fill="currentColor" stroke="none" 覆盖。
 *
 * 命名：icon-{语义} 中的“语义”部分，与路由 meta.icon / i18n 键同源。
 * 仅维护本文件一处，禁止把 SVG 散落到各组件（§3.7：不使用 emoji / 图片图标）。
 */

/** 通用描边组（圆形端点），减少重复声明 */
const g = (inner: string): string =>
  `<g fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">${inner}</g>`

export const ICONS: Record<string, string> = {
  /* ---- 侧栏导航 ---- */
  'music-note': g(
    '<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>',
  ),
  clock: g('<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>'),
  heart: g(
    '<path d="M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z"/>',
  ),
  'heart-filled':
    '<path fill="currentColor" stroke="none" d="M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z"/>',
  user: g('<path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/>'),
  disc: g('<circle cx="12" cy="12" r="9.5"/><circle cx="12" cy="12" r="2.2"/>'),
  layers: g('<path d="m12 2 9 5-9 5-9-5 9-5Z"/><path d="m3 12 9 5 9-5"/><path d="m3 17 9 5 9-5"/>'),
  'list-music': g(
    '<path d="M21 15V6"/><path d="M18.5 18a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5Z"/><path d="M12 12H3"/><path d="M16 6H3"/><path d="M12 18H3"/>',
  ),
  swap: g(
    '<path d="M8 3 4 7l4 4"/><path d="M4 7h16"/><path d="m16 21 4-4-4-4"/><path d="M20 17H4"/>',
  ),
  settings: g(
    '<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>',
  ),
  'chevron-left': g('<path d="m15 18-6-6 6-6"/>'),

  /* ---- 标题栏 / 窗口 ---- */
  search: g('<circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/>'),
  sun: g(
    '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"/>',
  ),
  moon: g('<path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"/>'),
  minus: g('<path d="M5 12h14"/>'),
  square: '<rect x="5.5" y="5.5" width="13" height="13" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.6"/>',
  close: g('<path d="M18 6 6 18M6 6l12 12"/>'),
  globe: g('<circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3a14 14 0 0 1 0 18 14 14 0 0 1 0-18"/>'),

  /* ---- 传输控制（播放/暂停/上下首为填充态） ---- */
  play: '<path fill="currentColor" stroke="none" d="M7 4.5 19 12 7 19.5Z"/>',
  pause:
    '<rect x="6.5" y="4.5" width="3.4" height="15" rx="1.2" fill="currentColor" stroke="none"/><rect x="14.1" y="4.5" width="3.4" height="15" rx="1.2" fill="currentColor" stroke="none"/>',
  next: '<path fill="currentColor" stroke="none" d="M6 5.5v13L16 12Z"/><rect x="17" y="5" width="2.2" height="14" rx="1" fill="currentColor" stroke="none"/>',
  previous:
    '<path fill="currentColor" stroke="none" d="M18 5.5v13L8 12Z"/><rect x="4.8" y="5" width="2.2" height="14" rx="1" fill="currentColor" stroke="none"/>',
  repeat: g(
    '<path d="m17 2 4 4-4 4"/><path d="M3 11v-1a4 4 0 0 1 4-4h14"/><path d="m7 22-4-4 4-4"/><path d="M21 13v1a4 4 0 0 1-4 4H3"/>',
  ),
  'repeat-one': g(
    '<path d="m17 2 4 4-4 4"/><path d="M3 11v-1a4 4 0 0 1 4-4h14"/><path d="m7 22-4-4 4-4"/><path d="M21 13v1a4 4 0 0 1-4 4H3"/><path d="M11 10h1v4"/>',
  ),
  shuffle: g(
    '<path d="M2 18h1.4c1.3 0 2.5-.6 3.3-1.7l6.1-8.6c.8-1.1 2-1.7 3.3-1.7H22"/><path d="m18 2 4 4-4 4"/><path d="M2 6h1.9c1.5 0 2.9.9 3.6 2.2"/><path d="M22 18h-5.9c-1.3 0-2.6-.7-3.3-1.8l-.5-.8"/><path d="m18 14 4 4-4 4"/>',
  ),
  'list-order': g('<path d="M10 6h11M10 12h11M10 18h11"/><path d="M4 6h1v4M4 16l2-2v4"/>'),

  /* ---- 播放条右区 ---- */
  'volume-high': g('<path d="M11 5 6 9H2v6h4l5 4Z"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14"/>'),
  'volume-low': g('<path d="M11 5 6 9H2v6h4l5 4Z"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/>'),
  'volume-mute': g('<path d="M11 5 6 9H2v6h4l5 4Z"/><path d="m22 9-6 6M16 9l6 6"/>'),
  lyrics: g('<path d="M4 6h16M4 12h10M4 18h13"/>'),
  // 桌面歌词条开关：矩形框内两行字幕
  captions: g(
    '<rect x="3" y="5" width="18" height="14" rx="2.5"/><path d="M7 12h3M14 12h3M7 15.5h3M14 15.5h3"/>',
  ),
  queue: g('<path d="M4 6h16M4 12h16M4 18h10"/>'),

  /* ---- 列表 / 工具条 ---- */
  'more-vertical':
    '<circle cx="12" cy="5" r="1.5" fill="currentColor" stroke="none"/><circle cx="12" cy="12" r="1.5" fill="currentColor" stroke="none"/><circle cx="12" cy="19" r="1.5" fill="currentColor" stroke="none"/>',
  'more-horizontal':
    '<circle cx="5" cy="12" r="1.5" fill="currentColor" stroke="none"/><circle cx="12" cy="12" r="1.5" fill="currentColor" stroke="none"/><circle cx="19" cy="12" r="1.5" fill="currentColor" stroke="none"/>',
  sort: g('<path d="m21 16-4 4-4-4"/><path d="M17 20V4"/><path d="m3 8 4-4 4 4"/><path d="M7 4v16"/>'),
  filter: g('<path d="M22 3H2l8 9.46V19l4 2v-8.54L22 3z"/>'),
  density: g('<path d="M4 6h16M4 12h16M4 18h16"/>'),
  grid: g('<rect x="3" y="3" width="7.5" height="7.5" rx="1.2"/><rect x="13.5" y="3" width="7.5" height="7.5" rx="1.2"/><rect x="13.5" y="13.5" width="7.5" height="7.5" rx="1.2"/><rect x="3" y="13.5" width="7.5" height="7.5" rx="1.2"/>'),
  list: g('<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/>'),
  check: g('<path d="M20 6 9 17l-5-5"/>'),
  'chevron-right': g('<path d="m9 18 6-6-6-6"/>'),
  'chevron-down': g('<path d="m6 9 6 6 6-6"/>'),
  back: g('<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>'),
  plus: g('<path d="M12 5v14M5 12h14"/>'),
  trash: g('<path d="M3 6h18"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/><path d="M10 11v5M14 11v5"/>'),
  edit: g('<path d="M12 20h9"/><path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z"/>'),
  copy: g('<rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>'),
  refresh: g('<path d="M3 12a9 9 0 0 1 15-6.7L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-15 6.7L3 16"/><path d="M3 21v-5h5"/>'),
  'external-link': g('<path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>'),
  download: g('<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5"/><path d="M12 15V3"/>'),
  folder: g('<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>'),
  'folder-plus': g('<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/><path d="M12 11v6M9 14h6"/>'),
  file: g('<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z"/><path d="M14 2v6h6"/>'),
  maximize: g('<path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7"/>'),

  /* ---- 状态 / 反馈 ---- */
  info: g('<circle cx="12" cy="12" r="9"/><path d="M12 16v-4M12 8h.01"/>'),
  'alert-triangle': g('<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/><path d="M12 9v4M12 17h.01"/>'),
  'x-circle': g('<circle cx="12" cy="12" r="9"/><path d="m15 9-6 6M9 9l6 6"/>'),
  'music-off': g('<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/><path d="m2 2 20 20"/>'),
  'cloud-offline': g('<path d="m2 2 20 20"/><path d="M17.5 19a4.5 4.5 0 0 0 .5-9 7 7 0 0 0-11.6-3.6"/>'),
  loader: g('<path d="M12 3a9 9 0 1 0 9 9"/>'),
  'corner-down-left': g('<path d="m9 10-5 5 5 5"/><path d="M4 15h11a5 5 0 0 0 5-5V5"/>'),

  /* ---- M1 列表 / 排序 / 空态 / 歌单动作 ---- */
  inbox: g(
    '<path d="M22 12h-6l-2 3h-4l-2-3H2"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11Z"/>',
  ),
  'arrow-up': g('<path d="M12 19V5"/><path d="m5 12 7-7 7 7"/>'),
  'arrow-down': g('<path d="M12 5v14"/><path d="m19 12-7 7-7-7"/>'),
  'arrow-up-down': g('<path d="m21 16-4 4-4-4"/><path d="M17 20V4"/><path d="m3 8 4-4 4 4"/><path d="M7 4v16"/>'),
  'heart-off': g(
    '<path d="M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z"/><path d="m2 2 20 20"/>',
  ),
  'list-plus': g('<path d="M11 6H3"/><path d="M11 12H3"/><path d="M11 18H3"/><path d="M18 9v6"/><path d="M15 12h6"/>'),
  'list-minus': g('<path d="M11 6H3"/><path d="M11 12H3"/><path d="M11 18H3"/><path d="M15 12h6"/>'),
  'trash-2': g('<path d="M3 6h18"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/><path d="M10 11v6M14 11v6"/>'),
  x: g('<path d="M18 6 6 18M6 6l12 12"/>'),
}

/** 图标是否存在（组件用于未知图标兜底） */
export function hasIcon(name: string): boolean {
  return Object.prototype.hasOwnProperty.call(ICONS, name)
}
