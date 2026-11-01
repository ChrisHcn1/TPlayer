/**
 * 实验室功能：皮肤配色（强调色 + 窗口底纹）
 *
 * 隔离说明：
 * - 强调色仅覆盖主题 CSS 变量（--color-brand 一族），**只在** lab 开关打开时生效；
 * - 窗口底纹仅在 `<html data-backdrop>` 存在时命中 CSS（见 assets/styles/lab-backdrop.css），
 *   并通过 `lab_window_set_effect` 运行时切换 OS 原生材质（免重启）；
 * - 关闭时清除全部自定义值、回落默认，**完全不影响现有主题与窗口外观**；
 * - 选色 / 底纹模式存 localStorage（功能本地数据，与 lab 开关解耦）；
 * - 废除该功能即删除本文件 + lab-backdrop.css，无其它代码依赖。
 */
import { ref, watch } from 'vue'

import { useLabStore } from '../../stores/lab'
import {
  labClearDailyWallpaper,
  labFetchDailyWallpaper,
  labImportWallpaper,
  labRemoveWallpaper,
  labWallpaperPath,
  labWindowSetEffect,
  type BackdropMode,
  type WallpaperFit,
} from '../../services/lab'

/* ===================== 强调色（自定义主题色） ===================== */

const ACCENT_KEY = 'tplayer.lab.theme.accent'
const DEFAULT_ACCENT = '#7a95ff'

/** 强调色预设 */
export const ACCENT_PRESETS: { nameKey: string; value: string }[] = [
  { nameKey: 'lab.preset.accent.blue', value: '#7a95ff' },
  { nameKey: 'lab.preset.accent.indigo', value: '#6366f1' },
  { nameKey: 'lab.preset.accent.purple', value: '#a78bfa' },
  { nameKey: 'lab.preset.accent.cyan', value: '#22d3ee' },
  { nameKey: 'lab.preset.accent.green', value: '#34d399' },
  { nameKey: 'lab.preset.accent.orange', value: '#fb923c' },
  { nameKey: 'lab.preset.accent.red', value: '#f87171' },
  { nameKey: 'lab.preset.accent.pink', value: '#f472b6' },
]

/** 受覆盖的主题变量（清除时逐一 removeProperty 回落默认） */
const ACCENT_VARS = [
  '--color-brand',
  '--color-brand-solid',
  '--color-brand-hover',
  '--color-brand-active',
  '--color-focus-ring',
]

function readAccent(): string {
  try {
    return localStorage.getItem(ACCENT_KEY) ?? DEFAULT_ACCENT
  } catch {
    return DEFAULT_ACCENT
  }
}

/** 当前自定义强调色（面板绑定此 ref） */
export const customAccent = ref<string>(readAccent())

function applyAccent(color: string): void {
  const root = document.documentElement
  for (const v of ACCENT_VARS) root.style.setProperty(v, color)
}

function clearAccent(): void {
  const root = document.documentElement
  for (const v of ACCENT_VARS) root.style.removeProperty(v)
}

/* ===================== 窗口底纹 ===================== */

const BACKDROP_KEY = 'tplayer.lab.backdrop.mode'
/** 默认纯色基底：即便原生材质不可用也一定有观感，且最接近现状、最不易翻车 */
const DEFAULT_BACKDROP: BackdropMode = 'solid'
/** 壁纸铺放方式持久化键 */
const WALLPAPER_FIT_KEY = 'tplayer.lab.backdrop.wallpaperFit'

const BACKDROP_MODES: readonly BackdropMode[] = [
  'off',
  'solid',
  'native',
  'wallpaper',
  'gradient',
  'texture',
]

function isBackdropMode(v: string | null): v is BackdropMode {
  return v != null && (BACKDROP_MODES as readonly string[]).includes(v)
}

function readBackdropMode(): BackdropMode {
  try {
    const v = localStorage.getItem(BACKDROP_KEY)
    if (isBackdropMode(v)) return v
  } catch {
    /* 隐私模式等忽略 */
  }
  return DEFAULT_BACKDROP
}

/** 当前底纹模式（面板绑定此 ref） */
export const backdropMode = ref<BackdropMode>(readBackdropMode())

/**
 * 原生材质是否真正生效。
 * 后端会回读 DWM 确认；未生效时前端已自动降级为 solid，此标志供面板提示用户。
 */
export const nativeActive = ref(false)

/* ---- 自定义壁纸 ---- */

