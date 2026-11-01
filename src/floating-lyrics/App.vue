<script setup lang="ts">
/**
 * 悬浮歌词窗口（独立 Tauri WebView）
 *
 *  - 整体可拖动（调用 getCurrentWindow().startDragging()）；窗口边缘可拖拽缩放（自定义手柄，无边框也能用）；
 *  - 实时显示当前歌词行，随播放进度同步（player store 订阅 playback://event，与后端同源）；
 *  - 控制：播放/暂停、下一首、设置（宽度/字号/颜色/背景/透明度）、隐藏（关闭本窗口）；
 *  - 宽度（滑块或边缘拖拽）/ 字号 / 颜色 / 背景 / 透明度均持久化到窗口级 localStorage，调整实时生效。
 */
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { getCurrentWindow, LogicalSize, PhysicalPosition } from '@tauri-apps/api/window'

import { usePlayerStore } from '../stores/player'
import { useLyricsStore } from '../stores/lyrics'
import { useFloatingLyrics } from '../lab/features/floating-lyrics'
import { t } from '../i18n'
import Icon from '../components/common/Icon.vue'

const playerStore = usePlayerStore()
const lyricsStore = useLyricsStore()
const fl = useFloatingLyrics()

// 在组件 setup 上下文里初始化播放 store：仅订阅 playback://event + 拉快照
// （skipBackendInit 避免触发后端 player_init 的续播副作用），与后端同源实时同步。
onMounted(async () => {
  playerStore
    .init({ skipBackendInit: true })
    .catch((e) => console.error('[floating-lyrics] player store init failed:', e))
  // 首帧完成后再按已保存的宽度/内容高度贴合一次窗口
  void nextTick(fitWindow)
  // 恢复上次拖动后的位置（无记录则保持 Rust 的居中）
  const saved = loadPos()
  if (saved) {
    try {
      await getCurrentWindow().setPosition(new PhysicalPosition(saved.x, saved.y))
    } catch (e) {
      console.error('[floating-lyrics] 恢复窗口位置失败:', e)
    }
  }
  // 副屏回落：记忆位置若已落在任何显示器之外（副屏拔除等），回到主屏居中
  await ensureOnScreen()
  // 拖拽结束后记录位置，使「可拖动」真正持久（重开/重启后保留）
  window.addEventListener('pointerup', savePos)
})

onUnmounted(() => {
  window.removeEventListener('pointerup', savePos)
})

const snapshot = computed(() => playerStore.snapshot)
const isPlaying = computed(() => snapshot.value.status === 'playing')

/** 拖动窗口：点击非按钮/表单区域即触发系统级拖动 */
async function onDragHandleDown(e: PointerEvent): Promise<void> {
  if ((e.target as HTMLElement).closest('button, input, select, textarea, label, [contenteditable="true"]'))
    return
  await getCurrentWindow().startDragging()
}

/** 歌词随曲目切换加载，并实时取当前行 */
watch(
  () => snapshot.value.trackId,
  (id) => {
    if (id) void lyricsStore.load(id)
    else lyricsStore.clear()
  },
  { immediate: true },
)

const currentLine = computed(() => {
  const l = lyricsStore.lyrics
  if (!l) return ''
  if (!l.synced) return l.lines[0]?.text ?? ''
  const idx = lyricsStore.activeLineIndexAt(snapshot.value.positionMs)
  return idx >= 0 ? l.lines[idx]?.text ?? '' : ''
})

/* ---- 个性化设置（持久化到窗口级 localStorage） ---- */
const STORE_PREFIX = 'tplayer.floatingLyrics.'
function load<T>(key: string, def: T): T {
  try {
    const v = localStorage.getItem(STORE_PREFIX + key)
    return v === null ? def : (JSON.parse(v) as T)
  } catch {
    return def
  }
}
function save(key: string, val: unknown): void {
  try {
    localStorage.setItem(STORE_PREFIX + key, JSON.stringify(val))
  } catch {
    /* 隐私模式等：忽略 */
  }
}

