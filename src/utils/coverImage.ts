// 封面图片字节流转 base64 data URL 的纯转换函数
// 消除原 App.vue 中重复出现的 bytes→binary→btoa→data URL 块

// 根据图片扩展名返回 MIME 类型
// 非法或未知扩展名回退到 image/jpeg（与原内联实现保持一致）
export function getImageMimeType(ext: string): string {
  const normalized = ext.toLowerCase().replace(/^\./, '')
  switch (normalized) {
    case 'png':
      return 'image/png'
    case 'jpg':
    case 'jpeg':
      return 'image/jpeg'
    case 'bmp':
      return 'image/bmp'
    case 'webp':
      return 'image/webp'
    default:
      return 'image/jpeg'
  }
}

// 字节数组（或 Uint8Array）转 base64 data URL
// 用于把 readFile 读到的封面字节包装成 <img src="data:..."> 可直接使用的形式
export function bytesToBase64DataUrl(
  imageData: Uint8Array | number[],
  ext: string
): string {
  const bytes = new Uint8Array(imageData)
  let binary = ''
  const len = bytes.byteLength
  for (let i = 0; i < len; i++) {
    binary += String.fromCharCode(bytes[i])
  }
  const base64Image = btoa(binary)
  const mimeType = getImageMimeType(ext)
  return `data:${mimeType};base64,${base64Image}`
}
