/**
 * 解码 Worker —— 所有解码 / 转码逻辑均在此线程执行，主线程零阻塞。
 *
 * 流程：
 *  1. 魔数识别真实格式（不依赖后缀名）；
 *  2. WAV 容器（含轻微头损坏）优先走 L1 轻量 JS 解码（G.711 / IMA / MS ADPCM / PCM）；
 *  3. L1 失败且允许时，懒加载 FFmpeg.wasm（L2）兜底；
 *  4. 输出标准 16-bit PCM WAV 回主线程，或返回 none（无法处理）。
 *
 * 任何异常都被捕获，绝不向主线程抛出（保证主播放器不受影响）。
 */
import { detectFormat, isNativePlayable } from './magic'
import { decodeWavL1, encodeWav, isL1Capable } from './decoders'
import { ffmpegDecode, preloadFfmpeg } from './ffmpeg'
import type { DecodeRequest, DecodeResponse } from './types'

function post(res: DecodeResponse): void {
  ;(self as unknown as Worker).postMessage(res)
}

self.onmessage = async (e: MessageEvent<DecodeRequest>) => {
  const req = e.data
  // 预热：仅提前加载 FFmpeg core，避免首次播放时的 32MB wasm 等待
  if (req.preload) {
    await preloadFfmpeg()
    post({ trackId: '', ok: true, level: 'none', preloaded: true })
    return
  }
  const start = (globalThis.performance?.now?.() ?? Date.now()) as number
  try {
    const detected = detectFormat(req.buffer)
    // 原生可播放且声明格式匹配时仍尝试（因为进入兜底的都是原播放失败的文件）
    if (isL1Capable(detected.kind)) {
      const l1 = decodeWavL1(req.buffer)
      if (l1) {
        const wav = encodeWav(l1.pcm, l1.sampleRate, l1.channels)
        const durationMs = (l1.pcm.length / l1.channels / l1.sampleRate) * 1000
        post({
          trackId: req.trackId,
          ok: true,
          level: 'l1',
          wav,
          sampleRate: l1.sampleRate,
          channels: l1.channels,
          durationMs,
        })
        return
      }
    }

    // L2：FFmpeg.wasm（仅在允许时）
    if (req.allowFfmpeg) {
      const wav = await ffmpegDecode(req.buffer)
      if (wav) {
        // FFmpeg 输出 44.1k/16-bit/双声道标准 WAV，直接读取头部得到元信息
        const dv = new DataView(wav)
        const channels = dv.getUint16(22, true)
        const sampleRate = dv.getUint32(24, true)
        const dataSize = dv.getUint32(40, true)
        const samples = dataSize / 2
        const durationMs = (samples / channels / sampleRate) * 1000
        post({
          trackId: req.trackId,
          ok: true,
          level: 'l2',
          wav,
          sampleRate,
          channels,
          durationMs,
        })
        return
      }
    }

    post({
      trackId: req.trackId,
      ok: false,
      level: 'none',
      error: `L1/L2 均无法处理该格式(${detected.description})`,
    })
  } catch (err) {
    post({
      trackId: req.trackId,
      ok: false,
      level: 'none',
      error: err instanceof Error ? err.message : String(err),
    })
  }
  void start
  void isNativePlayable
}
