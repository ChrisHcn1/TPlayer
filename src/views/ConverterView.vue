<script setup lang="ts">
/**
 * 音频转换（/converter，设置-工具页签内嵌）—— M3 接入 Rust 后端
 *
 * DESIGN §2.4 图 2-4-6 三卡片：待转文件 / 输出设置 / 任务队列。
 * Tauri 环境：文件经系统对话框选择（真实路径），转换由 Rust 侧 ffmpeg
 * 执行，进度/完成/失败经 converter://event 事件驱动（store 持有状态）。
 * 浏览器预览：文件选择降级为 <input>，任务由 Web mock 模拟，交互一致。
 */
import { computed, onMounted, ref } from 'vue'
import { storeToRefs } from 'pinia'

import BaseButton from '../components/common/BaseButton.vue'
import Icon from '../components/common/Icon.vue'
import { pickAudioFiles, pickDirectory } from '../services/dialog'
import { useConverterStore } from '../stores/converter'
import { useToastStore } from '../stores/toast'
import type { ConvertFormat } from '../types'

const FORMAT_OPTIONS: { value: ConvertFormat; label: string }[] = [
  { value: 'mp3-320', label: 'MP3 320k' },
  { value: 'flac', label: 'FLAC' },
  { value: 'wav', label: 'WAV' },
  { value: 'aac-256', label: 'AAC 256k' },
]
const SAMPLE_RATE_OPTIONS = [
  { value: 44100, label: '44.1 kHz' },
  { value: 48000, label: '48 kHz' },
  { value: 96000, label: '96 kHz' },
]
const BIT_DEPTH_OPTIONS = [16, 24] as const

const toastStore = useToastStore()
const converterStore = useConverterStore()
const { files, settings, tasks, running, isTauri, finishedCount } = storeToRefs(converterStore)

const dragOver = ref(false)
const fileInputRef = ref<HTMLInputElement | null>(null)

/** 有损格式不支持位深设置 */
const lossyFormat = computed(
  () => settings.value.format === 'mp3-320' || settings.value.format === 'aac-256',
)

const taskStatusText = (task: (typeof tasks.value)[number]): string => {
  switch (task.status) {
    case 'pending':
      return '等待中'
    case 'running':
      return `转换中 ${task.progress}%`
    case 'done':
      return '已完成'
    case 'failed':
      return `失败：${task.message ?? '未知错误'}`
    case 'cancelled':
      return '已取消'
  }
}

/** 选择待转文件：Tauri 用系统对话框（真实路径），浏览器降级为 input */
async function onPickClick(): Promise<void> {
  if (isTauri.value) {
    const paths = await pickAudioFiles()
    if (paths && paths.length > 0) await converterStore.addPaths(paths)
    return
  }
  fileInputRef.value?.click()
}

function onFileChange(event: Event): void {
  const input = event.target as HTMLInputElement
  const names = Array.from(input.files ?? []).map((file) => file.name)
  if (names.length > 0) void converterStore.addPaths(names)
  input.value = ''
}

function onDrop(event: DragEvent): void {
  dragOver.value = false
  // Tauri WebView 拖拽拿到的是 File 对象（无绝对路径）：浏览器行为与 input 一致
  const dropped = event.dataTransfer?.files
  if (dropped && dropped.length > 0) {
    void converterStore.addPaths(Array.from(dropped).map((file) => file.name))
  }
}

async function onPickOutputDir(): Promise<void> {
  const dir = await pickDirectory()
  if (dir) converterStore.updateSettings({ outputDir: dir })
}

async function startConvert(): Promise<void> {
  try {
    await converterStore.start()
    toastStore.info(`开始转换 ${files.value.length} 个文件（${formatLabel(settings.value.format)}）`)
  } catch (error) {
    toastStore.warning(error instanceof Error ? error.message : '启动转换失败')
  }
}

function formatLabel(value: ConvertFormat): string {
  return FORMAT_OPTIONS.find((item) => item.value === value)?.label ?? value
}

onMounted(() => {
  // 先订阅再启动任务（幂等）；store 常驻保证切页后任务状态不丢
  converterStore.init()
})
</script>

