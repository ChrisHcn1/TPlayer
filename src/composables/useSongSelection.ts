import { ref, type Ref } from 'vue'
import type { Song } from '../types/song'

type LogFn = (...args: any[]) => void

interface UseSongSelectionOptions {
  // 当前视图可见歌曲（筛选+搜索后），批量操作作用于其中被勾选的部分
  visibleSongs: Ref<Song[]>
  // 行点击在非选择模式下的默认动作（播放）
  playSong: (song: Song) => void | Promise<void>
  // 批量"加入歌单"：由库管理 composable 提供点选弹层
  openAddToPlaylistMenu: (target: Song | Song[], event: MouseEvent) => void
  // 单曲删除（含确认对话框），批量删除逐首复用
  deleteSong: (song: Song) => void | Promise<void>
  logInfo: LogFn
  logError: LogFn
}

// 歌曲列表多选：勾选集合、选择模式、Ctrl/点击行交互、批量播放/加歌单/删除。
// 选择状态自包含；播放与持久化动作经注入回调委托给容器。
export function useSongSelection(options: UseSongSelectionOptions) {
  const { visibleSongs, playSong, openAddToPlaylistMenu, deleteSong, logInfo, logError } = options

  const selectedSongIds = ref<Set<string>>(new Set())
  const isSelectionMode = ref(false)

  // 歌曲行点击处理
  const handleSongRowClick = (song: Song, event: MouseEvent) => {
    if (event.ctrlKey || event.metaKey) {
      // Ctrl/Cmd + 点击：选择歌曲
      toggleSongSelection(song)
    } else if (isSelectionMode.value || selectedSongIds.value.size > 0) {
      // 选择模式下：切换选择状态
      toggleSongSelection(song)
    } else {
      // 默认：播放歌曲
      playSong(song)
    }
  }

  const toggleSongSelection = (song: Song) => {
    const newSet = new Set(selectedSongIds.value)
    if (newSet.has(song.id)) {
      newSet.delete(song.id)
    } else {
      newSet.add(song.id)
    }
    selectedSongIds.value = newSet
  }

  const isSongSelected = (song: Song) => {
    return selectedSongIds.value.has(song.id)
  }

  const clearSelection = () => {
    selectedSongIds.value.clear()
    selectedSongIds.value = new Set()
    isSelectionMode.value = false
  }

  const addSelectedToPlaylist = (event: MouseEvent) => {
    const selectedSongs = visibleSongs.value.filter((s: Song) => selectedSongIds.value.has(s.id))
    if (selectedSongs.length === 0) return
    // 打开同一个点选弹层，一次选择目标歌单，不再逐首询问
    openAddToPlaylistMenu(selectedSongs, event)
    logInfo('[批量操作] 打开加入歌单弹层，共', selectedSongs.length, '首歌曲')
  }

  const playSelectedSongs = () => {
    const selectedSongsList = visibleSongs.value.filter((s: Song) => selectedSongIds.value.has(s.id))
    if (selectedSongsList.length > 0) {
      playSong(selectedSongsList[0])
    }
  }

  const deleteSelectedSongs = async () => {
    const selectedSongs = visibleSongs.value.filter((s: Song) => selectedSongIds.value.has(s.id))
    if (selectedSongs.length === 0) return

    const confirmed = confirm(`确定要删除选中的 ${selectedSongs.length} 首歌曲吗？此操作不可撤销。`)
    if (!confirmed) return

    for (const song of selectedSongs) {
      try {
        await deleteSong(song)
      } catch (e) {
        logError('删除歌曲失败:', song.title, e)
      }
    }
    logInfo('[批量操作] 已删除', selectedSongs.length, '首歌曲')
    clearSelection()
  }

  return {
    selectedSongIds,
    isSelectionMode,
    handleSongRowClick,
    toggleSongSelection,
    isSongSelected,
    clearSelection,
    addSelectedToPlaylist,
    playSelectedSongs,
    deleteSelectedSongs
  }
}
