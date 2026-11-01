<script setup lang="ts">
/**
 * 实验室面板（设置页内嵌）
 *
 * 每个实验功能只有两种状态：**关闭 / 开启**（拨动开关），默认全部关闭、
 * 与现有功能完全隔离。功能满足需求后由开发者把它从实验室「走出去」并入正式版
 * （代码从 src/lab/features/<id> 迁到主项目、并从 src/lab/registry.ts 移除条目），
 * 届时它不再是实验室条目、也不需要开关。
 */
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import { useLabStore } from '../stores/lab'
import { t } from '../i18n'
import {
  ACCENT_PRESETS,
  customAccent,
  backdropMode,
  wallpaperPath,
  wallpaperFit,
  wallpaperSource,
  dailyWallpaper,
  importWallpaper,
  removeWallpaper,
  setWallpaperSource,
  ensureDailyWallpaper,
  clearDailyWallpaper,
  GRADIENT_PRESETS,
  gradientId,
  TEXTURE_PRESETS,
  textureId,
} from '../lab/features/theme-colors'
import type { BackdropMode } from '../services/lab'
import { metaVerifyStatus } from '../services/lab'
import { isTauriRuntime } from '../services/ipc'
import { useFloatingLyrics } from '../lab/features/floating-lyrics'
import {
  VIZ_PRESETS,
  applyVizPreset,
  matchVizPreset,
  startThumbLoop,
  vizStyle,
  vizDensity,
  vizGradient,
  vizLineWidth,
  vizSpeed,
  vizCurve,
  vizDock,
  vizPos,
  type VizStyle,
  type VizDensity,
  type VizGradient,
  type VizLineWidth,
  type VizSpeed,
  type VizCurve,
  type VizDock,
} from '../lab/features/audio-visualizer'

const lab = useLabStore()
const floatingLyrics = useFloatingLyrics()

/** 顶层实验室卡片：自定义壁纸作为「皮肤配色」的子模块，不再单独成卡 */
const topFeatures = computed(() => lab.features.filter((f) => f.id !== 'custom-wallpaper'))

/**
 * 统一的功能开关处理：开启 / 关闭某个实验功能。
 */
function onFeatureToggle(f: { id: string }, enabled: boolean): void {
  lab.setEnabled(f.id, enabled)
  if (f.id === 'enhanced-metadata' && enabled) {
    void refreshVerifyStatus()
  }
  // 悬浮歌词：开启即弹出独立窗口，关闭则销毁（与现有功能完全隔离，零影响）
  if (f.id === 'floating-lyrics') {
    if (enabled) void floatingLyrics.open()
    else void floatingLyrics.close()
  }
}

/** 可视化波形样式选项 */
const VIZ_STYLES: { id: VizStyle; key: string }[] = [
  { id: 'bars', key: 'lab.audioVisualizer.styleBars' },
  { id: 'line', key: 'lab.audioVisualizer.styleLine' },
  { id: 'area', key: 'lab.audioVisualizer.styleArea' },
  { id: 'mirror', key: 'lab.audioVisualizer.styleMirror' },
]
/** 可视化显示密度选项 */
const VIZ_DENSITIES: { id: VizDensity; key: string }[] = [
  { id: 'sparse', key: 'lab.audioVisualizer.densitySparse' },
  { id: 'medium', key: 'lab.audioVisualizer.densityMedium' },
  { id: 'dense', key: 'lab.audioVisualizer.densityDense' },
]
/**
 * 可视化停靠位置选项：
 *  - sidebar / toolbar 为运行时测量真实布局得到的区域，四边贴合填满；
 *  - floating 为固定尺寸卡片，可拖动。
 */
const VIZ_DOCKS: { id: VizDock; key: string }[] = [
  { id: 'sidebar', key: 'lab.audioVisualizer.dockSidebar' },
  { id: 'toolbar', key: 'lab.audioVisualizer.dockToolbar' },
  { id: 'floating', key: 'lab.audioVisualizer.dockFloating' },
]

/** 切换停靠形态；离开自由浮动时清掉拖动坐标，回到贴合区域 */
function setVizDock(id: VizDock): void {
  vizDock.value = id
  if (id !== 'floating') vizPos.value = null
}

