<script setup lang="ts">
/**
 * 最近播放（/recent）：按最近播放时间倒序，上限 200（library store 已去重失效 id）。
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
const { playTracks } = useTrackActions()

const keyword = ref('')
const displayedTracks = computed(() =>
  libraryStore.filterTracks(libraryStore.recentTracks, keyword.value),
)

const buildContext = (): PlayContext => ({
  source: 'library',
  sourceId: null,
  trackIds: displayedTracks.value.map((track) => track.id),
})

const { selectedIds, count, clear, play, favorite, addToPlaylist } = useTrackSelection(
  displayedTracks,
  buildContext,
)

function playAll(): void {
  playTracks(displayedTracks.value, { playContext: buildContext() })
}
</script>

<template>
  <section class="view">
    <ListToolbar
      title="最近播放"
      :subtitle="`${displayedTracks.length} 首`"
      v-model:keyword="keyword"
      filter-placeholder="筛选最近播放"
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
      icon="clock"
      title="还没有播放记录"
      description="播放过的曲目会按时间倒序出现在这里"
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
        :show-remove="false"
        @play="play"
        @add-playlist="addToPlaylist"
        @favorite="favorite"
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