<template>
  <section class="converter">
    <header class="converter__header">
      <h1 class="converter__title">音频转换</h1>
      <p class="converter__subtitle">
        {{ isTauri ? '基于 FFmpeg 的本地批量转码，输出目录在曲库监控范围内时会自动入库。' : '浏览器预览为本地模拟任务，真实转换将在 Tauri 桌面端执行。' }}
      </p>
    </header>

    <div class="converter__grid">
      <!-- 待转文件 -->
      <section class="card">
        <h2 class="card__title">待转文件</h2>
        <div
          class="dropzone"
          :class="{ 'dropzone--over': dragOver }"
          role="button"
          tabindex="0"
          @click="onPickClick"
          @keydown.enter.prevent="onPickClick"
          @dragover.prevent="dragOver = true"
          @dragleave.prevent="dragOver = false"
          @drop.prevent="onDrop"
        >
          <Icon name="folder-plus" :size="26" />
          <p class="dropzone__text">拖入音频文件，或点击选择</p>
          <p class="dropzone__hint">支持 MP3 / FLAC / WAV / AAC / APE / DSD 等格式</p>
          <input
            ref="fileInputRef"
            type="file"
            multiple
            accept="audio/*,.dsf,.dff,.ape,.wv,.tta,.tak,.dts"
            hidden
            @change="onFileChange"
          />
        </div>

        <ul v-if="files.length > 0" class="file-list">
          <li v-for="file in files" :key="file.id" class="file-list__item">
            <Icon name="file" :size="16" />
            <span class="file-list__name" :title="file.path">{{ file.name }}</span>
            <span v-if="file.sizeMb > 0" class="file-list__size u-tabular">{{ file.sizeMb }} MB</span>
            <button
              type="button"
              class="file-list__remove"
              aria-label="移除"
              :disabled="running"
              @click="converterStore.removeFile(file.id)"
            >
              <Icon name="x" :size="14" />
            </button>
          </li>
        </ul>
        <p v-else class="card__hint">尚未添加文件</p>

        <div v-if="files.length > 0" class="card__footer">
          <BaseButton variant="ghost" size="sm" :disabled="running" @click="converterStore.clearFiles">
            清空列表
          </BaseButton>
        </div>
      </section>

      <!-- 输出设置 -->
      <section class="card">
        <h2 class="card__title">输出设置</h2>

        <label class="field">
          <span class="field__label">输出格式</span>
          <select
            class="field__control"
            :value="settings.format"
            @change="converterStore.updateSettings({ format: ($event.target as HTMLSelectElement).value as ConvertFormat })"
          >
            <option v-for="item in FORMAT_OPTIONS" :key="item.value" :value="item.value">{{ item.label }}</option>
          </select>
        </label>

        <label class="field">
          <span class="field__label">采样率</span>
          <select
            class="field__control"
            :value="settings.sampleRate"
            @change="converterStore.updateSettings({ sampleRate: Number(($event.target as HTMLSelectElement).value) })"
          >
            <option v-for="item in SAMPLE_RATE_OPTIONS" :key="item.value" :value="item.value">{{ item.label }}</option>
          </select>
        </label>

        <label class="field">
          <span class="field__label">位深</span>
          <select
            class="field__control"
            :disabled="lossyFormat"
            :value="settings.bitDepth"
            @change="converterStore.updateSettings({ bitDepth: Number(($event.target as HTMLSelectElement).value) as 16 | 24 })"
          >
            <option v-for="item in BIT_DEPTH_OPTIONS" :key="item" :value="item">{{ item }} bit</option>
          </select>
        </label>

        <div class="field">
          <span class="field__label">输出目录</span>
          <div class="field__row">
            <input
              class="field__control"
              type="text"
              placeholder="选择输出目录"
              :value="settings.outputDir"
              :disabled="isTauri"
              @input="converterStore.updateSettings({ outputDir: ($event.target as HTMLInputElement).value })"
            />
            <BaseButton variant="ghost" size="sm" @click="onPickOutputDir">
              <Icon name="folder" :size="14" />
              浏览
            </BaseButton>
          </div>
        </div>

        <div class="card__footer card__footer--end">
          <BaseButton v-if="running" variant="ghost" size="md" @click="converterStore.cancel">
            <Icon name="x" :size="16" />
            取消转换
          </BaseButton>
          <BaseButton variant="primary" size="md" :disabled="running" @click="startConvert">
            <Icon name="swap" :size="16" />
            开始转换
          </BaseButton>
        </div>
      </section>

      <!-- 任务队列 -->
      <section class="card card--wide">
        <div class="card__head">
          <h2 class="card__title">任务队列</h2>
          <BaseButton
            v-if="finishedCount > 0"
            variant="ghost"
            size="sm"
            :disabled="running"
            @click="converterStore.clearFinished"
          >
            <Icon name="trash-2" :size="14" />
            清除已结束
          </BaseButton>
        </div>

        <ul v-if="tasks.length > 0" class="task-list">
          <li v-for="task in tasks" :key="task.id" class="task-list__item">
            <div class="task-list__meta">
              <span class="task-list__name" :title="task.outputPath ?? task.name">{{ task.name }}</span>
              <span
                class="task-list__status"
                :class="{
                  'task-list__status--failed': task.status === 'failed',
                  'task-list__status--done': task.status === 'done',
                }"
              >
                {{ taskStatusText(task) }}
              </span>
            </div>
            <div class="task-list__bar">
              <div
                class="task-list__fill"
                :class="{
                  'task-list__fill--done': task.status === 'done',
                  'task-list__fill--failed': task.status === 'failed',
                  'task-list__fill--cancelled': task.status === 'cancelled',
                }"
                :style="{ width: `${task.status === 'done' ? 100 : task.progress}%` }"
              ></div>
            </div>
          </li>
        </ul>
        <p v-else class="card__hint">添加文件并点击「开始转换」后，任务将显示在这里。</p>
      </section>
    </div>
  </section>
