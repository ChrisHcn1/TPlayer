import { ref, reactive } from 'vue'
import localforage from 'localforage'
import { cloneDeep } from 'lodash-es'

// 定义类型
export interface SongType {
  id: string
  title: string
  artist: string
  album: string
  duration: string
  path: string
  lyrics?: string
  album_art?: string
}

export interface LocalPlaylistType {
  id: number
  name: string
  description?: string
  songs: string[] // 存储歌曲ID
  cover?: string
  createTime: number
  updateTime: number
}

// localDB
const localDB = localforage.createInstance({
  name: 'tplayer-data',
  description: 'Local data of TPlayer',
  storeName: 'local'
})

/**
 * 生成本地歌单 ID（16位数字）
 * 使用时间戳 + 随机数确保唯一性
 */
const generateLocalPlaylistId = (): number => {
  const timestamp = Date.now().toString() // 13位
  const random = Math.floor(Math.random() * 1000)
    .toString()
    .padStart(3, '0') // 3位
  return parseInt(timestamp + random, 10)
}

/**
 * 创建 localStore 实例
 * @returns localStore 实例
 */
const createLocalStore = () => {
  // 本地歌曲
  const localSongs = ref<SongType[]>([])
  // 本地歌单
  const localPlaylists = ref<LocalPlaylistType[]>([])
  // 我喜欢的歌曲ID列表
  const favoriteSongs = ref<string[]>([])
  // 是否初始化完成
  const isInitialized = ref(false)

  // 读取本地歌曲
  const readLocalSong = async (): Promise<SongType[]> => {
    try {
      const result = await localDB.getItem('local-songs')
      localSongs.value = (result as SongType[]) || []
      return localSongs.value
    } catch (error) {
      console.error('Error reading local songs:', error)
      throw error
    }
  }

  // 更新本地歌曲
  const updateLocalSong = async (songs: SongType[]) => {
    try {
      await localDB.setItem('local-songs', cloneDeep(songs))
      localSongs.value = songs
    } catch (error) {
      console.error('Error updating local songs:', error)
      throw error
    }
  }

  // 删除指定歌曲
  const deleteLocalSong = async (index: number) => {
    try {
      const playlist = cloneDeep(localSongs.value)
      playlist.splice(index, 1)
      await localDB.setItem('local-songs', playlist)
      localSongs.value = playlist
    } catch (error) {
      console.error('Error deleting local song:', error)
      throw error
    }
  }

  // 读取本地歌单列表
  const readLocalPlaylists = async (): Promise<LocalPlaylistType[]> => {
    try {
      const result = await localDB.getItem('local-playlists')
      localPlaylists.value = (result as LocalPlaylistType[]) || []
      // 读取我喜欢的歌曲
      const favResult = await localDB.getItem('favorite-songs')
      favoriteSongs.value = (favResult as string[]) || []
      isInitialized.value = true
      return localPlaylists.value
    } catch (error) {
      console.error('Error reading local playlists:', error)
      throw error
    }
  }

  // 保存本地歌单列表到存储
  const saveLocalPlaylists = async () => {
    try {
      await localDB.setItem('local-playlists', cloneDeep(localPlaylists.value))
    } catch (error) {
      console.error('Error saving local playlists:', error)
      throw error
    }
  }

  // 创建本地歌单
  const createLocalPlaylist = async (
    name: string,
    description?: string
  ): Promise<LocalPlaylistType> => {
    const now = Date.now()
    const newPlaylist: LocalPlaylistType = {
      id: generateLocalPlaylistId(),
      name,
      description,
      songs: [],
      createTime: now,
      updateTime: now
    }
    localPlaylists.value.push(newPlaylist)
    await saveLocalPlaylists()
    return newPlaylist
  }

  // 更新本地歌单信息
  const updateLocalPlaylist = async (
    id: number,
    data: Partial<Pick<LocalPlaylistType, 'name' | 'description'>>
  ): Promise<boolean> => {
    const index = localPlaylists.value.findIndex((p) => p.id === id)
    if (index === -1) return false
    const playlist = localPlaylists.value[index]
    if (data.name !== undefined) playlist.name = data.name
    if (data.description !== undefined) playlist.description = data.description
    playlist.updateTime = Date.now()
    await saveLocalPlaylists()
    return true
  }

  // 删除本地歌单
  const deleteLocalPlaylist = async (id: number): Promise<boolean> => {
    const index = localPlaylists.value.findIndex((p) => p.id === id)
    if (index === -1) return false
    localPlaylists.value.splice(index, 1)
    await saveLocalPlaylists()
    return true
  }

  // 添加歌曲到本地歌单
  const addSongsToLocalPlaylist = async (
    playlistId: number,
    songIds: string[]
  ): Promise<{ success: boolean; addedCount: number }> => {
    const playlist = localPlaylists.value.find((p) => p.id === playlistId)
    if (!playlist) return { success: false, addedCount: 0 }

    // 过滤已存在的歌曲
    const existingIds = new Set(playlist.songs)
    const newIds = songIds.filter((id) => !existingIds.has(id))
    if (newIds.length === 0) return { success: true, addedCount: 0 }
    // 后添加的歌曲放在前面
    playlist.songs.unshift(...newIds)
    playlist.updateTime = Date.now()
    await saveLocalPlaylists()
    return { success: true, addedCount: newIds.length }
  }

  // 从本地歌单移除歌曲
  const removeSongsFromLocalPlaylist = async (
    playlistId: number,
    songIds: string[]
  ): Promise<boolean> => {
    const playlist = localPlaylists.value.find((p) => p.id === playlistId)
    if (!playlist) return false

    const idsToRemove = new Set(songIds)
    playlist.songs = playlist.songs.filter((id) => !idsToRemove.has(id))
    playlist.updateTime = Date.now()
    await saveLocalPlaylists()
    return true
  }

  // 获取本地歌单详情（包含歌曲列表）
  const getLocalPlaylistDetail = (
    id: number
  ): { playlist: LocalPlaylistType; songs: SongType[] } | null => {
    const playlist = localPlaylists.value.find((p) => p.id === id)
    if (!playlist) return null

    // 根据歌单中的歌曲ID获取完整歌曲信息
    const songsMap = new Map(localSongs.value.map((s) => [s.id, s]))
    const songs = playlist.songs
      .map((songId) => songsMap.get(songId))
      .filter((s): s is SongType => s !== undefined)

    return { playlist, songs }
  }

  /**
   * 判断是否为本地歌单 ID
   * @param id 歌单 ID
   */
  const isLocalPlaylist = (id: number | string | undefined | null): boolean => {
    if (!id) return false
    const strId = id.toString()
    if (strId.length !== 16) return false
    // 检查是否存在于本地歌单列表
    return localPlaylists.value.some((p) => p.id.toString() === strId)
  }

  // 添加歌曲到我喜欢
  const addToFavorites = async (songId: string): Promise<boolean> => {
    if (favoriteSongs.value.includes(songId)) return false
    favoriteSongs.value.unshift(songId)
    await localDB.setItem('favorite-songs', cloneDeep(favoriteSongs.value))
    return true
  }

  // 从我喜欢移除歌曲
  const removeFromFavorites = async (songId: string): Promise<boolean> => {
    const index = favoriteSongs.value.indexOf(songId)
    if (index === -1) return false
    favoriteSongs.value.splice(index, 1)
    await localDB.setItem('favorite-songs', cloneDeep(favoriteSongs.value))
    return true
  }

  // 检查歌曲是否在我喜欢中
  const isFavorite = (songId: string): boolean => {
    return favoriteSongs.value.includes(songId)
  }

  // 获取我喜欢的歌曲列表
  const getFavoriteSongs = (): SongType[] => {
    const songsMap = new Map(localSongs.value.map((s) => [s.id, s]))
    return favoriteSongs.value
      .map((songId) => songsMap.get(songId))
      .filter((s): s is SongType => s !== undefined)
  }

  return reactive({
    localSongs,
    localPlaylists,
    favoriteSongs,
    isInitialized,
    readLocalSong,
    updateLocalSong,
    deleteLocalSong,
    readLocalPlaylists,
    createLocalPlaylist,
    updateLocalPlaylist,
    deleteLocalPlaylist,
    addSongsToLocalPlaylist,
    removeSongsFromLocalPlaylist,
    getLocalPlaylistDetail,
    isLocalPlaylist,
    addToFavorites,
    removeFromFavorites,
    isFavorite,
    getFavoriteSongs
  })
}

// 创建全局的 localStore 实例
const localStoreInstance = createLocalStore()

/**
 * 获取本地歌单存储实例
 * @returns 本地歌单存储实例
 */
export const useLocalStore = () => {
  return localStoreInstance
}