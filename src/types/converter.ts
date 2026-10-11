/**
 * 音频转换领域模型（前端）
 *
 * 与 Rust 侧 `converter.rs` 对齐：命令参数 camelCase，事件经
 * `converter://event` 推送（tag + payload 结构与 playback://event 同构）。
 */

/** 输出格式 */
export type ConvertFormat = 'mp3-320' | 'flac' | 'wav' | 'aac-256'

/** 转换任务输入（前端生成 ID，事件按此回执关联） */
export interface ConvertTaskStart {
  id: string
  /** 源文件绝对路径 */
  path: string
}

/** 输出设置 */
export interface ConvertOptions {
  format: ConvertFormat
  /** 采样率（Hz） */
  sampleRate: number
  /** 位深（16 / 24，仅无损格式生效） */
  bitDepth: 16 | 24
  /** 输出目录 */
  outputDir: string
}

/** 转换事件（后端 -> 前端推送） */
export type ConvertEvent =
  | { type: 'started'; payload: { taskId: string; outputPath: string } }
  | { type: 'progress'; payload: { taskId: string; progress: number } }
  | { type: 'completed'; payload: { taskId: string; outputPath: string } }
  | { type: 'failed'; payload: { taskId: string; message: string } }
  | { type: 'finished'; payload: { completed: number; failed: number; cancelled: boolean } }

/** 待转文件探测结果（添加文件时展示大小） */
export interface ConvertFileProbe {
  path: string
  sizeBytes: number
  exists: boolean
}

/**
 * 转换器服务契约
 * Tauri 实现：真实 ffmpeg 转码 + converter://event 订阅；
 * Web mock：本地定时器模拟进度，事件结构完全一致，保证浏览器可联调。
 */
export interface ConverterService {
  /** 启动一批转换任务；失败（ffmpeg 缺失/已有批次在跑等）抛出错误 */
  start(tasks: ConvertTaskStart[], options: ConvertOptions): Promise<void>
  /** 取消当前批次 */
  cancel(): Promise<void>
  /** 探测文件信息；非 Tauri 环境返回 null */
  probeFiles(paths: string[]): Promise<ConvertFileProbe[] | null>
  /** 订阅转换事件，返回取消订阅函数 */
  subscribe(listener: (event: ConvertEvent) => void): () => void
}
