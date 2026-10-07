/**
 * 服务层通用类型（前端）
 */

/** 统一返回结构：后端命令一律返回该形状，避免各处 try/catch 语义漂移 */
export interface AppResult<T> {
  ok: boolean
  data: T | null
  error: AppError | null
}

/** 统一错误结构 */
export interface AppError {
  /** 机器可读错误码 */
  code: string
  /** 面向用户的中文提示 */
  message: string
  /** 诊断用细节（不直接展示给用户） */
  detail: string | null
}

/** 在线模块开关（默认关闭，且默认不向外部发送本地库信息） */
export interface OnlineModuleConfig {
  enabled: boolean
  /** 是否允许上报本地曲库特征（默认 false） */
  allowLibraryUpload: boolean
  /** 使用的在线服务标识 */
  provider: 'none' | 'netease'
}
