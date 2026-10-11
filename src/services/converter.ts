/**
 * 音频转换服务
 *
 *  - Tauri 实现：converter_start / converter_cancel / converter_probe_files 命令，
 *    订阅 Rust 侧 converter://event（真实 ffmpeg 转码，进度/取消/失败全覆盖）；
 *  - Web mock：浏览器预览环境用本地定时器模拟同一套事件结构，UI 联调零改动。
 */
import { listen } from '@tauri-apps/api/event'

import type {
  ConvertEvent,
  ConvertFileProbe,
  ConvertOptions,
  ConvertTaskStart,
  ConverterService,
} from '../types'
import { callCommand, isTauriRuntime } from './ipc'

const tauriConverter: ConverterService = {
  async start(tasks: ConvertTaskStart[], options: ConvertOptions): Promise<void> {
    await callCommand('converter_start', { tasks, options })
  },

  async cancel(): Promise<void> {
    await callCommand('converter_cancel')
  },

  async probeFiles(paths: string[]): Promise<ConvertFileProbe[] | null> {
    try {
      return await callCommand<ConvertFileProbe[]>('converter_probe_files', { paths })
    } catch {
      return null
    }
  },

  subscribe(listener: (event: ConvertEvent) => void): () => void {
    let unlisten: (() => void) | null = null
    void listen<ConvertEvent>('converter://event', (event) => listener(event.payload)).then(
      (un) => {
        unlisten = un
      },
    )
    return () => {
      unlisten?.()
    }
  },
}

/** Web mock：定时器模拟顺序转码，事件结构与 Rust 侧完全一致 */
class WebConverter implements ConverterService {
  private listeners = new Set<(event: ConvertEvent) => void>()
  private timer: ReturnType<typeof setInterval> | null = null
  private queue: ConvertTaskStart[] = []
  private total = 0
  private currentProgress = 0
  private cancelled = false

  subscribe(listener: (event: ConvertEvent) => void): () => void {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  private emit(event: ConvertEvent): void {
    this.listeners.forEach((listener) => listener(event))
  }

  async start(tasks: ConvertTaskStart[]): Promise<void> {
    if (tasks.length === 0) throw new Error('没有待转换的文件')
    if (this.timer !== null) throw new Error('已有转换任务在进行中')
    this.queue = [...tasks]
    this.total = tasks.length
    this.currentProgress = 0
    this.cancelled = false
    this.timer = setInterval(() => this.tick(), 240)
  }

  private tick(): void {
    if (this.cancelled) {
      this.finish(true)
      return
    }
    const current = this.queue[0]
    if (!current) {
      this.finish(false)
      return
    }
    if (this.currentProgress === 0) {
      this.emit({ type: 'started', payload: { taskId: current.id, outputPath: `模拟输出/${current.path}` } })
    }
    this.currentProgress = Math.min(100, this.currentProgress + Math.round(8 + Math.random() * 14))
    if (this.currentProgress >= 100) {
      this.emit({ type: 'completed', payload: { taskId: current.id, outputPath: `模拟输出/${current.path}` } })
      this.queue.shift()
      this.currentProgress = 0
      if (this.queue.length === 0) this.finish(false)
    } else {
      this.emit({ type: 'progress', payload: { taskId: current.id, progress: this.currentProgress } })
    }
  }

  private finish(cancelled: boolean): void {
    if (this.timer !== null) {
      clearInterval(this.timer)
      this.timer = null
    }
    this.emit({
      type: 'finished',
      payload: { completed: this.total - this.queue.length, failed: 0, cancelled },
    })
    this.queue = []
    this.total = 0
    this.currentProgress = 0
  }

  async cancel(): Promise<void> {
    this.cancelled = true
  }

  async probeFiles(): Promise<ConvertFileProbe[] | null> {
    return null
  }
}

export const converterService: ConverterService = isTauriRuntime()
  ? tauriConverter
  : new WebConverter()
