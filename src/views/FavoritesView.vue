<script setup lang="ts">
/**
 * 我的收藏（/favorites）：收藏曲目列表；批量“移除”语义为取消收藏。
 */
import { computed, ref } from 'vue'

import { useLibraryStore } from '../stores/library'
import { useTrackActions } from '../composables/useTrackActions'
import { useTrackSelection } from '../composables/useTrackSelection'
import type { PlayContext } from '../types'
import ListToolbar from '../components/library/ListToolbar.vue'
import TrackList from '../components/library/TrackList.vue'
import SelectionBar from '../components/library/SelectionBar.vue'
import BaseButton from '../components/common/BaseButton.vue'
import Icon from '../components/common/Icon.vue'
import EmptyState from '../components/common/EmptyState.vue'

const libraryStore = useLibraryStore()
const { playTracks, toggleFavorite } = useTrackActions()

const keyword = ref('')
const displayedTracks = computed(() =>
  libraryStore.filterTracks(libraryStore.favoriteTracks, keyword.value),
)

const buildContext = (): PlayContext => ({
  source: 'library',
  sourceId: null,
  trackIds: displayedTracks.value.map((track) => track.id),
})

const { selectedIds, count, allFavorited, clear, play, favorite, addToPlaylist } = useTrackSelection(
  displayedTracks,
  buildContext,
)

/** 收藏页的“移除”= 取消收藏 */
async function unfavorite(): Promise<void> {
  const ids = new Set(selectedIds.value)
  const targets = displayedTracks.value.filter((track) => ids.has(track.id))
  await toggleFavorite(targets)
  clear()
}

function playAll(): void {
  playTracks(displayedTracks.value, { playContext: buildContext() })
}
</script>

<template>
  <section class="view">
    <ListToolbar
      title="我的收藏"
      :subtitle="`${displayedTracks.length} 首`"
      v-model:keyword="keyword"
      filter-placeholder="筛选已收藏曲目"
    >
      <template #actions>
        <BaseButton size="sm" variant="primary" :disabled="displayedTracks.length === 0" @click="playAll">
          <Icon name="play" :size="14" />
          播放全部
        </BaseButton>
      </template>
    </ListToolbar>

    <EmptyState
      v-if="displayedTracks.length === 0"
      icon="heart"
      title="还没有收藏"
      description="点击曲目旁的心形或使用右键菜单，把喜欢的歌收进来"
    />
    <template v-else>
      <TrackList
        :tracks="displayedTracks"
        source="library"
        v-model:selected-ids="selectedIds"
        empty-title="没有匹配的曲目"
      />
      <SelectionBar
        :count="count"
        :all-favorited="allFavorited"
        remove-title="取消收藏"
        @play="play"
        @add-playlist="addToPlaylist"
        @favorite="favorite"
        @remove="unfavorite"
        @clear="clear"
      />
    </template>
  </section>
</template>

<style scoped>
.view {
  display: flex;
  flex-direction: column;
  min-height: 100%;
  color: var(--color-text-primary);
}
</style>
