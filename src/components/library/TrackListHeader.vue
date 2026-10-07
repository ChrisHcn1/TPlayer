<script setup lang="ts">
/**
 * 曲目列表表头（DESIGN §4.6）：与 TrackRow 同列网格；sticky 置顶；
 * 全选三态复选框；标题/艺术家/专辑/时长可排序。
 */
import Icon from '../common/Icon.vue'

export type TrackSortField = 'title' | 'artist' | 'album' | 'duration'
export type SortDirection = 'asc' | 'desc'

interface Props {
  showArtist?: boolean
  showAlbum?: boolean
  allSelected: boolean
  partial: boolean
  sortField: TrackSortField | null
  sortDir: SortDirection
  trackCount: number
}

withDefaults(defineProps<Props>(), {
  showArtist: true,
  showAlbum: true,
})

const emit = defineEmits<{
  (e: 'toggle-all'): void
  (e: 'sort', field: TrackSortField): void
}>()

function sortIconFor(field: TrackSortField, sortField: TrackSortField | null, sortDir: SortDirection): string {
  if (field !== sortField) return 'arrow-up-down'
  return sortDir === 'asc' ? 'arrow-up' : 'arrow-down'
}
</script>

<template>
  <div
    class="track-header"
    :class="{ 'track-header--no-artist': !showArtist }"
    role="row"
  >
    <div class="track-header__check">
      <button
        type="button"
        class="track-header__checkbox"
        :class="{
          'track-header__checkbox--checked': allSelected,
          'track-header__checkbox--partial': partial,
        }"
        role="checkbox"
        :aria-checked="allSelected ? 'true' : partial ? 'mixed' : 'false'"
        :aria-label="allSelected ? '取消全选' : '全选'"
        :disabled="trackCount === 0"
        @click="emit('toggle-all')"
      >
        <Icon v-if="allSelected" name="check" :size="12" />
        <span v-else-if="partial" class="track-header__checkbox-dash" aria-hidden="true"></span>
      </button>
    </div>
    <div class="track-header__index">#</div>

    <button type="button" class="track-header__cell track-header__cell--title" @click="emit('sort', 'title')">
      <span>标题</span>
      <Icon :name="sortIconFor('title', sortField, sortDir)" :size="13" />
    </button>

    <button v-if="showArtist" type="button" class="track-header__cell" @click="emit('sort', 'artist')">
      <span>艺术家</span>
      <Icon :name="sortIconFor('artist', sortField, sortDir)" :size="13" />
    </button>

    <button v-if="showAlbum" type="button" class="track-header__cell" @click="emit('sort', 'album')">
      <span>专辑</span>
      <Icon :name="sortIconFor('album', sortField, sortDir)" :size="13" />
    </button>

    <button
      type="button"
      class="track-header__cell track-header__cell--duration"
      @click="emit('sort', 'duration')"
    >
      <Icon :name="sortIconFor('duration', sortField, sortDir)" :size="13" />
    </button>

    <div class="track-header__spacer"></div>
  </div>
</template>

<style scoped>
.track-header {
  position: sticky;
  top: 0;
  /* 独立合成层：避免长列表快速滚动 / 平滑定位时 WebView2 出现行内容残影 */
  z-index: 3;
  transform: translateZ(0);
  display: grid;
  grid-template-columns: 40px 40px minmax(0, 3fr) minmax(0, 2fr) minmax(0, 2fr) 64px 40px;
  align-items: center;
  min-height: var(--size-header-h);
  padding: 0 var(--space-2) 0 var(--space-1);
  background-color: var(--color-bg-surface);
  border-bottom: 1px solid var(--color-border-subtle);
  /* 吸顶时与滚过的曲目行形成明确层次，消除“悬浮 / 缝隙”观感 */
  box-shadow: var(--shadow-elevation-2);
}

.track-header--no-artist {
  grid-template-columns: 40px 40px minmax(0, 3fr) minmax(0, 2fr) 64px 40px;
}

.track-header__check,
.track-header__index {
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

.track-header__checkbox {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  color: transparent;
  background: transparent;
  border: 1px solid var(--color-border-strong);
  border-radius: var(--radius-xs);
}

.track-header__checkbox:disabled {
  opacity: 0.4;
}

.track-header__checkbox--checked {
  color: var(--color-on-brand);
  background-color: var(--color-brand-solid);
  border-color: var(--color-brand-solid);
}

.track-header__checkbox-dash {
  width: 10px;
  height: 2px;
  border-radius: 1px;
  background-color: var(--color-on-brand);
}

.track-header__checkbox--partial {
  color: transparent;
  background-color: var(--color-brand-solid);
  border-color: var(--color-brand-solid);
}

.track-header__cell {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  min-width: 0;
  padding-right: var(--space-3);
  font-size: var(--font-size-xs);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-tertiary);
  text-align: left;
}

.track-header__cell:hover {
  color: var(--color-text-primary);
}

.track-header__cell--duration {
  justify-content: flex-end;
  padding-right: var(--space-2);
}

.track-header__spacer {
  width: 100%;
}
</style>
