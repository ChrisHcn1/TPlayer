import { ref, onUnmounted } from 'vue'
import type { Song } from '../types/song'

// 歌曲列表右键菜单：选中歌曲、跟随鼠标定位、点击页面其他处自动关闭。
// document click 监听延迟 10ms 注册，避免触发菜单的本次 contextmenu 同轮事件立即关闭它。
// 菜单项触发的动作（播放/加歌单/收藏/编辑/删除）留在容器，经模板直接调用。
export function useSongContextMenu() {
  const showSongMenu = ref(false)
  const menuPosition = ref({ left: '0px', top: '0px' })
  const selectedSong = ref<Song | null>(null)

  const openSongMenu = (song: Song, event: MouseEvent) => {
    selectedSong.value = song
    menuPosition.value = {
      left: `${event.clientX}px`,
      top: `${event.clientY}px`
    }
    showSongMenu.value = true

    // 点击其他地方关闭菜单
    setTimeout(() => {
      document.addEventListener('click', closeSongMenu)
    }, 10)
  }

  const closeSongMenu = () => {
    showSongMenu.value = false
    document.removeEventListener('click', closeSongMenu)
  }

  // 组件卸载时清理可能残留的全局监听（与原容器 onUnmounted 行为一致）
  onUnmounted(() => {
    document.removeEventListener('click', closeSongMenu)
  })

  return {
    showSongMenu,
    menuPosition,
    selectedSong,
    openSongMenu,
    closeSongMenu
  }
}
