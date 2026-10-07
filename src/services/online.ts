/**
 * 在线能力服务（M0 骨架）
 *
 * 安全边界（硬约束，M1 实现时不得放宽）：
 *  1) 默认关闭：`OnlineModuleConfig.enabled` 默认 false，关闭时任何方法都必须先拒绝，
 *     不得出现"先发请求再判断开关"的代码路径；
 *  2) 默认不外发本地库信息：`allowLibraryUpload` 默认 false；为 false 时，请求体禁止包含
 *     本地文件路径、文件名、曲库统计、音频指纹等任何可关联本地数据的信息；
 *  3) 在线能力与本地曲库逻辑彻底解耦（旧版把网易云 API 代理与本地逻辑交织在一起），
 *     本模块是整个前端唯一的联网出口，便于审计与一键关闭。
 */
import type { Lyrics, OnlineModuleConfig } from '../types'

import { NotImplementedError } from './ipc'

/** 在线能力默认配置：关闭 + 不允许上报本地信息 */
export const DEFAULT_ONLINE_CONFIG: OnlineModuleConfig = {
  enabled: false,
  allowLibraryUpload: false,
  provider: 'none',
}

/** 在线搜索结果条目（服务层 DTO，不进入全局类型出口） */
export interface OnlineSearchItem {
  id: string
  title: string
  artist: string
  album: string | null
  durationMs: number | null
}

/** 在线模块未开启 */
export class OnlineDisabledError extends Error {
  constructor(action: string) {
    super(`在线模块未开启，已拒绝：${action}`)
    this.name = 'OnlineDisabledError'
  }
}

/** 在线请求的统一准入检查：未通过则直接拒绝，不产生任何网络访问 */
export function assertOnlineAllowed(config: OnlineModuleConfig, action: string): void {
  if (!config.enabled || config.provider === 'none') {
    throw new OnlineDisabledError(action)
  }
}

/** 本次请求是否允许携带本地库信息（只有显式开启上报时才为 true） */
export function canSendLibraryInfo(config: OnlineModuleConfig): boolean {
  return config.enabled && config.allowLibraryUpload
}

function pending<T>(action: string): Promise<T> {
  return Promise.reject(new NotImplementedError(action))
}

export const onlineService = {
  /** 在线搜索：仅发送用户输入的关键词，不携带本地库信息 */
  search(_keyword: string, config: OnlineModuleConfig): Promise<OnlineSearchItem[]> {
    assertOnlineAllowed(config, 'online_search')
    return pending('online_search')
  },

  /** 在线歌词：仅发送曲名/歌手等最小检索字段 */
  fetchLyrics(
    _trackId: string,
    _title: string,
    _artist: string,
    config: OnlineModuleConfig,
  ): Promise<Lyrics | null> {
    assertOnlineAllowed(config, 'online_fetch_lyrics')
    return pending('online_fetch_lyrics')
  },

  /** 在线封面：返回可展示的图片地址；下载落盘由后端负责 */
  fetchCover(
    _trackId: string,
    _title: string,
    _artist: string,
    config: OnlineModuleConfig,
  ): Promise<string | null> {
    assertOnlineAllowed(config, 'online_fetch_cover')
    return pending('online_fetch_cover')
  },
}
