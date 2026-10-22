/**
 * CompatPlayer —— 对现有 IPlayer 实现「零侵入」的包裹层（预判驱动版）。
 *
 * 核心契约（满足需求第 1 条）：
 *  - 仅包一层 play；正常文件（原生可播）完全走原生，零额外开销；
 *  - 播放前读文件头 64 字节做魔数预判（~1-3ms，不依赖后缀名）：
 *      · 原生可播（mp3/flac/ogg）或 WAV 容器 → 先试原生，失败再兜底；
 *      · 其余冷门格式（wma/aiff/ape/dts/dsf/amr 等）→ 直接走兜底解码，省去必败的原生探测；
 *  - 兜底解码（L1 轻量 JS / L2 FFmpeg.wasm）全部在 Web Worker，输出标准 WAV 用隐藏 <audio> 播放；
 *  - 兜底失败（解码不了 / 读字节失败）全部静默降级，绝不阻断主播放器。
 *
 * 接入方式（见 services/player.ts）：
 *   const base = isTauriRuntime() ? tauriPlayer : webPlayer
 *   export const playerService = createCompatPlayer(base)
 *
 * 不修改任何现有播放核心代码（tauriPlayer / webPlayer / WebPlayer / Rust 引擎）。
 */
import type {
  IPlayer,
  PlayContext,
  PlayMode,
  PlaybackEvent,
  PlaybackSnapshot,
  PlayerCapabilities,
  SleepTimerState,
} from '../../types'

import { loadCompatConfig, isFallbackAllowed } from './config'
import { CompatCache } from './cache'
import { Telemetry } from './telemetry'
import { FallbackPlayer } from './fallback-player'
import { detectFormat, isNativePlayable } from './magic'
import { useToastStore } from '../../stores/toast'
import type { CompatConfig, DecodeResponse, TrackMeta } from './types'

/** 主线程诊断日志：同时 console 输出并派发到 window 事件，供屏幕内调试浮层（CompatDebug）显示，不依赖 F12/devtools */
/** 主线程诊断日志开关：正式版关闭；置 true 可同时开启 console 输出与屏幕浮层事件 */
const COMPAT_DEBUG = false

function compatLog(...args: unknown[]): void {
  if (!COMPAT_DEBUG) return
  console.log(...args)
  try {
    if (typeof window !== 'undefined') {
      window.dispatchEvent(
        new CustomEvent('compat-log', {
          detail: args.map((a) => (typeof a === 'string' ? a : JSON.stringify(a))).join(' '),
        }),
      )
    }
  } catch {
    /* 忽略派发失败 */
  }
}

/** 是否运行在 Tauri 桌面环境（Web 预览模式不发声、无兜底，直接走原生） */
function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

export interface CompatOptions {
  /** 由接入层提供曲目元数据解析（filePath/size/modifiedAt）；不提供则尝试从曲库 store 取 */
  resolveTrack?: (trackId: string) => Promise<TrackMeta | null>
}

/** Worker 解码返回的 WAV blob/ab 中读取时长（兜底：Worker 计算异常时） */
export async function computeDurationFromWav(
  wav: ArrayBuffer | Blob,
  fallbackSampleRate?: number,
  fallbackChannels?: number,
): Promise<number> {
  const ab = wav instanceof Blob ? await wav.arrayBuffer() : wav
  try {
    const dv = new DataView(ab)
    const channels = dv.getUint16(22, true) || fallbackChannels || 2
    const sampleRate = dv.getUint32(24, true) || fallbackSampleRate || 44100
    let offset = 12
    while (offset < ab.byteLength - 8) {
      const chunkId = String.fromCharCode(
        dv.getUint8(offset),
        dv.getUint8(offset + 1),
        dv.getUint8(offset + 2),
        dv.getUint8(offset + 3),
      )
      const chunkSize = dv.getUint32(offset + 4, true)
      if (chunkId === 'data') {
        const samples = chunkSize / 2
        return Math.round((samples / channels / sampleRate) * 1000)
      }
      offset += 8 + chunkSize
      if (chunkSize % 2 !== 0) offset++
    }
  } catch {
    /* 忽略 header 解析失败 */
  }
  return 0
}