/** 已导入壁纸的后端路径（未导入为 null） */
export const wallpaperPath = ref<string | null>(null)
/** 壁纸铺放方式：cover=铺满（裁切）/ contain=适应（留边） */
export const wallpaperFit = ref<WallpaperFit>(readWallpaperFit())

function readWallpaperFit(): WallpaperFit {
  try {
    return localStorage.getItem(WALLPAPER_FIT_KEY) === 'contain' ? 'contain' : 'cover'
  } catch {
    return 'cover'
  }
}

function persistWallpaperFit(): void {
  try {
    localStorage.setItem(WALLPAPER_FIT_KEY, wallpaperFit.value)
  } catch {
    /* 忽略 */
  }
}

/* ---- 壁纸来源：本地导入图片 / 每日 Bing 壁纸 ---- */

/** 壁纸来源持久化键 */
const WALLPAPER_SOURCE_KEY = 'tplayer.lab.backdrop.wallpaperSource'

/** 当前壁纸来源（面板绑定此 ref）：local=本地导入 / daily=每日 Bing */
export const wallpaperSource = ref<'local' | 'daily'>(readWallpaperSource())

function readWallpaperSource(): 'local' | 'daily' {
  try {
    return localStorage.getItem(WALLPAPER_SOURCE_KEY) === 'daily' ? 'daily' : 'local'
  } catch {
    return 'local'
  }
}

function persistWallpaperSource(): void {
  try {
    localStorage.setItem(WALLPAPER_SOURCE_KEY, wallpaperSource.value)
  } catch {
    /* 忽略 */
  }
}

/** 每日壁纸元数据（版权 / 标题），面板展示用 */
export const dailyWallpaper = ref<{ date: string; copyright: string; title: string } | null>(
  null,
)

/** 重新同步壁纸层：确保 `data-backdrop` 与 image 变量一致。
 * 修复启动时 wallpaperPath 尚未就位导致降级纯色、之后补不到图片的问题。 */
export async function refreshWallpaper(): Promise<void> {
  if (backdropMode.value === 'wallpaper') {
    await applyBackdrop('wallpaper')
  }
}

/** 确保每日壁纸就位：命中当日缓存直接返回，否则拉取 Bing 并缓存（见后端命令）。
 * `forceIdx` 用于「换一张」（往日随机图）。成功后写入 wallpaperPath 并刷新 CSS。 */
export async function ensureDailyWallpaper(forceIdx?: number): Promise<void> {
  const res = await labFetchDailyWallpaper(forceIdx)
  if (!res) return
  wallpaperPath.value = res.path
  dailyWallpaper.value = { date: res.date, copyright: res.copyright, title: res.title }
  persistWallpaperSource()
  await refreshWallpaper()
}

/** 切换壁纸来源（local ↔ daily）；切换后即时同步当前壁纸。 */
export async function setWallpaperSource(src: 'local' | 'daily'): Promise<void> {
  if (src === wallpaperSource.value) return
  wallpaperSource.value = src
  persistWallpaperSource()
  if (src === 'daily') {
    await ensureDailyWallpaper()
  } else {
    wallpaperPath.value = await labWallpaperPath()
    await refreshWallpaper()
  }
}

/** 清除每日壁纸缓存（daily.jpg + daily.json）：删除本地缓存后回落纯色。 */
export async function clearDailyWallpaper(): Promise<void> {
  await labClearDailyWallpaper()
  dailyWallpaper.value = null
  if (wallpaperSource.value === 'daily') {
    wallpaperPath.value = null
    if (backdropMode.value === 'wallpaper') {
      backdropMode.value = 'solid'
    }
  }
}

/** 壁纸图层 CSS 变量（供 lab-backdrop.css 的 wallpaper 分支使用） */
const WALLPAPER_IMAGE_VAR = '--lab-wallpaper-image'
const WALLPAPER_SIZE_VAR = '--lab-wallpaper-size'

/** 把后端返回的本地路径转成 WebView 可加载的 URL（asset 协议） */
async function toWallpaperUrl(path: string): Promise<string | null> {
  try {
    const { convertFileSrc } = await import('@tauri-apps/api/core')
    return convertFileSrc(path)
  } catch {
    return null
  }
}

/** 清除壁纸 CSS 变量 */
function clearWallpaperVars(): void {
  const root = document.documentElement
  root.style.removeProperty(WALLPAPER_IMAGE_VAR)
  root.style.removeProperty(WALLPAPER_SIZE_VAR)
}

