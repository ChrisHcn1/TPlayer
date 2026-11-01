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
import { t } from '../i18n'

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
      title: t('playlists.create'),
      placeholder: t('playlists.namePlaceholder'),
      validate: (value) => (value.trim().length === 0 ? t('playlists.nameRequired') : null),
    })
    if (!name) return
    await playlistStore.create(name)
    toastStore.success(t('playlists.created', { name }))
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
    toastStore.warning(t('common.playlistEmpty'))
    return
  }
  playTracks(tracks, {
    playContext: { source: 'playlist', sourceId: playlist.id, trackIds: playlist.trackIds },
  })
}

async function rename(playlist: Playlist): Promise<void> {
  const name = await dialogStore.prompt({
    title: t('playlists.rename'),
    initialValue: playlist.name,
    placeholder: t('playlists.namePlaceholder'),
    validate: (value) => (value.trim().length === 0 ? t('playlists.nameRequired') : null),
  })
  if (!name || name === playlist.name) return
  await playlistStore.rename(playlist.id, name)
  toastStore.success(t('playlists.renamed'))
}

async function remove(playlist: Playlist): Promise<void> {
  const confirmed = await dialogStore.confirm({
    title: t('playlists.delete'),
    message: t('playlists.deleteMessage', { name: playlist.name }),
    confirmText: t('common.delete'),
    danger: true,
  })
  if (!confirmed) return
  await playlistStore.remove(playlist.id)
  toastStore.success(t('playlists.deleted'))
}

function onContextMenu(playlist: Playlist, event: MouseEvent): void {
  const items: MenuItem[] = [
    { key: 'play', label: t('playlists.play'), icon: 'play', onClick: () => play(playlist) },
  ]
  if (!playlist.isSystem) {
    items.push(
      { key: 'rename', label: t('playlists.renameMenu'), icon: 'edit', onClick: () => void rename(playlist) },
      { divider: true, key: 'd1' },
      {
        key: 'delete',
        label: t('playlists.delete'),
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
    <ListToolbar
      :title="t('nav.playlists')"
      :subtitle="t('common.playlistCount', { count: playlistStore.playlists.length })"
      :show-filter="false"
    >
      <template #actions>
        <BaseButton size="sm" variant="primary" @click="createPlaylist">
          <Icon name="plus" :size="14" />
          {{ t('playlists.create') }}
        </BaseButton>
      </template>
    </ListToolbar>

    <EmptyState
      v-if="playlistStore.playlists.length === 0"
      icon="list-music"
      :title="t('playlists.emptyTitle')"
      :description="t('playlists.emptyDescription')"
    />
    <div v-else class="media-grid" role="list">
      <MediaCard
        v-for="playlist in playlistStore.playlists"
        :key="playlist.id"
        variant="playlist"
        :title="playlist.name"
        :subtitle="t('common.trackCount', { count: playlist.trackIds.length })"
        :badge="playlist.isSystem ? t('playlists.systemBadge') : ''"
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
