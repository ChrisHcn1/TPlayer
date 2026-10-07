<script setup lang="ts">
/**
 * 曲目行（DESIGN §4.5）
 *
 * 列：选择框 40 · 序号/播放位 40 · 标题（弹性）· 艺术家 2fr · 专辑 2fr · 时长 64 · 更多 40
 * 状态：default / hover / selected / playing（左缘 3px 指示条 + 均衡器）；
 * 交互：双击播放、勾选多选（Shift 连选由 TrackList 处理）、右键菜单、更多按钮。
 */
import { computed } from 'vue'

import type { Track } from '../../types'
import Icon from '../common/Icon.vue'

interface Props {
  track: Track
  /** 基于 1 的展示序号（内部转 index 文本） */
  displayIndex: number
  selected: boolean
  /** 当前曲目 id 等于本行（无论是否在播放） */
  isCurrent: boolean
  isPlaying: boolean
  showArtist?: boolean
  showAlbum?: boolean
  /** 分轨专辑行：展示该轨在整盘中的起止时间提示等（预留） */
}

const props = withDefaults(defineProps<Props>(), {
  showArtist: true,
  showAlbum: true,
})

const emit = defineEmits<{
  (e: 'dblplay', track: Track): void
  (e: 'toggle-select', track: Track, event: MouseEvent): void
  (e: 'row-click', track: Track, event: MouseEvent): void
  (e: 'context-menu', track: Track, event: MouseEvent): void
}>()

