/**
 * 音频转换 store
 *
 * 持有待转文件、输出设置与任务队列；任务状态由 converter://event 事件驱动
 * （后端为唯一真相源）。输出设置持久化到 localStorage（UI 级偏好）。
 * store 常驻：转换进行中切走页面再回来，任务列表不丢。
 */
import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import { converterService } from '../services/converter'
import { isTauriRuntime } from '../services/ipc'
import type { ConvertEvent, ConvertFormat, ConvertOptions, ConvertTaskStart } from '../types'
import { useToastStore } from './toast'

/** 待转文件条目 */
export interface ConvertFile {
  id: string
  /** 源文件绝对路径（Web mock 下为文件名） */
  path: string
  name: string
  /** 文件大小（MB；探测失败为 0） */
  sizeMb: number
}

/** 任务状态 */
export type ConvertTaskStatus = 'pending' | 'running' | 'done' | 'failed' | 'cancelled'

/** 转换任务条目 */
export interface ConvertTask {
  id: string
  name: string
  status: ConvertTaskStatus
  /** 0 ~ 100 */
  progress: number
  /** 输出文件路径（started 事件回填） */
  outputPath: string | null
  /** 失败原因 */
  message: string | null
}

/** 输出设置（持久化部分） */
export interface ConverterSettings {
  format: ConvertFormat
  sampleRate: number
  bitDepth: 16 | 24
  outputDir: string
}

const SETTINGS_KEY = 'tplayer.converter.settings'

const DEFAULT_SETTINGS: ConverterSettings = {
  format: 'flac',
  sampleRate: 48000,
  bitDepth: 16,
  outputDir: '',
}

function loadSettings(): ConverterSettings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY)
    if (!raw) return { ...DEFAULT_SETTINGS }
    const parsed = JSON.parse(raw) as Partial<ConverterSettings>
    return { ...DEFAULT_SETTINGS, ...parsed }
  } catch {
    return { ...DEFAULT_SETTINGS }
  }
}

let seq = 0
function nextId(prefix: string): string {
  seq += 1
  return `${prefix}-${Date.now()}-${seq}`
}