/** 把当前壁纸状态写进 CSS 变量（无壁纸则清除变量，由 CSS 回落实底） */
async function applyWallpaperVars(): Promise<void> {
  const root = document.documentElement
  if (!wallpaperPath.value) {
    clearWallpaperVars()
    return
  }
  const url = await toWallpaperUrl(wallpaperPath.value)
  if (!url) {
    clearWallpaperVars()
    return
  }
  root.style.setProperty(WALLPAPER_IMAGE_VAR, `url("${url}")`)
  root.style.setProperty(
    WALLPAPER_SIZE_VAR,
    wallpaperFit.value === 'cover' ? 'cover' : 'contain',
  )
}

/* ---- 预设渐变（跟随明暗主题：停止色用 color-mix 从主题变量派生） ---- */

const GRADIENT_KEY = 'tplayer.lab.backdrop.gradient'

/** 渐变预设：css 为完整 background-image 值，停止色经 color-mix 与主题变量混色，
 * 因此明暗主题切换时自动跟随（亮色更亮、暗色更暗），无需维护两套。 */
export interface GradientPreset {
  id: string
  nameKey: string
  css: string
}
export const GRADIENT_PRESETS: GradientPreset[] = [
  {
    id: 'aurora',
    nameKey: 'lab.preset.gradient.aurora',
    css:
      'linear-gradient(135deg, color-mix(in srgb, var(--color-brand) 42%, var(--color-bg-base)), color-mix(in srgb, var(--color-brand) 14%, var(--color-bg-base)))',
  },
  {
    id: 'twilight',
    nameKey: 'lab.preset.gradient.twilight',
    css:
      'linear-gradient(150deg, color-mix(in srgb, #818cf8 46%, var(--color-bg-base)), color-mix(in srgb, #c084fc 16%, var(--color-bg-base)))',
  },
  {
    id: 'warm',
    nameKey: 'lab.preset.gradient.warm',
    css:
      'linear-gradient(150deg, color-mix(in srgb, #fb923c 46%, var(--color-bg-base)), color-mix(in srgb, #f472b6 16%, var(--color-bg-base)))',
  },
  {
    id: 'ocean',
    nameKey: 'lab.preset.gradient.ocean',
    css:
      'linear-gradient(150deg, color-mix(in srgb, #22d3ee 46%, var(--color-bg-base)), color-mix(in srgb, #6366f1 16%, var(--color-bg-base)))',
  },
  {
    id: 'rose',
    nameKey: 'lab.preset.gradient.rose',
    css:
      'linear-gradient(150deg, color-mix(in srgb, #f472b6 46%, var(--color-bg-base)), color-mix(in srgb, #fb7185 16%, var(--color-bg-base)))',
  },
]
/** 当前选中的渐变预设 id（面板绑定此 ref） */
export const gradientId = ref<string>(readGradientId())

function readGradientId(): string {
  try {
    const v = localStorage.getItem(GRADIENT_KEY)
    return GRADIENT_PRESETS.some((g) => g.id === v) ? (v as string) : 'aurora'
  } catch {
    return 'aurora'
  }
}

function persistGradientId(): void {
  try {
    localStorage.setItem(GRADIENT_KEY, gradientId.value)
  } catch {
    /* 忽略 */
  }
}

const GRADIENT_VAR = '--lab-gradient'

/** 写入渐变 CSS 变量（lab-backdrop.css 的 gradient 分支消费） */
function applyGradientVars(): void {
  const root = document.documentElement
  const preset = GRADIENT_PRESETS.find((g) => g.id === gradientId.value) ?? GRADIENT_PRESETS[0]!
  root.style.setProperty(GRADIENT_VAR, preset.css)
}

/* ---- 纹理图案（纯内联，不依赖外部图片；随明暗主题派生色调） ---- */

const TEXTURE_KEY = 'tplayer.lab.backdrop.texture'

/** 纹理预设：css 为完整 background-image 值。
 * 每条预设自带 `color-mix()` 派生的可见度（不共用统一淡色调，避免圆点/噪点被冲淡），
 * 依次为 网格 / 斜纹 / 圆点 / 噪点；明暗主题下自动跟随次要文本色。 */
