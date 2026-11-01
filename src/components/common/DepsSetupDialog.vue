<script setup lang="ts">
/**
 * 解码组件安装引导弹窗
 *
 * 由 deps store 驱动，三种触发场景：
 *  - 首次启动检测到缺少必需组件（非阻塞提示，基础格式仍可播放）；
 *  - 播放需要 FFmpeg 的格式失败（错误码 DEPENDENCY_MISSING）；
 *  - 用户在「设置 → 解码组件」手动点「重新检测」后仍缺失。
 *
 * 设计：绝不强制阻断——用户可以「稍后再说」继续用基础格式；
 * 弱网场景提供「打开目录手动放置」出口，避免死锁在无法下载的状态。
 */
import { computed, onBeforeUnmount, onMounted } from 'vue'

import { depsService } from '../../services/deps'
import { pickDepsPackage } from '../../services/dialog'
import { useDepsStore } from '../../stores/deps'
import { t } from '../../i18n'
import BaseButton from './BaseButton.vue'
import Icon from './Icon.vue'
import Modal from './Modal.vue'

const deps = useDepsStore()

/** 订阅后端进度事件 */
let unlisten: (() => void) | null = null

onMounted(async () => {
  unlisten = await depsService.onProgress((progress) => deps.onProgress(progress))
})

onBeforeUnmount(() => {
  unlisten?.()
})

/** 下载百分比（总大小未知时退化为不定进度） */
const percent = computed<number | null>(() => {
  const p = deps.progress
  if (!p || !p.total || p.total <= 0) return null
  return Math.min(100, Math.round((p.downloaded / p.total) * 100))
})

const sizeText = computed(() => {
  const p = deps.progress
  if (!p) return ''
  const mb = (bytes: number): string => (bytes / 1024 / 1024).toFixed(1)
  return p.total ? `${mb(p.downloaded)} / ${mb(p.total)} MB` : `${mb(p.downloaded)} MB`
})

/** 阶段提示文案 */
const phaseHint = computed(() => {
  if (deps.phase === 'error') return deps.error || t('settings.depsInstallFailed')
  if (deps.progress?.message) return deps.progress.message
  return t('settings.depsPreparing')
})

/** 缺失的必需工具名 */
const missingRequired = computed(() =>
  (deps.status?.tools ?? [])
    .filter((tool) => tool.required && !tool.installed)
    .map((tool) => deps.toolLabel(tool.name)),
)

/** 是否处于安装中（安装中禁止关闭，避免用户以为中断） */
const busy = computed(() => deps.installing)

async function onInstall(): Promise<void> {
  await deps.install()
}

async function onOpenFolder(): Promise<void> {
  await deps.openFolder()
}

/** 从本地 zip 安装（用户自行下载依赖包后选择） */
async function onInstallLocal(): Promise<void> {
  const picked = await pickDepsPackage()
  if (!picked) return
  await deps.installLocal(picked)
}
</script>

