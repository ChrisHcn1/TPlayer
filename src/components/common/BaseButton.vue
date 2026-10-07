<script setup lang="ts">
/**
 * 通用按钮（DESIGN §4.4）
 *
 * variant：
 *  - primary  品牌实心主操作（弹窗确认/主 CTA）
 *  - danger   危险实心（删除等破坏性确认）
 *  - subtle   次级实心（elevated 面 + subtle 边，弹窗取消/工具条按钮）
 *  - ghost    纯文本悬浮（工具条上的轻量动作）
 *  - icon     仅图标的正方形按钮（播放条/面板/行内更多）
 */
import { computed } from 'vue'

type ButtonVariant = 'primary' | 'danger' | 'subtle' | 'ghost' | 'icon'
type ButtonSize = 'sm' | 'md'

interface Props {
  variant?: ButtonVariant
  size?: ButtonSize
  type?: 'button' | 'submit'
  disabled?: boolean
  /** 危险幽灵按钮（如右键菜单“删除”由菜单项自己处理，这里仅个别场景使用） */
  active?: boolean
  ariaLabel?: string
  title?: string
}

const props = withDefaults(defineProps<Props>(), {
  variant: 'subtle',
  size: 'md',
  type: 'button',
  disabled: false,
  active: false,
  ariaLabel: undefined,
  title: undefined,
})

const classes = computed(() => [
  'btn',
  `btn--${props.variant}`,
  `btn--${props.size}`,
  { 'btn--active': props.active },
])
</script>

<template>
  <button
    :class="classes"
    :type="type"
    :disabled="disabled || undefined"
    :aria-label="ariaLabel"
    :title="title"
  >
    <slot />
  </button>
</template>

<style scoped>
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  flex: 0 0 auto;
  font-weight: var(--font-weight-medium);
  white-space: nowrap;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  transition:
    background-color var(--transition-colors),
    color var(--transition-colors),
    border-color var(--transition-colors);
}

.btn:disabled {
  pointer-events: none;
  opacity: 0.5;
}

.btn--sm {
  min-height: 28px;
  padding: 0 var(--space-3);
  font-size: var(--font-size-xs);
}

.btn--md {
  min-height: var(--size-control-h);
  padding: 0 var(--space-4);
  font-size: var(--font-size-sm);
}

.btn--primary {
  color: var(--color-on-brand);
  background-color: var(--color-brand-solid);
}

.btn--primary:hover {
  background-color: var(--color-brand-hover);
}

.btn--danger {
  color: var(--color-on-brand);
  background-color: var(--color-danger);
}

.btn--danger:hover {
  filter: brightness(1.08);
}

.btn--subtle {
  color: var(--color-text-primary);
  background-color: var(--color-bg-elevated);
  border-color: var(--color-border);
}

.btn--subtle:hover,
.btn--subtle.btn--active {
  background-color: var(--color-bg-hover);
  border-color: var(--color-border-strong);
}

.btn--ghost {
  color: var(--color-text-secondary);
  background: transparent;
}

.btn--ghost:hover,
.btn--ghost.btn--active {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}

.btn--icon {
  color: var(--color-text-secondary);
  background: transparent;
  border-radius: var(--radius-sm);
}

.btn--icon.btn--sm {
  width: 28px;
  height: 28px;
  padding: 0;
}

.btn--icon.btn--md {
  width: var(--size-control-h);
  height: var(--size-control-h);
  padding: 0;
}

.btn--icon:hover,
.btn--icon.btn--active {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}
</style>
