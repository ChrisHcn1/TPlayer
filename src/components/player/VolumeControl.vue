<script setup lang="ts">
/**
 * 音量控制（DESIGN §4.9 Volume）：
 * 点击图标静音/恢复（记住静音前音量）；悬停展开 100px 滑杆。
 */
import { computed, ref } from 'vue'
import { storeToRefs } from 'pinia'

import { usePlayerStore } from '../../stores/player'
import Icon from '../common/Icon.vue'

const playerStore = usePlayerStore()
const { snapshot } = storeToRefs(playerStore)

const volume = computed(() => snapshot.value.volume)
const muted = computed(() => snapshot.value.muted)
const effectiveVolume = computed(() => (muted.value ? 0 : volume.value))

/** 静音前的音量，用于恢复 */
const previousVolume = ref(0.7)

const iconName = computed(() => {
  if (muted.value || effectiveVolume.value === 0) return 'volume-mute'
  if (effectiveVolume.value < 0.5) return 'volume-low'
  return 'volume-high'
})

function toggleMute(): void {
  if (muted.value) {
    const restore = previousVolume.value > 0 ? previousVolume.value : 0.7
    void playerStore.setVolume(restore)
    void playerStore.setMuted(false)
    return
  }
  previousVolume.value = volume.value
  void playerStore.setMuted(true)
}

function onInput(event: Event): void {
  const value = Number((event.target as HTMLInputElement).value) / 100
  if (muted.value && value > 0) void playerStore.setMuted(false)
  void playerStore.setVolume(value)
}
</script>

<template>
  <div class="volume">
    <button
      type="button"
      class="volume__btn"
      :aria-label="muted ? '取消静音' : '静音'"
      :title="muted ? '取消静音（M）' : '静音（M）'"
      @click="toggleMute"
    >
      <Icon :name="iconName" :size="18" />
    </button>
    <input
      class="volume__slider"
      type="range"
      min="0"
      max="100"
      step="1"
      :value="Math.round(effectiveVolume * 100)"
      :style="{ '--value': `${Math.round(effectiveVolume * 100)}%` }"
      aria-label="音量"
      @input="onInput"
    />
  </div>
</template>

<style scoped>
.volume {
  display: flex;
  align-items: center;
  width: var(--size-control-h);
  overflow: hidden;
  transition: width var(--transition-colors);
}

.volume:hover,
.volume:focus-within {
  width: calc(var(--size-control-h) + 100px);
}

.volume__btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-h);
  height: var(--size-control-h);
  flex: 0 0 auto;
  color: var(--color-text-secondary);
  border-radius: var(--radius-sm);
}

.volume__btn:hover {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}

.volume__slider {
  flex: 0 0 100px;
  width: 100px;
  height: 4px;
  margin-left: var(--space-2);
  appearance: none;
  -webkit-appearance: none;
  background: transparent;
  cursor: pointer;
}

.volume__slider::-webkit-slider-runnable-track {
  height: 4px;
  border-radius: var(--radius-full);
  background: linear-gradient(
    to right,
    var(--color-brand) 0%,
    var(--color-brand) var(--value, 50%),
    var(--color-bg-hover) var(--value, 50%),
    var(--color-bg-hover) 100%
  );
}

.volume__slider::-webkit-slider-thumb {
  width: 12px;
  height: 12px;
  margin-top: -4px;
  appearance: none;
  -webkit-appearance: none;
  background-color: var(--color-brand);
  border-radius: var(--radius-full);
}

.volume__slider::-moz-range-track {
  height: 4px;
  border-radius: var(--radius-full);
  background-color: var(--color-bg-hover);
}

.volume__slider::-moz-range-progress {
  height: 4px;
  border-radius: var(--radius-full);
  background-color: var(--color-brand);
}

.volume__slider::-moz-range-thumb {
  width: 12px;
  height: 12px;
  border: none;
  background-color: var(--color-brand);
  border-radius: var(--radius-full);
}
</style>