<template>
  <Modal
    :open="deps.guideOpen"
    :title="t('settings.depsInstall')"
    size="md"
    :close-on-overlay="!busy"
    :show-close="!busy"
    @close="deps.closeGuide()"
  >
    <div class="deps-dialog">
      <p class="deps-dialog__lead">
        {{ t('depsDialog.leadPrefix') }}
        <b>{{ t('depsDialog.leadFormats') }}</b>
        {{ t('depsDialog.leadMiddle') }}
        <b>{{ t('depsDialog.leadComponent') }}</b>{{ t('depsDialog.leadEnd') }}
      </p>

      <ul class="deps-dialog__list">
        <li v-for="tool in deps.status?.tools ?? []" :key="tool.name" class="deps-dialog__item">
          <Icon
            :name="tool.installed ? 'check' : 'minus'"
            :size="14"
            :class="tool.installed ? 'deps-dialog__ok' : 'deps-dialog__miss'"
          />
          <span class="deps-dialog__name">{{ deps.toolLabel(tool.name) }}</span>
          <span class="deps-dialog__purpose">{{ deps.toolPurpose(tool.name) }}</span>
          <span class="deps-dialog__state">
            <template v-if="tool.installed">
              {{ deps.sourceLabel(tool.source) }}
            </template>
            <template v-else>{{ t('settings.notInstalled') }}</template>
          </span>
        </li>
      </ul>

      <p class="deps-dialog__note">
        {{ t('depsDialog.noteLead') }}<b>{{ t('depsDialog.noteHighlight') }}</b>{{ t('depsDialog.noteTail', { size: deps.sizeHint() }) }}
      </p>

      <!-- 安装进度 -->
      <div v-if="busy || deps.phase === 'error'" class="deps-dialog__progress">
        <div class="deps-dialog__hint" :class="{ 'deps-dialog__hint--error': deps.phase === 'error' }">
          <Icon :name="deps.phase === 'error' ? 'info' : 'loader'" :size="15" :spin="deps.phase !== 'error'" />
          <span>{{ phaseHint }}</span>
        </div>
        <div class="deps-dialog__bar">
          <div
            class="deps-dialog__fill"
            :class="{ 'deps-dialog__fill--indeterminate': percent === null }"
            :style="{ width: percent === null ? '100%' : `${percent}%` }"
          />
        </div>
        <div v-if="!deps.phase || deps.phase !== 'error'" class="deps-dialog__meta u-tabular">
          <span>{{ percent === null ? t('depsDialog.processing') : `${percent}%` }}</span>
          <span>{{ sizeText }}</span>
        </div>
      </div>

      <!-- 缺失组件清单 -->
      <p v-if="missingRequired.length > 0" class="deps-dialog__missing">
        {{ t('depsDialog.missing', { missing: missingRequired.join(t('settings.depsListSep')) }) }}
      </p>
    </div>

    <template #footer>
      <div class="deps-dialog__footer">
        <BaseButton variant="ghost" size="sm" :disabled="busy" @click="deps.dismissGuide()">
          {{ t('depsDialog.dontAskAgain') }}
        </BaseButton>
        <BaseButton variant="ghost" size="sm" :disabled="busy" @click="deps.closeGuide()">
          {{ t('depsDialog.later') }}
        </BaseButton>
        <BaseButton variant="ghost" size="sm" :disabled="busy" @click="onInstallLocal()">
          <Icon name="folder" :size="15" />
          {{ t('settings.depsInstallLocal') }}
        </BaseButton>
        <BaseButton variant="ghost" size="sm" :disabled="busy" @click="onOpenFolder()">
          <Icon name="folder" :size="15" />
          {{ t('depsDialog.manualPlace') }}
        </BaseButton>
        <BaseButton
          variant="primary"
          size="sm"
          :disabled="busy || !deps.status?.installable"
          @click="onInstall()"
        >
          <Icon name="download" :size="15" />
          {{ busy ? t('depsDialog.installing') : t('depsDialog.installNow') }}
        </BaseButton>
      </div>
    </template>
  </Modal>
</template>

<style scoped>
.deps-dialog {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.deps-dialog__lead {
  margin: 0;
  font-size: var(--font-size-sm);
  line-height: 1.7;
  color: var(--color-text-secondary);
}

.deps-dialog__lead b {
  color: var(--color-text-primary);
}

.deps-dialog__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: var(--space-3);
  list-style: none;
  background-color: var(--color-bg-base);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-sm);
}

.deps-dialog__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-sm);
}

.deps-dialog__ok {
  color: var(--color-success);
}

.deps-dialog__miss {
  color: var(--color-text-tertiary);
}

.deps-dialog__name {
  font-weight: var(--font-weight-medium);
  color: var(--color-text-primary);
}

.deps-dialog__purpose {
  flex: 1 1 auto;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

.deps-dialog__state {
  font-size: var(--font-size-xs);
  color: var(--color-text-secondary);
}

.deps-dialog__note {
  margin: 0;
  font-size: var(--font-size-xs);
  line-height: 1.7;
  color: var(--color-text-tertiary);
}

.deps-dialog__note b {
  color: var(--color-text-secondary);
}

.deps-dialog__missing {
  margin: 0;
  font-size: var(--font-size-xs);
  color: var(--color-warning);
}

.deps-dialog__progress {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.deps-dialog__hint {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
}

.deps-dialog__hint--error {
  color: var(--color-danger);
}

.deps-dialog__bar {
  height: 6px;
  overflow: hidden;
  border-radius: 999px;
  background-color: var(--color-bg-hover);
}

.deps-dialog__fill {
  height: 100%;
  border-radius: var(--radius-full);
  background-color: var(--color-brand-solid);
  transition: width var(--transition-colors);
}

.deps-dialog__fill--indeterminate {
  width: 40%;
  animation: deps-dialog-indeterminate 1.2s ease-in-out infinite;
}

.deps-dialog__meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

.deps-dialog__footer {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

@keyframes deps-dialog-indeterminate {
  0% {
    margin-left: -40%;
  }
  100% {
    margin-left: 100%;
  }
}
</style>
