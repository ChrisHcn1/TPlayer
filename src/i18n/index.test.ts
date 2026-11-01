import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

import { hasMessage, initI18n, LOCALES, t } from './index'

describe('i18n（国际化运行时）', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.restoreAllMocks()
  })

  it('LOCALES 清单含默认 zh-CN 与英文 en（新增语言 = 追加一条 + 放一份 json）', () => {
    const codes = LOCALES.map((item) => item.code)
    expect(codes).toContain('zh-CN')
    expect(codes).toContain('en')
  })

  it('initI18n 加载默认资源后，t() 支持占位符插值且缺键回落默认语言', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: true,
        status: 200,
        json: async () => ({
          test: { hello: 'Hello {name}' },
          nav: { songs: '全部歌曲' },
        }),
      })),
    )
    await initI18n()
    expect(t('test.hello', { name: 'World' })).toBe('Hello World')
    expect(t('nav.songs')).toBe('全部歌曲')
    expect(hasMessage('test.hello')).toBe(true)
  })

  it('任意语言均缺失某键时，回落键尾段而非暴露原始 nav.xxx', () => {
    expect(t('nav.unknown')).toBe('unknown')
    expect(t('foo.bar.baz')).toBe('baz')
  })
})