export const useConverterStore = defineStore('converter', () => {
  const toast = useToastStore()

  const files = ref<ConvertFile[]>([])
  const settings = ref<ConverterSettings>(loadSettings())
  const tasks = ref<ConvertTask[]>([])
  const running = ref(false)

  let unsubscribe: (() => void) | null = null
  let subscribed = false

  const isTauri = isTauriRuntime()
  const hasRunning = computed(() => running.value)
  const finishedCount = computed(
    () => tasks.value.filter((t) => t.status === 'done' || t.status === 'failed' || t.status === 'cancelled').length,
  )

  function ensureSubscribed(): void {
    if (subscribed) return
    subscribed = true
    unsubscribe = converterService.subscribe((event) => applyEvent(event))
  }

  /** 视图挂载时调用：先订阅再启动任务，避免首批事件丢失 */
  function init(): void {
    ensureSubscribed()
  }

  function applyEvent(event: ConvertEvent): void {
    switch (event.type) {
      case 'started': {
        const task = tasks.value.find((t) => t.id === event.payload.taskId)
        if (task) {
          task.status = 'running'
          task.progress = 0
          task.outputPath = event.payload.outputPath
        }
        return
      }
      case 'progress': {
        const task = tasks.value.find((t) => t.id === event.payload.taskId)
        if (task && task.status === 'running') task.progress = event.payload.progress
        return
      }
      case 'completed': {
        const task = tasks.value.find((t) => t.id === event.payload.taskId)
        if (task) {
          task.status = 'done'
          task.progress = 100
        }
        return
      }
      case 'failed': {
        const task = tasks.value.find((t) => t.id === event.payload.taskId)
        if (task) {
          task.status = 'failed'
          task.message = event.payload.message
        }
        return
      }
      case 'finished': {
        running.value = false
        // 取消时未开始的等待任务全部标记为 cancelled
        if (event.payload.cancelled) {
          for (const task of tasks.value) {
            if (task.status === 'pending') {
              task.status = 'cancelled'
              task.message = '已取消'
            }
          }
          toast.info(event.payload.completed > 0 ? `转换已取消，完成 ${event.payload.completed} 个` : '转换已取消')
          return
        }
        if (event.payload.failed > 0) {
          toast.warning(`转换结束：成功 ${event.payload.completed} 个，失败 ${event.payload.failed} 个`)
        } else {
          toast.success(`全部转换完成，共 ${event.payload.completed} 个文件`)
        }
        return
      }
    }
  }

  /** 添加待转文件（绝对路径）；按路径去重，Tauri 下探测文件大小 */
  async function addPaths(paths: string[]): Promise<void> {
    const existing = new Set(files.value.map((file) => file.path))
    const fresh = paths.filter((path) => path && !existing.has(path))
    if (fresh.length === 0) return

    let sizeMap = new Map<string, number>()
    const probes = await converterService.probeFiles(fresh)
    if (probes) {
      sizeMap = new Map(
        probes.filter((p) => p.exists).map((p) => [p.path, p.sizeBytes]),
      )
      // 探测确认不存在的文件直接拒绝
      const missing = probes.filter((p) => !p.exists)
      if (missing.length > 0) toast.warning(`已跳过 ${missing.length} 个不存在的文件`)
    }

    let added = 0
    for (const path of fresh) {
      if (probes && !sizeMap.has(path)) continue
      const name = path.split(/[\\/]/).pop() ?? path
      files.value.push({
        id: nextId('file'),
        path,
        name,
        sizeMb: Math.round(((sizeMap.get(path) ?? 0) / (1024 * 1024)) * 10) / 10,
      })
      added += 1
    }
    if (added > 0) toast.success(`已添加 ${added} 个待转文件`)
  }

  /** 浏览器 mock：按文件名添加（无真实路径） */
  function addNames(names: string[]): void {
    void addPaths(names)
  }

  function removeFile(id: string): void {
    files.value = files.value.filter((file) => file.id !== id)
  }

  function clearFiles(): void {
    files.value = []
  }

  function updateSettings(patch: Partial<ConverterSettings>): void {
    settings.value = { ...settings.value, ...patch }
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings.value))
  }

  /** 启动转换；失败时回滚任务列表并向上抛出（由视图提示） */
  async function start(): Promise<void> {
    if (files.value.length === 0) throw new Error('请先添加待转文件')
    if (running.value) throw new Error('转换进行中，请等待当前任务完成')
    if (isTauri && !settings.value.outputDir.trim()) throw new Error('请先选择输出目录')

    ensureSubscribed()
    const inputs: ConvertTaskStart[] = []
    tasks.value = files.value.map((file) => {
      const id = nextId('task')
      inputs.push({ id, path: file.path })
      return {
        id,
        name: file.name,
        status: 'pending' as const,
        progress: 0,
        outputPath: null,
        message: null,
      }
    })
    running.value = true

    const options: ConvertOptions = {
      format: settings.value.format,
      sampleRate: settings.value.sampleRate,
      bitDepth: settings.value.bitDepth,
      outputDir: settings.value.outputDir.trim(),
    }
    try {
      await converterService.start(inputs, options)
    } catch (error) {
      running.value = false
      tasks.value = []
      throw error
    }
  }

  /** 取消当前批次 */
  async function cancel(): Promise<void> {
    if (!running.value) return
    await converterService.cancel()
  }

  /** 清除已完成 / 已失败 / 已取消的任务 */
  function clearFinished(): void {
    tasks.value = tasks.value.filter((t) => t.status === 'pending' || t.status === 'running')
  }

  function dispose(): void {
    unsubscribe?.()
    unsubscribe = null
    subscribed = false
  }

  return {
    files,
    settings,
    tasks,
    running,
    isTauri,
    hasRunning,
    finishedCount,
    init,
    addPaths,
    addNames,
    removeFile,
    clearFiles,
    updateSettings,
    start,
    cancel,
    clearFinished,
    applyEvent,
    dispose,
  }
})
