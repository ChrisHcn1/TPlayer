/**
 * 服务层统一出口
 * 约定：组件与 store 只从本文件（或具体服务文件名）导入，禁止跨层直接 import @tauri-apps/api。
 */
export * from './ipc'
export { playerService, createIdleSnapshot } from './player'
export { libraryService } from './library'
export { lyricsService } from './lyrics'
export {
  DEFAULT_ONLINE_CONFIG,
  OnlineDisabledError,
  assertOnlineAllowed,
  canSendLibraryInfo,
  onlineService,
} from './online'
export type { OnlineSearchItem } from './online'
export { updateService } from './update'
export { windowService } from './window'
