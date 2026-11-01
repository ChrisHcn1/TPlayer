<script setup lang="ts">
/**
 * 多选操作浮条（DESIGN §4.8）：列表底部 sticky 浮出；
 * Esc / 取消清空选择（由父级或 TrackList 处理 Esc），count=0 时父级不渲染。
 */
import { computed } from 'vue'

import { t } from '../../i18n'
import Icon from '../common/Icon.vue'

interface Props {
  count: number
  /** 是否已全部收藏：控制心形按钮语义 */
  allFavorited?: boolean
  /** 是否展示移除按钮（最近播放等场景可关闭） */
  showRemove?: boolean
  /** 移除按钮的语义标题（收藏页为“取消收藏”） */
  removeTitle?: string
}

const props = withDefaults(defineProps<Props>(), {
  allFavorited: false,
  showRemove: true,
  removeTitle: '',
})

const removeTitleText = computed(
  () => props.removeTitle || t('selection.removeFromLibrary'),
)

const emit = defineEmits<{
  (e: 'play'): void
  (e: 'add-playlist', event: MouseEvent): void
  (e: 'favorite'): void
  (e: 'remove'): void
  (e: 'clear'): void
}>()
</script>

<template>
  <Transition name="selection">
    <div v-if="count > 0" class="selection-bar" role="toolbar" :aria-label="t('selection.selected', { count })">
      <span class="selection-bar__count u-tabular">{{ t('selection.selected', { count }) }}</span>

      <button type="button" class="selection-bar__btn selection-bar__btn--brand" :title="t('selection.playSelected')" @click="emit('play')">
        <Icon name="play" :size="15" />
        <span>{{ t('common.play') }}</span>
      </button>

      <button
        type="button"
        class="selection-bar__btn"
        :title="t('selection.addToPlaylist')"
        @click="emit('add-playlist', $event)"
      >
        <Icon name="list-plus" :size="15" />
        <span>{{ t('selection.addToPlaylist') }}</span>
      </button>

      <button type="button" class="selection-bar__btn" :title="allFavorited ? t('player.unfavorite') : t('player.favorite')" @click="emit('favorite')">
        <Icon :name="allFavorited ? 'heart-off' : 'heart'" :size="15" />
      </button>

      <button v-if="showRemove" type="button" class="selection-bar__btn selection-bar__btn--danger" :title="removeTitleText" @click="emit('remove')">
        <Icon name="trash-2" :size="15" />
      </button>

      <span class="selection-bar__divider" aria-hidden="true"></span>

      <button type="button" class="selection-bar__btn selection-bar__btn--ghost" :title="t('selection.clearHint')" @click="emit('clear')">
        <Icon name="close" :size="15" />
      </button>
    </div>
  </Transition>
</template>

<style scoped>
.selection-bar {
  position: sticky;
  bottom: calc(var(--size-playerbar-h) + var(--space-4));
  z-index: var(--z-dropdown);
  display: flex;
  align-items: center;
  gap: var(--space-1);
  width: fit-content;
  margin: 0 auto var(--space-4);
  padding: var(--space-2);
  background-color: var(--color-bg-elevated);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-full);
  box-shadow: var(--shadow-elevation-3);
}

.selection-bar__count {
  padding: 0 var(--space-3) 0 var(--space-2);
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-primary);
  white-space: nowrap;
}

.selection-bar__divider {
  width: 1px;
  height: 20px;
  margin: 0 var(--space-1);
  background-color: var(--color-border-subtle);
}

.selection-bar__btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  min-height: 30px;
  padding: 0 var(--space-3);
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  border-radius: var(--radius-full);
}

.selection-bar__btn:hover {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}

.selection-bar__btn--brand {
  color: var(--color-brand);
}

.selection-bar__btn--brand:hover {
  color: var(--color-on-brand);
  background-color: var(--color-brand-solid);
}

.selection-bar__btn--danger:hover {
  color: var(--color-danger);
  background-color: var(--color-danger-bg);
}

.selection-bar__btn--ghost {
  padding: 0 var(--space-2);
}

.selection-enter-active,
.selection-leave-active {
  transition:
    opacity 0.18s var(--ease-emphasized, cubic-bezier(0.2, 0, 0, 1)),
    transform 0.18s var(--ease-emphasized, cubic-bezier(0.2, 0, 0, 1));
}

.selection-enter-from,
.selection-leave-to {
  opacity: 0;
  transform: translateY(8px);
}
</style>
