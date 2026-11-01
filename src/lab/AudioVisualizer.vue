<script setup lang="ts">
/**
 * 实验室·音频可视化浮层
 *
 * 呈现方式：Teleport 到 body 的 `position: fixed` 覆盖层（不参与任何父级布局，
 * 因此**不会挤压/放大窗口**，也不影响播放条 grid 与内容区排版）。
 *
 * 三种停靠形态：
 *  - `sidebar`：测量 `.sidebar__scroll` 内容底部与 `.sidebar__footer` 顶部之间的空白，
 *    **四边贴合**填满（宽 = 侧栏内容宽，高 = 空白区高）；
 *  - `toolbar`：测量 `.list-toolbar__left` 右缘与 `.list-toolbar__right` 左缘之间的 flex
 *    间隙，按工具条内容盒高度**贴合**填满；
 *  - `floating`：固定 320×88 卡片，可拖动到任意位置。
 *
 * 由于区域尺寸随窗口/侧栏折叠/面板开合/滚动变化，这里用 rAF 合并的测量循环
 * （resize + 滚动 + 兜底轮询）实时重算矩形，canvas 由 ResizeObserver 同步 backing store。
 * 仅当 audio-visualizer 开启时由 PlayerBar 挂载；卸载即退订事件、断开监听与 RAF。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { t } from '../i18n'
import { useLayout } from '../composables/useLayout'
import { useAudioVisualizer, vizDock, vizPos, type VizDock } from './features/audio-visualizer'

interface Rect {
  left: number
  top: number
  width: number
  height: number
}

/** 自由浮动卡片尺寸 */
const FLOATING_W = 320
const FLOATING_H = 88
/** 停靠区域与相邻元素之间的留白（视觉呼吸，避免贴着文字/分隔线） */
const INNER_PAD = 8
/**
 * 侧栏停靠态的高度上限：侧栏空白往往有 250px 以上，整块填满会像一堵彩色墙，
 * 因此底部对齐 + 限高，视觉上成为一条"频谱条"，与侧栏留白比例协调。
 */
const SIDEBAR_MAX_H = 120

const cv = ref<HTMLCanvasElement | null>(null)
const box = ref<HTMLElement | null>(null)
const { startLoop } = useAudioVisualizer()
const { isSidebarCollapsed } = useLayout()

/**
 * 实际生效的停靠形态：侧栏折叠后其内容宽仅 48px，无法容纳频谱，
 * 此时自动改用顶部工具条位，无需用户进实验室手动切换。
 */
const effectiveDock = computed<VizDock>(() =>
  vizDock.value === 'sidebar' && isSidebarCollapsed.value ? 'toolbar' : vizDock.value,
)

/** 当前是否走了"无区域 → 顶部浮动预览"的退化路径（用于切换成卡片样式） */
const autoFloating = ref(false)

/** 是否以卡片外观呈现（自由浮动，或退化到顶部的浮动预览） */
const isCardLook = computed(() => effectiveDock.value === 'floating' || autoFloating.value)
/** 是否可拖动（仅真正的自由浮动；退化预览不可拖） */
const isDraggable = computed(() => effectiveDock.value === 'floating')

let stop: (() => void) | null = null
let observer: ResizeObserver | null = null
let frame = 0
let poll: ReturnType<typeof setInterval> | null = null
/** 上一次写入的矩形签名，避免无变化时反复改样式 */
let lastKey = ''

function playerBarHeight(): number {
  const raw = getComputedStyle(document.documentElement).getPropertyValue('--size-playerbar-h')
  return Number.parseFloat(raw) || 72
}

