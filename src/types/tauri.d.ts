declare global {
  interface Window {
    __TAURI__?: {
      event: {
        listen: <T>(event: string, handler: (event: { payload: T }) => void) => Promise<() => void>
      }
    }
  }
}

// CUE Track类型
export interface CueTrack {
  number: number
  title: string
  performer: string
  startTime: number
  endTime?: number
}

// CUE专辑类型
export interface CueAlbum {
  title: string
  performer: string
  filePath: string
  fileType: string
  tracks: CueTrack[]
}

// CUE Track Song类型（用于播放列表）
export interface CueTrackSong {
  id: string
  title: string
  artist: string
  album: string
  path: string
  duration: string
  startTime: string
  endTime?: string
  trackNumber: string
  isCueTrack: boolean
  parentFile: string
  cueInfo?: string
}

export {}
