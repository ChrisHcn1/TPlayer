/**
 * 本地曲库服务：Rust 侧 library 命令的前端封装（统一入口）
 *
 * 约定：
 *  - 本模块不持有任何状态（状态统一放 stores/library、stores/playlist）；
 *  - 传入的目录路径最终都由 Rust 侧 `security::ensure_allowed` 做白名单校验，
 *    前端不做"可信"假设，也不自行拼接绕过路径；
 *  - 运行在 Tauri 中时走 `callCommand`（命令名与 src-tauri/src/commands/library.rs 对应，
 *    M2 逐个接通）；运行在浏览器时落到 services/web 的本地预览实现，使界面可完整联调。
 */
import { convertFileSrc } from '@tauri-apps/api/core'

import type { Album, Artist, LibraryStats, Playlist, RecentEntry, Track } from '../types'

import { callCommand, isTauriRuntime } from './ipc'
import { webLibrary } from './web/library'

/**
 * 分组封面：Tauri 后端返回缓存文件绝对路径，经 asset 协议编码后供 <img> 加载
 * （相比 data URL 不占 base64 内存；scope 由 tauri.conf.json 限定在 cover-cache 目录）。
 * Web 预览实现直接返回 data URL，同样可用于 <img src>。
 */
function toAssetUrl(path: string | null): string | null {
  return path ? convertFileSrc(path) : null
}

/** 曲库服务契约：Tauri 与 Web 两套实现必须满足同一边界 */
export interface LibraryService {
  scan(): Promise<void>
  cancelScan(): Promise<void>
  tracks(): Promise<Track[]>
  /** 曲目封面（data URL，供播放条/取色使用）；无封面返回 null */
  cover(trackId: string): Promise<string | null>
  /** 专辑聚合封面（asset URL）：组内曲目回退 + 目录图/在线补全，后端两级缓存 */
  albumCover(albumId: string): Promise<string | null>
  /** 艺术家聚合封面（asset URL）：名下曲目专辑封面回退 */
  artistCover(artist: string): Promise<string | null>
  albums(): Promise<Album[]>
  artists(): Promise<Artist[]>
  stats(): Promise<LibraryStats>
  /** CUE 分轨专辑（普通专辑视图之外的独立集合） */
  cueAlbums(): Promise<Album[]>
  favoriteIds(): Promise<string[]>
  setFavorite(trackId: string, favorite: boolean): Promise<void>
  recent(): Promise<RecentEntry[]>
  recordPlayed(trackId: string): Promise<void>
  /** 从曲库索引移除（不删除磁盘文件），并清理收藏 / 歌单引用 */
  removeTracks(ids: string[]): Promise<void>
  addLibraryDir(path: string): Promise<void>
  removeLibraryDir(path: string): Promise<void>
  listPlaylists(): Promise<Playlist[]>
  createPlaylist(name: string): Promise<Playlist>
  renamePlaylist(id: string, name: string): Promise<void>
  deletePlaylist(id: string): Promise<void>
  setPlaylistTracks(id: string, trackIds: string[]): Promise<void>
  addTracksToPlaylist(id: string, trackIds: string[]): Promise<void>
  removeTrackFromPlaylist(id: string, trackId: string): Promise<void>
}

/** Tauri 实现：M2 命令接通前，新增命令在 Rust 侧未注册时会按错误路径进入 store 的 error 态 */
const tauriLibrary: LibraryService = {
  scan: () => callCommand('library_scan'),
  cancelScan: () => callCommand('library_cancel_scan'),
  tracks: () => callCommand<Track[]>('library_tracks'),
  cover: (trackId) => callCommand<string | null>('library_cover', { trackId }),
  albumCover: (albumId) =>
    callCommand<string | null>('library_album_cover', { albumId }).then(toAssetUrl),
  artistCover: (artist) =>
    callCommand<string | null>('library_artist_cover', { artist }).then(toAssetUrl),
  albums: () => callCommand<Album[]>('library_albums'),
  artists: () => callCommand<Artist[]>('library_artists'),
  stats: () => callCommand<LibraryStats>('library_stats'),
  cueAlbums: () => callCommand<Album[]>('library_cue_albums'),
  favoriteIds: () => callCommand<string[]>('library_favorite_ids'),
  setFavorite: (trackId, favorite) =>
    callCommand('library_set_favorite', { trackId, favorite }),
  recent: () => callCommand<RecentEntry[]>('library_recent'),
  recordPlayed: (trackId) => callCommand('library_record_played', { trackId }),
  removeTracks: (ids) => callCommand('library_remove_tracks', { ids }),
  addLibraryDir: (path) => callCommand('library_add_dir', { path }),
  removeLibraryDir: (path) => callCommand('library_remove_dir', { path }),
  listPlaylists: () => callCommand<Playlist[]>('library_list_playlists'),
  createPlaylist: (name) => callCommand<Playlist>('library_create_playlist', { name }),
  renamePlaylist: (id, name) => callCommand('library_rename_playlist', { id, name }),
  deletePlaylist: (id) => callCommand('library_delete_playlist', { id }),
  setPlaylistTracks: (id, trackIds) =>
    callCommand('library_set_playlist_tracks', { id, trackIds }),
  addTracksToPlaylist: (id, trackIds) =>
    callCommand('library_add_tracks_to_playlist', { id, trackIds }),
  removeTrackFromPlaylist: (id, trackId) =>
    callCommand('library_remove_track_from_playlist', { id, trackId }),
}

export const libraryService: LibraryService = isTauriRuntime() ? tauriLibrary : webLibrary
