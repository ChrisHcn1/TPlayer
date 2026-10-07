<script setup lang="ts">
/**
 * 音频转换（/converter）—— M1 可交互 mock 闭环
 *
 * DESIGN §2.4 图 2-4-6 三卡片：待转文件 / 输出设置 / 任务队列。
 * 浏览器环境不接真实转换后端：文件来自系统文件选择（仅取文件名），
 * 转换进度由本地定时器模拟；M3 替换为 converter 服务命令与进度事件。
 */
import { computed, onBeforeUnmount, ref } from 'vue'

import BaseButton from '../components/common/BaseButton.vue'
import Icon from '../components/common/Icon.vue'
import { useToastStore } from '../stores/toast'

interface ConvertFile {
  id: string
  name: string
  /** 文件大小（MB，mock） */
  sizeMb: number
}

interface ConvertTask {
  id: string
  name: string
  status: 'pending' | 'running' | 'done'
  progress: number
}

const FORMATS = ['MP3 320k', 'FLAC', 'WAV', 'AAC 256k'] as const
const SAMPLE_RATES = ['44.1 kHz', '48 kHz', '96 kHz'] as const
const BIT_DEPTHS = ['16 bit', '24 bit'] as const

const toastStore = useToastStore()

const files = ref<ConvertFile[]>([])
const format = ref<(typeof FORMATS)[number]>('FLAC')
const sampleRate = ref<(typeof SAMPLE_RATES)[number]>('48 kHz')
const bitDepth = ref<(typeof BIT_DEPTHS)[number]>('16 bit')
const outputDir = ref('~/Music/TPlayer Output')

const tasks = ref<ConvertTask[]>([])
const dragOver = ref(false)
const fileInputRef = ref<HTMLInputElement | null>(null)
let timer: number | null = null
let seq = 0

function nextId(prefix: string): string {
  seq += 1
  return `${prefix}-${Date.now()}-${seq}`
}

function addFileNames(names: string[]): void {
  const existing = new Set(files.value.map((file) => file.name))
  let added = 0
  for (const name of names) {
    if (existing.has(name)) continue
    existing.add(name)
    files.value.push({
      id: nextId('file'),
      name,
      sizeMb: Math.round((8 + Math.random() * 42) * 10) / 10,
    })
    added += 1
  }
  if (added > 0) toastStore.success(`已添加 ${added} 个待转文件`)
}

function onPickClick(): void {
  fileInputRef.value?.click()
}

function onFileChange(event: Event): void {
  const input = event.target as HTMLInputElement
  addFileNames(Array.from(input.files ?? []).map((file) => file.name))
  input.value = ''
}

function onDrop(event: DragEvent): void {
  dragOver.value = false
  const dropped = event.dataTransfer?.files
  if (dropped && dropped.length > 0) {
    addFileNames(Array.from(dropped).map((file) => file.name))
  }
}

function removeFile(id: string): void {
  files.value = files.value.filter((file) => file.id !== id)
}

function clearFiles(): void {
  files.value = []
}

const hasRunning = computed(() => tasks.value.some((task) => task.status === 'running'))
const doneCount = computed(() => tasks.value.filter((task) => task.status === 'done').length)

function startConvert(): void {
  if (files.value.length === 0) {
    toastStore.warning('请先添加待转文件')
    return
  }
  if (hasRunning.value) {
    toastStore.warning('转换进行中，请等待当前任务完成')
    return
  }
  tasks.value = files.value.map((file) => ({
    id: nextId('task'),
    name: file.name,
    status: 'pending',
    progress: 0,
  }))
  runQueue()
  toastStore.info(`开始转换 ${tasks.value.length} 个文件（${format.value}）`)
}

function runQueue(): void {
  if (timer !== null) window.clearInterval(timer)
  timer = window.setInterval(() => {
    const running = tasks.value.find((task) => task.status === 'running')
    if (running) {
      running.progress = Math.min(100, running.progress + Math.round(6 + Math.random() * 14))
      if (running.progress >= 100) {
        running.status = 'done'
        maybeFinish()
      }
      return
    }
    const next = tasks.value.find((task) => task.status === 'pending')
    if (next) next.status = 'running'
    else maybeFinish()
  }, 240)
}

