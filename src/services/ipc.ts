/**
 * IPC 基础设施（M0 骨架）
 *
 * 约束：
 *  - 所有 Tauri 命令调用必须经本模块；store / 组件不得直接 import `@tauri-apps/api` 的 invoke，
 *    以保证"前端 -> 后端"只有一个出入口（旧版把 IPC 调用散落在 App.vue 各处，难以审计）；
 *  - 统一把「后端返回 Err(AppError)」与「Promise 拒绝」两种形态收敛为前端 AppResult<T>，
 *    避免各处 try/catch 语义漂移。
 */
import { invoke } from '@tauri-apps/api/core'

import type { AppError, AppResult } from '../types'

/** 非 Tauri 运行环境（例如直接用浏览器打开 vite dev 页面）调用命令时抛出 */
export class IpcUnavailableError extends Error {
  constructor() {
    super('当前不在 Tauri 运行环境中，无法调用后端命令')
    this.name = 'IpcUnavailableError'
  }
}

/** 后端命令尚未实现（M0 骨架阶段的预期行为） */
export class NotImplementedError extends Error {
  constructor(action: string) {
    super(`M0 骨架未实现：${action}`)
    this.name = 'NotImplementedError'
  }
}

/** 是否运行在 Tauri WebView 中（M1 起用于浏览器预览模式下的降级判断） */
export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

/** 调用后端命令；失败时抛出，由调用方决定是否降级 */
export async function callCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntime()) {
    throw new IpcUnavailableError()
  }
  return invoke<T>(command, args)
}

/** 调用后端命令并把结果规整为 AppResult<T>（不抛异常，供 UI 层直接消费） */
export async function callCommandSafe<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<AppResult<T>> {
  try {
    const data = await callCommand<T>(command, args)
    return { ok: true, data, error: null }
  } catch (error) {
    return { ok: false, data: null, error: normalizeError(error) }
  }
}

/**
 * 把任意异常规整为 AppError。
 * 后端 `AppError` 已按 { code, message, detail } 序列化（见 src-tauri/src/error.rs），
 * 这里只做形状校验，不猜测语义。
 */
export function normalizeError(error: unknown): AppError {
  if (isAppError(error)) {
    return {
      code: error.code,
      message: error.message,
      detail: error.detail ?? null,
    }
  }

  return {
    code: 'UNKNOWN',
    message: '操作失败',
    detail: error instanceof Error ? error.message : String(error),
  }
}

function isAppError(value: unknown): value is AppError {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Partial<AppError>
  return typeof candidate.code === 'string' && typeof candidate.message === 'string'
}
