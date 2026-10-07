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
      title="全部歌曲"
      :subtitle="`${displayedTracks.length} 首`"
      v-model:keyword="keyword"
      filter-placeholder="筛选标题 / 艺术家 / 专辑"
    >
      <template #actions>
        <BaseButton size="sm" variant="primary" :disabled="displayedTracks.length === 0" @click="playAll">
          <Icon name="play" :size="14" />
          播放全部
        </BaseButton>
      </template>
    </ListToolbar>

    <EmptyState
      v-if="libraryStore.isEmpty"
      icon="music-off"
      title="曲库还是空的"
      description="在设置中添加音乐目录后点击“重新扫描”，曲目会出现在这里"
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
