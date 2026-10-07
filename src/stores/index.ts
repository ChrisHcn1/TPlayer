/**
 * Pinia store 统一出口
 * 约定：视图与组件从本文件导入 store，不跨目录深链到具体实现文件。
 * 注意：src/main.ts 直接 import './stores/player' 等具体文件亦可，两者等价。
 */
export { usePlayerStore } from './player'
export type { PlayRequestContext } from './player'
export { useLibraryStore } from './library'
export { usePlaylistStore } from './playlist'
export { useLyricsStore } from './lyrics'
export { useSettingsStore } from './settings'
export type { PersistedSettings } from './settings'
