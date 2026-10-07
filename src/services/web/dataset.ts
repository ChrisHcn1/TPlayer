/**
 * Web 预览数据源 —— 确定性 mock 数据集
 *
 * 仅用于非 Tauri（浏览器）环境：M1 阶段 Rust 侧扫描/持久化尚未打通，
 * 浏览器预览需要一份稳定、可交互的数据。所有 ID 均为确定性常量
 * （t1/t2...、a1...、cue-1...），因此收藏 / 歌单 / 最近播放等
 * localStorage 持久化在刷新后仍然对得上。
 *
 * 约束：本文件不访问文件系统、不发网络请求；filePath 仅为展示用伪路径。
 */
import type { Album, Artist, LibraryStats, Track } from '../../types'

interface AlbumSeed {
  id: string
  name: string
  artist: string
  year: number
  /** CUE 分轨专辑（id 以 cue- 开头，CueAlbums 视图据此与普通专辑区分） */
  cue?: boolean
  titles: string[]
}

/**
 * 专辑种子：曲名经过人工命名，避免“曲目 1/2”式占位数据。
 */
const ALBUM_SEEDS: AlbumSeed[] = [
  {
    id: 'a1',
    name: '夜航',
    artist: '林墨',
    year: 2023,
    titles: ['启程', '港口的风', '夜航', '无名灯塔', '暗流', '星群之间', '靠岸前', '黎明信标'],
  },
  {
    id: 'a2',
    name: '潮汐之间',
    artist: '林墨',
    year: 2024,
    titles: ['涨潮', '咸湿的梦', '潮汐之间', '远雷', '退潮', '海平面的光'],
  },
  {
    id: 'a3',
    name: '城市折叠',
    artist: '苏野',
    year: 2022,
    titles: ['早班地铁', '折叠城市', '玻璃幕墙', '午休十二分钟', '高架的夜', '便利店黄昏', '末班车'],
  },
  {
    id: 'a4',
    name: '缓慢光线',
    artist: '苏野',
    year: 2025,
    titles: ['午后三时', '缓慢光线', '尘埃跳舞', '旧窗帘', '落日之前'],
  },
  {
    id: 'a5',
    name: 'Echoes in Blue',
    artist: 'Aurora Lane',
    year: 2023,
    titles: [
      'Blue Hour',
      'Paper Planes',
      'Echoes in Blue',
      'Winter Harbour',
      'Static Heart',
      'Lantern Road',
      'Slow Tide',
      'Until Morning',
    ],
  },
  {
    id: 'a6',
    name: 'Northern Letters',
    artist: 'Aurora Lane',
    year: 2024,
    titles: ['First Snow', 'Northern Letters', 'Frozen Lake', 'Polar Night', 'Aurora', 'Homeward'],
  },
  {
    id: 'a7',
    name: '山野纪事',
    artist: '陈栖',
    year: 2021,
    titles: ['进山', '溪声', '野莓', '山雾', '柴火与雨', '下山的路'],
  },
  {
    id: 'a8',
    name: '山野纪事·续',
    artist: '陈栖',
    year: 2023,
    titles: ['又见山', '梯田风', '旧友', '夜行货车', '春雪'],
  },
  {
    id: 'a9',
    name: 'Glass Gardens',
    artist: 'Kenji Moreau',
    year: 2024,
    titles: ['Greenhouse', 'Glass Gardens', 'Velvet Leaves', 'Rainwork', 'Bloom Sequence', 'Terrarium'],
  },
  {
    id: 'cue-1',
    name: 'Studio Live 2024（分轨）',
    artist: '群星',
    year: 2024,
    cue: true,
    titles: ['Intro — 调音', '林墨 / 夜航（Live）', '苏野 / 折叠城市（Live）', '陈栖 / 溪声（Live）', 'Aurora Lane / Blue Hour（Live）', '合奏 / 无名灯塔', 'Outro — 谢幕'],
  },
]

