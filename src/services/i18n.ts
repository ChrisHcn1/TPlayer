// 语言服务

import { ref } from 'vue'

// 语言类型定义
export interface Locale {
  code: string
  name: string
}

// 支持的语言列表
export const supportedLocales: Locale[] = [
  { code: 'zh-CN', name: '简体中文' },
  { code: 'en-US', name: 'English' }
]

// 翻译函数类型
export type TranslateFunction = (key: string, fallback?: string) => string

// 语言服务类
class I18nService {
  private currentLanguage = ref<string>('zh-CN')
  private translations = ref<Record<string, any>>({})
  private initialized = ref<boolean>(false)

  // 初始化语言服务
  async initialize(language: string = 'zh-CN'): Promise<void> {
    try {
      // 确保语言代码有效
      const validLanguage = supportedLocales.some(locale => locale.code === language)
        ? language
        : 'zh-CN'

      this.currentLanguage.value = validLanguage
      await this.loadTranslations(validLanguage)
      this.initialized.value = true
    } catch (error) {
      console.error('Failed to initialize i18n service:', error)
      // 回退到默认语言
      this.currentLanguage.value = 'zh-CN'
      await this.loadTranslations('zh-CN')
      this.initialized.value = true
    }
  }

  // 加载翻译文件
  private async loadTranslations(language: string): Promise<void> {
    try {
      const response = await fetch(`/locales/${language}.json`)
      if (!response.ok) {
        throw new Error(`Failed to load translations for ${language}`)
      }
      this.translations.value = await response.json()
    } catch (error) {
      console.error(`Failed to load translations for ${language}:`, error)
      // 回退到默认语言
      if (language !== 'zh-CN') {
        await this.loadTranslations('zh-CN')
      }
    }
  }

  // 切换语言
  async changeLanguage(language: string): Promise<void> {
    if (language !== this.currentLanguage.value) {
      await this.initialize(language)
    }
  }

  // 获取当前语言
  getCurrentLanguage(): string {
    return this.currentLanguage.value
  }

  // 翻译函数
  t(key: string, fallback: string = key): string {
    if (!this.initialized.value) {
      return fallback
    }

    // 解析嵌套键，如 'common.appName'
    const keys = key.split('.')
    let result: any = this.translations.value

    for (const k of keys) {
      if (result && typeof result === 'object' && k in result) {
        result = result[k]
      } else {
        return fallback
      }
    }

    return typeof result === 'string' ? result : fallback
  }

  // 获取当前语言的响应式引用
  getCurrentLanguageRef() {
    return this.currentLanguage
  }

  // 获取翻译的响应式引用
  getTranslationsRef() {
    return this.translations
  }
}

// 导出单例实例
export const i18nService = new I18nService()

// 导出翻译函数
export const t: TranslateFunction = (key, fallback) => i18nService.t(key, fallback)
