import { describe, it, expect } from 'vitest'
import { getImageMimeType, bytesToBase64DataUrl } from '../coverImage'

describe('getImageMimeType', () => {
  it('png 扩展名返回 image/png', () => {
    expect(getImageMimeType('png')).toBe('image/png')
  })

  it('jpg 与 jpeg 都返回 image/jpeg', () => {
    expect(getImageMimeType('jpg')).toBe('image/jpeg')
    expect(getImageMimeType('jpeg')).toBe('image/jpeg')
  })

  it('bmp 返回 image/bmp', () => {
    expect(getImageMimeType('bmp')).toBe('image/bmp')
  })

  it('webp 返回 image/webp', () => {
    expect(getImageMimeType('webp')).toBe('image/webp')
  })

  it('未知扩展名回退到 image/jpeg', () => {
    expect(getImageMimeType('gif')).toBe('image/jpeg')
    expect(getImageMimeType('')).toBe('image/jpeg')
  })

  it('大小写不敏感（带点号也能归一化）', () => {
    expect(getImageMimeType('PNG')).toBe('image/png')
    expect(getImageMimeType('.Jpg')).toBe('image/jpeg')
  })
})

describe('bytesToBase64DataUrl', () => {
  it('字节数组转 jpeg data URL', () => {
    // [0x42, 0x42] -> "BB" -> base64 "QkI="
    expect(bytesToBase64DataUrl([0x42, 0x42], 'jpg')).toBe('data:image/jpeg;base64,QkI=')
  })

  it('png 扩展名生成 png data URL', () => {
    expect(bytesToBase64DataUrl([0x42, 0x42], 'png')).toBe('data:image/png;base64,QkI=')
  })

  it('Uint8Array 与 number[] 输入等价', () => {
    const asArray = bytesToBase64DataUrl([1, 2, 3], 'webp')
    const asUint8 = bytesToBase64DataUrl(new Uint8Array([1, 2, 3]), 'webp')
    expect(asArray).toBe(asUint8)
  })

  it('空字节数组生成空 base64', () => {
    expect(bytesToBase64DataUrl([], 'bmp')).toBe('data:image/bmp;base64,')
  })
})
