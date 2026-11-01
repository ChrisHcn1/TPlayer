/**
 * stores/deps.ts — 解码组件依赖管理 store 单元测试
 *
 * 覆盖方案要求的三个关键分支：
 *  1. 更新场景：本地已有依赖 → 检测到即复用，不弹引导；
 *  2. 首次安装：缺少必需组件 → 弹引导；
 *  3. 缺依赖反馈：安装失败保留原因（网络 / 校验 / 解压）。
 *
 * 后端调用全部 mock（services/deps），不触碰真实文件系统与网络。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

vi.mock('../services/deps', () => ({
  DEPS_SIZE_HINT: '约 430 MB',
  depsService: {
    status: vi.fn(),
    install: vi.fn(),
    installLocal: vi.fn(),
    openFolder: vi.fn(),
    remove: vi.fn(),
    onProgress: vi.fn(async () => () => {}),
  },
}))

vi.mock('../i18n', () => ({
  t: (key: string, params?: Record<string, string | number>) => {
    const map: Record<string, string> = {
      'settings.depsPurpose.ffmpeg': '解码 APE / DSD / WMA 等格式',
      'settings.depsPurpose.ffprobe': '探测时长、采样率与声道数',
      'settings.depsPurpose.sacd_extract': '解码 SACD / DST 音轨',
      'settings.depsSource.deps': '独立依赖包',
      'settings.depsSource.programDir': '程序内置',
      'settings.depsSource.system': '系统安装',
      'settings.depsSource.missing': '未安装',
      'settings.depsInstalledVersion': '解码组件已安装（v{version}）',
      'settings.depsAlreadyUpToDate': '解码组件已是最新版本，无需更新',
      'settings.depsInstallFailedPrefix': '安装解码组件失败：',
      'settings.depsOpenFolderHint': '已打开依赖目录，请将 ffmpeg / ffprobe / sacd_extract 放入其中',
      'settings.depsUninstalled': '已卸载解码组件',
      'settings.depsUninstallFailedPrefix': '卸载解码组件失败：',
      'settings.depsChecking': '正在检测…',
      'settings.depsNotChecked': '尚未检测',
      'settings.depsInstalledPack': '已安装 {pack}',
      'settings.depsReady': '已就绪',
      'settings.depsListSep': '、',
      'settings.depsMissing': '缺少 {missing}',
      'settings.depsOpenFolderFailedPrefix': '打开依赖目录失败：',
      'settings.depsSizeHint': '约 430 MB（FFmpeg + FFprobe + SACD Extract）',
    }
    let s = map[key] ?? key
    if (params) {
      for (const [k, v] of Object.entries(params)) {
        s = s.replace(`{${k}}`, String(v))
      }
    }
    return s
  },
}))

import { depsService, type DepsStatus } from '../services/deps'
import { useDepsStore } from './deps'

/** 构造一份依赖状态 */
function makeStatus(overrides: Partial<DepsStatus> = {}): DepsStatus {
  return {
    depsDir: 'C:\\Users\\test\\AppData\\Roaming\\com.tplayer\\deps',
    platform: 'windows-x86_64',
    installedPack: null,
    expectedPack: '1.0.0',
    outdated: false,
    tools: [
      {
        name: 'ffmpeg',
        required: true,
        installed: false,
        path: null,
        source: 'missing',
        packVersion: null,
      },
      {
        name: 'ffprobe',
        required: false,
        installed: false,
        path: null,
        source: 'missing',
        packVersion: null,
      },
      {
        name: 'sacd_extract',
        required: false,
        installed: false,
        path: null,
        source: 'missing',
        packVersion: null,
      },
    ],
    missing: ['ffmpeg', 'ffprobe', 'sacd_extract'],
    ok: false,
    installable: true,
    ...overrides,
  }
}

/** 全部就绪的状态（更新场景：已装依赖 → 复用） */
function readyStatus(): DepsStatus {
  const status = makeStatus({
    installedPack: '1.0.0',
    missing: [],
    ok: true,
  })
  status.tools = status.tools.map((tool) => ({
    ...tool,
    installed: true,
    source: 'deps',
    path: 'C:\\deps\\packs\\1.0.0\\' + tool.name + '.exe',
    packVersion: '1.0.0',
  }))
  return status
}

beforeEach(() => {
  setActivePinia(createPinia())
  localStorage.clear()
  vi.clearAllMocks()
})

describe('useDepsStore — 检测', () => {
  it('refresh 成功后写入状态', async () => {
    const next = readyStatus()
    vi.mocked(depsService.status).mockResolvedValue(next)
    const store = useDepsStore()
    const got = await store.refresh()
    expect(got).toEqual(next)
    expect(store.status?.ok).toBe(true)
    expect(store.unavailable).toBe(false)
  })

  it('检测失败时静默降级（不抛错）', async () => {
    vi.mocked(depsService.status).mockRejectedValue(new Error('IPC unavailable'))
    const store = useDepsStore()
    const got = await store.refresh()
    expect(got).toBeNull()
    expect(store.unavailable).toBe(true)
    expect(store.error).toContain('IPC unavailable')
  })
})

