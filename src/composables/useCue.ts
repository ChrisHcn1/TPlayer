import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { CueAlbum, CueTrackSong } from '../types/tauri'

// CUE专辑列表
export const cueAlbums = ref<CueAlbum[]>([])
// CUE Tracks
export const cueTracks = ref<CueTrackSong[]>([])
// 当前选中的CUE专辑
export const selectedCueAlbum = ref<CueAlbum | null>(null)
// 是否显示CUE专辑视图
export const showCueView = ref(false)

// 从title中解析时间信息
export function parseTimeFromTitle(title: string): { title: string; startTime?: string; endTime?: string } {
  const parts = title.split('::')
  if (parts.length >= 3) {
    return {
      title: parts[0],
      startTime: parts[1],
      endTime: parts[2] || undefined
    }
  }
  return { title, startTime: undefined, endTime: undefined }
}

// 获取Track的开始时间（优先从title解析，回退到startTime字段）
export function getTrackStartTime(track: CueTrackSong | any): string | undefined {
  const parsed = parseTimeFromTitle(track.title)
  if (parsed.startTime) {
    return parsed.startTime
  }
  return track.startTime
}

// 获取Track的结束时间（优先从title解析，回退到endTime字段）
export function getTrackEndTime(track: CueTrackSong | any): string | undefined {
  const parsed = parseTimeFromTitle(track.title)
  if (parsed.endTime && parsed.endTime !== '') {
    return parsed.endTime
  }
  return track.endTime
}

// 扫描CUE文件
export async function scanCueFiles(directory: string): Promise<void> {
  try {
    const result = await invoke<{
      albums: CueAlbum[]
      tracks: CueTrackSong[]
      count: number
    }>('scan_cue_files', { directory })
    
    cueAlbums.value = result.albums
    cueTracks.value = result.tracks
    
    console.log(`扫描到 ${result.count} 个CUE Track`)
  } catch (error) {
    console.error('扫描CUE文件失败:', error)
    throw error
  }
}

// 解析单个CUE文件
export async function parseCueFile(cuePath: string): Promise<CueAlbum> {
  try {
    const result = await invoke<CueAlbum>('parse_cue_file_command', { cuePath })
    return result
  } catch (error) {
    console.error('解析CUE文件失败:', error)
    throw error
  }
}

// 选择CUE专辑
export function selectCueAlbum(album: CueAlbum | null): void {
  selectedCueAlbum.value = album
}

// 切换CUE视图
export function toggleCueView(): void {
  showCueView.value = !showCueView.value
}

// 获取CUE专辑的tracks
export function getCueAlbumTracks(albumFilePath: string): CueTrackSong[] {
  return cueTracks.value.filter(track => track.parentFile === albumFilePath)
}

// 将CUE Track添加到播放列表
export function addCueTracksToPlaylist(tracks: CueTrackSong[]): void {
  // 这里需要与主应用的播放列表集成
  // 可以通过事件或全局状态来实现
  console.log('添加CUE Tracks到播放列表:', tracks)
}

// 播放CUE Track
export async function playCueTrack(
  track: CueTrackSong,
  volume?: number
): Promise<void> {
  try {
    const startTime = getTrackStartTime(track)
    const endTime = getTrackEndTime(track)

    await invoke('play_song', {
      path: track.parentFile,
      volume: volume ?? 80,
      forceTranscode: false,
      position: 0,
      startTime: startTime,
      endTime: endTime
    })

    const parsedTitle = parseTimeFromTitle(track.title)
    console.log(`播放CUE Track: ${parsedTitle.title}, 开始时间: ${startTime}, 结束时间: ${endTime}`)
  } catch (error) {
    console.error('播放CUE Track失败:', error)
    throw error
  }
}

// 计算CUE Track的时长（秒）
export function getCueTrackDuration(track: CueTrackSong): number {
  const startTime = getTrackStartTime(track)
  const endTime = getTrackEndTime(track)

  if (startTime && endTime) {
    const start = parseFloat(startTime)
    const end = parseFloat(endTime)
    if (!isNaN(start) && !isNaN(end)) {
      return end - start
    }
  }
  // 如果没有结束时间，返回0
  return 0
}

// 格式化CUE Track时长
export function formatCueDuration(seconds: number): string {
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

// 获取显示用的标题（去掉时间部分）
export function getDisplayTitle(title: string): string {
  const parsed = parseTimeFromTitle(title)
  return parsed.title
}
