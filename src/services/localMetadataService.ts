import { parseBlob } from 'music-metadata-browser'

export interface LocalMetadata {
  title: string
  artist: string
  album: string
  albumArtist?: string
  genre?: string
  year?: number
  trackNumber?: number
  discNumber?: number
  cover?: Uint8Array
  lyric?: string
  duration?: number
}

class LocalMetadataService {
  /**
   * 从文件读取元数据
   */
  async readMetadata(file: File): Promise<LocalMetadata> {
    try {
      const metadata = await parseBlob(file)
      
      return {
        title: metadata.common.title || file.name.replace(/\.[^.]+$/, ''),
        artist: metadata.common.artist || '',
        album: metadata.common.album || '',
        albumArtist: metadata.common.albumartist,
        // music-metadata 中 genre 为数组，这里合并为分号分隔的字符串
        genre: metadata.common.genre?.join('; ') || undefined,
        year: metadata.common.year,
        // track/disk 在文件缺少音轨/碟片号时整个对象为 undefined，且 no 可能为 null
        trackNumber: metadata.common.track?.no ?? undefined,
        discNumber: metadata.common.disk?.no ?? undefined,
        cover: metadata.common.picture?.[0]?.data,
        lyric: metadata.common.lyrics?.[0],
        duration: metadata.format.duration ? metadata.format.duration * 1000 : undefined
      }
    } catch (error) {
      console.error('读取元数据失败:', error)
      return {
        title: file.name.replace(/\.[^.]+$/, ''),
        artist: '',
        album: '',
        cover: undefined,
        lyric: undefined
      }
    }
  }

  /**
   * 从文件提取封面
   */
  async extractCover(file: File): Promise<string | null> {
    try {
      const metadata = await parseBlob(file)
      const picture = metadata.common.picture?.[0]

      if (!picture) return null

      // music-metadata-browser 返回 Node 风格 Buffer，复制到 ArrayBuffer 支撑的
      // Uint8Array 后才能满足 lib.dom 的 BlobPart 类型并安全构造 Blob
      const bytes = new Uint8Array(picture.data.length)
      bytes.set(picture.data)
      const blob = new Blob([bytes], { type: picture.format })
      return URL.createObjectURL(blob)
    } catch {
      return null
    }
  }

  /**
   * 从文件提取歌词
   */
  async extractLyric(file: File): Promise<string | null> {
    try {
      const metadata = await parseBlob(file)
      return metadata.common.lyrics?.[0] || null
    } catch {
      return null
    }
  }
}

export const localMetadataService = new LocalMetadataService()
