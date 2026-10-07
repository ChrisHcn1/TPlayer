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
    <ListToolbar title="艺术家" :subtitle="`${artists.length} 位`" v-model:keyword="keyword" />

    <EmptyState
      v-if="artists.length === 0"
      icon="user"
      title="暂无艺术家"
      description="扫描曲库后，按艺术家标签聚合的结果会出现在这里"
    />
    <div v-else class="media-grid" role="list">
      <MediaCard
        v-for="artist in artists"
        :key="artist.id"
        variant="artist"
        :title="artist.name"
        :subtitle="`${artist.trackCount} 首 · ${artist.albumCount} 张专辑`"
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
