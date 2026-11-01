<script setup lang="ts">
/**
 * 艺术家（/artists）：按艺术家聚合的网格。
 */
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'

import { useLibraryStore } from '../stores/library'
import { useTrackActions } from '../composables/useTrackActions'
import type { Artist } from '../types'
import ListToolbar from '../components/library/ListToolbar.vue'
import MediaCard from '../components/library/MediaCard.vue'
import EmptyState from '../components/common/EmptyState.vue'
import { t } from '../i18n'

const router = useRouter()
const libraryStore = useLibraryStore()
const { openTrackMenu, playTracks } = useTrackActions()

const keyword = ref('')

const artists = computed(() => {
  const key = keyword.value.trim().toLowerCase()
  if (!key) return libraryStore.artists
  return libraryStore.artists.filter((artist) => artist.name.toLowerCase().includes(key))
})

function open(artist: Artist): void {
  router.push(`/artists/${encodeURIComponent(artist.name)}`)
}

function play(artist: Artist): void {
  const tracks = libraryStore.tracksOfArtist(artist.name)
  playTracks(tracks, {
    playContext: { source: 'artist', sourceId: artist.name, trackIds: tracks.map((t) => t.id) },
  })
}

function onContextMenu(artist: Artist, event: MouseEvent): void {
  const tracks = libraryStore.tracksOfArtist(artist.name)
  openTrackMenu(event, tracks, {
    playContext: { source: 'artist', sourceId: artist.name, trackIds: tracks.map((t) => t.id) },
  })
}
</script>

<template>
  <section class="view">
    <ListToolbar
      :title="t('nav.artists')"
      :subtitle="t('common.artistCount', { count: artists.length })"
      v-model:keyword="keyword"
    />

    <EmptyState
      v-if="artists.length === 0"
      icon="user"
      :title="t('artists.emptyTitle')"
      :description="t('artists.emptyDescription')"
    />
    <div v-else class="media-grid" role="list">
      <MediaCard
        v-for="artist in artists"
        :key="artist.id"
        variant="artist"
        :title="artist.name"
        :subtitle="t('common.artistMeta', { tracks: artist.trackCount, albums: artist.albumCount })"
        :cover-key="`artist:${artist.name}`"
        role="listitem"
        @open="open(artist)"
        @play="play(artist)"
        @context-menu="onContextMenu(artist, $event)"
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
