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
        genre: metadata.common.genre,
        year: metadata.common.year,
        trackNumber: metadata.common.track.no,
        discNumber: metadata.common.disk.no,
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
      const metadata = await parseBlob(file, { skipAudioTags: true })
      const picture = metadata.common.picture?.[0]
      
      if (!picture) return null

      const blob = new Blob([picture.data], { type: picture.format })
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
      const metadata = await parseBlob(file, { skipAudioTags: true })
      return metadata.common.lyrics?.[0] || null
    } catch {
      return null
    }
  }
}

export const localMetadataService = new LocalMetadataService()
