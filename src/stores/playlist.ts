/**
 * playlist store（M0 骨架）
 *
 * 职责：歌单列表与当前歌单；歌单数据由 Rust 侧持久化在应用数据目录（不落盘到音频目录）。
 * 约定：system 歌单（如"最近播放"）由后端标记 isSystem，前端只允许读取与播放，禁止改名/删除。
 */
import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import { libraryService } from '../services/library'
import { normalizeError } from '../services/ipc'
import type { Playlist } from '../types'

export const usePlaylistStore = defineStore('playlist', () => {
  const playlists = ref<Playlist[]>([])
  const currentId = ref<string | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  /** 当前选中歌单 */
  const current = computed(
    () => playlists.value.find((playlist) => playlist.id === currentId.value) ?? null,
  )

  /** 系统歌单（只读） */
  const systemPlaylists = computed(() => playlists.value.filter((playlist) => playlist.isSystem))

  /** 用户可编辑歌单 */
  const editablePlaylists = computed(() => playlists.value.filter((playlist) => !playlist.isSystem))

  async function refresh(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      playlists.value = await libraryService.listPlaylists()
    } catch (cause) {
      error.value = normalizeError(cause).message
    } finally {
      loading.value = false
    }
  }

  async function create(name: string): Promise<void> {
    loading.value = true
    error.value = null
    try {
      const created = await libraryService.createPlaylist(name)
      playlists.value = [...playlists.value, created]
      currentId.value = created.id
    } catch (cause) {
      error.value = normalizeError(cause).message
    } finally {
      loading.value = false
    }
  }

  async function rename(id: string, name: string): Promise<void> {
    // 系统歌单不可编辑，前端先拦一层，后端仍会二次校验
    if (playlists.value.find((playlist) => playlist.id === id)?.isSystem) return
    try {
      await libraryService.renamePlaylist(id, name)
      playlists.value = playlists.value.map((playlist) =>
        playlist.id === id ? { ...playlist, name } : playlist,
      )
    } catch (cause) {
      error.value = normalizeError(cause).message
    }
  }

  async function remove(id: string): Promise<void> {
    if (playlists.value.find((playlist) => playlist.id === id)?.isSystem) return
    try {
      await libraryService.deletePlaylist(id)
      playlists.value = playlists.value.filter((playlist) => playlist.id !== id)
      if (currentId.value === id) currentId.value = null
    } catch (cause) {
      error.value = normalizeError(cause).message
    }
  }

  async function setTracks(id: string, trackIds: string[]): Promise<void> {
    try {
      await libraryService.setPlaylistTracks(id, trackIds)
      playlists.value = playlists.value.map((playlist) =>
        playlist.id === id ? { ...playlist, trackIds: [...trackIds] } : playlist,
      )
    } catch (cause) {
      error.value = normalizeError(cause).message
    }
  }

  /** 向歌单追加曲目（去重）；系统歌单忽略 */
  async function addTracks(id: string, trackIds: string[]): Promise<void> {
    const target = playlists.value.find((playlist) => playlist.id === id)
    if (!target || target.isSystem || trackIds.length === 0) return
    const merged = [...target.trackIds]
    for (const trackId of trackIds) {
      if (!merged.includes(trackId)) merged.push(trackId)
    }
    const previous = playlists.value
    playlists.value = playlists.value.map((playlist) =>
      playlist.id === id ? { ...playlist, trackIds: merged } : playlist,
    )
    try {
      await libraryService.addTracksToPlaylist(id, trackIds)
    } catch (cause) {
      playlists.value = previous
      error.value = normalizeError(cause).message
    }
  }

  /** 从歌单移除单曲；系统歌单忽略 */
  async function removeTrack(id: string, trackId: string): Promise<void> {
    const target = playlists.value.find((playlist) => playlist.id === id)
    if (!target || target.isSystem) return
    const previous = playlists.value
    playlists.value = playlists.value.map((playlist) =>
      playlist.id === id
        ? { ...playlist, trackIds: playlist.trackIds.filter((item) => item !== trackId) }
        : playlist,
    )
    try {
      await libraryService.removeTrackFromPlaylist(id, trackId)
    } catch (cause) {
      playlists.value = previous
      error.value = normalizeError(cause).message
    }
  }

  function byId(id: string): Playlist | undefined {
    return playlists.value.find((playlist) => playlist.id === id)
  }

  function select(id: string | null): void {
    currentId.value = id
  }

  return {
    playlists,
    currentId,
    loading,
    error,
    current,
    systemPlaylists,
    editablePlaylists,
    refresh,
    create,
    rename,
    remove,
    setTracks,
    addTracks,
    removeTrack,
    byId,
    select,
  }
})
