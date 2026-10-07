<script setup lang="ts">
/**
 * 歌单（/playlists）：用户歌单网格；新建 / 重命名 / 删除均走统一确认流。
 */
import { ref } from 'vue'
import { useRouter } from 'vue-router'

import { usePlaylistStore } from '../stores/playlist'
import { useLibraryStore } from '../stores/library'
import { useDialogStore } from '../stores/dialog'
import { useToastStore } from '../stores/toast'
import { useTrackActions } from '../composables/useTrackActions'
import { useContextMenu, type MenuItem } from '../composables/useContextMenu'
import type { Playlist } from '../types'
import ListToolbar from '../components/library/ListToolbar.vue'
import MediaCard from '../components/library/MediaCard.vue'
import BaseButton from '../components/common/BaseButton.vue'
import Icon from '../components/common/Icon.vue'
import EmptyState from '../components/common/EmptyState.vue'

const router = useRouter()
const playlistStore = usePlaylistStore()
const libraryStore = useLibraryStore()
const dialogStore = useDialogStore()
const toastStore = useToastStore()
const { playTracks } = useTrackActions()
const { openAt } = useContextMenu()

const creating = ref(false)

function tracksOf(playlist: Playlist) {
  return playlist.trackIds
    .map((id) => libraryStore.getTrack(id))
    .filter((track): track is NonNullable<typeof track> => Boolean(track))
}

async function createPlaylist(): Promise<void> {
  if (creating.value) return
  creating.value = true
  try {
    const name = await dialogStore.prompt({
      title: '新建歌单',
      placeholder: '歌单名称',
      validate: (value) => (value.trim().length === 0 ? '请输入歌单名称' : null),
    })
    if (!name) return
    await playlistStore.create(name)
    toastStore.success(`歌单「${name}」已创建`)
  } finally {
    creating.value = false
  }
}

function open(playlist: Playlist): void {
  router.push(`/playlists/${playlist.id}`)
}

function play(playlist: Playlist): void {
  const tracks = tracksOf(playlist)
  if (tracks.length === 0) {
    toastStore.warning('歌单还是空的')
    return
  }
  playTracks(tracks, {
    playContext: { source: 'playlist', sourceId: playlist.id, trackIds: playlist.trackIds },
  })
}

async function rename(playlist: Playlist): Promise<void> {
  const name = await dialogStore.prompt({
    title: '重命名歌单',
    initialValue: playlist.name,
    placeholder: '歌单名称',
    validate: (value) => (value.trim().length === 0 ? '请输入歌单名称' : null),
  })
  if (!name || name === playlist.name) return
  await playlistStore.rename(playlist.id, name)
  toastStore.success('歌单已重命名')
}

async function remove(playlist: Playlist): Promise<void> {
  const confirmed = await dialogStore.confirm({
    title: '删除歌单',
    message: `确定删除歌单「${playlist.name}」吗？歌单内曲目不会从曲库移除。`,
    confirmText: '删除',
    danger: true,
  })
  if (!confirmed) return
  await playlistStore.remove(playlist.id)
  toastStore.success('歌单已删除')
}

function onContextMenu(playlist: Playlist, event: MouseEvent): void {
  const items: MenuItem[] = [
    { key: 'play', label: '播放歌单', icon: 'play', onClick: () => play(playlist) },
  ]
  if (!playlist.isSystem) {
    items.push(
      { key: 'rename', label: '重命名…', icon: 'edit', onClick: () => void rename(playlist) },
      { divider: true, key: 'd1' },
      {
        key: 'delete',
        label: '删除歌单',
        icon: 'trash-2',
        danger: true,
        onClick: () => void remove(playlist),
      },
    )
  }
  openAt(event.clientX, event.clientY, items, playlist.name)
}
</script>

<template>
  <section class="view">
    <ListToolbar title="歌单" :subtitle="`${playlistStore.playlists.length} 个`" :show-filter="false">
      <template #actions>
        <BaseButton size="sm" variant="primary" @click="createPlaylist">
          <Icon name="plus" :size="14" />
          新建歌单
        </BaseButton>
      </template>
    </ListToolbar>

    <EmptyState
      v-if="playlistStore.playlists.length === 0"
      icon="list-music"
      title="还没有歌单"
      description="点击“新建歌单”，或在曲目右键菜单中把歌曲加入歌单"
    />
    <div v-else class="media-grid" role="list">
      <MediaCard
        v-for="playlist in playlistStore.playlists"
        :key="playlist.id"
        variant="playlist"
        :title="playlist.name"
        :subtitle="`${playlist.trackIds.length} 首`"
        :badge="playlist.isSystem ? '系统' : ''"
        role="listitem"
        @open="open(playlist)"
        @play="play(playlist)"
        @context-menu="onContextMenu(playlist, $event)"
      />
    </div>
  </section>
</template>

<style scoped>
.view {
  display: flex;
  flex-direction: column;
  min-height: 100%;
  color: var(--color-text-primary);
}

.media-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: var(--space-2);
  padding: var(--space-2) 0 var(--space-6);
}
</style>
