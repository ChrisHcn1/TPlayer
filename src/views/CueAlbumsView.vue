<script setup lang="ts">
/**
 * 分轨专辑（/cue）：CUE 整盘与 SACD ISO 的虚拟多轨专辑统一网格。
 *
 * SACD 分轨已并入本模块（不再单独占一个标签页）：两类专辑在此合并展示，
 * 通过角标（CUE / SACD）与卡片变体区分，点击均进入统一的 /cue/:id 详情。
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

/** CUE 与 SACD 虚拟专辑合并为单一列表（按名称排序，统一入口） */
const albums = computed<Album[]>(() => {
  const list = [...libraryStore.cueAlbums, ...libraryStore.sacdAlbums]
  const key = keyword.value.trim().toLowerCase()
  const filtered = key
    ? list.filter((album) => `${album.name} ${album.artists.join(' ')}`.toLowerCase().includes(key))
    : list
  return [...filtered].sort((a, b) => a.name.localeCompare(b.name))
})

function badgeFor(album: Album): string {
  return album.isSacd ? 'SACD' : 'CUE'
}

function variantFor(album: Album): 'cue' | 'sacd' {
  return album.isSacd ? 'sacd' : 'cue'
}

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
    <ListToolbar title="分轨专辑" :subtitle="`${albums.length} 张`" v-model:keyword="keyword" />

    <EmptyState
      v-if="albums.length === 0"
      icon="layers"
      title="暂无分轨专辑"
      description="带 CUE 索引的整盘音频或 SACD ISO 会被解析为多轨专辑，出现在这里"
    />
    <div v-else class="media-grid" role="list">
      <MediaCard
        v-for="album in albums"
        :key="album.id"
        :variant="variantFor(album)"
        :title="album.name"
        :subtitle="album.artists.join(' / ')"
        :cover-key="album.id"
        :badge="badgeFor(album)"
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
