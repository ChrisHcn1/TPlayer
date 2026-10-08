<template>
  <div class="toast-container" aria-live="polite">
    <TransitionGroup name="toast">
      <div
        v-for="t in toasts"
        :key="t.id"
        class="toast"
        :class="`toast-${t.type}`"
        @mouseenter="onEnter(t.id)"
        @mouseleave="onLeave(t.id)"
      >
        <span class="toast-icon">{{ iconFor(t.type) }}</span>
        <span class="toast-content">{{ t.content }}</span>
        <button class="toast-close" @click="removeToast(t.id)" aria-label="关闭">×</button>
      </div>
    </TransitionGroup>
  </div>
</template>

<script setup lang="ts">
import { useMessage, type MessageType } from '../composables/useMessage'

const { toasts, removeToast, pauseToast, resumeToast } = useMessage()

const ICONS: Record<MessageType, string> = {
  error: '❌',
  success: '✅',
  warning: '⚠️',
  info: 'ℹ️'
}
const iconFor = (t: MessageType) => ICONS[t]

const onEnter = (id: string) => pauseToast(id)
const onLeave = (id: string) => resumeToast(id)
</script>

<style scoped>
.toast-container {
  position: fixed;
  top: 16px;
  right: 16px;
  z-index: 99999;
  display: flex;
  flex-direction: column;
  gap: 8px;
  pointer-events: none;
  max-width: min(380px, 92vw);
}

.toast {
  pointer-events: auto;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: 8px;
  color: #fff;
  font-size: 14px;
  line-height: 1.4;
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.25);
  /* 左侧颜色条，更直观区分类型 */
  border-left: 4px solid rgba(255, 255, 255, 0.85);
}

.toast-icon {
  font-size: 16px;
  line-height: 1;
}

.toast-content {
  flex: 1 1 auto;
  white-space: pre-wrap;
  word-break: break-word;
}

.toast-close {
  flex: 0 0 auto;
  background: transparent;
  border: 0;
  color: rgba(255, 255, 255, 0.85);
  font-size: 18px;
  line-height: 1;
  cursor: pointer;
  padding: 0 4px;
  border-radius: 4px;
}
.toast-close:hover {
  background: rgba(255, 255, 255, 0.18);
}

.toast-error   { background: #e53935; }
.toast-success { background: #43a047; }
.toast-warning { background: #fb8c00; }
.toast-info    { background: #1e88e5; }

/* 进入：淡入右滑；退出：淡出右滑 */
.toast-enter-active,
.toast-leave-active {
  transition: all 0.25s ease;
}
.toast-enter-from {
  opacity: 0;
  transform: translateX(60px);
}
.toast-leave-to {
  opacity: 0;
  transform: translateX(60px);
}
/* 列表内同时移除时的位移过渡 */
.toast-move {
  transition: transform 0.25s ease;
}
</style>