/** 侧栏底部空白：导航内容之下、底部固定组之上的整块空区 */
function sidebarRect(): Rect | null {
  const scroll = document.querySelector<HTMLElement>('.sidebar__scroll')
  if (!scroll) return null
  const sr = scroll.getBoundingClientRect()
  // 折叠态 / 极窄窗口下不显示（避免只剩一条细缝）
  if (sr.width < 96) return null

  const footer = document.querySelector<HTMLElement>('.sidebar__footer')
  const footerTop = footer ? footer.getBoundingClientRect().top : sr.bottom

  // 导航内容实际占用的底部（若无子节点则从滚动区顶部算起）
  let contentBottom = sr.top
  const inner = scroll.firstElementChild
  if (inner instanceof HTMLElement) {
    contentBottom = Math.max(contentBottom, inner.getBoundingClientRect().bottom)
  }

  // 空白区上沿 = 导航内容底部，下沿 = 滚动区底部与底部固定组顶部中的较早者
  const areaTop = Math.min(contentBottom + INNER_PAD, sr.bottom)
  const bottom = Math.min(sr.bottom, footerTop - INNER_PAD)
  if (bottom - areaTop < 48) return null

  // 宽度铺满侧栏内容宽（横向左右贴边），高度底部对齐并限高，避免整块空白被填成彩色墙
  const height = Math.min(bottom - areaTop, SIDEBAR_MAX_H)
  return { left: sr.left, top: bottom - height, width: sr.width, height }
}

/** 列表工具条中部空白：左侧标题/操作组与右侧筛选组之间的 flex 间隙 */
function toolbarRect(): Rect | null {
  const bar = document.querySelector<HTMLElement>('.list-toolbar')
  if (!bar) return null
  const br = bar.getBoundingClientRect()
  if (br.width < 200) return null

  const style = getComputedStyle(bar)
  const padTop = Number.parseFloat(style.paddingTop) || 0
  const padBottom = Number.parseFloat(style.paddingBottom) || 0
  const padLeft = Number.parseFloat(style.paddingLeft) || 0
  const padRight = Number.parseFloat(style.paddingRight) || 0
  const gap = Number.parseFloat(style.columnGap) || 16

  const leftGroup = bar.querySelector<HTMLElement>('.list-toolbar__left')
  const rightGroup = bar.querySelector<HTMLElement>('.list-toolbar__right')
  const left = leftGroup
    ? leftGroup.getBoundingClientRect().right + gap / 2
    : br.left + padLeft
  const right = rightGroup
    ? rightGroup.getBoundingClientRect().left - gap / 2
    : br.right - padRight

  const width = right - left
  const height = br.height - padTop - padBottom
  if (width < 120 || height < 24) return null
  return { left, top: br.top + padTop, width, height }
}

function regionRect(dock: VizDock): Rect | null {
  if (dock === 'sidebar') return sidebarRect()
  if (dock === 'toolbar') return toolbarRect()
  return null
}

function writeBox(el: HTMLElement, rect: Rect, visible: boolean): void {
  const key = [
    Math.round(rect.left),
    Math.round(rect.top),
    Math.round(rect.width),
    Math.round(rect.height),
    visible ? 1 : 0,
  ].join('|')
  if (key === lastKey) return
  lastKey = key
  el.style.visibility = visible ? 'visible' : 'hidden'
  el.style.left = `${Math.round(rect.left)}px`
  el.style.top = `${Math.round(rect.top)}px`
  el.style.width = `${Math.round(rect.width)}px`
  el.style.height = `${Math.round(rect.height)}px`
}

/** 计算并写入覆盖层矩形（直接改样式，避免响应式更新晚一帧导致滚动抖动） */
function applyRect(): void {
  const el = box.value
  if (!el) return

  if (effectiveDock.value === 'floating') {
    const x = vizPos.value?.x ?? window.innerWidth - FLOATING_W - 20
    const y = vizPos.value?.y ?? window.innerHeight - playerBarHeight() - FLOATING_H - 16
    writeBox(el, { left: x, top: y, width: FLOATING_W, height: FLOATING_H }, true)
    return
  }

  const rect = regionRect(effectiveDock.value)
  if (!rect) {
    // 该视图没有可停靠的区域（例如实验室页没有列表工具条）：
    // 退化为内容区顶部的浮动预览，避免"收起侧栏后可视化凭空消失"。
    const content = document.querySelector<HTMLElement>('.app-shell__content')
    if (!content) {
      el.style.visibility = 'hidden'
      lastKey = ''
      autoFloating.value = false
      return
    }
    const cr = content.getBoundingClientRect()
    autoFloating.value = true
    writeBox(
      el,
      { left: cr.right - FLOATING_W - 20, top: cr.top + 16, width: FLOATING_W, height: FLOATING_H },
      true,
    )
    return
  }
  autoFloating.value = false
  // 区域被滚出视口（工具条随列表滚动）时隐藏，避免浮在无关内容上
  const visible = rect.top + rect.height > 44 && rect.top < window.innerHeight - 80
  writeBox(el, rect, visible)
}

