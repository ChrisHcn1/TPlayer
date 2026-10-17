/**
 * L2 极端兜底：FFmpeg.wasm 懒加载转码。
 *
 * 设计约束：
 *  - 仅在远程开关 `ffmpegWasmEnabled=true` 且 L1 无法处理时才调用；
 *  - 静态 import：依赖已确定安装；worker 内「变量名动态 import 裸包名」无法被
 *    浏览器原生解析（@vite-ignore 会跳过 Vite 的模块说明符转换）；
 *  - core 文件需放置于 `public/ffmpeg/`（ffmpeg-core.js / ffmpeg-core.wasm），
 *    或用自建 CDN；生产环境务必自托管而非依赖公网；
 *  - FFmpeg 内部 spawn 的 worker 经 classWorkerURL 显式传入（?worker&url 打包）；
 *  - CSP 需允许 script-src blob: 与 'wasm-unsafe-eval'（见 tauri.conf.json）。
 */
import { FFmpeg } from '@ffmpeg/ffmpeg'
import { toBlobURL } from '@ffmpeg/util'
// 用 ?worker&url 让 Vite 打包并返回真实 URL，经 classWorkerURL 显式传入
import classWorkerUrl from '@ffmpeg/ffmpeg/worker?worker&url'

let ffmpegInstance: FFmpeg | null = null
let loadingPromise: Promise<FFmpeg> | null = null

function withTimeout<T>(p: Promise<T>, ms: number, tag: string): Promise<T> {
  return Promise.race([
    p,
    new Promise<T>((_, reject) =>
      setTimeout(() => reject(new Error(`${tag} 超时（${ms}ms，多半是内部 worker 未就绪/CSP 拦截）`)), ms),
    ),
  ])
}

/**
 * 预加载 FFmpeg core（仅加载，不解码具体文件）。
 * 在 app 空闲时调用可消除首次播放最大的等待开销（32MB wasm 下载 + 实例化）。
 * 失败静默，不影响正式播放（首次播放时会再次尝试）。
 */
export async function preloadFfmpeg(): Promise<void> {
  try {
    await loadFfmpeg()
  } catch {
    /* 预加载失败，首次播放时再尝试 */
  }
}

/** 初始化 FFmpeg 实例（并发安全：多次调用复用同一加载 Promise） */
function loadFfmpeg(): Promise<FFmpeg> {
  if (ffmpegInstance) return Promise.resolve(ffmpegInstance)
  if (loadingPromise) return loadingPromise

  loadingPromise = (async () => {
    const base = ((import.meta as any).env?.BASE_URL as string | undefined) ?? './'
    const coreURL = `${base}ffmpeg/ffmpeg-core.js`
    const wasmURL = `${base}ffmpeg/ffmpeg-core.wasm`

    const ff = new FFmpeg()
    ff.on('progress', () => {})

    await withTimeout(
      ff.load({
        coreURL: await toBlobURL(coreURL, 'text/javascript'),
        wasmURL: await toBlobURL(wasmURL, 'application/wasm'),
        classWorkerURL: classWorkerUrl,
      }),
      60000,
      'ffmpeg core load',
    )
    ffmpegInstance = ff
    return ff
  })()

  loadingPromise.catch(() => {
    // 加载失败允许下次重试
    loadingPromise = null
  })
  return loadingPromise
}

/** 用 FFmpeg.wasm 把任意音频字节转码为 16-bit PCM WAV。失败返回 null。 */
export async function ffmpegDecode(buffer: ArrayBuffer): Promise<ArrayBuffer | null> {
  try {
    const ff = await loadFfmpeg()

    await ff.writeFile('input', new Uint8Array(buffer))
    await withTimeout(
      ff.exec(['-i', 'input', '-f', 'wav', '-acodec', 'pcm_s16le', '-ar', '44100', 'output.wav']),
      120000,
      'ffmpeg exec',
    )
    const data = (await ff.readFile('output.wav')) as Uint8Array
    console.log('[compat-ffmpeg] decoded output bytes=', data.byteLength)
    return data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength) as ArrayBuffer
  } catch (err) {
    console.log('[compat-ffmpeg] error', err)
    return null
  }
}
