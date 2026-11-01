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
import { t } from '../i18n'

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
      :title="t('favorites.title')"
      :subtitle="t('common.trackCount', { count: displayedTracks.length })"
      v-model:keyword="keyword"
      :filter-placeholder="t('favorites.filterPlaceholder')"
    >
      <template #actions>
        <BaseButton size="sm" variant="primary" :disabled="displayedTracks.length === 0" @click="playAll">
          <Icon name="play" :size="14" />
          {{ t('common.playAll') }}
        </BaseButton>
      </template>
    </ListToolbar>

    <EmptyState
      v-if="displayedTracks.length === 0"
      icon="heart"
      :title="t('favorites.emptyTitle')"
      :description="t('favorites.emptyDescription')"
    />
    <template v-else>
      <TrackList
        :tracks="displayedTracks"
        source="library"
        v-model:selected-ids="selectedIds"
        :empty-title="t('common.noMatch')"
      />
      <SelectionBar
        :count="count"
        :all-favorited="allFavorited"
        :remove-title="t('common.unfavorite')"
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