export class CompatPlayer implements IPlayer {
  private base: IPlayer
  private options: CompatOptions

  private config: CompatConfig | null = null
  private configLoaded = false
  private cache = new CompatCache()
  private telemetry = new Telemetry()
  private fallback: FallbackPlayer
  private listeners = new Set<(e: PlaybackEvent) => void>()
  private worker: Worker | null = null
  private pending = new Map<string, (res: DecodeResponse) => void>()
  private fallbackTrackId: string | null = null
  /** FFmpeg core 是否已加载就绪（worker 预热回执置位） */
  private ffmpegReady = false
  /** core 未就绪时，因播放意图（手动/连播/续播）延后、待 core 就绪自动切播的曲目 */
  private pendingAuto: { trackId: string; meta: TrackMeta } | null = null

  constructor(base: IPlayer, options: CompatOptions = {}) {
    this.base = base
    this.options = options
    this.fallback = new FallbackPlayer((e) => this.onFallbackEvent(e))
    // 空闲时预热 FFmpeg core，消除首次播放的最大等待开销（32MB wasm 下载 + 实例化）
    this.scheduleWarmup()
  }

  /** 延迟构造解码 Worker（避免首屏额外线程开销），并注册消息/错误处理 */
  private ensureWorker(): void {
    if (this.worker) return
    this.worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' })
    this.worker.onmessage = (e: MessageEvent<DecodeResponse>) => {
      const res = e.data
      if (res.preloaded) {
        this.ffmpegReady = true
        const pa = this.pendingAuto
        this.pendingAuto = null
        // core 就绪：自动切播先前因 core 未就绪而被延后的曲目
        if (pa) void this.runFallback(pa.trackId, pa.meta)
        return
      }
      const resolve = this.pending.get(res.trackId)
      if (resolve) {
        this.pending.delete(res.trackId)
        resolve(res)
      }
    }
    this.worker.onerror = () => {
      compatLog('[compat] worker onerror —— module worker 加载/运行失败（检查 Tauri 是否支持 Worker type:module）')
      this.pending.forEach((resolve, id) =>
        resolve({ trackId: id, ok: false, level: 'none', error: 'Worker 异常' }),
      )
      this.pending.clear()
      // 预热失败：清理"待 core 就绪自动切播"的曲目并明确报错，
      // 否则用户会看到"稍后自动播放"却永远等不到回执（永久无声）。
      const pa = this.pendingAuto
      this.pendingAuto = null
      if (pa) {
        this.dispatch({
          type: 'error',
          payload: { message: '解码组件加载失败，无法播放该曲目', trackId: pa.trackId },
        })
      }
    }
  }

  /** 预热：创建 Worker 并提前加载 FFmpeg core */
  public warmup(): void {
    this.ensureWorker()
    this.worker!.postMessage({ preload: true })
  }

  private scheduleWarmup(): void {
    const run = () => this.warmup()
    // 双触发：优先空闲即加载（不阻塞首屏），setTimeout 兜底确保 ~1.5s 内必开始预热；
    // 即便用户打开 app 立刻播放第一首需要 core 的曲目，core 也已提前加载或复用同一加载，
    // 不会重复下载 32MB（worker 侧 loadingPromise 为单例）。
    const ric = (globalThis as unknown as {
      requestIdleCallback?: (cb: () => void, opts?: { timeout: number }) => void
    }).requestIdleCallback
    if (typeof ric === 'function') ric(run, { timeout: 1500 })
    setTimeout(run, 1500)
  }

  /* ---------------- 事件 ---------------- */

  subscribe(listener: (event: PlaybackEvent) => void): () => void {
    this.listeners.add(listener)
    const unBase = this.base.subscribe((e) => this.onBaseEvent(e))
    return () => {
      this.listeners.delete(listener)
      unBase()
    }
  }

