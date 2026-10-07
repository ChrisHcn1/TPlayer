// playSong 选择播放引擎时认定需要 FFplay 的格式（HTML5 audio 无法解码）。
// 这是全项目唯一的“格式 -> 是否走 FFplay 引擎”事实来源。
// 注意：.m4a / .aac 可被 HTML5 Audio 原生解码，不在此列。
export const FFPLAY_ENGINE_FORMATS = [
  '.dsf', '.dff', '.dsd', '.mqa', '.wv', '.tta', '.ape', '.wma'
]

// 判断某个文件路径是否需要走 FFplay 引擎（按扩展名，大小写不敏感）
export function needsFFplayEngine(filePath: string): boolean {
  const lower = filePath.toLowerCase()
  return FFPLAY_ENGINE_FORMATS.some(ext => lower.endsWith(ext))
}
