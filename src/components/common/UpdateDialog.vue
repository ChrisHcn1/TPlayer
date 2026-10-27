<script setup lang="ts">
/**
 * 应用内更新确认 / 进度弹窗（自动更新编排 UI）
 *
 * 由 updater store 驱动：
 *  - available 阶段：展示 当前 → 新版本、发布说明，提供「立即升级 / 稍后提醒 / 跳过此版本」；
 *  - downloading / installing / error 阶段：展示下载进度（含百分比与已下载量）、失败重试。
 *
 * 设计：后台检查发现的更新才弹此框；用户未确认前绝不下载，绝不影响使用。
 */
import { computed } from 'vue'

import { useUpdaterStore } from '../../stores/updater'
import Modal from './Modal.vue'
import BaseButton from './BaseButton.vue'
import Icon from './Icon.vue'

const updater = useUpdaterStore()

const isConfirm = computed(() => updater.phase === 'available' && updater.update !== null)
const isProgress = computed(
  () => updater.phase === 'downloading' || updater.phase === 'installing' || updater.phase === 'error',
)

const dialogTitle = computed(() => (isConfirm.value ? '发现新版本' : '正在更新'))

/** 下载百分比；服务端未返回总大小时为 null（退化为不定进度） */
const percent = computed<number | null>(() => {
  const { downloaded, total } = updater.progress
  if (!total || total <= 0) return null
  return Math.min(100, Math.round((downloaded / total) * 100))
})

const sizeText = computed(() => {
  const mb = (bytes: number): string => (bytes / 1024 / 1024).toFixed(1)
  const { downloaded, total } = updater.progress
  return total
    ? `${mb(downloaded)} / ${mb(total)} MB`
    : `${mb(downloaded)} MB`
})

const progressHint = computed(() => {
  switch (updater.phase) {
    case 'installing':
      return '正在安装并更新，应用即将重启…'
    case 'error':
      return `下载失败：${updater.error}`
    default:
      return updater.retryAttempt > 0
        ? `下载中断，正在重试（第 ${updater.retryAttempt} 次）…`
        : '正在下载更新…'
  }
})
</script>

<template>
  <Modal
    :open="updater.dialogOpen"
    :title="dialogTitle"
    size="md"
    :close-on-overlay="!isProgress"
    :show-close="!isProgress"
    @close="updater.close()"
  >
    <!-- 确认阶段 -->
    <div v-if="isConfirm && updater.update" class="update-dialog__confirm">
      <p class="update-dialog__ver">
        v{{ updater.update.currentVersion }}
        <Icon name="arrow-right" :size="14" />
        <b>v{{ updater.update.version }}</b>
      </p>
      <pre v-if="updater.update.body" class="update-dialog__notes">{{ updater.update.body }}</pre>
      <p v-else class="update-dialog__empty">本次更新未附带发布说明。</p>
    </div>

    <!-- 下载 / 安装 / 失败阶段 -->
    <div v-else class="update-dialog__progress">
      <div class="update-dialog__hint" :class="{ 'update-dialog__hint--error': updater.phase === 'error' }">
        <Icon
          :name="updater.phase === 'error' ? 'info' : 'loader'"
          :size="15"
          :spin="updater.phase !== 'error'"
        />
        <span>{{ progressHint }}</span>
      </div>

      <div class="update-dialog__bar">
        <div
          class="update-dialog__fill"
          :class="{ 'update-dialog__fill--indeterminate': percent === null }"
          :style="{ width: percent === null ? '100%' : `${percent}%` }"
        />
      </div>

      <div class="update-dialog__meta u-tabular">
        <span>{{ percent === null ? '下载中' : `${percent}%` }}</span>
        <span>{{ sizeText }}</span>
      </div>
    </div>

    <template #footer>
      <div v-if="isConfirm" class="update-dialog__footer">
        <BaseButton variant="ghost" size="sm" @click="updater.skipVersion()">跳过此版本</BaseButton>
        <BaseButton variant="ghost" size="sm" @click="updater.later()">稍后提醒</BaseButton>
        <BaseButton variant="primary" size="sm" @click="updater.confirmInstall()">
          <Icon name="download" :size="15" />
          立即升级
        </BaseButton>
      </div>
      <div v-else class="update-dialog__footer">
        <BaseButton
          v-if="updater.phase === 'error'"
          variant="primary"
          size="sm"
          @click="updater.retry()"
        >
          <Icon name="refresh" :size="15" />
          重试
        </BaseButton>
        <BaseButton
          v-else
          variant="ghost"
          size="sm"
          :disabled="updater.phase === 'installing'"
          @click="updater.close()"
        >
          后台继续
        </BaseButton>
      </div>
    </template>
  </Modal>
</template>

<style scoped>
.update-dialog__confirm {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.update-dialog__ver {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--font-size-md);
  color: var(--color-text-primary);
}

.update-dialog__notes {
  margin: 0;
  max-height: 220px;
  overflow-y: auto;
  padding: var(--space-3) var(--space-4);
  font-family: var(--font-mono, monospace);
  font-size: var(--font-size-xs);
  line-height: 1.6;
  white-space: pre-wrap;
  color: var(--color-text-secondary);
  background-color: var(--color-bg-base);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-sm);
  scrollbar-gutter: stable;
}

.update-dialog__empty {
  margin: 0;
  font-size: var(--font-size-sm);
  color: var(--color-text-tertiary);
}

.update-dialog__progress {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.update-dialog__hint {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
}

.update-dialog__hint--error {
  color: var(--color-danger);
}

.update-dialog__bar {
  height: 6px;
  overflow: hidden;
  border-radius: 999px;
  background-color: var(--color-bg-hover);
}

.update-dialog__fill {
  height: 100%;
  border-radius: var(--radius-full);
  background-color: var(--color-brand-solid);
  transition: width var(--transition-colors);
}

.update-dialog__fill--indeterminate {
  width: 40%;
  animation: update-dialog-indeterminate 1.2s ease-in-out infinite;
}

.update-dialog__meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

@keyframes update-dialog-indeterminate {
  0% {
    margin-left: -40%;
  }
  100% {
    margin-left: 100%;
  }
}
</style>