/** 颜色渐变预设 */
const VIZ_GRADIENTS: { id: VizGradient; key: string }[] = [
  { id: 'aurora', key: 'lab.audioVisualizer.gradAurora' },
  { id: 'sunset', key: 'lab.audioVisualizer.gradSunset' },
  { id: 'mono', key: 'lab.audioVisualizer.gradMono' },
  { id: 'neon', key: 'lab.audioVisualizer.gradNeon' },
  { id: 'ice', key: 'lab.audioVisualizer.gradIce' },
]
/** 线条粗细 */
const VIZ_LINE_WIDTHS: { id: VizLineWidth; key: string }[] = [
  { id: 'thin', key: 'lab.audioVisualizer.lineThin' },
  { id: 'normal', key: 'lab.audioVisualizer.lineNormal' },
  { id: 'thick', key: 'lab.audioVisualizer.lineThick' },
]
/** 动效速度 */
const VIZ_SPEEDS: { id: VizSpeed; key: string }[] = [
  { id: 'slow', key: 'lab.audioVisualizer.speedSlow' },
  { id: 'normal', key: 'lab.audioVisualizer.speedNormal' },
  { id: 'fast', key: 'lab.audioVisualizer.speedFast' },
]
/** 曲线形态（作用于 线条 / 面积） */
const VIZ_CURVES: { id: VizCurve; key: string }[] = [
  { id: 'smooth', key: 'lab.audioVisualizer.curveSmooth' },
  { id: 'straight', key: 'lab.audioVisualizer.curveStraight' },
  { id: 'stepped', key: 'lab.audioVisualizer.curveStepped' },
]

/** 当前组合命中的预设 id；null = 已手动微调（自定义） */
const activePreset = computed(() => matchVizPreset())

/** 风格预设缩略图：每个卡片一个小 canvas，复用 drawWith 实时渲染同一份当前音频（非静态图） */
const thumbCanvases = {} as Record<string, HTMLCanvasElement | null>
const thumbStops: Array<() => void> = []

/** 收集每个预设卡片的 canvas 引用（函数式 ref） */
function setThumbRef(el: unknown, id: string): void {
  thumbCanvases[id] = (el as HTMLCanvasElement | null) ?? null
}

/** 启动所有缩略图循环（按各自预设配置渲染同一份实时音频） */
function startThumbs(): void {
  stopThumbs()
  const dpr = window.devicePixelRatio || 1
  for (const p of VIZ_PRESETS) {
    const c = thumbCanvases[p.id]
    if (!c) continue
    c.width = Math.max(1, Math.round(c.clientWidth * dpr))
    c.height = Math.max(1, Math.round(c.clientHeight * dpr))
    thumbStops.push(startThumbLoop(c, p.config))
  }
}

/** 停止所有缩略图循环 */
function stopThumbs(): void {
  thumbStops.forEach((s) => s())
  thumbStops.length = 0
}

// 功能开启且卡片可见时启动；关闭或离场时停止，避免空转
watch(
  () => lab.isEnabled('audio-visualizer'),
  (on) => {
    if (on) void nextTick(startThumbs)
    else stopThumbs()
  },
)
onMounted(() => {
  if (lab.isEnabled('audio-visualizer')) void nextTick(startThumbs)
})
onUnmounted(stopThumbs)

/* ---------- 实验室·元数据增强：批量校验运行状态（重试按单曲进行） ---------- */
const verifyRunning = ref(false)
/** 校验完成事件的解绑句柄 */
let unlistenVerify: UnlistenFn | undefined

async function refreshVerifyStatus(): Promise<void> {
  const s = await metaVerifyStatus()
  verifyRunning.value = s.running
}

/** 预设列表（对象转数组，便于模板 v-for） */
// 进入实验室页时回显在线校验状态（失败项可在此重试）；
// 后台校验任务完成时实时刷新（library://meta-verify-done）
onMounted(async () => {
  if (lab.isEnabled('enhanced-metadata')) await refreshVerifyStatus()
  if (isTauriRuntime()) {
    unlistenVerify = await listen('library://meta-verify-done', () => {
      void refreshVerifyStatus()
    })
  }
})
onUnmounted(() => {
  unlistenVerify?.()
})