/** 读取已记忆的窗口位置；无记录返回 null（届时由 Rust 居中） */
function loadPos(): { x: number; y: number } | null {
  return load<{ x: number; y: number } | null>('position', null)
}
/** 拖拽结束后记录位置，使「可拖动」跨重开/重启保留 */
function savePos(): void {
  getCurrentWindow()
    .outerPosition()
    .then((p) => save('position', { x: p.x, y: p.y }))
    .catch(() => {})
}

/** 副屏回落：若记忆位置已落在当前屏幕之外（如副屏被拔除 / 分辨率变化），回到当前屏居中 */
async function ensureOnScreen(): Promise<void> {
  try {
    const win = getCurrentWindow()
    const pos = await win.outerPosition()
    const size = await win.outerSize()
    // 该 Tauri 版本未导出 availableMonitors；用 DOM screen 判定当前可见屏边界
    const scr = screen as unknown as { left: number; top: number; width: number; height: number }
    const sl = scr.left ?? 0
    const st = scr.top ?? 0
    const sw = scr.width ?? 0
    const sh = scr.height ?? 0
    if (!sw || !sh) return
    const cx = pos.x + size.width / 2
    const cy = pos.y + size.height / 2
    const visible = cx >= sl && cx <= sl + sw && cy >= st && cy <= st + sh
    if (!visible) await win.center()
  } catch (e) {
    console.error('[floating-lyrics] 副屏回落失败:', e)
  }
}

/** 用户手动设定的高度（px）；0 表示未设定，高度按内容自适应 */
const manualHeight = ref(load('height', 0))
watch(manualHeight, (v) => save('height', v))

/**
 * 边缘拖拽缩放（窗口无边框，原生边缘无法抓取，故用自定义手柄）。
 * 方向：e/w 改宽度，s/se/sw 改高度；顶部有控制按钮，故不做上边缘缩放。
 * 左边缘（w）缩放时同步平移窗口位置，使右边缘保持不动。
 */
type ResizeDir = 'e' | 'w' | 's' | 'se' | 'sw'
const resizing = ref<{
  dir: ResizeDir
  startX: number
  startY: number
  startW: number
  startH: number
  startPosX: number
  startPosY: number
} | null>(null)

function startResize(e: PointerEvent, dir: ResizeDir): void {
  if ((e.target as HTMLElement).closest('button, input, select, textarea, label')) return
  e.preventDefault()
  e.stopPropagation()
  const root = document.querySelector('.fl-win') as HTMLElement | null
  const startH = manualHeight.value > 0 ? manualHeight.value : Math.ceil(root?.getBoundingClientRect().height ?? 120)
  getCurrentWindow()
    .outerPosition()
    .then((p) => {
      resizing.value = {
        dir,
        startX: e.clientX,
        startY: e.clientY,
        startW: width.value,
        startH,
        startPosX: p.x,
        startPosY: p.y,
      }
      window.addEventListener('pointermove', onResizeMove)
      window.addEventListener('pointerup', onResizeUp)
    })
    .catch(() => {})
}

function onResizeMove(e: PointerEvent): void {
  const r = resizing.value
  if (!r) return
  const dx = e.clientX - r.startX
  const dy = e.clientY - r.startY
  let newW = r.startW
  let newH = r.startH
  if (r.dir.includes('e')) newW = r.startW + dx
  if (r.dir.includes('w')) newW = r.startW - dx
  if (r.dir.includes('s')) newH = r.startH + dy
  newW = Math.max(260, Math.round(newW))
  newH = Math.max(64, Math.round(newH))
  width.value = newW
  if (r.dir.includes('s')) manualHeight.value = newH
  // 左边缘缩放：固定右边缘 → 同步平移窗口位置
  if (r.dir.includes('w')) {
    const newX = r.startPosX + (r.startW - newW)
    getCurrentWindow().setPosition(new PhysicalPosition(newX, r.startPosY)).catch(() => {})
  }
}

function onResizeUp(): void {
  resizing.value = null
  window.removeEventListener('pointermove', onResizeMove)
  window.removeEventListener('pointerup', onResizeUp)
}

