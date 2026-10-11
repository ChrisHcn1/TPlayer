/**
 * 目录自动监控服务
 *
 *  - 查询监控状态：后端 watcher_status 命令（Rust notify 实时监听）；
 *  - 订阅增量同步事件：Rust 侧在每次批量同步后推送 `watcher://event`，
 *    前端据此刷新曲库与设置页状态展示；
 *  - 浏览器预览环境降级为空实现（监控仅 Tauri 运行时可用）。
 */
import { listen } from '@tauri-apps/api/event'

import { callCommand, isTauriRuntime } from './ipc'

/** 监控状态（与 Rust watch::WatchStatus 对齐） */
export interface WatcherStatus {
  enabled: boolean
  dirs: string[]
  last_event_at: string | null
  last_event_desc: string | null
}

/** 增量同步事件负载（Rust 推送 watcher://event） */
export interface WatcherEventPayload {
  desc: string
  at: string
}

/** 查询目录自动监控状态；非 Tauri 环境返回 null */
export async function getWatcherStatus(): Promise<WatcherStatus | null> {
  if (!isTauriRuntime()) return null
  try {
    return await callCommand<WatcherStatus>('watcher_status')
  } catch {
    return null
  }
}

/** 订阅增量同步事件，返回取消订阅函数；非 Tauri 环境返回空操作 */
export function subscribeWatcher(listener: (payload: WatcherEventPayload) => void): () => void {
  if (!isTauriRuntime()) return () => undefined
  let unlisten: (() => void) | null = null
  void listen<WatcherEventPayload>('watcher://event', (event) => listener(event.payload)).then(
    (un) => {
      unlisten = un
    },
  )
  return () => {
    unlisten?.()
  }
}