const FORMATS = ['flac', 'mp3', 'm4a', 'wav', 'ogg'] as const
const BASE_TIME = 1_700_000_000

function buildTracks(): Track[] {
  const list: Track[] = []
  let seq = 0
  for (const seed of ALBUM_SEEDS) {
    seed.titles.forEach((title, index) => {
      seq += 1
      const duration = 156 + ((seq * 37 + index * 11) % 138) // 156~293 秒，确定性
      const format = FORMATS[seq % FORMATS.length]
      const artists = seed.cue
        ? [cueArtistFor(index)]
        : [seed.artist]
      list.push({
        id: `t${seq}`,
        title,
        artists,
        album: seed.name,
        albumId: seed.id,
        duration,
        trackNumber: index + 1,
        year: seed.year,
        filePath: `X:/Music/${artists[0]}/${seed.name}/${String(index + 1).padStart(2, '0')} - ${title}.${format}`,
        fileSize: duration * 32_000 + seq * 1024,
        format,
        bitrate: format === 'flac' ? 1_050_000 : 320_000,
        sampleRate: format === 'flac' ? 48_000 : 44_100,
        hasCover: seq % 4 !== 0,
        addedAt: BASE_TIME + seq * 86_400,
        modifiedAt: BASE_TIME + seq * 86_400,
      })
    })
  }
  return list
}

function cueArtistFor(index: number): string {
  const names = ['主持人', '林墨', '苏野', '陈栖', 'Aurora Lane', '全体', '主持人']
  return names[index] ?? '群星'
}

/** 全部曲目（模块加载时一次性构建；删除操作在 web/library 层过滤） */
const ALL_TRACKS = buildTracks()

/** CUE 专辑 id 集合（mock 约定：以 cue- 前缀标识） */
export const CUE_ALBUM_IDS = new Set(ALBUM_SEEDS.filter((seed) => seed.cue).map((seed) => seed.id))

export function getAllTracks(): Track[] {
  return ALL_TRACKS
}

export function getTrackById(trackId: string): Track | undefined {
  return ALL_TRACKS.find((track) => track.id === trackId)
}

/** 由曲目集合聚合专辑视图（counts / hasCover 由曲目事实推导，不另行维护） */
export function aggregateAlbums(tracks: Track[]): Album[] {
  const map = new Map<string, Album>()
  for (const track of tracks) {
    if (!track.albumId) continue
    const existing = map.get(track.albumId)
    if (existing) {
      existing.trackCount += 1
      existing.hasCover = existing.hasCover || track.hasCover
      for (const artist of track.artists) {
        if (!existing.artists.includes(artist)) existing.artists.push(artist)
      }
      continue
    }
    map.set(track.albumId, {
      id: track.albumId,
      name: track.album,
      artists: [...track.artists],
      year: track.year,
      trackCount: 1,
      hasCover: track.hasCover,
    })
  }
  // 专辑排序：按种子声明顺序（年份/名字稳定），这里用首曲出现顺序天然保持
  return [...map.values()]
}

export function aggregateArtists(tracks: Track[]): Artist[] {
  const map = new Map<
    string,
    { name: string; albums: Set<string>; tracks: number }
  >()
  for (const track of tracks) {
    for (const name of track.artists) {
      const entry = map.get(name) ?? { name, albums: new Set<string>(), tracks: 0 }
      entry.tracks += 1
      if (track.albumId) entry.albums.add(track.albumId)
      map.set(name, entry)
    }
  }
  return [...map.values()].map((entry) => ({
    id: `artist-${entry.name}`,
    name: entry.name,
    albumCount: entry.albums.size,
    trackCount: entry.tracks,
  }))
}

export function buildStats(tracks: Track[], albums: Album[], artists: Artist[]): LibraryStats {
  return {
    trackCount: tracks.length,
    albumCount: albums.length,
    artistCount: artists.length,
    totalDuration: tracks.reduce((sum, track) => sum + track.duration, 0),
    lastScanAt: BASE_TIME,
  }
}
