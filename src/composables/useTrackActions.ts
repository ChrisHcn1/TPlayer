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
import { useLabStore } from '../stores/lab'
import { useMetaRetryStore } from '../stores/metaRetry'
import { t } from '../i18n'

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
  const labStore = useLabStore()
  const metaRetryStore = useMetaRetryStore()
  const { open } = useContextMenu()

  function playTracks(tracks: Track[], options: TrackMenuOptions): void {
    if (tracks.length === 0) return
    const context: PlayContext = options.playContext ?? {
      source: 'library',
      sourceId: null,
      trackIds: tracks.map((track) => track.id),
    }
    // 随机播放模式 + 多首曲目：从队列中挑一首随机曲目作为起点，
    // 后端再用 Fisher-Yates 洗牌排列后续曲目（一轮内每首各播一次）。
    // 单首曲目（双击某行 / 菜单「播放」单曲）不随机，严格播用户点的那首。
    const shuffle = playerStore.snapshot.mode === 'shuffle'
    const startIndex = shuffle && tracks.length > 1
      ? Math.floor(Math.random() * tracks.length)
      : 0
    void playerStore.playFromContext(tracks[startIndex]!.id, context)
  }

  function buildAddToPlaylistItems(tracks: Track[]): MenuItem[] {
    const items: MenuItem[] = playlistStore.playlists.map((playlist) => ({
      key: `pl-${playlist.id}`,
      label: playlist.name,
      icon: 'list-music',
      onClick: () => {
        void playlistStore.addTracks(playlist.id, tracks.map((track) => track.id))
        toastStore.success(t('actions.addedToPlaylist', { count: tracks.length, name: playlist.name }))
      },
    }))
    items.push({ divider: true, key: 'pl-divider' })
    items.push({
      key: 'pl-new',
      label: t('actions.newPlaylist'),
      icon: 'plus',
      onClick: async () => {
        const name = await dialogStore.prompt({
          title: t('actions.newPlaylistTitle'),
          placeholder: t('actions.playlistNamePlaceholder'),
          validate: (value) => (value.length === 0 ? t('actions.playlistNameRequired') : null),
        })
        if (!name) return
        await playlistStore.create(name)
        const created = playlistStore.current
        if (created) {
          await playlistStore.addTracks(created.id, tracks.map((track) => track.id))
          toastStore.success(t('actions.playlistCreated', { name, count: tracks.length }))
        }
      },
    })
    return items
  }

  async function removeFromLibrary(tracks: Track[]): Promise<void> {
    const confirmed = await dialogStore.confirm({
      title: t('actions.removeFromLibraryTitle'),
      message:
        tracks.length === 1
          ? t('actions.removeFromLibraryMessageOne', { title: tracks[0]?.title ?? '' })
          : t('actions.removeFromLibraryMessageMany', { count: tracks.length }),
      confirmText: t('common.remove'),
      danger: true,
    })
    if (!confirmed) return
    await libraryStore.removeTracks(tracks.map((track) => track.id))
    toastStore.success(t('actions.removed', { count: tracks.length }))
  }

  /** 打开曲目右键菜单；tracks 为当前应作用的曲目集合（单选或多选） */
  function openTrackMenu(event: MouseEvent, tracks: Track[], options: TrackMenuOptions = {}): void {
    if (tracks.length === 0) return
    const ids = new Set(tracks.map((track) => track.id))
    const allFavorited = tracks.every((track) => libraryStore.isFavorite(track.id))
    // 单曲：标题由菜单信息头承载（完整元数据 / 歌词 / 封面 / 路径），不再重复显示
    const isSingle = tracks.length === 1
    const title = isSingle ? undefined : t('actions.selectedCount', { count: tracks.length })

    const items: MenuItem[] = [
      {
        key: 'play',
        label: t('actions.playNow'),
        icon: 'play',
        onClick: () => playTracks(tracks, options),
      },
      {
        key: 'favorite',
        label: allFavorited ? t('player.unfavorite') : t('player.favorite'),
        icon: allFavorited ? 'heart-off' : 'heart',
        onClick: () => void toggleFavorite(tracks),
      },
      {
        key: 'add-playlist',
        label: t('actions.addToPlaylist'),
        icon: 'list-plus',
        children: buildAddToPlaylistItems(tracks),
      },
    ]

    if (options.playlistId) {
      items.push({
        key: 'remove-from-playlist',
        label: t('actions.removeFromPlaylist'),
        icon: 'list-minus',
        onClick: () => {
          const playlistId = options.playlistId as string
          for (const id of ids) void playlistStore.removeTrack(playlistId, id)
          toastStore.success(t('actions.removedFromPlaylist', { count: tracks.length }))
        },
      })
    }

    items.push({ divider: true, key: 'd1' })

    if (tracks.length === 1) {
      const track = tracks[0] as Track
      items.push({
        key: 'goto-album',
        label: t('actions.gotoAlbum'),
        icon: 'disc',
        onClick: () => {
          if (!track.albumId) return
          const id = track.albumId
          if (libraryStore.cueAlbums.some((album) => album.id === id)) router.push(`/cue/${id}`)
          else if (libraryStore.sacdAlbums.some((album) => album.id === id))
            router.push(`/cue/${id}`)
          else router.push(`/albums/${id}`)
        },
      })
      const artist = track.artists[0]
      if (artist) {
        items.push({
          key: 'goto-artist',
          label: t('actions.gotoArtist'),
          icon: 'user',
          onClick: () => router.push(`/artists/${encodeURIComponent(artist)}`),
        })
      }
      // 实验室·元数据增强：单曲识别（只重新识别这一首，不影响其他曲目）
      if (labStore.isEnabled('enhanced-metadata')) {
        items.push({
          key: 'meta-identify',
          label: t('actions.identifyMetadata'),
          icon: 'refresh',
          onClick: () => void identifyMetadata(track),
        })
      }
      items.push({ divider: true, key: 'd2' })
    }

    items.push({
      key: 'remove-library',
      label: tracks.length === 1 ? t('actions.removeFromLibraryTitle') : t('actions.removeFromLibraryCount', { count: tracks.length }),
      icon: 'trash-2',
      danger: true,
      onClick: () => void removeFromLibrary(tracks),
    })

    open(event, items, title, isSingle ? { trackId: tracks[0]!.id } : undefined)
  }

  /** 单曲识别：只重新解析 / 联网识别这一首曲目（结果记录进识别日志并自动保存） */
  async function identifyMetadata(track: Track): Promise<void> {
    const rec = await metaRetryStore.retry(track.id)
    if (!rec) {
      toastStore.error(t('actions.identifyNotCompleted'))
      return
    }
    if (rec.status === 'ok') {
      const saved =
        rec.savedTo === 'tag'
          ? t('actions.identifyDoneTag', { detail: rec.detail })
          : rec.savedTo === 'sidecar'
            ? t('actions.identifyDoneSidecar', { detail: rec.detail })
            : rec.savedTo === 'none'
              ? rec.saveDetail
                ? t('actions.identifyDoneSaveFailedDetail', { detail: rec.detail, saveDetail: rec.saveDetail })
                : t('actions.identifyDoneSaveFailed', { detail: rec.detail })
              : t('actions.identifyDone', { detail: rec.detail })
      toastStore.success(saved)
    } else if (rec.status === 'failed') toastStore.error(t('actions.identifyFailed', { detail: rec.detail }))
    else toastStore.info(t('actions.identifyDone', { detail: rec.detail }))
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
    open(event, buildAddToPlaylistItems(tracks), t('actions.addToPlaylistMenuTitle', { count: tracks.length }))
  }

  return {
    openTrackMenu,
    playTracks,
    toggleFavorite,
    removeFromLibrary,
    openAddToPlaylistMenu,
  }
}