/** 窗口宽度（像素）。高度按内容自适应（manualHeight>0 时不低于该值），故仅持久化宽度。 */
const width = ref(load('width', 440))
/** 字号（px） */
const fontSize = ref(load('fontSize', 22))
/** 歌词文字颜色 */
const color = ref(load('color', '#ffffff'))
/** 面板背景颜色（实色，配合 opacity 实现半透明磨砂） */
const bgColor = ref(load('bgColor', '#12141a'))
/** 整体不透明度（0.3~1） */
const opacity = ref(load('opacity', 0.85))
/** 是否展开设置面板（展开会改变窗口高度，需重新贴合窗口） */
const showSettings = ref(false)
/** 鼠标是否悬停在组件上：仅悬停时显示控制按钮，保持默认极简 */
const hovering = ref(false)

watch(width, (v) => save('width', v))
watch(fontSize, (v) => save('fontSize', v))
watch(color, (v) => save('color', v))
watch(bgColor, (v) => save('bgColor', v))
watch(opacity, (v) => save('opacity', v))

/**
 * 把（用户设定的宽度 + 内容自适应高度）同步到独立窗口。
 * - 宽度：完全由滑块控制，用户可自由拉伸；
 * - 高度：按当前内容实测，避免固定高度导致「设置展开时被裁切」或「窄屏时留白」。
 */
async function fitWindow(): Promise<void> {
  const root = document.querySelector('.fl-win') as HTMLElement | null
  if (!root) return
  const h = Math.ceil(root.getBoundingClientRect().height)
  try {
    await getCurrentWindow().setSize(new LogicalSize(Math.round(width.value), h + 1))
  } catch (e) {
    console.error('[floating-lyrics] 调整窗口尺寸失败:', e)
  }
}

// 宽度 / 字号变化会改变窗口尺寸；设置面板展开、悬停进出都会改高度 —— 均需重新贴合窗口
watch([width, fontSize, showSettings, hovering], () => {
  void nextTick(fitWindow)
})

function togglePlay(): void {
  if (isPlaying.value) void playerStore.pause()
  else void playerStore.resume()
}
function next(): void {
  void playerStore.next()
}
function hide(): void {
  void fl.close()
}
</script>

<template>
  <div
    class="fl-win"
    :class="{ 'is-hover': hovering }"
    :style="{
      fontSize: fontSize + 'px',
      color,
      background: bgColor,
      opacity: String(opacity),
      minHeight: manualHeight > 0 ? manualHeight + 'px' : undefined,
    }"
    @pointerdown="onDragHandleDown"
    @mouseenter="hovering = true"
    @mouseleave="hovering = false"
  >
    <div class="fl-win__line">
      {{ currentLine || t('lab.floating.noLyrics') }}
    </div>

    <div class="fl-win__controls">
      <button
        type="button"
        :aria-label="isPlaying ? t('lab.floating.pause') : t('lab.floating.play')"
        :title="isPlaying ? t('lab.floating.pause') : t('lab.floating.play')"
        @click.stop="togglePlay"
      >
        <Icon :name="isPlaying ? 'pause' : 'play'" :size="16" />
      </button>
      <button type="button" :aria-label="t('lab.floating.next')" :title="t('lab.floating.next')" @click.stop="next">
        <Icon name="next" :size="16" />
      </button>
      <button
        type="button"
        :aria-label="t('lab.floating.settings')"
        :title="t('lab.floating.settings')"
        @click.stop="showSettings = !showSettings"
      >
        <Icon name="settings" :size="16" />
      </button>
      <button type="button" :aria-label="t('lab.floating.hide')" :title="t('lab.floating.hide')" @click.stop="hide">
        <Icon name="x" :size="16" />
      </button>
    </div>

    <div v-if="showSettings && hovering" class="fl-win__settings" @pointerdown.stop>
      <div class="fl-grp">
        <div class="fl-grp__title">{{ t('lab.floating.size') }}</div>
        <label>
          {{ t('lab.floating.width') }}
          <input type="range" min="260" max="900" step="10" v-model.number="width" />
        </label>
        <label>
          {{ t('lab.floating.fontSize') }}
          <input type="range" min="12" max="48" step="1" v-model.number="fontSize" />
        </label>
      </div>
      <div class="fl-grp">
        <div class="fl-grp__title">{{ t('lab.floating.appearance') }}</div>
        <label>
          {{ t('lab.floating.color') }}
          <input type="color" v-model="color" />
        </label>
        <label>
          {{ t('lab.floating.bgColor') }}
          <input type="color" v-model="bgColor" />
        </label>
        <label>
          {{ t('lab.floating.opacity') }}
          <input type="range" min="0.3" max="1" step="0.05" v-model.number="opacity" />
        </label>
      </div>
    </div>

    <!-- 边缘拖拽缩放：e/w 改宽，s/se/sw 改高（顶部有控制按钮，故不做上边缘缩放） -->
    <div class="fl-resize fl-resize--e" @pointerdown.stop.prevent="startResize($event, 'e')" />
    <div class="fl-resize fl-resize--w" @pointerdown.stop.prevent="startResize($event, 'w')" />
    <div class="fl-resize fl-resize--s" @pointerdown.stop.prevent="startResize($event, 's')" />
    <div class="fl-resize fl-resize--se" @pointerdown.stop.prevent="startResize($event, 'se')" />
    <div class="fl-resize fl-resize--sw" @pointerdown.stop.prevent="startResize($event, 'sw')" />
  </div>