/** 合并到下一帧执行，滚动/缩放高频触发时只量一次 */
function schedule(): void {
  if (frame !== 0) return
  frame = requestAnimationFrame(() => {
    frame = 0
    applyRect()
  })
}

/** 按容器 CSS 尺寸 × dpr 同步画布 backing store，保证清晰且与区域等大 */
function syncCanvas(): void {
  const el = box.value
  const canvas = cv.value
  if (!el || !canvas) return
  const dpr = window.devicePixelRatio || 1
  const width = Math.max(1, Math.round(el.clientWidth * dpr))
  const height = Math.max(1, Math.round(el.clientHeight * dpr))
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width
    canvas.height = height
  }
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

/** 拖动（仅自由浮动形态）：按下即改坐标，移动时夹取在视口内 */
function onPointerDown(e: PointerEvent): void {
  if (!isDraggable.value) return
  const el = box.value
  if (!el) return
  const rect = el.getBoundingClientRect()
  const offsetX = e.clientX - rect.left
  const offsetY = e.clientY - rect.top
  const move = (ev: PointerEvent) => {
    vizPos.value = {
      x: clamp(ev.clientX - offsetX, 8, window.innerWidth - rect.width - 8),
      y: clamp(ev.clientY - offsetY, 8, window.innerHeight - rect.height - 8),
    }
  }
  const up = () => {
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', up)
  }
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', up)
  e.preventDefault()
}

onMounted(() => {
  applyRect()
  syncCanvas()
  stop = startLoop(cv.value)

  observer = new ResizeObserver(() => syncCanvas())
  if (box.value) observer.observe(box.value)

  window.addEventListener('resize', schedule)
  // 捕获阶段监听所有滚动容器（内容区 / 侧栏），让停靠矩形跟随滚动同步
  document.addEventListener('scroll', schedule, true)
  // 兜底：切换路由/视图时工具条元素被替换，无需依赖具体事件也能在 300ms 内复位
  poll = setInterval(schedule, 300)
})

onBeforeUnmount(() => {
  stop?.()
  observer?.disconnect()
  observer = null
  window.removeEventListener('resize', schedule)
  document.removeEventListener('scroll', schedule, true)
  if (poll !== null) {
    clearInterval(poll)
    poll = null
  }
  if (frame !== 0) {
    cancelAnimationFrame(frame)
    frame = 0
  }
})

// 生效形态变化（切换停靠 / 收起侧栏导致的自动改位）立即重算矩形
watch(effectiveDock, () => {
  lastKey = ''
  schedule()
})
watch(vizPos, () => {
  lastKey = ''
  schedule()
}, { deep: true })
</script>

<template>
  <Teleport to="body">
    <div
      ref="box"
      class="av-widget"
      :class="[
        isCardLook ? 'av-widget--floating' : 'av-widget--docked',
        { 'av-widget--inert': autoFloating },
      ]"
      :title="isDraggable ? t('lab.audioVisualizer.dragHint') : undefined"
      :aria-label="t('lab.feature.audioVisualizer.name')"
      @pointerdown="onPointerDown"
    >
      <canvas ref="cv" class="av-widget__canvas" />
    </div>
  </Teleport>
</template>

<style scoped>
/* 覆盖层：定位/尺寸全部由 script 按测量结果写入，不参与任何布局流 */
.av-widget {
  position: fixed;
  z-index: 40;
  overflow: hidden;
  background: transparent;
  pointer-events: none;
}

/* 停靠形态：无边框/圆角/阴影，与所在区域共用背景，视觉上严丝合缝 */
.av-widget--docked {
  border: 0;
}

/* 自由浮动形态：独立卡片，可拖动 */
.av-widget--floating {
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-lg);
  background: color-mix(in srgb, var(--color-bg-elevated) 88%, transparent);
  box-shadow: var(--shadow-elevation-2);
  backdrop-filter: blur(10px);
  cursor: grab;
  pointer-events: auto;
  user-select: none;
  touch-action: none;
}
.av-widget--floating:active {
  cursor: grabbing;
}

/* 退化到顶部的浮动预览：只作展示，不拦截任何点击 */
.av-widget--inert {
  cursor: default;
  pointer-events: none;
}

.av-widget__canvas {
  display: block;
  width: 100%;
  height: 100%;
}
</style>
