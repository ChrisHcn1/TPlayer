import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { logInfo, logError, logDebug, ENABLE_LOGS, LOG_LEVEL } from '../logger'

describe('logger', () => {
  let logSpy: ReturnType<typeof vi.spyOn>
  let errorSpy: ReturnType<typeof vi.spyOn>

  beforeEach(() => {
    logSpy = vi.spyOn(console, 'log').mockImplementation(() => {})
    errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
  })

  afterEach(() => {
    logSpy.mockRestore()
    errorSpy.mockRestore()
  })

  it('ENABLE_LOGS 默认开启（true），LOG_LEVEL 默认 2', () => {
    expect(ENABLE_LOGS).toBe(true)
    expect(LOG_LEVEL).toBe(2)
  })

  it('logInfo 在 ENABLE_LOGS=true 时调用 console.log', () => {
    logInfo('hello', 123)
    expect(logSpy).toHaveBeenCalledTimes(1)
    expect(logSpy).toHaveBeenCalledWith('hello', 123)
  })

  it('logError 在 ENABLE_LOGS=true 时调用 console.error', () => {
    logError('err', { code: 1 })
    expect(errorSpy).toHaveBeenCalledTimes(1)
    expect(errorSpy).toHaveBeenCalledWith('err', { code: 1 })
  })

  it('logDebug 在 LOG_LEVEL=2（<3）时不调用 console.log', () => {
    logDebug('debug-info')
    expect(logSpy).not.toHaveBeenCalled()
  })

  it('logInfo 支持任意参数数量透传', () => {
    logInfo('a', 'b', 'c', 1, 2, 3)
    expect(logSpy).toHaveBeenCalledWith('a', 'b', 'c', 1, 2, 3)
  })

  it('logError 支持零参数调用', () => {
    logError()
    expect(errorSpy).toHaveBeenCalledTimes(1)
    expect(errorSpy).toHaveBeenCalledWith()
  })
})
