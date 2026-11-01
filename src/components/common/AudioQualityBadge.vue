<script setup lang="ts">
/**
 * 音质标识徽章（行业对齐样式）
 *
 * 严格对齐分级规范：圆角 2px、字重 600（Hi-Res 700）、底部对齐文字基线、
 * 暗色模式下背景透明度 85% 而文字保持不透明。
 * 尺寸/字号以 dp/sp 计，前端按 1dp≈1px 渲染。
 */
import { computed } from 'vue'

import { QUALITY_LEVELS, type AudioQualityLevel } from '../../services/audio-quality'
import { t } from '../../i18n'

const props = defineProps<{
  level: AudioQualityLevel
  /** 悬停说明（默认取分级说明文案） */
  title?: string
}>()

const meta = computed(() => QUALITY_LEVELS[props.level])

const styleVars = computed(() => ({
  '--q-bg': meta.value.grad
    ? `linear-gradient(135deg, ${meta.value.grad[0]}, ${meta.value.grad[1]})`
    : meta.value.bg,
  '--q-fg': meta.value.fg,
  fontWeight: `${meta.value.weight ?? 600}`,
}))

const label = computed(() => t(meta.value.labelKey))
const titleText = computed(() => props.title ?? t(meta.value.descKey))
</script>

<template>
  <span class="q-badge" :style="styleVars" :title="titleText" role="img" :aria-label="titleText">
    <span class="q-badge__bg" aria-hidden="true"></span>
    <span class="q-badge__text">{{ label }}</span>
  </span>
</template>

<style scoped>
.q-badge {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 16px;
  padding: 0 6px;
  border-radius: 3px;
  font-size: 10px;
  line-height: 1;
  letter-spacing: 0.02em;
  white-space: nowrap;
  overflow: hidden;
  flex: none;
  align-self: center;
  user-select: none;
}

/* 背景层独立，便于暗色模式对其降透明度而不影响文字 */
.q-badge__bg {
  position: absolute;
  inset: 0;
  background: var(--q-bg);
}

.q-badge__text {
  position: relative;
  color: var(--q-fg);
  white-space: nowrap;
  letter-spacing: 0.02em;
  text-transform: uppercase;
}

/* 暗色主题：背景 85% 透明度，文字保持不透明，避免深色下刺眼 */
:global([data-theme='dark']) .q-badge__bg {
  opacity: 0.85;
}
</style>
