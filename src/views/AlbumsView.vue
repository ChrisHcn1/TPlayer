<script setup lang="ts">
/**
 * 专辑（/albums）：普通专辑网格（CUE 分轨专辑在独立页签视图）。
 */
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'

import { useLibraryStore } from '../stores/library'
import { useTrackActions } from '../composables/useTrackActions'
import type { Album } from '../types'
import ListToolbar from '../components/library/ListToolbar.vue'
import MediaCard from '../components/library/MediaCard.vue'
import EmptyState from '../components/common/EmptyState.vue'

const router = useRouter()
const libraryStore = useLibraryStore()
const { openTrackMenu, playTracks } = useTrackActions()

const keyword = ref('')

const albums = computed(() => {
  const key = keyword.value.trim().toLowerCase()
  if (!key) return libraryStore.albums
  return libraryStore.albums.filter((album) =>
    `${album.name} ${album.artists.join(' ')}`.toLowerCase().includes(key),
  )
})

function open(album: Album): void {
  router.push(`/albums/${album.id}`)
}

function play(album: Album): void {
  const tracks = libraryStore.tracksOfAlbum(album.id)
  playTracks(tracks, {
    playContext: { source: 'album', sourceId: album.id, trackIds: tracks.map((t) => t.id) },
  })
}

function onContextMenu(album: Album, event: MouseEvent): void {
  openTrackMenu(event, libraryStore.tracksOfAlbum(album.id), {
    playContext: {
      source: 'album',
      sourceId: album.id,
      trackIds: libraryStore.tracksOfAlbum(album.id).map((t) => t.id),
    },
  })
}
</script>

<template>
  <section class="view">
    <ListToolbar title="专辑" :subtitle="`${albums.length} 张`" v-model:keyword="keyword" />

    <EmptyState
      v-if="albums.length === 0"
      icon="disc"
      title="暂无专辑"
      description="扫描曲库后，按专辑标签聚合的唱片会出现在这里"
    />
    <div v-else class="media-grid" role="list">
      <MediaCard
        v-for="album in albums"
        :key="album.id"
        variant="album"
        :title="album.name"
        :subtitle="album.artists.join(' / ')"
        :cover-key="album.id"
        role="listitem"
        @open="open(album)"
        @play="play(album)"
        @context-menu="onContextMenu(album, $event)"
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