  private dispatch(e: PlaybackEvent): void {
    this.listeners.forEach((l) => l(e))
  }

  /** 兜底激活时，屏蔽原生通道的 track/status/progress/ended/rate（由兜底通道接管） */
  private onBaseEvent(e: PlaybackEvent): void {
    if (this.fallbackTrackId) {
      if (e.type === 'track' && e.payload.trackId === this.fallbackTrackId) return
      if (e.type === 'ended' && e.payload.trackId === this.fallbackTrackId) return
      if (e.type === 'status' || e.type === 'progress' || e.type === 'rate') return
    }
    this.dispatch(e)
  }

  /** 兜底通道事件：ended 时驱动后端队列推进（原生通道被接管，不会自行发 ended） */
  private onFallbackEvent(e: PlaybackEvent): void {
    if (e.type === 'ended' && e.payload.trackId === this.fallbackTrackId) {
      // 复用 next()：退出兜底通道后交由后端按队列/循环模式裁决下一曲
      // （下一曲若仍需兜底解码，后端会再次发 compat-play 让本层继续接管）
      void this.next().catch(() => {})
    }
    this.dispatch(e)
  }

  /* ---------------- 配置 / 遥测 ---------------- */

  private async ensureConfig(): Promise<CompatConfig> {
    if (this.configLoaded && this.config) return this.config
    this.config = await loadCompatConfig()
    this.telemetry.setConfig(this.config)
    this.configLoaded = true
    return this.config
  }

  /* ---------------- play（核心接入点） ---------------- */

  async play(trackId: string, context?: PlayContext): Promise<void> {
    // 浏览器预览模式：不发声、无兜底，直接原生
    if (!isTauriRuntime()) {
      await this.base.play(trackId, context)
      return
    }
    const cfg = await this.ensureConfig()
    // 新播放意图：取消任何待 core 就绪自动切播的曲目
    this.pendingAuto = null
    // 未命中灰度 / 回滚：直接走原生，零开销
    if (!isFallbackAllowed(trackId, cfg)) {
      this.stopAll()
      await this.base.play(trackId, context)
      return
    }

    const meta = await this.resolveTrackMeta(trackId)
    if (!meta) {
      this.stopAll()
      await this.base.play(trackId, context)
      return
    }

    // 预判：读文件头 64 字节（极轻 ~1-3ms），决定「先试原生」还是「直接兜底」
    const head = await this.probeHead(meta.filePath)
    let goNative = true
    let detectedKind = 'n/a'
    if (head) {
      const detected = detectFormat(head)
      detectedKind = detected.kind
      const nativePlayable = isNativePlayable(detected.kind)
      // 仅原生可播（mp3/flac/ogg/wav）先试原生；
      // 冷门格式（dts/ape/wma/dsf/dff 等）直接走兜底 L1/L2，省去必败的原生探测
      // （注：WAV 容器在 symphonia 中原生支持良好，已加入 isNativePlayable，
      //   不再强制走 fallback）
      if (!nativePlayable) goNative = false
    }
    compatLog('[compat] play', trackId, 'detected=', detectedKind, 'goNative=', goNative, 'ffmpegReady=', this.ffmpegReady)

    if (goNative) {
      this.stopAll()
      const failed = await this.tryBasePlay(trackId, context)
      if (!failed) return // 原生成功，完全不受影响
      await this.runFallback(trackId, meta) // 原生失败 → 兜底（失败则保持原生错误态）
    } else {
      // 需兜底（DTS/DSD/APE 等）：交后端占位（同步队列上下文/快照 + 发 compat-play 事件），
      // 由 compat-play 监听方走 playCompat 静默接管。手动点歌 / 自动连播 / 断点续播
      // 三条路径统一走这一条链路，保证队列 index 推进与事件序一致。
      // 与 goNative 分支一致：先停掉所有旧通道，否则上一曲的兜底隐藏 <audio>
      // 仍在发声，切换后出现双音频叠加。
      this.stopAll()
      await this.base.play(trackId, context)
    }
  }

