// 浏览器（非 Tauri）环境下扫描目录时允许导入的音频格式。
// 这些是 HTML5 Audio 可原生解码、且浏览器 File API 可直接读取的格式；
// dsf/ape/wma 等无损格式在浏览器环境无法处理，不在此列
// （Tauri 桌面端的格式过滤由 Rust 端负责，与此列表无关）。
export const BROWSER_SCANNABLE_AUDIO_EXTENSIONS = [
  '.mp3', '.flac', '.wav', '.ogg', '.aac', '.m4a'
]

// 提取文件扩展名（含点号、小写）。
// 无扩展名或隐藏文件（如 ".bashrc"）返回空字符串。
export function getFileExtension(fileName: string): string {
  const dotIndex = fileName.lastIndexOf('.')
  // dotIndex <= 0：没有点，或以点开头（隐藏文件，无扩展名）
  if (dotIndex <= 0) return ''
  return fileName.slice(dotIndex).toLowerCase()
}

// 判断文件名是否属于浏览器环境可扫描导入的音频格式
export function isBrowserScannableAudio(fileName: string): boolean {
  return BROWSER_SCANNABLE_AUDIO_EXTENSIONS.includes(getFileExtension(fileName))
}
