import { ref, onUnmounted, type Ref } from 'vue'
import { localStorageService, type Playlist } from '../stores/local'
import { useMessage } from './useMessage'
import type { Song } from '../types/song'

type LogFn = (...args: any[]) => void

interface UseLibraryActionsOptions {
  // 播放列表（删除时原地 splice）
  songs: Ref<Song[]>
  // 收藏歌曲 id 列表
  favorites: Ref<string[]>
  // 用户自建歌单
  playlists: Ref<Playlist[]>
  // 右键菜单项动作完成后关闭菜单
  closeSongMenu: () => void
  logError: LogFn
}

// 歌曲库管理动作：收藏切换、加入歌单（可视化点选弹层）、删除歌曲、创建歌单。
// 均为 localStorage 持久化编排，不涉及播放时序；共享状态由容器注入并原地更新。
export function useLibraryActions(options: UseLibraryActionsOptions) {
  const { songs, favorites, playlists, closeSongMenu, logError } = options
  const { showError, showSuccess, showInfo, confirmAction } = useMessage()

  const toggleFavorite = async (song: Song) => {
    try {
      const newStatus = !song.isFavorite
      song.isFavorite = newStatus

      if (newStatus) {
        await localStorageService.addToFavorites(song.id)
      } else {
        await localStorageService.removeFromFavorites(song.id)
      }

      // 更新收藏列表
      favorites.value = await localStorageService.getFavorites()
    } catch (error) {
      logError('更新收藏状态失败:', error)
      // 回滚状态
      song.isFavorite = !song.isFavorite
      showError('收藏操作失败,请重试')
    }
  }

  // ===== 加入歌单：点选弹层（替代旧的序号输入 prompt） =====
  // 支持单首（右键菜单）与多首（多选工具栏批量添加）
  const showAddToPlaylistMenu = ref(false)
  const addMenuPosition = ref({ left: '0px', top: '0px' })
  const pendingSongs = ref<Song[]>([])

  const closeAddToPlaylistMenu = () => {
    showAddToPlaylistMenu.value = false
    pendingSongs.value = []
    document.removeEventListener('click', closeAddToPlaylistMenu)
  }

  const openAddToPlaylistMenu = (target: Song | Song[], event: MouseEvent) => {
    pendingSongs.value = Array.isArray(target) ? target : [target]
    addMenuPosition.value = {
      left: `${event.clientX}px`,
      top: `${event.clientY}px`
    }
    showAddToPlaylistMenu.value = true
    closeSongMenu()

    // 延迟注册，避免触发弹层的本次点击立即把它关掉
    setTimeout(() => {
      document.addEventListener('click', closeAddToPlaylistMenu)
    }, 10)
  }

  // 点选已有歌单后直接加入（已在歌单内的歌曲自动跳过）
  const addToPlaylist = async (playlistId: string) => {
    const songsToAdd = pendingSongs.value
    if (songsToAdd.length === 0) return

    const playlist = playlists.value.find(p => p.id === playlistId)
    if (!playlist) return

    const newIds = songsToAdd.map(s => s.id).filter(id => !playlist.songs.includes(id))
    if (newIds.length === 0) {
      showInfo(`歌曲已全部在歌单 "${playlist.name}" 中`)
    } else {
      // 创建新数组而非原地 push 响应式代理数组，避免响应式代理与 localforage 序列化交互的潜在陷阱
      const newSongs = [...playlist.songs, ...newIds]
      await localStorageService.updatePlaylist(playlist.id, { songs: newSongs })
      // 重新同步 playlists.value，确保内存状态与 IndexedDB 一致（与 createPlaylistAndAdd/createPlaylist 保持一致）
      playlists.value = await localStorageService.getPlaylists()
      const skipped = songsToAdd.length - newIds.length
      showSuccess(
        skipped > 0
          ? `已添加 ${newIds.length} 首到歌单 "${playlist.name}"，跳过已存在的 ${skipped} 首`
          : `已添加 ${newIds.length} 首到歌单 "${playlist.name}"`
      )
    }
    closeAddToPlaylistMenu()
  }

  // 弹层内"新建歌单"：取名 → 创建 → 自动把当前歌曲加入
  const createPlaylistAndAdd = async () => {
    const songsToAdd = pendingSongs.value
    if (songsToAdd.length === 0) return

    const name = prompt('请输入歌单名称:')
    if (!name || !name.trim()) return

    try {
      const newPlaylist = await localStorageService.createPlaylist(name.trim())
      newPlaylist.songs.push(...songsToAdd.map(s => s.id))
      await localStorageService.updatePlaylist(newPlaylist.id, { songs: newPlaylist.songs })
      playlists.value = await localStorageService.getPlaylists()
      showSuccess(`已创建歌单 "${name.trim()}" 并添加 ${songsToAdd.length} 首歌曲`)
      closeAddToPlaylistMenu()
    } catch (error) {
      logError('创建歌单并添加歌曲失败:', error)
      showError('创建歌单失败，请重试')
    }
  }

  // 从曲库列表移除歌曲（无确认），供批量删除复用，避免逐首弹确认框
  const removeSongFromLibrary = (song: Song) => {
    const index = songs.value.findIndex(s => s.id === song.id)
    if (index !== -1) {
      songs.value.splice(index, 1)
    }
  }

  const deleteSong = (song: Song) => {
    if (confirmAction('确定要删除这首歌吗？')) {
      removeSongFromLibrary(song)
    }
    closeSongMenu()
  }

  // 侧边栏"创建歌单"：返回新建的歌单，供容器跳转进入
  const createPlaylist = async (): Promise<Playlist | undefined> => {
    const name = prompt('请输入歌单名称:')
    if (name && name.trim()) {
      try {
        const newPlaylist = await localStorageService.createPlaylist(name.trim())
        playlists.value = await localStorageService.getPlaylists()
        showSuccess(`歌单 "${name.trim()}" 创建成功`)
        return newPlaylist
      } catch (error) {
        logError('创建歌单失败:', error)
        showError('创建歌单失败')
      }
    }
    return undefined
  }

  // 卸载时清理可能残留的全局监听
  onUnmounted(() => {
    document.removeEventListener('click', closeAddToPlaylistMenu)
  })

  return {
    toggleFavorite,
    showAddToPlaylistMenu,
    addMenuPosition,
    openAddToPlaylistMenu,
    addToPlaylist,
    createPlaylistAndAdd,
    removeSongFromLibrary,
    deleteSong,
    createPlaylist
  }
}
