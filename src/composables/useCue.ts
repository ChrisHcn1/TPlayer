import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { CueAlbum, CueTrackSong } from '../types/tauri'

// CUE专辑列表
export const cueAlbums = ref<CueAlbum[]>([])
// CUE Tracks
export const cueTracks = ref<CueTrackSong[]>([])
// 当前选中的CUE专辑
export const selectedCueAlbum = ref<CueAlbum | null>(null)

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

// 选择CUE专辑
export function selectCueAlbum(album: CueAlbum | null): void {
  selectedCueAlbum.value = album
}

// 获取CUE专辑的tracks
export function getCueAlbumTracks(albumFilePath: string): CueTrackSong[] {
  return cueTracks.value.filter(track => track.parentFile === albumFilePath)
}