</template>

<style scoped>
.converter {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  /* 外边距由外壳 .app-shell__view-pad 统一提供（独立路由 / 内嵌设置页两种用法均一致） */
}

.converter__title {
  margin: 0;
  font-size: var(--font-size-lg);
  font-weight: var(--font-weight-semibold);
}

.converter__subtitle {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

.converter__grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: var(--space-4);
  align-items: start;
}

.card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  background-color: var(--color-bg-elevated);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-elevation-1);
}

.card--wide {
  grid-column: 1 / -1;
}

.card__head,
.card__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.card__footer--end {
  justify-content: flex-end;
  gap: var(--space-2);
}

.card__title {
  margin: 0;
  font-size: var(--font-size-md);
  font-weight: var(--font-weight-semibold);
}

.card__hint {
  margin: 0;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

/* ---- 拖放区 ---- */
.dropzone {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-6) var(--space-4);
  color: var(--color-text-tertiary);
  background-color: var(--color-bg-surface);
  border: 1px dashed var(--color-border-strong);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition:
    border-color var(--transition-colors),
    color var(--transition-colors);
}

.dropzone:hover,
.dropzone--over {
  color: var(--color-brand);
  border-color: var(--color-brand);
}

.dropzone__text {
  margin: 0;
  font-size: var(--font-size-sm);
}

.dropzone__hint {
  margin: 0;
  font-size: var(--font-size-xs);
  color: var(--color-text-disabled);
}

.dropzone:focus-visible {
  outline: 2px solid var(--color-focus-ring);
  outline-offset: 2px;
}

/* ---- 文件列表 ---- */
.file-list {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  max-height: 220px;
  overflow-y: auto;
}

.file-list__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2);
  color: var(--color-text-secondary);
  border-radius: var(--radius-xs);
}

.file-list__item:hover {
  background-color: var(--color-bg-hover);
}

.file-list__name {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  font-size: var(--font-size-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-list__size {
  flex: 0 0 auto;
  font-size: var(--font-size-2xs, 11px);
  color: var(--color-text-tertiary);
}

.file-list__remove {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-h);
  height: var(--size-control-h);
  color: var(--color-text-tertiary);
  border-radius: var(--radius-xs);
}

.file-list__remove:hover {
  color: var(--color-danger);
  background-color: var(--color-danger-bg);
}

/* ---- 表单 ---- */
.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.field__label {
  font-size: var(--font-size-xs);
  color: var(--color-text-secondary);
}

.field__row {
  display: flex;
  gap: var(--space-2);
  align-items: stretch;
}

.field__row .field__control {
  flex: 1 1 auto;
  min-width: 0;
}

.field__control {
  width: 100%;
  padding: var(--space-2) var(--space-3);
  font-size: var(--font-size-sm);
  color: var(--color-text-primary);
  background-color: var(--color-bg-input);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
}

.field__control:focus-visible {
  outline: 2px solid var(--color-focus-ring);
  outline-offset: 1px;
}

.field__control:disabled {
  color: var(--color-text-disabled);
  cursor: not-allowed;
}

/* ---- 任务队列 ---- */
.task-list {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.task-list__meta {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-3);
  margin-bottom: var(--space-1);
}

.task-list__name {
  overflow: hidden;
  font-size: var(--font-size-xs);
  color: var(--color-text-primary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-list__status {
  flex: 0 0 auto;
  font-size: var(--font-size-2xs, 11px);
  color: var(--color-text-tertiary);
}

.task-list__status--failed {
  color: var(--color-danger);
}

.task-list__status--done {
  color: var(--color-success);
}

.task-list__bar {
  height: 4px;
  background-color: var(--color-bg-hover);
  border-radius: var(--radius-full);
  overflow: hidden;
}

.task-list__fill {
  height: 100%;
  background-color: var(--color-brand);
  border-radius: var(--radius-full);
  transition: width var(--transition-colors);
}

.task-list__fill--done {
  background-color: var(--color-success);
}

.task-list__fill--failed {
  background-color: var(--color-danger);
}

.task-list__fill--cancelled {
  background-color: var(--color-text-disabled);
}
</style>
