<script setup lang="ts">
/**
 * 程序化对话框宿主（DESIGN §4.15 / §5.7）：全局唯一，挂载于 App。
 * 渲染 dialog store 中的唯一 confirm/prompt；危险确认焦点落“取消”。
 */
import { storeToRefs } from 'pinia'

import { useDialogStore } from '../../stores/dialog'
import Modal from './Modal.vue'
import BaseButton from './BaseButton.vue'

const dialogStore = useDialogStore()
const { state, error } = storeToRefs(dialogStore)
</script>

<template>
  <Modal
    :open="state !== null"
    :title="state?.title ?? ''"
    size="sm"
    :danger="state?.danger ?? false"
    :close-on-overlay="!(state?.danger ?? false)"
    @close="dialogStore.cancel()"
  >
    <p v-if="state?.message" class="dialog__message">{{ state.message }}</p>

    <div v-if="state?.kind === 'prompt'" class="dialog__field">
      <input
        :value="state.value"
        class="dialog__input"
        :class="{ 'dialog__input--invalid': error !== null }"
        type="text"
        :placeholder="state.placeholder"
        data-autofocus
        @input="dialogStore.setValue(($event.target as HTMLInputElement).value)"
        @keydown.enter.prevent="dialogStore.accept()"
      />
      <p v-if="error" class="dialog__error" role="alert">{{ error }}</p>
    </div>

    <template #footer>
      <BaseButton
        size="sm"
        variant="subtle"
        :data-autofocus="state?.danger ? '' : undefined"
        @click="dialogStore.cancel()"
      >
        {{ state?.cancelText ?? '取消' }}
      </BaseButton>
      <BaseButton
        size="sm"
        :variant="state?.danger ? 'danger' : 'primary'"
        :data-autofocus="state && !state.danger && state.kind === 'confirm' ? '' : undefined"
        @click="dialogStore.accept()"
      >
        {{ state?.confirmText ?? '确定' }}
      </BaseButton>
    </template>
  </Modal>
</template>

<style scoped>
.dialog__message {
  color: var(--color-text-secondary);
}

.dialog__field {
  margin-top: var(--space-4);
}

.dialog__input {
  width: 100%;
  min-height: var(--size-control-h);
  padding: 0 var(--space-3);
  color: var(--color-text-primary);
  background-color: var(--color-bg-input);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
}

.dialog__input:focus {
  border-color: var(--color-brand);
  outline: 2px solid var(--color-focus-ring);
  outline-offset: 0;
}

.dialog__input--invalid {
  border-color: var(--color-danger);
}

.dialog__error {
  margin-top: var(--space-2);
  font-size: var(--font-size-xs);
  color: var(--color-danger);
}
</style>
