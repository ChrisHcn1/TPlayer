<script setup lang="ts">
/**
 * 搜索结果（/search?q=）：标题栏提交或 Ctrl+K 无命中回车后进入。
 * 展示命中曲目；专辑 / 艺术家 / 歌单的直达在命令浮层内完成。
 */
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useLibraryStore } from '../stores/library'
import { useTrackSelection } from '../composables/useTrackSelection'
import type { PlayContext } from '../types'
import ListToolbar from '../components/library/ListToolbar.vue'
import TrackList from '../components/library/TrackList.vue'
import SelectionBar from '../components/library/SelectionBar.vue'
import EmptyState from '../components/common/EmptyState.vue'
import { t } from '../i18n'

const route = useRoute()
const router = useRouter()
const libraryStore = useLibraryStore()

const keyword = ref(String(route.query.q ?? ''))

watch(
  () => route.query.q,
  (q) => {
    keyword.value = String(q ?? '')
  },
)

watch(keyword, (value) => {
  router.replace({ name: 'search', query: value.trim() ? { q: value.trim() } : {} })
})

const displayedTracks = computed(() => {
  const q = keyword.value.trim()
  if (!q) return []
  return libraryStore.filterTracks(libraryStore.tracks, q)
})

const buildContext = (): PlayContext => ({
  source: 'search',
  sourceId: null,
  trackIds: displayedTracks.value.map((track) => track.id),
})

const { selectedIds, count, allFavorited, clear, play, favorite, remove, addToPlaylist } =
  useTrackSelection(displayedTracks, buildContext)
</script>

<template>
  <section class="view">
    <ListToolbar
      :title="t('search.title')"
      :subtitle="
        keyword.trim()
          ? t('search.resultSubtitle', { keyword: keyword.trim(), count: displayedTracks.length })
          : t('search.promptSubtitle')
      "
      v-model:keyword="keyword"
      :filter-placeholder="t('search.filterPlaceholder')"
    />

    <EmptyState
      v-if="!keyword.trim()"
      icon="search"
      :title="t('search.emptyTitle')"
      :description="t('search.emptyDescription')"
    />
    <EmptyState
      v-else-if="displayedTracks.length === 0"
      icon="search"
      :title="t('common.noMatch')"
      :description="t('search.noMatchDescription')"
    />
    <template v-else>
      <TrackList
        :tracks="displayedTracks"
        source="search"
        v-model:selected-ids="selectedIds"
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
