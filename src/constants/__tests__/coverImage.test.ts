import { describe, it, expect } from 'vitest'
import { COVER_EXTENSIONS, COMMON_COVER_NAMES } from '../coverImage'

describe('COVER_EXTENSIONS', () => {
  it('包含 jpg/jpeg/png/bmp/webp', () => {
    expect(COVER_EXTENSIONS).toContain('jpg')
    expect(COVER_EXTENSIONS).toContain('jpeg')
    expect(COVER_EXTENSIONS).toContain('png')
    expect(COVER_EXTENSIONS).toContain('bmp')
    expect(COVER_EXTENSIONS).toContain('webp')
  })

  it('全小写无点号（与 getImageMimeType 输入约定一致）', () => {
    for (const ext of COVER_EXTENSIONS) {
      expect(ext).toBe(ext.toLowerCase())
      expect(ext.startsWith('.')).toBe(false)
    }
  })

  it('无重复项', () => {
    const set = new Set(COVER_EXTENSIONS)
    expect(set.size).toBe(COVER_EXTENSIONS.length)
  })
})

describe('COMMON_COVER_NAMES', () => {
  it('包含 cover/folder/album/front', () => {
    expect(COMMON_COVER_NAMES).toContain('cover')
    expect(COMMON_COVER_NAMES).toContain('folder')
    expect(COMMON_COVER_NAMES).toContain('album')
    expect(COMMON_COVER_NAMES).toContain('front')
  })

  it('全小写（与文件名约定一致）', () => {
    for (const name of COMMON_COVER_NAMES) {
      expect(name).toBe(name.toLowerCase())
    }
  })
})