/** 窗口底纹模式选项 */
const BACKDROP_OPTIONS: { mode: BackdropMode; labelKey: string; hintKey: string; ready: boolean }[] = [
  { mode: 'off', labelKey: 'lab.backdrop.off', hintKey: 'lab.backdrop.offHint', ready: true },
  {
    mode: 'solid',
    labelKey: 'lab.backdrop.solid',
    hintKey: 'lab.backdrop.solidHint',
    ready: true,
  },
  {
    mode: 'native',
    labelKey: 'lab.backdrop.native',
    hintKey: 'lab.backdrop.nativeHint',
    ready: true,
  },
  {
    mode: 'gradient',
    labelKey: 'lab.backdrop.gradient',
    hintKey: 'lab.backdrop.gradientHint',
    ready: true,
  },
  {
    mode: 'texture',
    labelKey: 'lab.backdrop.texture',
    hintKey: 'lab.backdrop.textureHint',
    ready: true,
  },
]

/** 当前底纹模式的说明键（模板经 t() 翻译） */
const backdropHintKey = computed(
  () => BACKDROP_OPTIONS.find((o) => o.mode === backdropMode.value)?.hintKey ?? '',
)

const importing = ref(false)
const wallpaperMsg = ref('')

async function selectBackdrop(mode: BackdropMode): Promise<void> {
  if (!BACKDROP_OPTIONS.find((o) => o.mode === mode)?.ready) return
  if (mode === 'wallpaper' && wallpaperSource.value === 'local' && !wallpaperPath.value) {
    wallpaperMsg.value = 'lab.wallpaper.needImport'
    return
  }
  wallpaperMsg.value = ''
  backdropMode.value = mode
  // 进入壁纸模式且为每日来源但图尚未就位：立即拉取
  if (mode === 'wallpaper' && wallpaperSource.value === 'daily' && !wallpaperPath.value) {
    await ensureDailyWallpaper()
  }
}

async function onRefreshDaily(): Promise<void> {
  wallpaperMsg.value = ''
  await ensureDailyWallpaper(1 + Math.floor(Math.random() * 7))
}

async function onClearDaily(): Promise<void> {
  wallpaperMsg.value = ''
  await clearDailyWallpaper()
  wallpaperMsg.value = 'lab.wallpaper.dailyCleared'
}

async function onImportWallpaper(): Promise<void> {
  importing.value = true
  wallpaperMsg.value = ''
  try {
    const ok = await importWallpaper()
    if (!ok) {
      wallpaperMsg.value = 'lab.wallpaper.importFailed'
      return
    }
    // 导入成功：自动切到壁纸模式，让用户立刻看到效果
    backdropMode.value = 'wallpaper'
    wallpaperMsg.value = ''
  } finally {
    importing.value = false
  }
}

async function onRemoveWallpaper(): Promise<void> {
  await removeWallpaper()
  wallpaperMsg.value = 'lab.wallpaper.removed'
}

function onFitChange(fit: 'cover' | 'contain'): void {
  wallpaperFit.value = fit
}
</script>

