// 日志开关：设置为 false 可禁用所有日志输出
export const ENABLE_LOGS = true

// 调试日志级别：0=无日志，1=仅错误，2=基本信息，3=详细信息
export const LOG_LEVEL = 2

// 日志函数 - 使用const声明避免作用域问题
export const logInfo = (...args: any[]) => {
  // 输出所有日志
  if (ENABLE_LOGS) {
    console.log(...args)
  }
}

export const logError = (...args: any[]) => {
  // 输出错误日志
  if (ENABLE_LOGS) {
    console.error(...args)
  }
}

// 详细日志函数（仅在LOG_LEVEL=3时输出）
export const logDebug = (...args: any[]) => {
  // 禁用详细日志
  if (ENABLE_LOGS && LOG_LEVEL >= 3) {
    console.log(...args)
  }
}
