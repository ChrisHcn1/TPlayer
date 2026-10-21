/**
 * Vitest 全局初始化
 * - 兼容 Tauri 运行时：测试环境下 @tauri-apps/* 的动态 import 不存在真实实现，
 *   这里通过 globalThis 提供最小 stub，避免 import.meta.env 分支或 window undefined 报错。
 */
import { vi } from 'vitest'

// jsdom 下 window.matchMedia 默认缺失，ProgressBar 以外的组件也可能用到
if (typeof window !== 'undefined' && !window.matchMedia) {
  vi.stubGlobal(
    'matchMedia',
    vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  )
}
