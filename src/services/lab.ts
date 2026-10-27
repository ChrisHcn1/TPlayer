/**
 * 实验室（Lab）服务
 *
 * 与主设置解耦：只经 lab_get / lab_set_feature / lab_set_status 三个独立命令
 * 读写 config.json 的 lab 段（浏览器预览回落 localStorage）。store 不直接碰
 * IPC / localStorage，统一走本服务。
 */
import { callCommand, isTauriRuntime } from './ipc'

/** 实验室配置（与 Rust config::LabConfig 对齐） */
export interface LabState {
  /** 功能 id → 是否开启（缺省视为关闭） */
  features: Record<string, boolean>
}

const LAB_STORAGE_KEY = 'tplayer.lab'

function emptyState(): LabState {
  return { features: {} }
}

function webLoad(): LabState {
  try {
    const raw = localStorage.getItem(LAB_STORAGE_KEY)
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<LabState>
      return { features: parsed.features ?? {} }
    }
  } catch {
    /* 忽略损坏数据 */
  }
  return emptyState()
}

function webSave(state: LabState): void {
  try {
    localStorage.setItem(LAB_STORAGE_KEY, JSON.stringify(state))
  } catch {
    /* 隐私模式等忽略 */
  }
}

/** 读取实验室配置 */
export async function labGet(): Promise<LabState> {
  if (isTauriRuntime()) {
    try {
      return await callCommand<LabState>('lab_get')
    } catch {
      return emptyState()
    }
  }
  return webLoad()
}

/** 设置某功能开启 / 关闭 */
export async function labSetFeature(id: string, enabled: boolean): Promise<void> {
  if (isTauriRuntime()) {
    await callCommand('lab_set_feature', { id, enabled })
    return
  }
  const s = webLoad()
  s.features[id] = enabled
  webSave(s)
}

/* ---------- 实验室·元数据增强：在线校验状态与单曲重试 ---------- */

/** 在线校验状态 */
export interface MetaVerifyStatus {
  /** 是否有后台批量校验任务在运行 */
  running: boolean
}

/** 查询元数据在线校验状态；浏览器环境返回 false */
export async function metaVerifyStatus(): Promise<MetaVerifyStatus> {
  if (isTauriRuntime()) {
    try {
      return await callCommand<MetaVerifyStatus>('library_meta_verify_status')
    } catch {
      return { running: false }
    }
  }
  return { running: false }
}

/** 单曲重试记录（与 Rust verify::RetryRecord 对齐） */
export interface MetaRetryRecord {
  /** 曲目 ID（被重试的那一首） */
  trackId: string
  /** 重试时间（本地 "YYYY-MM-DD HH:MM:SS"） */
  at: string
  /** 结果状态：ok（已更新）/ failed（无可信结果）/ skipped（无需变更） */
  status: 'ok' | 'failed' | 'skipped'
  /** 中文说明（成功列出变更字段，失败 / 跳过给出原因） */
  detail: string
  /** 重试后的标题 */
  title: string
  /** 重试后的艺术家 */
  artist: string
  /** 重试后的专辑 */
  album: string
  /** 识别结果的保存位置：tag（音频标签）/ sidecar（附属文件）/ none（未保存） */
  savedTo?: 'tag' | 'sidecar' | 'none'
  /** 保存说明（写入的标签类型 / 降级原因 / 失败原因） */
  saveDetail?: string
}

/** 读取完整重试日志（按时间正序，同一曲目可能多条） */
export async function metaVerifyLog(): Promise<MetaRetryRecord[]> {
  if (isTauriRuntime()) {
    try {
      return await callCommand<MetaRetryRecord[]>('library_meta_verify_log')
    } catch {
      return []
    }
  }
  return []
}

/**
 * 对指定单曲发起重试：只重新识别这一首（重新解析文件名 + 联网查找），
 * 不影响其他曲目；结果同时追加进重试日志。浏览器环境返回 null。
 */
export async function metaVerifyRetryTrack(trackId: string): Promise<MetaRetryRecord | null> {
  if (isTauriRuntime()) {
    try {
      return await callCommand<MetaRetryRecord>('library_meta_verify_retry_track', { trackId })
    } catch {
      return null
    }
  }
  return null
}

/** 窗口底纹模式（实验室·皮肤配色） */
export type BackdropMode = 'off' | 'solid' | 'native' | 'wallpaper' | 'gradient' | 'texture'

/** 壁纸铺放方式 */
export type WallpaperFit = 'cover' | 'contain'

/** 每日壁纸（Bing 每日图像）返回数据 */
export interface DailyWallpaper {
  /** 本地缓存图片完整路径（前端经 convertFileSrc 加载） */
  path: string
  /** 本地日期 YYYYMMDD（是否已缓存当日的判据） */
  date: string
  /** 图片版权信息 */
  copyright: string
  /** 图片标题 */
  title: string
}

/**
 * 拉取每日壁纸（Bing 每日图像，与 Edge「每日图像」同源）。
 * `idx`：归档偏移（0 = 今日，1..7 = 往日），用于「换一张」制造变化。
 * 浏览器环境返回 null。
 */
export async function labFetchDailyWallpaper(idx?: number): Promise<DailyWallpaper | null> {
  if (isTauriRuntime()) {
    try {
      return await callCommand<DailyWallpaper | null>('lab_fetch_daily_wallpaper', { idx })
    } catch {
      return null
    }
  }
  return null
}

/** 清除每日壁纸缓存（daily.jpg + daily.json）；浏览器环境无操作 */
export async function labClearDailyWallpaper(): Promise<void> {
  if (isTauriRuntime()) {
    try {
      await callCommand('lab_clear_daily_wallpaper')
    } catch {
      /* 忽略 */
    }
  }
}

/**
 * 导入自定义壁纸：把用户选中的本地图片复制到 app 数据目录，返回可加载路径。
 * 浏览器环境返回 null（无原生文件 IO）。
 */
export async function labImportWallpaper(sourcePath: string): Promise<string | null> {
  if (isTauriRuntime()) {
    try {
      return await callCommand<string>('lab_import_wallpaper', { sourcePath })
    } catch {
      return null
    }
  }
  return null
}

/** 查询已导入的壁纸路径；未导入返回 null */
export async function labWallpaperPath(): Promise<string | null> {
  if (isTauriRuntime()) {
    try {
      return await callCommand<string | null>('lab_wallpaper_path')
    } catch {
      return null
    }
  }
  return null
}

/** 移除已导入的壁纸 */
export async function labRemoveWallpaper(): Promise<void> {
  if (isTauriRuntime()) {
    await callCommand('lab_remove_wallpaper')
  }
}

/**
 * 设置主窗口原生材质（运行时切换，无需重启）。
 *
 * 后端返回**原生特效是否真正生效**：生效 → 前端用透明表层让 OS 材质透出；
 * 未生效（Win7 / Linux / 系统关闭透明效果等）→ 前端自动降级到 CSS 纯色基底，
 * 任何情况下都不会出现全透明裸窗。
 */
export async function labWindowSetEffect(mode: BackdropMode): Promise<boolean> {
  if (isTauriRuntime()) {
    try {
      return await callCommand<boolean>('lab_window_set_effect', { mode })
    } catch {
      return false
    }
  }
  return false
}
