/**
 * 国际化（i18n）运行时（M3）
 *
 * 设计目标（与 src/i18n/nav.ts 注释中的规划一致）：
 *  - **独立语言资源文件**：每种语言一份 JSON，置于 `public/locales/<code>.json`，
 *    由前端在运行时按需 `fetch` 加载，不打包进核心逻辑；
 *  - **集中管理**：所有界面文案与提示信息统一走 `t(key)`，键在资源文件中维护；
 *  - **按语言自动加载与切换**：`initI18n()` 启动即载入已选语言，`setLocale()` 运行时热切换；
 *  - **默认语言兜底**：缺翻译先回落默认语言（zh-CN），再回落键尾段，绝不暴露原始 `nav.xxx`；
 *  - **易扩展**：新增语言 = 在 `public/locales/` 放一份 `<code>.json` + 在 `LOCALES` 追加一条，
 *    核心代码零改动（见 `LOCALES` 注释）。
 *
 * 资源文件结构（嵌套命名空间，键以 `.` 连接，如 `nav.songs`）：
 *   {
 *     "nav":   { "songs": "全部歌曲", "group": { "library": "曲库" } },
 *     "common": { "confirm": "确定" },
 *     "player": { "eq": "均衡器" },
 *     "settings": { "language": "语言" }
 *   }
 */

import { ref, shallowRef } from 'vue'

import { DEFAULT_LOCALE } from '../constants'

/** 单条语言元信息（用于选择器展示） */
export interface LocaleMeta {
  /** BCP-47 代码，同时作为资源文件名，如 `zh-CN` / `en` */
  code: string
  /** 英文名（选择器兜底展示） */
  name: string
  /** 母语名（endonym），如 `简体中文` / `English` */
  nativeName: string
}

/**
 * 支持的语言清单（manifest）。
 *
 * 新增语言接入流程（保持一致、无需改核心）：
 *  1. 在 `public/locales/` 下新增 `<code>.json`，键结构对齐现有文件（缺失键自动回落默认语言）；
 *  2. 在此追加一条 `{ code, name, nativeName }`。
 * 加载 / 切换 / 兜底逻辑全自动。
 */
export const LOCALES: LocaleMeta[] = [
  { code: 'zh-CN', name: 'Chinese (Simplified)', nativeName: '简体中文' },
  { code: 'en', name: 'English', nativeName: 'English' },
]

/** 资源文件根路径（与 `public/locales` 对应） */
const LOCALES_DIR = 'locales'

type Messages = Record<string, unknown>

/** 默认语言资源（始终常驻，作为兜底来源） */
const fallbackMessages = shallowRef<Messages>({})
/** 当前语言资源 */
const messages = shallowRef<Messages>({})
/** 当前语言代码 */
const currentLocale = ref<string>(DEFAULT_LOCALE)
/** 默认语言是否已就绪（启动预载后即为 true，保证首屏有兜底） */
const ready = ref(false)
/** 已加载语言缓存，避免重复 fetch */
const cache = new Map<string, Messages>()

/** 按点路径解析嵌套对象中的字符串值 */
function resolve(obj: Messages, key: string): string | undefined {
  const parts = key.split('.')
  let cur: unknown = obj
  for (const part of parts) {
    if (cur && typeof cur === 'object' && part in (cur as Messages)) {
      cur = (cur as Messages)[part]
    } else {
      return undefined
    }
  }
  return typeof cur === 'string' ? cur : undefined
}

/** 占位符替换：`Hello {name}` + `{ name: 'X' }` → `Hello X` */
function interpolate(template: string, params?: Record<string, string | number>): string {
  if (!params) return template
  return template.replace(/\{(\w+)\}/g, (_, name: string) =>
    name in params ? String(params[name]) : `{${name}}`,
  )
}

/** 当前/兜底语言中是否存在该键（用于决定是否走静态兜底） */
export function hasMessage(key: string): boolean {
  return resolve(messages.value, key) !== undefined || resolve(fallbackMessages.value, key) !== undefined
}

/**
 * 翻译主入口。
 * 查找顺序：当前语言 → 默认语言兜底 → 键尾段（如 `nav.songs` → `songs`）。
 */
export function t(key: string, params?: Record<string, string | number>): string {
  const value = resolve(messages.value, key) ?? resolve(fallbackMessages.value, key)
  if (value === undefined) {
    const segments = key.split('.')
    return segments[segments.length - 1] ?? key
  }
  return interpolate(value, params)
}

/** 读取资源文件（带缓存）；失败向上抛出，由调用方决定兜底策略 */
async function fetchLocale(code: string): Promise<Messages> {
  const cached = cache.get(code)
  if (cached) return cached
  const base = (import.meta as { env?: { BASE_URL?: string } }).env?.BASE_URL ?? '/'
  const url = `${base}${LOCALES_DIR}/${code}.json`
  const response = await fetch(url)
  if (!response.ok) throw new Error(`locale "${code}" not found (HTTP ${response.status})`)
  const json = (await response.json()) as Messages
  cache.set(code, json)
  return json
}

/** 加载某语言：默认语言恒为兜底来源；与当前语言一致时同步到活动资源 */
async function loadLocale(code: string): Promise<Messages> {
  const loaded = await fetchLocale(code)
  if (code === DEFAULT_LOCALE) fallbackMessages.value = loaded
  if (code === currentLocale.value) messages.value = loaded
  return loaded
}

/**
 * 初始化 i18n：先确保默认语言兜底常驻，再加载已选语言。
 * 推荐在应用挂载前（main.ts）与设置恢复后（App.vue）各调用一次——
 * 前者消除首屏闪烁，后者应用用户在设置中实际选择的语言。
 *
 * settings store 经动态 import 引入：i18n 是底层工具，若静态依赖 store 会形成
 * （i18n → stores/settings → services/player → services/compat → i18n）循环依赖，
 * 在测试与首次模块求值时触发 "Cannot access before initialization"。
 */
export async function initI18n(): Promise<void> {
  const { useSettingsStore } = await import('../stores/settings')
  const locale = useSettingsStore().locale || DEFAULT_LOCALE
  currentLocale.value = locale
  await loadLocale(DEFAULT_LOCALE)
  if (locale !== DEFAULT_LOCALE) {
    try {
      await loadLocale(locale)
    } catch {
      // 选中语言资源缺失：回落默认语言，不阻断启动
      messages.value = fallbackMessages.value
    }
  } else {
    messages.value = fallbackMessages.value
  }
  ready.value = true
  applyDocumentLang(locale)
}

/**
 * 切换语言：加载资源（失败回落默认）+ 更新 <html lang> + 持久化到设置。
 * 仅接受清单内语言，非法值直接忽略，避免写入脏 locale。
 */
export async function setLocale(code: string): Promise<void> {
  if (!LOCALES.some((item) => item.code === code)) return
  currentLocale.value = code
  try {
    await loadLocale(code)
  } catch {
    messages.value = fallbackMessages.value
  }
  applyDocumentLang(code)
  const { useSettingsStore } = await import('../stores/settings')
  await useSettingsStore().setLocale(code)
}

/** 同步文档语言属性（SEO / 无障碍） */
function applyDocumentLang(code: string): void {
  if (typeof document !== 'undefined') document.documentElement.lang = code
}

/** 组合式入口：模板与脚本统一取用 */
export function useI18n() {
  return {
    t,
    hasMessage,
    currentLocale,
    ready,
    availableLocales: LOCALES,
    setLocale,
    initI18n,
  }
}
