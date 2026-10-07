<script setup lang="ts">
/**
 * Toast 宿主（DESIGN §4.16）：全局唯一，挂载于 App。
 * 右上角堆叠；左侧 3px 状态色条；悬停暂停自动关闭；error 使用 assertive 播报。
 */
import { onBeforeUnmount, watch } from 'vue'
import { storeToRefs } from 'pinia'

import { useToastStore, type ToastVariant } from '../../stores/toast'
import Icon from './Icon.vue'

const toastStore = useToastStore()
const { toasts } = storeToRefs(toastStore)

const ICON_BY_VARIANT: Record<ToastVariant, string> = {
  info: 'info',
  success: 'check',
  warning: 'alert-triangle',
  error: 'x-circle',
}

const timers = new Map<number, ReturnType<typeof setTimeout>>()

function arm(id: number, duration: number): void {
  disarm(id)
  timers.set(
    id,
    setTimeout(() => {
      timers.delete(id)
      toastStore.dismiss(id)
    }, duration),
  )
}

function disarm(id: number): void {
  const timer = timers.get(id)
  if (timer) {
    clearTimeout(timer)
    timers.delete(id)
  }
}

// 入队即开始计时（悬停暂停在模板里通过 mouseenter/leave 处理）
watch(
  () => toasts.value.map((item) => item.id),
  (ids, oldIds = []) => {
    for (const id of ids) {
      if (!oldIds.includes(id)) {
        const item = toasts.value.find((entry) => entry.id === id)
        if (item) arm(id, item.duration)
      }
    }
  },
)

onBeforeUnmount(() => {
  timers.forEach((timer) => clearTimeout(timer))
  timers.clear()
})

function runAction(id: number): void {
  const item = toasts.value.find((entry) => entry.id === id)
  item?.action?.onClick()
  toastStore.dismiss(id)
}
</script>

<template>
  <div class="toast-container">
    <TransitionGroup name="toast" tag="div" class="toast-stack">
      <div
        v-for="item in toasts"
        :key="item.id"
        class="toast"
        :class="`toast--${item.variant}`"
        :role="item.variant === 'error' ? 'alert' : 'status'"
        :aria-live="item.variant === 'error' ? 'assertive' : 'polite'"
        @mouseenter="disarm(item.id)"
        @mouseleave="arm(item.id, item.duration)"
      >
        <span class="toast__bar" aria-hidden="true"></span>
        <Icon class="toast__icon" :name="ICON_BY_VARIANT[item.variant]" :size="18" />
        <div class="toast__body">
          <p class="toast__title">{{ item.title }}</p>
          <p v-if="item.description" class="toast__description">{{ item.description }}</p>
        </div>
        <button
          v-if="item.action"
          type="button"
          class="toast__action"
          @click="runAction(item.id)"
        >
          {{ item.action.label }}
        </button>
        <button
          type="button"
          class="toast__close"
          aria-label="关闭通知"
          @click="toastStore.dismiss(item.id)"
        >
          <Icon name="close" :size="14" />
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toast-container {
  position: fixed;
  top: calc(var(--size-titlebar-h) + var(--space-4));
  right: var(--space-5);
  z-index: var(--z-toast);
  pointer-events: none;
}

.toast-stack {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.toast {
  position: relative;
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  width: 340px;
  max-width: calc(100vw - var(--space-10));
  padding: var(--space-3) var(--space-4) var(--space-3) var(--space-3);
  overflow: hidden;
  background-color: var(--color-bg-elevated);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-elevation-3);
  pointer-events: auto;
}

.toast__bar {
  position: absolute;
  inset: 0 auto 0 0;
  width: 3px;
}

.toast--info .toast__bar {
  background-color: var(--color-info);
}
.toast--success .toast__bar {
  background-color: var(--color-success);
}
.toast--warning .toast__bar {
  background-color: var(--color-warning);
}
.toast--error .toast__bar {
  background-color: var(--color-danger);
}

.toast--info .toast__icon {
  color: var(--color-info);
}
.toast--success .toast__icon {
  color: var(--color-success);
}
.toast--warning .toast__icon {
  color: var(--color-warning);
}
.toast--error .toast__icon {
  color: var(--color-danger);
}

.toast__icon {
  flex: 0 0 auto;
  margin-top: 1px;
}

.toast__body {
  flex: 1 1 auto;
  min-width: 0;
}

.toast__title {
  margin: 0;
  font-size: var(--font-size-base);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-primary);
}

.toast__description {
  margin: 2px 0 0;
  font-size: var(--font-size-sm);
  line-height: 1.45;
  color: var(--color-text-secondary);
}

.toast__action {
  flex: 0 0 auto;
  align-self: center;
  padding: var(--space-1) var(--space-2);
  font-size: var(--font-size-sm);
  color: var(--color-brand);
  background: transparent;
  border: none;
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.toast__action:hover {
  background-color: var(--color-bg-hover);
}

.toast__close {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  color: var(--color-text-tertiary);
  background: transparent;
  border: none;
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.toast__close:hover {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}

.toast-enter-active,
.toast-leave-active {
  transition:
    opacity 0.2s var(--ease-emphasized, ease),
    transform 0.2s var(--ease-emphasized, ease);
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateX(16px);
}
</style>
