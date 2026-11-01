<script setup lang="ts">
/**
 * 实验室·悬浮歌词（浏览器预览回退）
 *
 * 仅在非 Tauri 环境下使用：因浏览器无独立窗口能力，退化为应用内浮层。
 * Tauri 环境改用独立窗口（src/floating-lyrics/App.vue），本组件不挂载。
 * 显隐由 useFloatingLyrics().overlayVisible 控制。
 */
import { computed, ref, watch } from 'vue'

import { usePlayerStore } from '../../stores/player'
import { useLyricsStore } from '../../stores/lyrics'
import { useFloatingLyrics } from '../../lab/features/floating-lyrics'
import { t } from '../../i18n'

const playerStore = usePlayerStore()
const lyricsStore = useLyricsStore()
const fl = useFloatingLyrics()

const snapshot = computed(() => playerStore.snapshot)
const currentLine = computed(() => {
  const l = lyricsStore.lyrics
  if (!l) return ''
  if (!l.synced) return l.lines[0]?.text ?? ''
  const idx = lyricsStore.activeLineIndexAt(snapshot.value.positionMs)
  return idx >= 0 ? l.lines[idx]?.text ?? '' : ''
})
watch(
  () => snapshot.value.trackId,
  (id) => {
    if (id) void lyricsStore.load(id)
    else lyricsStore.clear()
  },
  { immediate: true },
)

const pos = ref({ x: 120, y: 120 })
const dragging = ref(false)
const dragOffset = ref({ x: 0, y: 0 })
function onPointerDown(e: PointerEvent): void {
  dragging.value = true
  dragOffset.value = { x: e.clientX - pos.value.x, y: e.clientY - pos.value.y }
  ;(e.target as HTMLElement)?.setPointerCapture?.(e.pointerId)
}
function onPointerMove(e: PointerEvent): void {
  if (!dragging.value) return
  pos.value = {
    x: Math.max(0, e.clientX - dragOffset.value.x),
    y: Math.max(0, e.clientY - dragOffset.value.y),
  }
}
function onPointerUp(): void {
  dragging.value = false
}
window.addEventListener('pointermove', onPointerMove)
window.addEventListener('pointerup', onPointerUp)
</script>

<template>
  <div
    v-if="fl.overlayVisible.value"
    class="floating-lyrics"
    :style="{ left: pos.x + 'px', top: pos.y + 'px' }"
    :class="{ 'floating-lyrics--dragging': dragging }"
    @pointerdown="onPointerDown"
  >
    <span class="floating-lyrics__text">{{ currentLine || t('lab.floating.noLyrics') }}</span>
    <button
      type="button"
      class="floating-lyrics__toggle"
      :aria-label="t('lab.floating.hide')"
      :title="t('lab.floating.hide')"
      @click.stop="fl.close()"
    >
      ×
    </button>
  </div>
</template>

<style scoped>
.floating-lyrics {
  position: fixed;
  z-index: 60;
  display: flex;
  align-items: center;
  gap: 8px;
  max-width: 420px;
  padding: 8px 12px;
  border-radius: 999px;
  color: var(--color-text-primary);
  background-color: var(--color-bg-elevated);
  border: 1px solid var(--color-border-subtle);
  box-shadow: var(--shadow-elevation-2);
  cursor: grab;
  user-select: none;
  backdrop-filter: blur(6px);
}
.floating-lyrics--dragging {
  cursor: grabbing;
}
.floating-lyrics__text {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--font-size-sm);
}
.floating-lyrics__toggle {
  flex: 0 0 auto;
  width: 20px;
  height: 20px;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
}
.floating-lyrics__toggle:hover {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}
</style>
