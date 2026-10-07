/**
 * 曲目批量动作（DESIGN §5.5 右键动作 / §5.7 统一确认流）
 *
 * 各视图、队列、卡片共用同一套右键菜单构造：播放 / 收藏 / 加入歌单 /
 * 从歌单移除 / 从曲库移除 / 跳转专辑 / 跳转艺术家。
 */
import { useRouter } from 'vue-router'

import type { PlayContext, Track } from '../types'
import { useContextMenu, type MenuItem } from './useContextMenu'
import { useLibraryStore } from '../stores/library'
import { usePlaylistStore } from '../stores/playlist'
import { usePlayerStore } from '../stores/player'
import { useDialogStore } from '../stores/dialog'
import { useToastStore } from '../stores/toast'

export interface TrackMenuOptions {
  /** “播放”使用的列表上下文；缺省时按传入曲目自身构成 library 上下文 */
  playContext?: PlayContext
  /** 歌单详情页内传入：出现“从歌单移除” */
  playlistId?: string
}

export function useTrackActions() {
  const router = useRouter()
  const libraryStore = useLibraryStore()
  const playlistStore = usePlaylistStore()
  const playerStore = usePlayerStore()
  const dialogStore = useDialogStore()
  const toastStore = useToastStore()
  const { open } = useContextMenu()

  function playTracks(tracks: Track[], options: TrackMenuOptions): void {
    if (tracks.length === 0) return
    const context: PlayContext = options.playContext ?? {
      source: 'library',
      sourceId: null,
      trackIds: tracks.map((track) => track.id),
    }
    void playerStore.playFromContext(tracks[0]!.id, context)
  }

  function buildAddToPlaylistItems(tracks: Track[]): MenuItem[] {
    const items: MenuItem[] = playlistStore.playlists.map((playlist) => ({
      key: `pl-${playlist.id}`,
      label: playlist.name,
      icon: 'list-music',
      onClick: () => {
        void playlistStore.addTracks(playlist.id, tracks.map((track) => track.id))
        toastStore.success(`已添加 ${tracks.length} 首到「${playlist.name}」`)
      },
    }))
    items.push({ divider: true, key: 'pl-divider' })
    items.push({
      key: 'pl-new',
      label: '新建歌单…',
      icon: 'plus',
      onClick: async () => {
        const name = await dialogStore.prompt({
          title: '新建歌单',
          placeholder: '歌单名称',
          validate: (value) => (value.length === 0 ? '请输入歌单名称' : null),
        })
        if (!name) return
        await playlistStore.create(name)
        const created = playlistStore.current
        if (created) {
          await playlistStore.addTracks(created.id, tracks.map((track) => track.id))
          toastStore.success(`已创建「${name}」并加入 ${tracks.length} 首`)
        }
      },
    })
    return items
  }

  async function removeFromLibrary(tracks: Track[]): Promise<void> {
    const confirmed = await dialogStore.confirm({
      title: '从曲库移除',
      message:
        tracks.length === 1
          ? `确定从曲库移除「${tracks[0]?.title ?? ''}」吗？不会删除磁盘文件。`
          : `确定从曲库移除选中的 ${tracks.length} 首吗？不会删除磁盘文件。`,
      confirmText: '移除',
      danger: true,
    })
    if (!confirmed) return
    await libraryStore.removeTracks(tracks.map((track) => track.id))
    toastStore.success(`已移除 ${tracks.length} 首`)
  }

  /** 打开曲目右键菜单；tracks 为当前应作用的曲目集合（单选或多选） */
  function openTrackMenu(event: MouseEvent, tracks: Track[], options: TrackMenuOptions = {}): void {
    if (tracks.length === 0) return
    const ids = new Set(tracks.map((track) => track.id))
    const allFavorited = tracks.every((track) => libraryStore.isFavorite(track.id))
    const title = tracks.length > 1 ? `已选 ${tracks.length} 首` : tracks[0]?.title

    const items: MenuItem[] = [
      {
        key: 'play',
        label: '立即播放',
        icon: 'play',
        onClick: () => playTracks(tracks, options),
      },
      {
        key: 'favorite',
        label: allFavorited ? '取消收藏' : '收藏',
        icon: allFavorited ? 'heart-off' : 'heart',
        onClick: () => void toggleFavorite(tracks),
      },
      {
        key: 'add-playlist',
        label: '添加到歌单',
        icon: 'list-plus',
        children: buildAddToPlaylistItems(tracks),
      },
    ]

    if (options.playlistId) {
      items.push({
        key: 'remove-from-playlist',
        label: '从歌单移除',
        icon: 'list-minus',
        onClick: () => {
          const playlistId = options.playlistId as string
          for (const id of ids) void playlistStore.removeTrack(playlistId, id)
          toastStore.success(`已从当前歌单移除 ${tracks.length} 首`)
        },
      })
    }

    items.push({ divider: true, key: 'd1' })

    if (tracks.length === 1) {
      const track = tracks[0] as Track
      items.push({
        key: 'goto-album',
        label: '转到专辑',
        icon: 'disc',
        onClick: () => {
          if (!track.albumId) return
          const cueIds = new Set(libraryStore.cueAlbums.map((album) => album.id))
          if (cueIds.has(track.albumId)) router.push(`/cue/${track.albumId}`)
          else router.push(`/albums/${track.albumId}`)
        },
      })
      const artist = track.artists[0]
      if (artist) {
        items.push({
          key: 'goto-artist',
          label: '转到艺术家',
          icon: 'user',
          onClick: () => router.push(`/artists/${encodeURIComponent(artist)}`),
        })
      }
      items.push({ divider: true, key: 'd2' })
    }

    items.push({
      key: 'remove-library',
      label: tracks.length === 1 ? '从曲库移除' : `从曲库移除（${tracks.length} 首）`,
      icon: 'trash-2',
      danger: true,
      onClick: () => void removeFromLibrary(tracks),
    })

    open(event, items, title)
  }

  /** 批量收藏/取消收藏：以第一首当前状态为准统一取反 */
  async function toggleFavorite(tracks: Track[]): Promise<void> {
    if (tracks.length === 0) return
    const favorited = libraryStore.isFavorite(tracks[0]!.id)
    for (const track of tracks) {
      if (favorited === libraryStore.isFavorite(track.id)) {
        await libraryStore.toggleFavorite(track.id)
      }
    }
  }

  /** 在指定锚点（按钮）打开“添加到歌单”菜单（SelectionBar 使用） */
  function openAddToPlaylistMenu(event: MouseEvent, tracks: Track[]): void {
    open(event, buildAddToPlaylistItems(tracks), `加入歌单（${tracks.length} 首）`)
  }

  return {
    openTrackMenu,
    playTracks,
    toggleFavorite,
    removeFromLibrary,
    openAddToPlaylistMenu,
  }
}
