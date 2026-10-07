<script setup lang="ts">
/**
 * 通用图标组件（DESIGN §3.7）
 *
 * 用法：<Icon name="play" :size="20" />
 * - 24×24 网格内联 SVG，颜色随 currentColor（继承文本色 / --color-brand 等）；
 * - 尺寸以 em 给出，由 font-size 控制：传 size 像素时写入内联 font-size，
 *   也可被父级 CSS 的 font-size 等比缩放；
 * - 装饰性图标默认 aria-hidden；需要语义时由父元素提供 aria-label。
 */
import { computed } from 'vue'

import { hasIcon, ICONS } from './icons'

interface Props {
  /** 语义图标名（对应 icons.ts 的键） */
  name: string
  /** 像素尺寸（宽高等比）；默认 20，等同 --size-icon-md */
  size?: number
  /** 是否随 .is-spinning 旋转（加载态） */
  spin?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  size: 20,
  spin: false,
})

const markup = computed(() =>
  hasIcon(props.name) ? ICONS[props.name] : ICONS['alert-triangle'],
)
const style = computed(() => `font-size:${props.size}px;`)
</script>

<template>
  <svg
    class="icon"
    :class="{ 'icon--spin': spin }"
    :style="style"
    viewBox="0 0 24 24"
    width="1em"
    height="1em"
    fill="none"
    stroke="currentColor"
    aria-hidden="true"
    focusable="false"
    v-html="markup"
  />
</template>

<style scoped>
.icon {
  flex: 0 0 auto;
  display: inline-block;
  vertical-align: middle;
}

.icon--spin {
  animation: icon-spin 0.9s linear infinite;
}

@keyframes icon-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .icon--spin {
    animation-duration: 2.4s;
  }
}
</style>
