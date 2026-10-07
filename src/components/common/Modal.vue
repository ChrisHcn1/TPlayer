<script setup lang="ts">
/**
 * 通用模态框壳（DESIGN §4.15）
 *
 * 由 DialogHost / 业务弹窗复用：统一遮罩、尺寸档、Esc / 点遮罩关闭、
 * 打开聚焦、关闭后焦点归还。破坏性确认框由调用方关闭“点遮罩取消”。
 */
import { nextTick, onBeforeUnmount, ref, watch } from 'vue'

import Icon from './Icon.vue'

type ModalSize = 'sm' | 'md' | 'lg' | 'xl'

interface Props {
  open: boolean
  title: string
  size?: ModalSize
  /** 危险操作：标题展示警示图标，配色交宿主主按钮（danger）处理 */
  danger?: boolean
  /** 点击遮罩是否关闭（破坏性确认框应传 false） */
  closeOnOverlay?: boolean
  /** 是否展示右上关闭 X（输入类弹窗可保留；纯确认一般也保留） */
  showClose?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  size: 'sm',
  danger: false,
  closeOnOverlay: true,
  showClose: true,
})

const emit = defineEmits<{ (e: 'close'): void }>()

const panelRef = ref<HTMLElement | null>(null)
let previouslyFocused: HTMLElement | null = null

function focusables(): HTMLElement[] {
  if (!panelRef.value) return []
  return Array.from(
    panelRef.value.querySelectorAll<HTMLElement>(
      'input, textarea, select, button, [tabindex]:not([tabindex="-1"])',
    ),
  ).filter((el) => !el.hasAttribute('disabled'))
}

watch(
  () => props.open,
  async (open) => {
    if (!open) {
      previouslyFocused?.focus?.()
      return
    }
    previouslyFocused = document.activeElement as HTMLElement | null
    await nextTick()
    const target =
      panelRef.value?.querySelector<HTMLElement>('[data-autofocus]') ?? focusables()[0] ?? null
    target?.focus()
  },
)

function onKeydown(event: KeyboardEvent): void {
  if (!props.open) return
  if (event.key === 'Escape') {
    event.stopPropagation()
    emit('close')
    return
  }
  if (event.key === 'Tab' && panelRef.value) {
    const items = focusables()
    if (items.length === 0) return
    const first = items[0]
    const last = items[items.length - 1]
    const active = document.activeElement
    if (event.shiftKey && active === first) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && active === last) {
      event.preventDefault()
      first.focus()
    }
  }
}

function onOverlayClick(event: MouseEvent): void {
  if (!props.closeOnOverlay) return
  if (event.target === event.currentTarget) emit('close')
}

onBeforeUnmount(() => previouslyFocused?.focus?.())
</script>

<template>
  <Teleport to="body">
    <Transition name="modal">
      <div v-if="open" class="modal-overlay" :class="`modal-overlay--${size}`" @mousedown="onOverlayClick">
        <div
          ref="panelRef"
          class="modal"
          :class="`modal--${size}`"
          role="dialog"
          aria-modal="true"
          :aria-label="title"
          tabindex="-1"
          @keydown="onKeydown"
        >
          <header class="modal__header">
            <h2 class="modal__title">
              <Icon v-if="danger" name="alert-triangle" :size="20" class="modal__danger-icon" />
              {{ title }}
            </h2>
            <button
              v-if="showClose"
              type="button"
              class="modal__close"
              aria-label="关闭对话框"
              @click="emit('close')"
            >
              <Icon name="close" :size="16" />
            </button>
          </header>

          <div class="modal__body">
            <slot />
          </div>

          <footer v-if="$slots.footer" class="modal__footer">
            <slot name="footer" />
          </footer>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal-backdrop);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
  background-color: var(--color-overlay);
}

.modal {
  display: flex;
  flex-direction: column;
  max-height: 80vh;
  background-color: var(--color-bg-elevated);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-elevation-4);
  outline: none;
}

.modal--sm {
  width: 400px;
}
.modal--md {
  width: 560px;
}
.modal--lg {
  width: 760px;
}
.modal--xl {
  width: 960px;
}

.modal__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  flex: 0 0 auto;
  padding: var(--space-4) var(--space-5);
  border-bottom: 1px solid var(--color-border-subtle);
}

.modal__title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--font-size-md);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
}

.modal__danger-icon {
  color: var(--color-danger);
}

.modal__close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  color: var(--color-text-tertiary);
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
}

.modal__close:hover {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}

.modal__body {
  flex: 1 1 auto;
  min-height: 0;
  padding: var(--space-5);
  overflow-y: auto;
  font-size: var(--font-size-sm);
  line-height: 1.6;
  color: var(--color-text-secondary);
  scrollbar-gutter: stable;
}

.modal__footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-3);
  flex: 0 0 auto;
  padding: var(--space-3) var(--space-5);
  border-top: 1px solid var(--color-border-subtle);
}

.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.12s ease;
}

.modal-enter-active .modal,
.modal-leave-active .modal {
  transition:
    opacity 0.18s var(--ease-emphasized, cubic-bezier(0.2, 0, 0, 1)),
    transform 0.18s var(--ease-emphasized, cubic-bezier(0.2, 0, 0, 1));
}

.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}

.modal-enter-from .modal,
.modal-leave-to .modal {
  opacity: 0;
  transform: scale(0.98);
}
</style>