  /** compat-play 事件入口：后端已占位（队列/快照已同步），本层静默接管解码播放 */
  async playCompat(trackId: string, _context?: PlayContext): Promise<void> {
    if (!isTauriRuntime()) return
    const meta = await this.resolveTrackMeta(trackId)
    if (!meta) return
    // core 未就绪：不打断当前播放，后台预热，就绪后自动切播（pendingAuto 消费）
    if (!this.ffmpegReady) {
      this.warmup()
      this.pendingAuto = { trackId, meta }
      useToastStore().info('该曲目需加载解码组件，正在后台准备，稍后自动播放')
      compatLog('[compat] 需要 core 未就绪，自动播待命（就绪后切）', trackId)
      return
    }
    // 仅退兜底通道（原生引擎由后端 stage_compat 已停；不调 base.stop，
    // 避免把后端刚同步好的 Loading/Track 状态打回 Stopped）
    if (this.fallbackTrackId !== null || this.fallback.isActive) {
      this.fallback.stop()
      this.fallbackTrackId = null
    }
    const ok = await this.runFallback(trackId, meta)
    compatLog('[compat] playCompat fallback ok=', ok, '→', trackId)
    if (!ok) {
      // L1/L2 解码失败：置错误态。不回退 base.play——原生对该格式必败，
      // 且会再次触发 stage_compat → compat-play 接管回环
      this.dispatch({
        type: 'error',
        payload: { message: '兜底解码失败，无法播放该曲目', trackId },
      })
    }
  }

  /** 切换曲目：停掉所有旧通道（兜底隐藏 <audio> + 原生），避免切换时双声叠加 */
  private stopAll(): void {
    if (this.fallbackTrackId !== null || this.fallback.isActive) {
      this.fallback.stop()
      this.fallbackTrackId = null
    }
    void this.base.stop().catch(() => {})
  }

  /** 返回 true 表示原生播放失败（需兜底） */
  private async tryBasePlay(trackId: string, context?: PlayContext): Promise<boolean> {
    try {
      await this.base.play(trackId, context)
    } catch {
      return true
    }
    // 原生 play 可能已 resolve，但随后异步推送 error 事件（Rust 解码初始化失败）
    return this.waitForError(trackId, 2000)
  }

  private waitForError(trackId: string, ms: number): Promise<boolean> {
    return new Promise((resolve) => {
      const un = this.base.subscribe((e) => {
        if (e.type === 'error' && (e.payload.trackId === trackId || e.payload.trackId === null)) {
          un()
          resolve(true)
        }
      })
      setTimeout(() => {
        un()
        resolve(false)
      }, ms)
    })
  }

