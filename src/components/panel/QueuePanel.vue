<script setup lang="ts">
/**
 * 右面板 · 播放队列页签
 *
 * 渲染 player store 的 currentContext.trackIds；当前曲目高亮；
 * 双击某行以同一上下文从该曲目开始播放（playFromContext）。
 */
import { computed } from 'vue'
import { storeToRefs } from 'pinia'

import { usePlayerStore } from '../../stores/player'
import { useLibraryStore } from '../../stores/library'
import Icon from '../common/Icon.vue'

const playerStore = usePlayerStore()
const libraryStore = useLibraryStore()
const { snapshot, currentContext, queueTrackIds, currentIndex } = storeToRefs(playerStore)

interface QueueRow {
  id: string
  title: string
  artist: string
  missing: boolean
}

const rows = computed<QueueRow[]>(() =>
  queueTrackIds.value.map((id) => {
    const track = libraryStore.getTrack(id)
    return {
      id,
      title: track?.title ?? '未知曲目',
      artist: track ? track.artists.join(' / ') : '曲目缺失',
      missing: !track,
    }
  }),
)

const contextName = computed(() => {
  switch (currentContext.value?.source) {
    case 'album':
      return '专辑'
    case 'artist':
      return '艺术家'
    case 'playlist':
      return '歌单'
    case 'search':
      return '搜索结果'
    default:
      return '曲库'
  }
})

function playAt(rowId: string): void {
  if (!currentContext.value) return
  void playerStore.playFromContext(rowId, { ...currentContext.value })
}
</script>

<template>
  <div class="queue-panel">
    <div class="queue-panel__head">
      <span class="queue-panel__name">{{ contextName }}队列</span>
      <span class="queue-panel__count">{{ rows.length }} 首</span>
    </div>

    <div v-if="rows.length === 0" class="queue-panel__empty">
      <Icon name="queue" :size="26" />
      <p>队列为空</p>
      <p class="queue-panel__empty-hint">从曲库中播放曲目以建立队列</p>
    </div>

    <ul v-else class="queue-panel__list">
      <li v-for="(row, index) in rows" :key="`${row.id}-${index}`">
        <button
          type="button"
          class="queue-row"
          :class="{ 'queue-row--current': row.id === snapshot.trackId, 'queue-row--missing': row.missing }"
          :title="`播放：${row.title}`"
          @dblclick="playAt(row.id)"
        >
          <span class="queue-row__index">
            <Icon v-if="row.id === snapshot.trackId && snapshot.status === 'playing'" name="loader" :size="13" spin />
            <template v-else>{{ index + 1 }}</template>
          </span>
          <span class="queue-row__text">
            <span class="queue-row__title">{{ row.title }}</span>
            <span class="queue-row__artist">{{ row.artist }}</span>
          </span>
          <span
            v-if="index === currentIndex"
            class="queue-row__badge"
            aria-label="当前播放"
          ></span>
        </button>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.queue-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.queue-panel__head {
  flex: 0 0 auto;
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  padding: var(--space-2) var(--space-3) var(--space-3);
}

.queue-panel__name {
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
}

.queue-panel__count {
  font-size: var(--font-size-2xs, 11px);
  color: var(--color-text-tertiary);
}

.queue-panel__list {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  margin: 0;
  padding: 0 var(--space-2) var(--space-3);
  list-style: none;
  scrollbar-width: thin;
}

.queue-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-1) var(--space-2);
  text-align: left;
  background: transparent;
  border: none;
  border-left: 3px solid transparent;
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.queue-row:hover {
  background-color: var(--color-bg-hover);
}

.queue-row--current {
  border-left-color: var(--color-brand);
  background-color: var(--color-bg-selected);
}

.queue-row__index {
  flex: 0 0 var(--size-control-h);
  width: var(--size-control-h);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

.queue-row--current .queue-row__index {
  color: var(--color-brand);
}

.queue-row__text {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.queue-row__title {
  overflow: hidden;
  font-size: var(--font-size-xs);
  color: var(--color-text-primary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.queue-row--current .queue-row__title {
  color: var(--color-brand);
}

.queue-row__artist {
  overflow: hidden;
  font-size: var(--font-size-2xs, 11px);
  color: var(--color-text-tertiary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.queue-row--missing .queue-row__title,
.queue-row--missing .queue-row__artist {
  color: var(--color-text-disabled);
}

.queue-row__badge {
  flex: 0 0 auto;
  width: 6px;
  height: 6px;
  background-color: var(--color-brand);
  border-radius: var(--radius-full);
}

.queue-panel__empty {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-6) var(--space-3);
  color: var(--color-text-tertiary);
  font-size: var(--font-size-xs);
  text-align: center;
}

.queue-panel__empty-hint {
  margin: 0;
  font-size: var(--font-size-2xs, 11px);
  color: var(--color-text-disabled);
}
</style>
