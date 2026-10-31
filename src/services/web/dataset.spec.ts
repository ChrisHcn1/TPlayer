/**
 * 数据初始化一致性测试（种子数据）
 *
 * 验证基于现有音乐数据模型的示例数据满足：
 *  - 艺术家 → 多张专辑
 *  - 专辑 → 多条分轨（曲目），且分轨归属正确专辑与艺人
 *  - CUE 分轨专辑 / SACD 专辑标记正确，分轨字段自洽
 *  - 可为空字段均给出默认值（不出现 undefined）
 *  - 统计与曲目 / 专辑 / 艺人数量自洽
 *
 * 这些断言同时保障「界面展示」（aggregates 三列表非空）与「功能测试」基础数据正确。
 */
import { describe, it, expect } from 'vitest'
import {
  getAllTracks,
  aggregateAlbums,
  aggregateArtists,
  buildStats,
  CUE_ALBUM_IDS,
  SACD_ALBUM_IDS,
} from './dataset'
import type { Track } from '../../types'

const tracks = getAllTracks()
const albums = aggregateAlbums(tracks)
const artists = aggregateArtists(tracks)

describe('种子数据：实体关系一致性', () => {
  it('至少存在一位对应多张专辑的艺术家', () => {
    const multi = artists.filter((artist) => artist.albumCount >= 2)
    expect(multi.length).toBeGreaterThan(0)
  })

  it('每张专辑的曲目都归属该专辑且包含其艺人', () => {
    for (const album of albums) {
      const albumTracks = tracks.filter((track) => track.albumId === album.id)
      expect(albumTracks.length).toBe(album.trackCount)
      for (const track of albumTracks) {
        expect(track.albumId).toBe(album.id)
        expect(track.album).toBe(album.name)
        // 曲目艺人应为专辑聚合艺人的子集（普通专辑为单艺人，CUE 专辑为各分轨艺人并集）
        for (const artist of track.artists) {
          expect(album.artists).toContain(artist)
        }
      }
    }
  })

  it('CUE 分轨专辑的曲目归属正确专辑，且标记为 CUE 分轨', () => {
    expect(CUE_ALBUM_IDS.size).toBeGreaterThan(0)
    for (const id of CUE_ALBUM_IDS) {
      const album = albums.find((a) => a.id === id)
      expect(album).toBeDefined()
      expect(album?.isCue).toBe(true)
      const cueTracks = tracks.filter((track) => track.albumId === id)
      expect(cueTracks.length).toBeGreaterThan(0)
      for (const track of cueTracks) {
        expect(track.albumId).toBe(id)
        expect(track.isCue).toBe(true)
        expect(track.album).toBe(album?.name)
      }
    }
  })

  it('SACD 专辑标记正确，分轨携带 SACD 字段', () => {
    expect(SACD_ALBUM_IDS.size).toBeGreaterThan(0)
    for (const id of SACD_ALBUM_IDS) {
      const album = albums.find((a) => a.id === id)
      expect(album?.isSacd).toBe(true)
      const sacdTracks = tracks.filter((track) => track.albumId === id)
      for (const track of sacdTracks) {
        expect(track.isSacd).toBe(true)
        expect(track.sacdTrackIndex).toBeGreaterThanOrEqual(0)
        expect(track.sacdArea).toBe('Stereo2ch')
        expect(track.sacdDsdRate).toBe(2_822_400)
      }
    }
  })

  it('可为空字段均给出默认值（不出现 undefined）', () => {
    for (const track of tracks) {
      const t = track as Track & Record<string, unknown>
      expect(t.year).not.toBeUndefined()
      expect(t.trackNumber).not.toBeUndefined()
      expect(t.bitrate).not.toBeUndefined()
      expect(t.sampleRate).not.toBeUndefined()
      expect(t.replaygainDb).toBeNull()
      expect(t.replaygainPeak).toBeNull()
      expect(t.metadataPending).toBe(false)
      expect(t.isoPlaceholder).toBe(false)
    }
  })

  it('aggregates 三个列表均非空（供界面展示）', () => {
    const normal = albums.filter(
      (a) => !CUE_ALBUM_IDS.has(a.id) && !SACD_ALBUM_IDS.has(a.id),
    )
    expect(normal.length).toBeGreaterThan(0)
    expect(CUE_ALBUM_IDS.size).toBeGreaterThan(0)
    expect(SACD_ALBUM_IDS.size).toBeGreaterThan(0)
    expect(artists.length).toBeGreaterThan(0)
  })

  it('统计与曲目 / 专辑 / 艺人数量自洽', () => {
    const stats = buildStats(tracks, albums, artists)
    expect(stats.trackCount).toBe(tracks.length)
    expect(stats.albumCount).toBe(albums.length)
    expect(stats.artistCount).toBe(artists.length)
    expect(stats.totalDuration).toBeCloseTo(
      tracks.reduce((sum, track) => sum + track.duration, 0),
    )
  })
})
