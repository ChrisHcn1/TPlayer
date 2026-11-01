<script setup lang="ts">
/**
 * 全部歌曲（/songs）
 *
 * 形态（DESIGN §2.4 图 2-4-1）：工具条（播放全部 / 筛选）+ 全量曲目表
 * （sticky 表头、多选、排序、双击播放、右键动作），多选时底部浮 SelectionBar。
 */
import { computed, ref } from 'vue'

import { useLibraryStore } from '../stores/library'
import { useTrackActions } from '../composables/useTrackActions'
import { useTrackSelection } from '../composables/useTrackSelection'
import ListToolbar from '../components/library/ListToolbar.vue'
import TrackList from '../components/library/TrackList.vue'
import SelectionBar from '../components/library/SelectionBar.vue'
import BaseButton from '../components/common/BaseButton.vue'
import Icon from '../components/common/Icon.vue'
import EmptyState from '../components/common/EmptyState.vue'
import { t } from '../i18n'

const libraryStore = useLibraryStore()
const { playTracks } = useTrackActions()

const keyword = ref('')

const displayedTracks = computed(() => libraryStore.filterTracks(libraryStore.tracks, keyword.value))

const context = () => ({
  source: 'library' as const,
  sourceId: null,
  trackIds: displayedTracks.value.map((track) => track.id),
})

const {
  selectedIds,
  count,
  allFavorited,
  clear,
  play,
  favorite,
  remove,
  addToPlaylist,
} = useTrackSelection(displayedTracks, context)

function playAll(): void {
  playTracks(displayedTracks.value, { playContext: context() })
}
</script>

<template>
  <section class="view">
    <ListToolbar
      :title="t('nav.songs')"
      :subtitle="t('common.trackCount', { count: displayedTracks.length })"
      v-model:keyword="keyword"
      :filter-placeholder="t('songs.filterPlaceholder')"
    >
      <template #actions>
        <BaseButton size="sm" variant="primary" :disabled="displayedTracks.length === 0" @click="playAll">
          <Icon name="play" :size="14" />
          {{ t('common.playAll') }}
        </BaseButton>
      </template>
    </ListToolbar>

    <EmptyState
      v-if="libraryStore.isEmpty"
      icon="music-off"
      :title="t('songs.emptyTitle')"
      :description="t('songs.emptyDescription')"
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
        @play="play"
        @add-playlist="addToPlaylist"
        @favorite="favorite"
        @remove="remove"
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
