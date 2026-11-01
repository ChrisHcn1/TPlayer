/**
 * 实验室·悬浮歌词
 *
 * 实现「脱离主窗口」的悬浮歌词：开启后由后端创建一个**独立**的 Tauri WebView 窗口
 * （label = "floating-lyrics"），与主窗口互不依附，可自由拖动、置顶、半透明。
 * 该窗口挂载独立的精简 Vue 应用（src/floating-lyrics/App.vue），通过 IPC 连到同一
 * 后端并订阅 playback://event，因此主窗口最小化 / 隐藏 / 关闭（托盘驻留）都不影响其显示。
 *
 * 非 Tauri 环境（浏览器预览）无独立窗口能力，退化为应用内浮层（FloatingLyricsOverlay），
 * 由本模块的 overlayVisible 控制显隐。
 */
import { computed, ref } from 'vue'

import { isTauriRuntime, callCommand } from '../../services/ipc'
import { useLabStore } from '../../stores/lab'

const isTauri = isTauriRuntime()

/** 非 Tauri 环境下的应用内浮层显隐（浏览器预览回退） */
const overlayVisible = ref(false)
/** Tauri 环境下悬浮窗口是否已开的乐观状态 */
const windowOpen = ref(false)

export function useFloatingLyrics() {
  const lab = useLabStore()
  const enabled = computed(() => lab.isEnabled('floating-lyrics'))
  /** 当前是否「显示」：Tauri 看窗口、非 Tauri 看浮层 */
  const isOpen = computed(() => (isTauri ? windowOpen.value : overlayVisible.value))

  async function open(): Promise<void> {
    if (isTauri) {
      try {
        await callCommand('floating_lyrics_open')
        windowOpen.value = true
      } catch (e) {
        // 创建窗口失败（权限/运行时异常）时打印，避免静默无窗口
        console.error('[floating-lyrics] 打开悬浮窗口失败:', e)
        throw e
      }
    } else {
      overlayVisible.value = true
    }
  }

  async function close(): Promise<void> {
    if (isTauri) {
      try {
        await callCommand('floating_lyrics_close')
      } catch (e) {
        console.error('[floating-lyrics] 关闭悬浮窗口失败:', e)
      }
      windowOpen.value = false
    } else {
      overlayVisible.value = false
    }
  }

  async function toggleVisible(): Promise<void> {
    if (isOpen.value) await close()
    else await open()
  }

  return { enabled, isOpen, overlayVisible, open, close, toggleVisible }
}