<template>
  <div class="lab">
    <div class="lab__intro">
      {{ t('lab.intro') }}
    </div>

    <div v-for="f in topFeatures" :key="f.id" class="lab__card">
      <div class="lab__head">
        <div class="lab__meta">
          <div class="lab__name">{{ t(f.nameKey) }}</div>
          <div class="lab__desc">{{ t(f.descriptionKey) }}</div>
        </div>

        <!-- 关闭 / 开启（Switch，与 SettingsView 原开关一致） -->
        <label class="lab__switch">
          <input
            type="checkbox"
            class="switch"
            :checked="f.enabled"
            @change="onFeatureToggle(f, ($event.target as HTMLInputElement).checked)"
          />
        </label>
      </div>

      <div v-if="f.id === 'theme-colors' && f.enabled" class="lab__theme">
        <div class="lab__group">
          <div class="lab__group-title">{{ t('lab.accent') }}</div>
          <div class="lab__swatches">
            <button
              v-for="p in ACCENT_PRESETS"
              :key="p.value"
              class="lab__swatch"
              :class="{ 'lab__swatch--active': customAccent === p.value }"
              :style="{ background: p.value }"
              :title="t(p.nameKey)"
              :aria-label="t(p.nameKey)"
              @click="customAccent = p.value"
            />
          </div>
          <label class="lab__custom">
            {{ t('lab.custom') }}
            <input type="color" v-model="customAccent" />
          </label>
        </div>

        <div class="lab__group">
          <div class="lab__group-title">{{ t('lab.backdropTitle') }}</div>
          <div class="lab__modes">
            <button
              v-for="o in BACKDROP_OPTIONS"
              :key="o.mode"
              class="lab__mode"
              :class="{
                'lab__mode--active': backdropMode === o.mode,
                'lab__mode--todo': !o.ready,
              }"
              :disabled="!o.ready"
              :title="t(o.hintKey)"
              @click="selectBackdrop(o.mode)"
            >
              {{ t(o.labelKey) }}{{ o.ready ? '' : t('lab.devSuffix') }}
            </button>
          </div>
          <p class="lab__hint">
            {{ backdropHintKey ? t(backdropHintKey) : '' }}
          </p>

          <!-- 预设渐变：选择渐变方案 -->
          <div v-if="backdropMode === 'gradient'" class="lab__presets">
            <button
              v-for="g in GRADIENT_PRESETS"
              :key="g.id"
              class="lab__preset"
              :class="{ 'lab__preset--active': gradientId === g.id }"
              :title="t(g.nameKey)"
              :style="{ background: g.css }"
              @click="gradientId = g.id"
            >
              <span class="lab__preset-label">{{ t(g.nameKey) }}</span>
            </button>
          </div>

          <!-- 纹理图案：选择纹理样式 -->
          <div v-if="backdropMode === 'texture'" class="lab__presets">
            <button
              v-for="tex in TEXTURE_PRESETS"
              :key="tex.id"
              class="lab__preset lab__preset--chip"
              :class="{ 'lab__preset--active': textureId === tex.id }"
              :title="t(tex.nameKey)"
              @click="textureId = tex.id"
            >
              {{ t(tex.nameKey) }}
            </button>
          </div>
        </div>
        <!-- 自定义壁纸：皮肤配色下的子模块 -->
        <div class="lab__submodule">
          <div class="lab__head">
            <div class="lab__meta">
              <div class="lab__name">{{ t('lab.wallpaper.title') }}</div>
            </div>
            <label class="lab__switch">
              <input
                type="checkbox"
                class="switch"
                :checked="lab.isEnabled('custom-wallpaper')"
                @change="lab.setEnabled('custom-wallpaper', ($event.target as HTMLInputElement).checked)"
              />
            </label>
          </div>

          <div v-if="lab.isEnabled('custom-wallpaper')" class="lab__wallpaper-panel">
            <p class="lab__hint">
              {{ t('lab.wallpaper.desc') }}
            </p>

            <div class="lab__wallpaper-row">
              <span class="lab__group-title">{{ t('lab.source') }}</span>
              <div class="lab__radio lab__radio--sm">
                <label class="lab__radio-item" :class="{ 'lab__radio-item--on': wallpaperSource === 'local' }">
                  <input
                    type="radio"
                    name="lab-wallpaper-source"
                    :checked="wallpaperSource === 'local'"
                    @change="setWallpaperSource('local')"
                  />
                  <span>{{ t('lab.sourceLocal') }}</span>
                </label>
                <label class="lab__radio-item" :class="{ 'lab__radio-item--on': wallpaperSource === 'daily' }">
                  <input
                    type="radio"
                    name="lab-wallpaper-source"
                    :checked="wallpaperSource === 'daily'"
                    @change="setWallpaperSource('daily')"
                  />
                  <span>{{ t('lab.sourceDaily') }}</span>
                </label>
              </div>
            </div>

            <!-- 本地图片来源 -->
            <template v-if="wallpaperSource === 'local'">
              <div class="lab__wallpaper-row">
                <button class="lab__btn" :disabled="importing" @click="onImportWallpaper">
                  {{
                    importing
                      ? t('lab.wallpaper.importing')
                      : wallpaperPath
                        ? t('lab.wallpaper.replace')
                        : t('lab.wallpaper.import')
                  }}
                </button>
                <button v-if="wallpaperPath" class="lab__btn lab__btn--danger" @click="onRemoveWallpaper">
                  {{ t('common.remove') }}
                </button>
                <span class="lab__wallpaper-state">
                  {{ wallpaperPath ? t('lab.wallpaper.imported') : t('lab.wallpaper.notImported') }}
                </span>
              </div>
              <div v-if="wallpaperPath" class="lab__wallpaper-row">
                <span class="lab__group-title">{{ t('lab.fit') }}</span>
                <div class="lab__radio lab__radio--sm">
                  <label class="lab__radio-item" :class="{ 'lab__radio-item--on': wallpaperFit === 'cover' }">
                    <input
                      type="radio"
                      name="lab-wallpaper-fit"
                      :checked="wallpaperFit === 'cover'"
                      @change="onFitChange('cover')"
                    />
                    <span>{{ t('lab.fitCover') }}</span>
                  </label>
                  <label class="lab__radio-item" :class="{ 'lab__radio-item--on': wallpaperFit === 'contain' }">
                    <input
                      type="radio"
                      name="lab-wallpaper-fit"
                      :checked="wallpaperFit === 'contain'"
                      @change="onFitChange('contain')"
                    />
                    <span>{{ t('lab.fitContain') }}</span>
                  </label>
                </div>
              </div>
            </template>

            <!-- 每日 Bing 来源 -->
            <template v-else>
              <div class="lab__wallpaper-row">
                <button class="lab__btn" @click="onRefreshDaily">{{ t('lab.wallpaper.refresh') }}</button>
                <button class="lab__btn" @click="onClearDaily">{{ t('lab.wallpaper.clearCache') }}</button>
                <button class="lab__btn lab__btn--danger" @click="onRemoveWallpaper">
                  {{ t('lab.wallpaper.stopDaily') }}
                </button>
              </div>
              <p v-if="dailyWallpaper" class="lab__hint">
                {{ dailyWallpaper.title ? `${dailyWallpaper.title} · ` : '' }}{{ dailyWallpaper.copyright || t('lab.wallpaper.dailyFallback') }}
              </p>
              <p class="lab__hint">{{ t('lab.wallpaper.dailyHint') }}</p>
            </template>

            <p v-if="wallpaperMsg" class="lab__hint lab__warn">{{ t(wallpaperMsg) }}</p>
          </div>
        </div>
      </div>

      <div v-else-if="f.id === 'enhanced-metadata' && f.enabled" class="lab__group">
        <div class="lab__wallpaper-row">
          <span class="lab__group-title">{{ t('lab.metaVerify') }}</span>
          <span class="lab__hint">{{ verifyRunning ? t('lab.metaVerifyRunning') : t('lab.metaVerifyIdle') }}</span>
        </div>
        <p class="lab__hint">
          {{ t('lab.metaVerifyDesc') }}
        </p>
      </div>

      <!-- 音频可视化：开启后实时渲染频谱，提供样式 / 密度 / 位置设置 -->
      <div v-else-if="f.id === 'audio-visualizer' && f.enabled" class="lab__group">
        <p class="lab__hint">{{ t('lab.audioVisualizer.desc') }}</p>

        <!-- 风格预设：带缩略图，按外观一眼选取；微调见下方「高级设置」 -->
        <div class="lab__group">
          <div class="lab__group-title">{{ t('lab.audioVisualizer.preset') }}</div>
          <div class="lab__presets">
            <button
              v-for="p in VIZ_PRESETS"
              :key="p.id"
              class="lab__style-preset"
              :class="{ 'lab__style-preset--active': activePreset === p.id }"
              :title="t(p.key)"
              @click="applyVizPreset(p.id)"
            >
              <canvas class="lab__style-preset-pv" :ref="(el) => setThumbRef(el, p.id)" />
              <span class="lab__style-preset-label">{{ t(p.key) }}</span>
            </button>
            <span v-if="!activePreset" class="lab__mode lab__mode--tag">
              {{ t('lab.audioVisualizer.presetCustom') }}
            </span>
          </div>
        </div>

        <div class="lab__group">
          <div class="lab__group-title">{{ t('lab.audioVisualizer.position') }}</div>
          <div class="lab__modes">
            <button
              v-for="a in VIZ_DOCKS"
              :key="a.id"
              class="lab__mode"
              :class="{ 'lab__mode--active': vizDock === a.id }"
              @click="setVizDock(a.id)"
            >
              {{ t(a.key) }}
            </button>
          </div>
          <p class="lab__hint">{{ t('lab.audioVisualizer.dragHint') }}</p>
          <button
            v-if="vizDock === 'floating' && vizPos"
            class="lab__btn"
            @click="vizPos = null"
          >
            {{ t('lab.audioVisualizer.resetPos') }}
          </button>
        </div>

        <!-- 高级设置：默认收起，避免一次铺开 21 个选项 -->
        <details class="lab__adv">
          <summary class="lab__adv-summary">{{ t('lab.audioVisualizer.advanced') }}</summary>
          <div class="lab__adv-body">
            <div class="lab__adv-group">
              <div class="lab__adv-group-title">{{ t('lab.audioVisualizer.advancedAppearance') }}</div>
              <div class="lab__group">
                <div class="lab__group-title">{{ t('lab.audioVisualizer.style') }}</div>
                <div class="lab__modes">
                  <button
                    v-for="s in VIZ_STYLES"
                    :key="s.id"
                    class="lab__mode"
                    :class="{ 'lab__mode--active': vizStyle === s.id }"
                    @click="vizStyle = s.id"
                  >
                    {{ t(s.key) }}
                  </button>
                </div>
              </div>

              <div class="lab__group">
                <div class="lab__group-title">{{ t('lab.audioVisualizer.density') }}</div>
                <div class="lab__modes">
                  <button
                    v-for="d in VIZ_DENSITIES"
                    :key="d.id"
                    class="lab__mode"
                    :class="{ 'lab__mode--active': vizDensity === d.id }"
                    @click="vizDensity = d.id"
                  >
                    {{ t(d.key) }}
                  </button>
                </div>
              </div>

              <div class="lab__group">
                <div class="lab__group-title">{{ t('lab.audioVisualizer.gradient') }}</div>
                <div class="lab__modes">
                  <button
                    v-for="g in VIZ_GRADIENTS"
                    :key="g.id"
                    class="lab__mode"
                    :class="{ 'lab__mode--active': vizGradient === g.id }"
                    @click="vizGradient = g.id"
                  >
                    {{ t(g.key) }}
                  </button>
                </div>
              </div>

              <div class="lab__group">
                <div class="lab__group-title">{{ t('lab.audioVisualizer.lineWidth') }}</div>
                <div class="lab__modes">
                  <button
                    v-for="l in VIZ_LINE_WIDTHS"
                    :key="l.id"
                    class="lab__mode"
                    :class="{ 'lab__mode--active': vizLineWidth === l.id }"
                    @click="vizLineWidth = l.id"
                  >
                    {{ t(l.key) }}
                  </button>
                </div>
              </div>
            </div>

            <div class="lab__adv-group">
              <div class="lab__adv-group-title">{{ t('lab.audioVisualizer.advancedMotion') }}</div>
              <div class="lab__group">
                <div class="lab__group-title">{{ t('lab.audioVisualizer.speed') }}</div>
                <div class="lab__modes">
                  <button
                    v-for="sp in VIZ_SPEEDS"
                    :key="sp.id"
                    class="lab__mode"
                    :class="{ 'lab__mode--active': vizSpeed === sp.id }"
                    @click="vizSpeed = sp.id"
                  >
                    {{ t(sp.key) }}
                  </button>
                </div>
              </div>

              <div class="lab__group">
                <div class="lab__group-title">{{ t('lab.audioVisualizer.curve') }}</div>
                <div class="lab__modes">
                  <button
                    v-for="c in VIZ_CURVES"
                    :key="c.id"
                    class="lab__mode"
                    :class="{ 'lab__mode--active': vizCurve === c.id }"
                    @click="vizCurve = c.id"
                  >
                    {{ t(c.key) }}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </details>
      </div>

      <!-- 悬浮歌词：开启后弹出独立窗口，支持显隐切换 -->
      <div v-else-if="f.id === 'floating-lyrics' && f.enabled" class="lab__group">
        <div class="lab__wallpaper-row">
          <button class="lab__btn" @click="floatingLyrics.toggleVisible()">
            {{ floatingLyrics.isOpen.value ? t('lab.floatingLyrics.hide') : t('lab.floatingLyrics.show') }}
          </button>
        </div>
        <p class="lab__hint">{{ t('lab.floatingLyrics.desc') }}</p>
        <p class="lab__hint">{{ t('lab.floatingLyrics.features') }}</p>
      </div>

      <!-- 未实现的占位功能提示（已实现但无需面板的功能已排除） -->
      <div
        v-else-if="
          f.id !== 'theme-colors' &&
          f.id !== 'equalizer' &&
          f.id !== 'enhanced-metadata' &&
          f.id !== 'cue-sacd-convert' &&
          f.id !== 'audio-visualizer' &&
          f.id !== 'floating-lyrics'
        "
        class="lab__wip"
      >
        {{ t('lab.wip') }}
      </div>
    </div>
  </div>