function maybeFinish(): void {
  if (tasks.value.every((task) => task.status === 'done')) {
    if (timer !== null) {
      window.clearInterval(timer)
      timer = null
    }
    toastStore.success(`全部转换完成，共 ${tasks.value.length} 个文件`)
  }
}

function clearFinished(): void {
  tasks.value = tasks.value.filter((task) => task.status !== 'done')
}

onBeforeUnmount(() => {
  if (timer !== null) window.clearInterval(timer)
})
</script>

<template>
  <section class="converter">
    <header class="converter__header">
      <h1 class="converter__title">音频转换</h1>
      <p class="converter__subtitle">浏览器预览为本地模拟任务，真实转换将在 Tauri 桌面端执行。</p>
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
          <p class="dropzone__hint">支持 MP3 / FLAC / WAV / AAC 等格式</p>
          <input
            ref="fileInputRef"
            type="file"
            multiple
            accept="audio/*"
            hidden
            @change="onFileChange"
          />
        </div>

        <ul v-if="files.length > 0" class="file-list">
          <li v-for="file in files" :key="file.id" class="file-list__item">
            <Icon name="file" :size="16" />
            <span class="file-list__name" :title="file.name">{{ file.name }}</span>
            <span class="file-list__size u-tabular">{{ file.sizeMb }} MB</span>
            <button
              type="button"
              class="file-list__remove"
              aria-label="移除"
              @click="removeFile(file.id)"
            >
              <Icon name="x" :size="14" />
            </button>
          </li>
        </ul>
        <p v-else class="card__hint">尚未添加文件</p>

        <div v-if="files.length > 0" class="card__footer">
          <BaseButton variant="ghost" size="sm" @click="clearFiles">清空列表</BaseButton>
        </div>
      </section>

      <!-- 输出设置 -->
      <section class="card">
        <h2 class="card__title">输出设置</h2>

        <label class="field">
          <span class="field__label">输出格式</span>
          <select v-model="format" class="field__control">
            <option v-for="item in FORMATS" :key="item" :value="item">{{ item }}</option>
          </select>
        </label>

        <label class="field">
          <span class="field__label">采样率</span>
          <select v-model="sampleRate" class="field__control">
            <option v-for="item in SAMPLE_RATES" :key="item" :value="item">{{ item }}</option>
          </select>
        </label>

        <label class="field">
          <span class="field__label">位深</span>
          <select v-model="bitDepth" class="field__control" :disabled="format.startsWith('MP3') || format.startsWith('AAC')">
            <option v-for="item in BIT_DEPTHS" :key="item" :value="item">{{ item }}</option>
          </select>
        </label>

        <label class="field">
          <span class="field__label">输出目录</span>
          <input v-model="outputDir" type="text" class="field__control" placeholder="选择输出目录" />
        </label>

        <div class="card__footer card__footer--end">
          <BaseButton variant="primary" size="md" :disabled="hasRunning" @click="startConvert">
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
            v-if="doneCount > 0"
            variant="ghost"
            size="sm"
            :disabled="hasRunning"
            @click="clearFinished"
          >
            <Icon name="trash-2" :size="14" />
            清除已完成
          </BaseButton>
        </div>

        <ul v-if="tasks.length > 0" class="task-list">
          <li v-for="task in tasks" :key="task.id" class="task-list__item">
            <div class="task-list__meta">
              <span class="task-list__name" :title="task.name">{{ task.name }}</span>
              <span class="task-list__status">
                <template v-if="task.status === 'pending'">等待中</template>
                <template v-else-if="task.status === 'running'">转换中 {{ task.progress }}%</template>
                <template v-else>已完成</template>
              </span>
            </div>
            <div class="task-list__bar">
              <div
                class="task-list__fill"
                :class="{ 'task-list__fill--done': task.status === 'done' }"
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
</style>