  /** 进入兜底：读全量字节 → Worker 解码（L1/L2）→ 隐藏 <audio> 播放。返回是否成功接管 */
  private async runFallback(trackId: string, meta: TrackMeta): Promise<boolean> {
    const start = (globalThis.performance?.now?.() ?? Date.now()) as number
    const cfg = this.config!
    let level: DecodeResponse['level'] = 'none'
    try {
      const cached = await this.cache.get(trackId, meta.fileSize, meta.modifiedAt)
      let blob: Blob | null = null
      let sampleRate = 0
      let channels = 0
      let durationMs = 0

      if (cached) {
        blob = cached.wav
        sampleRate = cached.sampleRate
        channels = cached.channels
        durationMs = cached.durationMs
        // 缓存也可能有 durationMs=0（旧版本写入），从 WAV blob 兜底
        if (durationMs <= 0) {
          durationMs = await computeDurationFromWav(cached.wav, sampleRate, channels)
        }
        level = 'l1'
      } else {
        const bytes = await this.readBytes(meta.filePath)
        if (!bytes) {
          this.telemetry.record(this.sample(trackId, false, 'none', start, '无法读取文件字节'))
          return false
        }
        const res = await this.decode(bytes, trackId, meta, cfg.ffmpegWasmEnabled)
        compatLog('[compat] decode result ok=', res.ok, 'level=', res.level, 'error=', res.error ?? '')
        if (!res.ok || !res.wav) {
          this.telemetry.record(
            this.sample(trackId, false, res.level, start, res.error ?? '解码失败'),
          )
          return false
        }
        blob = new Blob([res.wav], { type: 'audio/wav' })
        sampleRate = res.sampleRate ?? 44100
        channels = res.channels ?? 2
        durationMs = res.durationMs ?? 0
        // 兜底：Worker 计算的 durationMs 为 0（pcm/channels/sampleRate 任一为 0），
        // 直接从 WAV blob 头部 data chunk 计算真实时长
        if (durationMs <= 0) {
          durationMs = await computeDurationFromWav(res.wav!, sampleRate, channels)
        }
        level = res.level
        await this.cache.put(trackId, meta.fileSize, meta.modifiedAt, {
          wav: blob,
          sampleRate,
          channels,
          durationMs,
          size: meta.fileSize,
          modifiedAt: meta.modifiedAt,
        })
      }

      // 接管该曲目：置位后由兜底通道发出 track/status/progress
      this.fallbackTrackId = trackId
      this.fallback.play(trackId, blob, durationMs)
      this.telemetry.record(this.sample(trackId, true, level, start))
      return true
    } catch (err) {
      this.telemetry.record(
        this.sample(trackId, false, level, start, err instanceof Error ? err.message : String(err)),
      )
      return false
    }
  }

  private sample(
    trackId: string,
    success: boolean,
    level: DecodeResponse['level'],
    start: number,
    error?: string,
  ) {
    const durationMs = Math.round((globalThis.performance?.now?.() ?? Date.now()) as number - start)
    return {
      trackId,
      triggered: true,
      success,
      level,
      durationMs: durationMs >= 0 ? durationMs : 0,
      error,
      ts: Date.now(),
    }
  }

  /* ---------------- 元数据 / 字节读取 ---------------- */

  private async resolveTrackMeta(trackId: string): Promise<TrackMeta | null> {
    if (this.options.resolveTrack) {
      try {
        return await this.options.resolveTrack(trackId)
      } catch {
        return null
      }
    }
    // 默认：从曲库 store 动态获取（避免顶层静态依赖，保持模块解耦）
    try {
      const mod = await import('../../stores/library')
      const store = (mod as unknown as { useLibraryStore: () => any }).useLibraryStore()
      const track =
        store.getTrackById?.(trackId) ??
        (store.tracks as unknown[])?.find((t: any) => t.id === trackId)
      if (!track) return null
      return {
        id: track.id,
        filePath: track.filePath,
        fileSize: track.fileSize,
        modifiedAt: track.modifiedAt,
        format: track.format,
      }
    } catch {
      return null
    }
  }