</template>

<style scoped>
.lab {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.lab__intro {
  color: var(--color-text-secondary, #9aa4b2);
  font-size: 13px;
  line-height: 1.5;
}
.lab__card {
  border: 1px solid var(--color-border, rgba(255, 255, 255, 0.08));
  border-radius: 10px;
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.lab__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}
.lab__name {
  font-weight: 600;
}
.lab__desc {
  color: var(--color-text-secondary, #9aa4b2);
  font-size: 12px;
  margin-top: 4px;
}
/* 关闭 / 开启（Switch，与 SettingsView 一致） */
.lab__switch {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  margin-top: 2px;
}

/* 开关（与 SettingsView .switch 保持一致） */
.switch {
  position: relative;
  flex: 0 0 auto;
  width: 38px;
  height: 22px;
  margin: 0;
  appearance: none;
  -webkit-appearance: none;
  background-color: var(--color-bg-hover, rgba(255, 255, 255, 0.06));
  border: 1px solid var(--color-border, rgba(255, 255, 255, 0.14));
  border-radius: 999px;
  cursor: pointer;
  transition: background-color 0.2s ease;
}
.switch::after {
  content: '';
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  background-color: var(--color-text-inverse, #fff);
  border-radius: 999px;
  transition: transform 0.2s ease;
}
.switch:checked {
  background-color: var(--color-brand-solid, #7a95ff);
  border-color: var(--color-brand-solid, #7a95ff);
}
.switch:checked::after {
  transform: translateX(16px);
}
.switch:disabled {
  cursor: not-allowed;
}
.switch:focus-visible {
  outline: 2px solid var(--color-focus-ring, #7a95ff);
  outline-offset: 2px;
}
.lab__theme {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.lab__submodule {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-top: 4px;
  padding-top: 14px;
  border-top: 1px dashed var(--color-border, rgba(255, 255, 255, 0.12));
}
.lab__group {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.lab__group-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-secondary, #9aa4b2);
}
.lab__modes {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.lab__mode {
  font-size: 12px;
  padding: 6px 12px;
  border-radius: 999px;
  border: 1px solid var(--color-border, rgba(255, 255, 255, 0.14));
  background: transparent;
  color: var(--color-text-primary, #fff);
  cursor: pointer;
}
.lab__mode:hover:not(:disabled) {
  background: var(--color-brand-subtle, rgba(122, 149, 255, 0.12));
}
.lab__mode--active {
  border-color: var(--color-brand, #7a95ff);
  color: var(--color-brand, #7a95ff);
}
.lab__mode--todo {
  opacity: 0.45;
  cursor: not-allowed;
}
/* 「自定义」标签：只是状态提示，不可点击 */
.lab__mode--tag {
  cursor: default;
  border-style: dashed;
  color: var(--color-text-secondary, #9aa4b2);
}
/* 高级设置：默认收起，展开后与普通分组同构 */
.lab__adv {
  border-top: 1px solid var(--color-border, rgba(255, 255, 255, 0.12));
  padding-top: 10px;
}
.lab__adv-summary {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-secondary, #9aa4b2);
  cursor: pointer;
  list-style: none;
  user-select: none;
}
.lab__adv-summary::-webkit-details-marker {
  display: none;
}
.lab__adv-summary::after {
  content: '▾';
  font-size: 10px;
  transition: transform 0.15s ease;
}
.lab__adv[open] .lab__adv-summary::after {
  transform: rotate(180deg);
}
.lab__adv-body {
  display: flex;
  flex-direction: column;
  gap: 0;
  padding-top: 12px;
}
.lab__adv-group {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.lab__adv-group + .lab__adv-group {
  margin-top: 14px;
  padding-top: 14px;
  border-top: 1px dashed var(--color-border, rgba(255, 255, 255, 0.12));
}
.lab__adv-group-title {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
  color: var(--color-text-secondary, #9aa4b2);
}
.lab__hint {
  margin: 0;
  font-size: 12px;
  color: var(--color-text-secondary, #9aa4b2);
}
.lab__warn {
  color: #fbbf24;
}
.lab__swatches {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.lab__swatch {
  width: 26px;
  height: 26px;
  border-radius: 50%;
  border: 2px solid transparent;
  cursor: pointer;
  padding: 0;
}
.lab__swatch--active {
  border-color: var(--color-text-primary, #fff);
}
.lab__custom {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--color-text-secondary, #9aa4b2);
}
.lab__wip {
  font-size: 12px;
  color: var(--color-text-secondary, #9aa4b2);
}
.lab__wallpaper {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-top: 2px;
}
.lab__wallpaper-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.lab__wallpaper-state {
  font-size: 12px;
  color: var(--color-text-secondary, #9aa4b2);
}
.lab__radio--sm .lab__radio-item {
  padding: 3px 10px;
  font-size: 12px;
}
.lab__btn {
  font-size: 12px;
  padding: 5px 12px;
  border-radius: 6px;
  border: 1px solid var(--color-border, rgba(255, 255, 255, 0.14));
  background: transparent;
  color: var(--color-text-primary, #fff);
  cursor: pointer;
}
.lab__btn:hover:not(:disabled) {
  background: var(--color-brand-subtle, rgba(122, 149, 255, 0.12));
}
.lab__btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.lab__btn--danger {
  color: #f87171;
  border-color: rgba(248, 113, 113, 0.4);
}
.lab__presets {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.lab__preset {
  position: relative;
  width: 84px;
  height: 46px;
  border-radius: 8px;
  border: 2px solid transparent;
  cursor: pointer;
  overflow: hidden;
  background-color: var(--color-bg-base, #1a1d24);
}
.lab__preset--active {
  border-color: var(--color-brand, #7a95ff);
}
.lab__preset-label {
  position: absolute;
  left: 6px;
  bottom: 4px;
  font-size: 11px;
  font-weight: 600;
  color: #fff;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.6);
}
.lab__preset--chip {
  width: auto;
  height: auto;
  min-width: 56px;
  padding: 7px 12px;
  font-size: 12px;
  background: transparent;
  color: var(--color-text-primary, #fff);
}
.lab__preset--chip .lab__preset-label {
  position: static;
}
/* 音频可视化·风格预设卡片（缩略图 + 名称） */
.lab__style-preset {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  width: 78px;
  padding: 8px 6px 6px;
  border-radius: 12px;
  border: 1px solid var(--color-border, rgba(255, 255, 255, 0.14));
  background: transparent;
  color: var(--color-text-primary, #fff);
  cursor: pointer;
}
.lab__style-preset--active {
  border-color: var(--color-brand, #7a95ff);
  color: var(--color-brand, #7a95ff);
}
.lab__style-preset-pv {
  display: block;
  width: 72px;
  height: 32px;
  border-radius: 6px;
  overflow: hidden;
  background: var(--color-bg-base, #1a1d24);
}
.lab__style-preset-pv svg {
  display: block;
  width: 100%;
  height: 100%;
}
.lab__style-preset-label {
  font-size: 11px;
  font-weight: 600;
}

/* 实验室·均衡器 */
.lab__eq {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.lab__link {
  margin-left: 8px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--color-primary, #5b8cff);
  font-size: 12px;
  cursor: pointer;
}
.lab__link:hover {
  text-decoration: underline;
}
.lab__bands {
  display: flex;
  gap: 10px;
  /* 10 段竖向并排；容器过窄时横向滚动，避免滑块被挤压变形 */
  overflow-x: auto;
  padding: 4px 0 6px;
}
.lab__band {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  /* 等分宽度，窄屏下不低于可拖动宽度 */
  flex: 1 1 0;
  min-width: 32px;
}
.lab__slider {
  /*
   * 竖向滑块（EQ 惯例：向上拖 = 提升增益）。
   *
   * 仅使用标准的 `writing-mode`：它把 range 的行内轴转为垂直方向。默认行内轴
   * 自上而下，即**最小值在上**，与 EQ 惯例相反，故再用 `direction: rtl` 反转
   * 行内轴，使**最大值在上**。不再使用已废弃的 `appearance: slider-vertical`。
   */
  writing-mode: vertical-lr;
  direction: rtl;
  width: 22px;
  height: 150px;
  accent-color: var(--color-primary, #5b8cff);
  cursor: pointer;
}
.lab__band-freq {
  font-size: 11px;
  color: var(--color-text-secondary, #9aa4b2);
}
.lab__band-gain {
  font-size: 11px;
  /* 等宽数字：拖动时数值不跳动 */
  font-variant-numeric: tabular-nums;
  color: var(--color-text-secondary, #9aa4b2);
}


</style>