describe('useDepsStore — 引导触发分支', () => {
  it('更新场景：已有依赖 → 直接复用，不弹引导', async () => {
    vi.mocked(depsService.status).mockResolvedValue(readyStatus())
    const store = useDepsStore()
    await store.init()
    expect(store.guideOpen).toBe(false)
    expect(store.summary()).toContain('已安装')
  })

  it('首次安装：缺少必需组件 → 弹引导', async () => {
    vi.mocked(depsService.status).mockResolvedValue(makeStatus())
    const store = useDepsStore()
    await store.init()
    expect(store.guideOpen).toBe(true)
    expect(store.summary()).toContain('FFmpeg')
  })

  it('仅缺少非必需组件（ffprobe）→ 不打扰用户', async () => {
    const partial = makeStatus({ missing: ['ffprobe'], ok: true })
    vi.mocked(depsService.status).mockResolvedValue(partial)
    const store = useDepsStore()
    await store.init()
    expect(store.guideOpen).toBe(false)
  })

  it('用户选择「不再提示」→ 后续启动不再弹引导', async () => {
    vi.mocked(depsService.status).mockResolvedValue(makeStatus())
    const store = useDepsStore()
    await store.init()
    expect(store.guideOpen).toBe(true)
    store.dismissGuide()
    expect(store.guideOpen).toBe(false)
    expect(store.guideIgnored).toBe(true)

    // 重新初始化（模拟下次启动）：读取持久化标记后不再弹窗
    const store2 = useDepsStore()
    await store2.init()
    expect(store2.guideOpen).toBe(false)
  })
})

describe('useDepsStore — 安装', () => {
  it('安装成功 → 关闭引导并刷新状态', async () => {
    const ready = readyStatus()
    vi.mocked(depsService.install).mockResolvedValue({
      version: '1.0.0',
      changed: true,
      status: ready,
    })
    const store = useDepsStore()
    await store.init()
    const ok = await store.install()
    expect(ok).toBe(true)
    expect(store.guideOpen).toBe(false)
    expect(store.status?.installedPack).toBe('1.0.0')
    expect(store.phase).toBe('idle')
  })

  it('安装失败 → 保留具体原因（供 UI 展示分类提示）', async () => {
    vi.mocked(depsService.install).mockRejectedValue(new Error('下载解码组件失败：网络不可达'))
    const store = useDepsStore()
    const ok = await store.install()
    expect(ok).toBe(false)
    expect(store.error).toContain('网络不可达')
    expect(store.phase).toBe('error')
  })

  it('安装失败（后端返回错误对象）→ 取可读 message，绝不出现 [object Object]', async () => {
    // 后端 AppError 经 IPC 是普通对象 { code, message, detail }，并非 Error 实例
    vi.mocked(depsService.install).mockRejectedValue({
      code: 'DEPS_INSTALL',
      message: '下载解码组件失败：HTTP 404',
      detail: null,
    })
    const store = useDepsStore()
    const ok = await store.install()
    expect(ok).toBe(false)
    expect(store.error).toContain('HTTP 404')
    expect(store.error).not.toContain('[object Object]')
    expect(store.phase).toBe('error')
  })

  it('重新检测成功后清理上一次安装失败残留的错误阶段', async () => {
    vi.mocked(depsService.install).mockRejectedValue({
      code: 'DEPS_INSTALL',
      message: '网络不可达',
      detail: null,
    })
    vi.mocked(depsService.status).mockResolvedValue(readyStatus())
    const store = useDepsStore()
    await store.install()
    expect(store.phase).toBe('error')
    await store.refresh()
    expect(store.phase).toBe('idle')
    expect(store.error).toBe('')
  })

  it('安装中重复调用被忽略（防并发）', async () => {
    let resolveFn: (() => void) | undefined
    vi.mocked(depsService.install).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveFn = () => resolve({ version: '1.0.0', changed: true, status: readyStatus() })
        }),
    )
    const store = useDepsStore()
    const first = store.install()
    const second = await store.install()
    expect(second).toBe(false)
    expect(store.installing).toBe(true)
    resolveFn?.()
    await first
    expect(store.installing).toBe(false)
  })
})

describe('useDepsStore — 来源与文案', () => {
  it('来源中文映射正确', () => {
    const store = useDepsStore()
    expect(store.sourceLabel('deps')).toBe('独立依赖包')
    expect(store.sourceLabel('programDir')).toBe('程序内置')
    expect(store.sourceLabel('system')).toBe('系统安装')
    expect(store.sourceLabel('missing')).toBe('未安装')
  })

  it('工具名与用途有中文说明', () => {
    const store = useDepsStore()
    expect(store.toolLabel('ffmpeg')).toBe('FFmpeg')
    expect(store.toolLabel('sacd_extract')).toBe('SACD Extract')
    expect(store.toolPurpose('ffprobe')).toContain('采样率')
  })
})
