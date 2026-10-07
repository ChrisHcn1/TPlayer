<script setup lang="ts">
/**
 * 播放进度（DESIGN §4.9 Progress）
 *
 * - 拖拽期间只更新本地预览位置与气泡，松手（pointerup）才提交 seek（不实时 seek）；
 * - role=slider：←/→ ±5s，Shift+←/→ ±30s，Home/End 跳转首尾；
 * - 等宽数字时间码，避免抖动（u-tabular）。
 */
import { computed, ref } from 'vue'

interface Props {
  positionMs: number
  durationMs: number
  disabled?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  disabled: false,
})

const emit = defineEmits<{ (e: 'seek', positionMs: number): void }>()

const SEEK_SMALL = 5_000
const SEEK_LARGE = 30_000

const dragging = ref(false)
const dragRatio = ref(0)
const hoverRatio = ref<number | null>(null)
const trackRef = ref<HTMLElement | null>(null)

const effectiveDuration = computed(() => Math.max(0, props.durationMs))
const ratio = computed(() => {
  if (effectiveDuration.value === 0) return 0
  if (dragging.value) return dragRatio.value
  return Math.min(1, Math.max(0, props.positionMs / effectiveDuration.value))
})

function format(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000))
  const minutes = Math.floor(total / 60)
  const seconds = total % 60
  return `${minutes}:${seconds.toString().padStart(2, '0')}`
}

const positionText = computed(() => format(ratio.value * effectiveDuration.value))
const durationText = computed(() => format(effectiveDuration.value))
const hoverText = computed(() =>
  hoverRatio.value === null ? '' : format(hoverRatio.value * effectiveDuration.value),
)

function ratioFromEvent(event: PointerEvent | MouseEvent): number {
  const el = trackRef.value
  if (!el) return 0
  const rect = el.getBoundingClientRect()
  if (rect.width === 0) return 0
  return Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width))
}

function onPointerDown(event: PointerEvent): void {
  if (props.disabled || effectiveDuration.value === 0) return
  dragging.value = true
  dragRatio.value = ratioFromEvent(event)
  ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
}

function onPointerMove(event: PointerEvent): void {
  if (!trackRef.value) return
  const rect = trackRef.value.getBoundingClientRect()
  hoverRatio.value = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width))
  if (dragging.value) dragRatio.value = hoverRatio.value
}

function onPointerUp(event: PointerEvent): void {
  if (!dragging.value) return
  dragging.value = false
  const target = Math.round(dragRatio.value * effectiveDuration.value)
  emit('seek', target)
  hoverRatio.value = ratioFromEvent(event)
}

function onPointerLeave(): void {
  if (!dragging.value) hoverRatio.value = null
}

function nudge(deltaMs: number): void {
  if (props.disabled || effectiveDuration.value === 0) return
  const target = Math.min(
    effectiveDuration.value,
    Math.max(0, props.positionMs + deltaMs),
  )
  emit('seek', target)
}

function onKeydown(event: KeyboardEvent): void {
  if (props.disabled || effectiveDuration.value === 0) return
  const step = event.shiftKey ? SEEK_LARGE : SEEK_SMALL
  if (event.key === 'ArrowRight') {
    event.preventDefault()
    nudge(step)
  } else if (event.key === 'ArrowLeft') {
    event.preventDefault()
    nudge(-step)
  } else if (event.key === 'Home') {
    event.preventDefault()
    emit('seek', 0)
  } else if (event.key === 'End') {
    event.preventDefault()
    emit('seek', effectiveDuration.value)
  }
}
</script>

<template>
  <div class="progress" :class="{ 'progress--disabled': disabled }">
    <span class="progress__time u-tabular">{{ positionText }}</span>

    <div
      ref="trackRef"
      class="progress__track"
      role="slider"
      aria-label="播放进度"
      :aria-valuemin="0"
      :aria-valuemax="Math.round(effectiveDuration / 1000)"
      :aria-valuenow="Math.round((ratio * effectiveDuration) / 1000)"
      aria-valuetext="播放进度"
      :tabindex="disabled ? -1 : 0"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointerleave="onPointerLeave"
      @keydown="onKeydown"
    >
      <div class="progress__fill" :style="{ width: `${ratio * 100}%` }"></div>
      <div class="progress__thumb" :class="{ 'progress__thumb--drag': dragging }" :style="{ left: `${ratio * 100}%` }"></div>
      <span
        v-if="hoverRatio !== null && !dragging"
        class="progress__bubble u-tabular"
        :style="{ left: `${hoverRatio * 100}%` }"
      >{{ hoverText }}</span>
    </div>

    <span class="progress__time u-tabular">{{ durationText }}</span>
  </div>
</template>

<style scoped>
.progress {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
}

.progress__time {
  flex: 0 0 auto;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

.progress__track {
  position: relative;
  flex: 1 1 auto;
  height: var(--size-control-h);
  display: flex;
  align-items: center;
  cursor: pointer;
  touch-action: none;
}

.progress--disabled .progress__track {
  cursor: default;
}

.progress__track::before {
  content: '';
  display: block;
  width: 100%;
  height: 4px;
  background-color: var(--color-bg-hover);
  border-radius: var(--radius-full);
}

.progress__fill {
  position: absolute;
  left: 0;
  top: 50%;
  height: 4px;
  border-radius: var(--radius-full);
  /* 封面皮肤开启时用中间调进度色，与皮肤色（按钮/选中）和歌词色区分 */
  background-color: var(--color-cover-progress, var(--color-brand));
  transform: translateY(-50%);
  pointer-events: none;
}

.progress__thumb {
  position: absolute;
  top: 50%;
  width: 12px;
  height: 12px;
  background-color: var(--color-cover-progress, var(--color-brand));
  border-radius: var(--radius-full);
  opacity: 0;
  transform: translate(-50%, -50%) scale(0.8);
  transition:
    opacity var(--transition-colors),
    transform var(--transition-colors);
  pointer-events: none;
}

.progress__track:hover .progress__thumb,
.progress__thumb--drag {
  opacity: 1;
  transform: translate(-50%, -50%) scale(1);
}

.progress__bubble {
  position: absolute;
  bottom: calc(50% + 10px);
  padding: 2px var(--space-2);
  font-size: var(--font-size-2xs, 11px);
  color: var(--color-text-primary);
  background-color: var(--color-bg-elevated);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-xs);
  box-shadow: var(--shadow-elevation-2);
  transform: translateX(-50%);
  pointer-events: none;
}

.progress__track:focus-visible {
  outline: 2px solid var(--color-focus-ring);
  outline-offset: 4px;
  border-radius: var(--radius-full);
}
</style>