export interface TexturePreset {
  id: string
  nameKey: string
  css: string
}
export const TEXTURE_PRESETS: TexturePreset[] = [
  {
    id: 'grid',
    nameKey: 'lab.preset.texture.grid',
    css:
      'repeating-linear-gradient(0deg, color-mix(in srgb, var(--color-text-secondary) 20%, transparent) 0 1px, transparent 1px 20px), repeating-linear-gradient(90deg, color-mix(in srgb, var(--color-text-secondary) 20%, transparent) 0 1px, transparent 1px 20px)',
  },
  {
    id: 'diagonal',
    nameKey: 'lab.preset.texture.diagonal',
    css:
      'repeating-linear-gradient(45deg, color-mix(in srgb, var(--color-text-secondary) 20%, transparent) 0 7px, transparent 7px 16px)',
  },
  {
    id: 'dots',
    nameKey: 'lab.preset.texture.dots',
    css:
      'radial-gradient(color-mix(in srgb, var(--color-text-secondary) 34%, transparent) 2.2px, transparent 2.6px) 0 0 / 16px 16px',
  },
  {
    id: 'noise',
    nameKey: 'lab.preset.texture.noise',
    css:
      "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='120' height='120'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='2' stitchTiles='stitch'/%3E%3CfeColorMatrix type='saturate' values='0'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='0.3'/%3E%3C/svg%3E\")",
  },
]
/** 当前选中的纹理预设 id（面板绑定此 ref） */
export const textureId = ref<string>(readTextureId())

function readTextureId(): string {
  try {
    const v = localStorage.getItem(TEXTURE_KEY)
    return TEXTURE_PRESETS.some((t) => t.id === v) ? (v as string) : 'grid'
  } catch {
    return 'grid'
  }
}

function persistTextureId(): void {
  try {
    localStorage.setItem(TEXTURE_KEY, textureId.value)
  } catch {
    /* 忽略 */
  }
}

const TEXTURE_VAR = '--lab-texture'

/** 写入纹理 CSS 变量（lab-backdrop.css 的 texture 分支消费） */
function applyTextureVars(): void {
  const root = document.documentElement
  const preset = TEXTURE_PRESETS.find((t) => t.id === textureId.value) ?? TEXTURE_PRESETS[0]!
  // 各预设自带 color-mix 可见度，直接写入 background-image 即可
  root.style.setProperty(TEXTURE_VAR, preset.css)
}

/**
 * 应用底纹（幂等，可重复调用）：
 * - `off`：移除 data-backdrop + 清除原生特效 → 回落现有不透明外观；
 * - `native`：先问后端能否真正生效，**生效**才用透明表层让 OS 材质透出，
 *   **未生效**（Win7 / Linux / 系统关闭透明效果）自动降级为 solid，绝不裸窗；
 * - `wallpaper`：有壁纸才用该模式；没有则降级为 solid（并提示用户先导入）；
 * - `gradient` / `texture`：由 CSS 底纹层呈现，各自写入对应 CSS 变量；
 * - 其它（solid / 预留模式）：直接设置 data-backdrop，由 CSS 底纹层呈现。
 */
async function applyBackdrop(mode: BackdropMode): Promise<void> {
  const root = document.documentElement
  if (mode === 'off') {
    root.removeAttribute('data-backdrop')
    await labWindowSetEffect('off')
    nativeActive.value = false
    return
  }
  // 非原生模式：先清除任何已生效的 OS 原生材质，否则会透过半透明外壳露出
  if (mode !== 'native') {
    await labWindowSetEffect('off')
  }
  let effective: BackdropMode = mode
  if (mode === 'native') {
    effective = (await labWindowSetEffect('native')) ? 'native' : 'solid'
  } else if (mode === 'wallpaper' && !wallpaperPath.value) {
    // 未导入壁纸：降级纯色（面板会提示先导入）
    effective = 'solid'
  }
  nativeActive.value = effective === 'native'
  root.setAttribute('data-backdrop', effective)
  // 各模式同步对应 CSS 变量（非该模式则清除其变量）
  if (mode === 'wallpaper') {
    await applyWallpaperVars()
  } else {
    clearWallpaperVars()
  }
  if (mode === 'gradient') {
    applyGradientVars()
  } else {
    root.style.removeProperty(GRADIENT_VAR)
  }
  if (mode === 'texture') {
    applyTextureVars()
  } else {
    root.style.removeProperty(TEXTURE_VAR)
  }
}

/**
 * 挂载皮肤配色的反应式生效：
 * - 强调色：开关打开 → 应用自定义强调色；关闭 → 清除回落默认；
 * - 窗口底纹：开关打开 → 应用当前模式；关闭 → 整体移除（含原生特效），
 *   窗口外观回到与现状完全一致的不透明状态。
 *
 * 仅当 lab.isEnabled('theme-colors') 为真时应用，关闭时零影响。
 */
