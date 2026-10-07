<script setup lang="ts">
/**
 * 分轨专辑（/cue）：CUE 整盘（演出 / 合辑 / 有声书）网格。
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

const cueAlbums = computed(() => {
  const key = keyword.value.trim().toLowerCase()
  if (!key) return libraryStore.cueAlbums
  return libraryStore.cueAlbums.filter((album) =>
    `${album.name} ${album.artists.join(' ')}`.toLowerCase().includes(key),
  )
})

function open(album: Album): void {
  router.push(`/cue/${album.id}`)
}

function play(album: Album): void {
  const tracks = libraryStore.tracksOfAlbum(album.id)
  playTracks(tracks, {
    playContext: { source: 'album', sourceId: album.id, trackIds: tracks.map((t) => t.id) },
  })
}

function onContextMenu(album: Album, event: MouseEvent): void {
  const tracks = libraryStore.tracksOfAlbum(album.id)
  openTrackMenu(event, tracks, {
    playContext: { source: 'album', sourceId: album.id, trackIds: tracks.map((t) => t.id) },
  })
}
</script>

<template>
  <section class="view">
    <ListToolbar title="分轨专辑" :subtitle="`${cueAlbums.length} 张`" v-model:keyword="keyword" />

    <EmptyState
      v-if="cueAlbums.length === 0"
      icon="layers"
      title="暂无分轨专辑"
      description="带 CUE 索引的整盘音频（演出、合辑、有声书）会出现在这里"
    />
    <div v-else class="media-grid" role="list">
      <MediaCard
        v-for="album in cueAlbums"
        :key="album.id"
        variant="cue"
        :title="album.name"
        :subtitle="album.artists.join(' / ')"
        :cover-key="album.id"
        badge="CUE"
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