const durationText = computed(() => {
  const totalSeconds = Math.round(props.track.duration)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes}:${seconds.toString().padStart(2, '0')}`
})

function onCheckboxClick(event: MouseEvent): void {
  emit('toggle-select', props.track, event)
}
</script>

<template>
  <div
    class="track-row"
    :class="{
      'track-row--selected': selected,
      'track-row--current': isCurrent,
      'track-row--playing': isPlaying,
      'track-row--no-artist': !showArtist,
    }"
    role="row"
    @click="emit('row-click', track, $event)"
    @dblclick="emit('dblplay', track)"
    @contextmenu="emit('context-menu', track, $event)"
  >
    <span class="track-row__indicator" aria-hidden="true"></span>

    <div class="track-row__check">
      <button
        type="button"
        class="track-row__checkbox"
        :class="{ 'track-row__checkbox--checked': selected }"
        role="checkbox"
        :aria-checked="selected"
        :aria-label="selected ? '取消选择' : '选择'"
        :tabindex="selected ? 0 : -1"
        @click.stop="onCheckboxClick($event)"
      >
        <Icon v-if="selected" name="check" :size="12" />
      </button>
    </div>

    <div class="track-row__index">
      <span class="track-row__index-num u-tabular">{{ displayIndex }}</span>
      <Icon name="play" :size="14" class="track-row__index-play" />
      <span v-if="isCurrent" class="track-row__equalizer" aria-hidden="true">
        <span></span><span></span><span></span>
      </span>
    </div>

    <div class="track-row__title">
      <span class="track-row__title-text" :class="{ 'track-row__title-text--current': isCurrent }">
        {{ track.title }}
      </span>
    </div>

    <div v-if="showArtist" class="track-row__artist">
      <span class="u-truncate">{{ track.artists.join(' / ') }}</span>
    </div>

    <div v-if="showAlbum" class="track-row__album">
      <span class="u-truncate">{{ track.album }}</span>
    </div>

    <div class="track-row__duration u-tabular">{{ durationText }}</div>

    <div class="track-row__actions">
      <button
        type="button"
        class="track-row__more"
        aria-label="更多操作"
        @click.stop="emit('context-menu', track, $event)"
      >
        <Icon name="more-horizontal" :size="16" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.track-row {
  position: relative;
  display: grid;
  grid-template-columns: 40px 40px minmax(0, 3fr) minmax(0, 2fr) minmax(0, 2fr) 64px 40px;
  align-items: center;
  min-height: var(--size-row-h);
  padding: 0 var(--space-2) 0 var(--space-1);
  border-radius: var(--radius-sm);
  cursor: default;
}

.track-row--no-artist {
  grid-template-columns: 40px 40px minmax(0, 3fr) minmax(0, 2fr) 64px 40px;
}

/* 左缘播放指示条 */
.track-row__indicator {
  position: absolute;
  left: 0;
  top: 50%;
  width: 3px;
  height: 0;
  border-radius: var(--radius-full);
  background-color: var(--color-brand);
  transform: translateY(-50%);
  transition: height var(--transition-colors);
}

.track-row--current .track-row__indicator {
  height: 16px;
}

.track-row:hover {
  background-color: var(--color-bg-hover);
}

.track-row--selected {
  background-color: var(--color-bg-selected);
}

.track-row--selected:hover {
  background-color: var(--color-bg-active);
}

/* 选择框 */
.track-row__check,
.track-row__index {
  display: flex;
  align-items: center;
  justify-content: center;
}

.track-row__checkbox {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  color: transparent;
  background: transparent;
  border: 1px solid var(--color-border-strong);
  border-radius: var(--radius-xs);
  opacity: 0;
  transition:
    opacity var(--transition-colors),
    background-color var(--transition-colors),
    border-color var(--transition-colors);
}

.track-row:hover .track-row__checkbox,
.track-row--selected .track-row__checkbox,
.track-row__checkbox:focus-visible {
  opacity: 1;
}

.track-row__checkbox--checked {
  opacity: 1;
  color: var(--color-on-brand);
  background-color: var(--color-brand-solid);
  border-color: var(--color-brand-solid);
}

/* 序号 / 悬停播放 / 均衡器三态切换 */
.track-row__index {
  position: relative;
  color: var(--color-text-tertiary);
  font-size: var(--font-size-sm);
}

.track-row__index-play {
  position: absolute;
  color: var(--color-text-primary);
  opacity: 0;
}

.track-row:hover .track-row__index-num,
.track-row--current .track-row__index-num {
  opacity: 0;
}

.track-row:hover .track-row__index-play {
  opacity: 1;
}

.track-row--current .track-row__index-play {
  opacity: 0;
}

.track-row__title-text--current {
  color: var(--color-brand);
}

.track-row--current .track-row__index {
  color: var(--color-brand);
}

/* 均衡器（播放中） */
.track-row__equalizer {
  position: absolute;
  display: flex;
  align-items: flex-end;
  gap: 2px;
  height: 14px;
}

.track-row__equalizer span {
  width: 3px;
  height: 100%;
  background-color: var(--color-brand);
  border-radius: 1px;
  animation: eq-bounce 0.9s ease-in-out infinite;
  transform-origin: bottom;
}

.track-row__equalizer span:nth-child(1) {
  animation-delay: -0.4s;
}
.track-row__equalizer span:nth-child(2) {
  animation-delay: -0.2s;
}
.track-row__equalizer span:nth-child(3) {
  animation-delay: -0.6s;
}

.track-row:not(.track-row--playing) .track-row__equalizer span {
  animation-play-state: paused;
  transform: scaleY(0.4);
}

@keyframes eq-bounce {
  0%,
  100% {
    transform: scaleY(0.35);
  }
  50% {
    transform: scaleY(1);
  }
}

/* 文本列 */
.track-row__title-text,
.track-row__artist span,
.track-row__album span {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track-row__title-text {
  padding-right: var(--space-2);
  font-size: var(--font-size-sm);
  color: var(--color-text-primary);
}

.track-row__artist,
.track-row__album {
  min-width: 0;
  padding-right: var(--space-3);
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
}

.track-row__duration {
  font-size: var(--font-size-sm);
  color: var(--color-text-tertiary);
  text-align: right;
}

/* 更多按钮 */
.track-row__actions {
  display: flex;
  align-items: center;
  justify-content: center;
}

.track-row__more {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-h);
  height: var(--size-control-h);
  color: var(--color-text-secondary);
  border-radius: var(--radius-sm);
  opacity: 0;
  transition: opacity var(--transition-colors);
}

.track-row:hover .track-row__more,
.track-row--selected .track-row__more {
  opacity: 1;
}

.track-row__more:hover {
  color: var(--color-text-primary);
  background-color: var(--color-bg-active);
}
</style>