export function useThemeColors(): void {
  const lab = useLabStore()
  watch(
    [() => lab.isEnabled('theme-colors'), () => customAccent.value],
    ([on, color]) => {
      if (on) {
        applyAccent(color as string)
        try {
          localStorage.setItem(ACCENT_KEY, color as string)
        } catch {
          /* 忽略 */
        }
      } else {
        clearAccent()
      }
    },
    { immediate: true },
  )

  watch(
    [() => lab.isEnabled('theme-colors'), () => backdropMode.value],
    ([on]) => {
      const mode = on ? backdropMode.value : 'off'
      // 持久化用户选择（含 off）
      try {
        localStorage.setItem(BACKDROP_KEY, backdropMode.value)
      } catch {
        /* 忽略 */
      }
      void applyBackdrop(mode)
    },
    { immediate: true },
  )

  // 壁纸铺放方式变化：实时刷新 CSS 变量（无需重新导入）
  watch(wallpaperFit, () => {
    persistWallpaperFit()
    void applyWallpaperVars()
  })

  // 渐变预设切换：实时刷新 CSS 变量（仅当处于渐变模式时可见）
  watch(gradientId, () => {
    persistGradientId()
    if (backdropMode.value === 'gradient') applyGradientVars()
  })

  // 纹理预设切换：实时刷新 CSS 变量（仅当处于纹理模式时可见）
  watch(textureId, () => {
    persistTextureId()
    if (backdropMode.value === 'texture') applyTextureVars()
  })
}

/* ===================== 壁纸操作（面板调用） ===================== */

/** 启动时加载当前壁纸（按来源：每日 Bing 或本地导入）并同步 CSS 层 */
export async function loadWallpaper(): Promise<void> {
  if (wallpaperSource.value === 'daily') {
    await ensureDailyWallpaper()
  } else {
    wallpaperPath.value = await labWallpaperPath()
  }
  await refreshWallpaper()
}

/**
 * 导入壁纸：弹出系统图片选择器 → 复制到 app 数据目录 → 写入 CSS 变量。
 * 导入即视为「本地图片」来源。成功返回 true；用户取消 / 失败返回 false。
 */
export async function importWallpaper(): Promise<boolean> {
  const { pickImageFile } = await import('../../services/dialog')
  const picked = await pickImageFile()
  if (!picked) return false
  const stored = await labImportWallpaper(picked)
  if (!stored) return false
  wallpaperSource.value = 'local'
  persistWallpaperSource()
  wallpaperPath.value = stored
  await refreshWallpaper()
  return true
}

/** 移除壁纸：本地来源删除本地副本；每日来源停止每日、切回本地。
 *  若当前处于壁纸模式且无可用图，则降级为纯色。 */
export async function removeWallpaper(): Promise<void> {
  if (wallpaperSource.value === 'daily') {
    wallpaperSource.value = 'local'
    persistWallpaperSource()
  } else {
    await labRemoveWallpaper()
  }
  wallpaperPath.value = null
  clearWallpaperVars()
  if (backdropMode.value === 'wallpaper') {
    backdropMode.value = 'solid'
  }
}

/* ===================== 实验室·自定义壁纸（独立功能门控） ===================== */

/**
 * 自定义壁纸（含每日 Bing）作为**独立实验室功能**的激活钩子：
 * - 开启 → 自动切到壁纸模式，并按当前来源（本地导入 / 每日 Bing）加载图片、刷新 CSS；
 * - 关闭 → 移除壁纸、降级纯色，窗口回到与现状一致状态。
 *
 * 仅当 `lab.isEnabled('custom-wallpaper')` 为真时生效；关闭时零影响。
 * 与「皮肤配色」解耦：强调色 / 渐变 / 纹理仍归皮肤配色，壁纸图源归本功能。
 */
export function useCustomWallpaper(): void {
  const lab = useLabStore()
  watch(
    () => lab.isEnabled('custom-wallpaper'),
    (on) => {
      if (on) {
        // 切到壁纸模式；loadWallpaper 会按来源（本地 / 每日）拉取图片并刷新 CSS 层
        backdropMode.value = 'wallpaper'
        void loadWallpaper()
      } else {
        wallpaperPath.value = null
        clearWallpaperVars()
        if (backdropMode.value === 'wallpaper') {
          backdropMode.value = 'off'
        }
      }
    },
    { immediate: true },
  )
}