  /** 读文件头前 4KB 做魔数预判（极轻，一次 IO；足够覆盖带 LIST/INFO/封面 chunk 的 WAV 头并深检 data 起始）；失败返回 null */
  private async probeHead(filePath: string): Promise<ArrayBuffer | null> {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const bytes = (await invoke('compat_read_head', { path: filePath, len: 4096 })) as ArrayBuffer
      if (!bytes || bytes.byteLength === 0) return null
      return bytes
    } catch (err) {
      compatLog('[compat] probeHead failed:', typeof err === 'string' ? err : JSON.stringify(err))
      return null
    }
  }

  private async readBytes(filePath: string): Promise<ArrayBuffer | null> {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const bytes = (await invoke('compat_read_full', { path: filePath })) as ArrayBuffer
      if (!bytes || bytes.byteLength === 0) return null
      return bytes
    } catch (err) {
      compatLog('[compat] readBytes failed:', typeof err === 'string' ? err : JSON.stringify(err))
      return null
    }
  }

  /* ---------------- 解码（Worker） ---------------- */

  private decode(
    buffer: ArrayBuffer,
    trackId: string,
    meta: TrackMeta,
    allowFfmpeg: boolean,
  ): Promise<DecodeResponse> {
    this.ensureWorker()
    return new Promise((resolve) => {
      this.pending.set(trackId, resolve)
      this.worker!.postMessage({
        trackId,
        buffer,
        size: meta.fileSize,
        modifiedAt: meta.modifiedAt,
        format: meta.format,
        allowFfmpeg,
      })
    })
  }

  /* ---------------- 其余 IPlayer 方法：兜底通道优先，否则透传原生 ---------------- */

  async pause(): Promise<void> {
    if (this.fallbackTrackId && this.fallback.isActive) {
      this.fallback.pause()
      return
    }
    await this.base.pause()
  }

  async resume(): Promise<void> {
    if (this.fallbackTrackId && this.fallback.isActive) {
      this.fallback.resume()
      return
    }
    await this.base.resume()
  }

  async stop(): Promise<void> {
    if (this.fallbackTrackId) {
      this.fallback.stop()
      this.fallbackTrackId = null
    }
    await this.base.stop()
  }

  async seek(positionMs: number): Promise<void> {
    if (this.fallbackTrackId && this.fallback.isActive) {
      this.fallback.seek(positionMs)
      return
    }
    await this.base.seek(positionMs)
  }

  async setVolume(volume: number): Promise<void> {
    this.fallback.setVolume(volume)
    await this.base.setVolume(volume)
  }

  async setMuted(muted: boolean): Promise<void> {
    this.fallback.setMuted(muted)
    await this.base.setMuted(muted)
  }

  async next(): Promise<void> {
    // 切歌时退出兜底（下一首交由原生处理，失败再兜底）
    if (this.fallbackTrackId) {
      this.fallback.stop()
      this.fallbackTrackId = null
    }
    await this.base.next()
  }

  async previous(): Promise<void> {
    if (this.fallbackTrackId) {
      this.fallback.stop()
      this.fallbackTrackId = null
    }
    await this.base.previous()
  }

  async setMode(mode: PlayMode): Promise<void> {
    await this.base.setMode(mode)
  }

  async setRate(rate: number): Promise<void> {
    // 兜底通道（<audio>）有 rate 属性，立即生效；原生通道透传
    if (this.fallbackTrackId && this.fallback.isActive) {
      this.fallback.setRate(rate)
    }
    await this.base.setRate(rate)
  }

  async setCrossfade(enabled: boolean, durationMs: number): Promise<void> {
    await this.base.setCrossfade(enabled, durationMs)
  }

  async setReplaygain(enabled: boolean): Promise<void> {
    await this.base.setReplaygain(enabled)
  }

  async setSleepTimer(delayMs: number, waitTrackEnd: boolean): Promise<void> {
    await this.base.setSleepTimer(delayMs, waitTrackEnd)
  }

  async cancelSleepTimer(): Promise<void> {
    await this.base.cancelSleepTimer()
  }

  async sleepTimerStatus(): Promise<SleepTimerState> {
    return this.base.sleepTimerStatus()
  }

  async removeFromQueue(index: number): Promise<void> {
    await this.base.removeFromQueue(index)
  }

  async init(): Promise<PlayContext | null> {
    return this.base.init()
  }

  async capabilities(): Promise<PlayerCapabilities> {
    return this.base.capabilities()
  }

  async snapshot(): Promise<PlaybackSnapshot> {
    const base = await this.base.snapshot()
    if (this.fallbackTrackId && this.fallback.isActive) {
      const f = this.fallback.snapshotFields
      return {
        ...base,
        status: f.status,
        trackId: f.trackId,
        positionMs: f.positionMs,
        durationMs: f.durationMs,
        volume: f.volume,
        muted: f.muted,
        error: null,
      }
    }
    return base
  }

  async dispose(): Promise<void> {
    this.worker?.terminate()
    this.fallback.stop()
    await this.base.dispose()
  }
}

/** 工厂：包裹任意 IPlayer 实现，返回兼容层实例 */
export function createCompatPlayer(base: IPlayer, options?: CompatOptions): IPlayer {
  return new CompatPlayer(base, options)
}