</template>

<style scoped>
.fl-win {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 0;
  padding: 10px 14px;
  border-radius: 12px;
  /* 半透明磨砂底：保证任意桌面背景下文字可读，又满足「半透明」诉求 */
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  box-shadow: 0 4px 18px rgba(0, 0, 0, 0.35);
  user-select: none;
  cursor: grab;
  width: 100%;
  box-sizing: border-box;
}
.fl-win:active {
  cursor: grabbing;
}
.fl-win__line {
  font-weight: 600;
  line-height: 1.3;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  /* 歌词居中显示 */
  text-align: center;
}
.fl-win__controls {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  /* 默认收起：不占布局，保证「仅显示歌词」的极简状态 */
  max-height: 0;
  margin-top: 0;
  opacity: 0;
  overflow: hidden;
  pointer-events: none;
  transition: opacity 0.15s ease;
}
/* 悬停时展开：高度即时变化（避免 CSS 高度过渡干扰窗口 fitWindow 测量），仅淡入按钮 */
.fl-win.is-hover .fl-win__controls {
  max-height: 32px;
  margin-top: 6px;
  opacity: 1;
  pointer-events: auto;
}
.fl-win__controls button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  color: inherit;
  background: rgba(255, 255, 255, 0.12);
  border: none;
  border-radius: 50%;
  cursor: pointer;
}
.fl-win__controls button:hover {
  background: rgba(255, 255, 255, 0.22);
}
.fl-win__settings {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 6px;
  padding-top: 6px;
  font-size: 12px;
}
.fl-grp {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.fl-grp__title {
  font-size: 11px;
  opacity: 0.7;
  letter-spacing: 0.04em;
}
.fl-grp label {
  display: flex;
  align-items: center;
  gap: 8px;
  justify-content: space-between;
}
.fl-grp input[type='range'] {
  flex: 1 1 auto;
  accent-color: #7a95ff;
}
/* 边缘拖拽缩放手柄：无边框窗口用自定义手柄实现；仅 6px 细条 + 四角，避免遮挡歌词与按钮 */
.fl-resize {
  position: absolute;
  z-index: 6;
}
.fl-resize--e {
  top: 0;
  right: 0;
  width: 6px;
  height: 100%;
  cursor: ew-resize;
}
.fl-resize--w {
  top: 0;
  left: 0;
  width: 6px;
  height: 100%;
  cursor: ew-resize;
}
.fl-resize--s {
  left: 0;
  right: 0;
  bottom: 0;
  height: 6px;
  cursor: ns-resize;
}
.fl-resize--se {
  right: 0;
  bottom: 0;
  width: 14px;
  height: 14px;
  cursor: nwse-resize;
}
.fl-resize--sw {
  left: 0;
  bottom: 0;
  width: 14px;
  height: 14px;
  cursor: nesw-resize;
}
</style>

<!-- 该窗口为独立文档，重置默认边距并透明化，避免主题样式在窗口边缘留白/底色 -->
<style>
html,
body,
#app {
  margin: 0 !important;
  padding: 0 !important;
  width: 100% !important;
  height: auto !important;
  background: transparent !important;
  overflow: hidden !important;
}
</style>
