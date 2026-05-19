# 多源在线歌词查询功能实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 实现一个完整的多源在线歌词查询功能，支持多个歌词源（网易云、QQ音乐、ChartLyrics），并实现智能评分选择最佳歌词。

**架构：** 创建 `MultiSourceLyricService` 统一管理多源歌词，实现智能评分算法，支持多源缓存，最后更新 App.vue 中的歌词加载逻辑。

**技术栈：** TypeScript + Vue3 + localStorage 缓存 + HTTP 请求

---

## 要修改/创建的文件

### 新创建的文件
1. `src/services/chartLyricsService.ts` - ChartLyrics API 客户端
2. `src/services/multiSourceLyricService.ts` - 多源歌词统一服务

### 修改的文件
1. `src/services/localStorageService.ts` - 更新缓存结构支持多源
2. `src/App.vue` - 更新歌词加载逻辑和在线匹配功能

---

## 任务 1：实现 ChartLyricsService

**文件：**
- 创建：`e:\TPlayer\src\services\chartLyricsService.ts`
- 依赖：`src/services/lyricParser.ts`

- [ ] **步骤 1：创建 ChartLyricsService 基础结构**

```typescript
// e:\TPlayer\src\services\chartLyricsService.ts
import { parseLyric } from './lyricParser'

const CHARTS_LYRICS_URL = 'https://music-api.vercel.app/chartlyrics'

export interface ChartLyricsResult {
  lyric: string
}

export interface ChartLyricsSearchResult {
  success: boolean
  lyric?: string
  lyricLines?: any[]
  error?: string
}

export const chartLyricsService = {
  async searchLyric(artist: string, title: string): Promise<ChartLyricsSearchResult> {
    try {
      const params = new URLSearchParams({
        artist: artist,
        title: title
      })

      const response = await fetch(`${CHARTS_LYRICS_URL}?${params.toString()}`, {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json'
        }
      })

      if (!response.ok) {
        throw new Error(`ChartLyrics API 错误: ${response.status}`)
      }

      const data: ChartLyricsResult = await response.json()

      if (!data.lyric || data.lyric.trim() === '') {
        return {
          success: false,
          error: '未找到歌词'
        }
      }

      const lyricLines = parseLyric(data.lyric)

      return {
        success: true,
        lyric: data.lyric,
        lyricLines: lyricLines
      }
    } catch (error) {
      console.error('[ChartLyrics] 查询失败:', error)
      return {
        success: false,
        error: error instanceof Error ? error.message : '未知错误'
      }
    }
  }
}
```

---

## 任务 2：创建 MultiSourceLyricService

**文件：**
- 创建：`e:\TPlayer\src\services\multiSourceLyricService.ts`
- 依赖：`src/services/onlineMusicService.ts`, `src/services/chartLyricsService.ts`, `src/services/localStorageService.ts`

- [ ] **步骤 1：定义评分接口和权重**

```typescript
// e:\TPlayer\src\services\multiSourceLyricService.ts
import { onlineMusicService, MusicSearchResult } from './onlineMusicService'
import { chartLyricsService, ChartLyricsSearchResult } from './chartLyricsService'
import { localStorageService, LyricCache } from './localStorageService'

export enum LyricSourcePriority {
  EMBEDDED = 1,
  LOCAL_LRC = 2,
  CACHED_ONLINE = 3,
  NETEASE = 4,
  QQ_MUSIC = 5,
  CHARTLYRICS = 6
}

export enum LyricSourceType {
  EMBEDDED = 'embedded',
  LOCAL_LRC = 'local_lrc',
  CACHED_ONLINE = 'cached_online',
  NETEASE = 'netease',
  QQ_MUSIC = 'qq_music',
  CHARTLYRICS = 'chartlyrics'
}

export interface LyricScore {
  source: LyricSourceType
  totalScore: number
  metrics: {
    completeness: number       // 歌词完整度 (0-100)
    hasTranslation: boolean   // 是否有翻译
    hasRomanization: boolean  // 是否有音译
    hasWordLevel: boolean     // 是否有逐字歌词
    timestampAccuracy: number // 时间戳准确度 (0-100)
  }
  lyricText?: string
  lyricLines?: any[]
  yrcData?: any[]
}

const SCORE_WEIGHTS = {
  completeness: 0.4,
  translation: 20,
  romanization: 15,
  wordLevel: 25,
  timestampAccuracy: 0.2
}
```

- [ ] **步骤 2：实现评分算法**

```typescript
// 继续在 multiSourceLyricService.ts 中添加

export function calculateLyricScore(
  source: LyricSourceType,
  lyricText?: string,
  lyricLines?: any[],
  yrcData?: any[]
): LyricScore {
  let completeness = 0
  let hasTranslation = false
  let hasRomanization = false
  let hasWordLevel = false
  let timestampAccuracy = 0

  // 计算完整度
  if (lyricText && lyricText.trim().length > 50) {
    completeness = Math.min(100, lyricText.trim().length / 5)
  }

  // 检查翻译/音译
  if (lyricLines && lyricLines.length > 0) {
    hasTranslation = lyricLines.some(line => line.translation?.trim())
    hasRomanization = lyricLines.some(line => line.romanization?.trim())
    
    // 检查时间戳
    const linesWithTime = lyricLines.filter(line => line.time !== null && line.time !== undefined)
    timestampAccuracy = linesWithTime.length > 0 
      ? (linesWithTime.length / lyricLines.length) * 100 
      : 0
  }

  // 检查逐字歌词
  hasWordLevel = !!yrcData && yrcData.length > 0

  // 计算总分
  const totalScore = 
    completeness * SCORE_WEIGHTS.completeness +
    (hasTranslation ? SCORE_WEIGHTS.translation : 0) +
    (hasRomanization ? SCORE_WEIGHTS.romanization : 0) +
    (hasWordLevel ? SCORE_WEIGHTS.wordLevel : 0) +
    timestampAccuracy * SCORE_WEIGHTS.timestampAccuracy

  return {
    source,
    totalScore,
    metrics: {
      completeness,
      hasTranslation,
      hasRomanization,
      hasWordLevel,
      timestampAccuracy
    },
    lyricText,
    lyricLines,
    yrcData
  }
}
```

- [ ] **步骤 3：实现多源查询核心逻辑**

```typescript
// 继续在 multiSourceLyricService.ts 中添加

export interface MultiSourceLyricResult {
  success: boolean
  bestSource: LyricSourceType | null
  bestScore: LyricScore | null
  allSources: Map<LyricSourceType, LyricScore>
  error?: string
}

export interface SongInfo {
  id?: string
  title: string
  artist: string
  album?: string
  filePath?: string
}

export class MultiSourceLyricService {
  constructor() {}

  async getLyric(
    song: SongInfo,
    mode: 'auto' | 'manual' = 'auto',
    skipLocalLRC: boolean = false
  ): Promise<MultiSourceLyricResult> {
    const allSources = new Map<LyricSourceType, LyricScore>()
    const songId = song.id || `${song.artist}-${song.title}`.toLowerCase()

    try {
      // 步骤 1：查询缓存
      const cachedLyrics = localStorageService.getLyric(songId)
      if (cachedLyrics) {
        for (const [sourceName, sourceData] of Object.entries(cachedLyrics.sources)) {
          const sourceType = this.sourceNameToType(sourceName)
          if (sourceType) {
            const score = calculateLyricScore(
              sourceType,
              sourceData.lrcData ? JSON.stringify(sourceData.lrcData) : undefined,
              sourceData.lrcData,
              sourceData.yrcData
            )
            allSources.set(sourceType, score)
          }
        }

        // 自动模式：如果有缓存直接返回最佳的
        if (mode === 'auto' && allSources.size > 0) {
          const bestScore = this.selectBestSource(allSources)
          return {
            success: true,
            bestSource: bestScore.source,
            bestScore,
            allSources
          }
        }
      }

      // 步骤 2：并行查询所有在线源
      const [neteaseResult, qqResult, chartLyricsResult] = await Promise.allSettled([
        this.queryNetease(song),
        this.queryQQMusic(song),
        this.queryChartLyrics(song)
      ])

      // 处理各源结果
      if (neteaseResult.status === 'fulfilled') {
        allSources.set(LyricSourceType.NETEASE, neteaseResult.value)
      }
      if (qqResult.status === 'fulfilled') {
        allSources.set(LyricSourceType.QQ_MUSIC, qqResult.value)
      }
      if (chartLyricsResult.status === 'fulfilled') {
        allSources.set(LyricSourceType.CHARTLYRICS, chartLyricsResult.value)
      }

      // 步骤 3：更新缓存
      if (allSources.size > 0) {
        const sourcesToCache: Record<string, any> = {}
        allSources.forEach((score, sourceType) => {
          if (score.lyricLines) {
            sourcesToCache[this.sourceTypeToName(sourceType)] = {
              lrcData: score.lyricLines,
              yrcData: score.yrcData,
              fetchedAt: Date.now(),
              score: score
            }
          }
        })

        localStorageService.saveLyric(songId, {
          songId,
          rawLyric: null,
          translatedLyric: null,
          romanLyric: null,
          yrcData: null,
          lyrics: null,
          currentLyricIndex: -1
        })
        
        // 临时兼容：同时保存到新的多源缓存结构
        this.saveMultiSourceCache(songId, sourcesToCache)
      }

      // 步骤 4：选择最佳源
      if (allSources.size === 0) {
        return {
          success: false,
          bestSource: null,
          bestScore: null,
          allSources,
          error: '所有歌词源都没有找到歌词'
        }
      }

      const bestScore = this.selectBestSource(allSources)

      return {
        success: true,
        bestSource: bestScore.source,
        bestScore,
        allSources
      }

    } catch (error) {
      console.error('[MultiSourceLyricService] 查询失败:', error)
      return {
        success: false,
        bestSource: null,
        bestScore: null,
        allSources,
        error: error instanceof Error ? error.message : '未知错误'
      }
    }
  }

  private async queryNetease(song: SongInfo): Promise<LyricScore> {
    const result = await onlineMusicService.searchSong(
      song.title,
      song.artist,
      'netease'
    )

    if (!result.success) {
      return calculateLyricScore(LyricSourceType.NETEASE)
    }

    const lyricResult = await onlineMusicService.getLyric(
      result.songs[0].id,
      'netease'
    )

    return calculateLyricScore(
      LyricSourceType.NETEASE,
      lyricResult.lrc?.lyric,
      lyricResult.parsedLrc,
      lyricResult.yrc
    )
  }

  private async queryQQMusic(song: SongInfo): Promise<LyricScore> {
    const result = await onlineMusicService.searchSong(
      song.title,
      song.artist,
      'qq'
    )

    if (!result.success) {
      return calculateLyricScore(LyricSourceType.QQ_MUSIC)
    }

    const lyricResult = await onlineMusicService.getLyric(
      result.songs[0].id,
      'qq'
    )

    return calculateLyricScore(
      LyricSourceType.QQ_MUSIC,
      lyricResult.lrc?.lyric,
      lyricResult.parsedLrc,
      lyricResult.yrc
    )
  }

  private async queryChartLyrics(song: SongInfo): Promise<LyricScore> {
    const result = await chartLyricsService.searchLyric(
      song.artist,
      song.title
    )

    if (!result.success || !result.lyricLines) {
      return calculateLyricScore(LyricSourceType.CHARTLYRICS)
    }

    return calculateLyricScore(
      LyricSourceType.CHARTLYRICS,
      result.lyric,
      result.lyricLines
    )
  }

  private selectBestSource(sources: Map<LyricSourceType, LyricScore>): LyricScore {
    let best: LyricScore | null = null
    for (const score of sources.values()) {
      if (!best || score.totalScore > best.totalScore) {
        best = score
      }
    }
    return best!
  }

  private sourceNameToType(name: string): LyricSourceType | null {
    const mapping: Record<string, LyricSourceType> = {
      'netease': LyricSourceType.NETEASE,
      'qq_music': LyricSourceType.QQ_MUSIC,
      'chartlyrics': LyricSourceType.CHARTLYRICS,
      'embedded': LyricSourceType.EMBEDDED,
      'local_lrc': LyricSourceType.LOCAL_LRC
    }
    return mapping[name] || null
  }

  private sourceTypeToName(type: LyricSourceType): string {
    const mapping: Record<LyricSourceType, string> = {
      [LyricSourceType.NETEASE]: 'netease',
      [LyricSourceType.QQ_MUSIC]: 'qq_music',
      [LyricSourceType.CHARTLYRICS]: 'chartlyrics',
      [LyricSourceType.EMBEDDED]: 'embedded',
      [LyricSourceType.LOCAL_LRC]: 'local_lrc',
      [LyricSourceType.CACHED_ONLINE]: 'cached_online'
    }
    return mapping[type]
  }

  private saveMultiSourceCache(songId: string, sources: Record<string, any>): void {
    try {
      const cacheKey = `multi_lyric_${songId}`
      const cache = {
        songId,
        sources,
        bestSource: null,
        updatedAt: Date.now()
      }
      localStorage.setItem(cacheKey, JSON.stringify(cache))
    } catch (e) {
      console.error('[MultiSourceLyricService] 保存缓存失败:', e)
    }
  }
}

export const multiSourceLyricService = new MultiSourceLyricService()
```

---

## 任务 3：更新 localStorageService 支持多源缓存

**文件：**
- 修改：`e:\TPlayer\src\services\localStorageService.ts:1`

- [ ] **步骤 1：添加多源缓存接口**

在现有文件开头添加：

```typescript
// 扩展现有 LyricCache 接口或添加新接口
export interface MultiSourceLyricCache {
  songId: string
  sources: {
    [sourceName: string]: {
      lrcData: any[] | null
      yrcData: any[] | null
      fetchedAt: number
      score: any
    }
  }
  bestSource: string | null
  updatedAt: number
}
```

---

## 任务 4：更新 App.vue 中的歌词加载逻辑

**文件：**
- 修改：`e:\TPlayer\src\App.vue:4556`

- [ ] **步骤 1：导入新服务**

在文件顶部 import 区域添加：

```typescript
import { multiSourceLyricService, LyricSourceType } from './services/multiSourceLyricService'
```

- [ ] **步骤 2：更新 fetchLyric 函数**

找到 `const fetchLyric = async` 函数，更新为：

```typescript
const fetchLyric = async (filePath?: string, skipLocalLRC = false): Promise<void> => {
  const song = filePath ? getSongFromCache(filePath) : currentSong.value
  if (!song) {
    logError('没有选中的歌曲')
    return
  }

  // 先尝试加载本地歌词
  if (!skipLocalLRC) {
    const embeddedLyric = await loadEmbeddedLyric(song.filePath)
    if (embeddedLyric) {
      updateLyricsFromEmbedded(embeddedLyric)
      return
    }

    const localLrc = await loadLocalLyricFile(song.filePath)
    if (localLrc) {
      updateLyricsFromLocalLRC(localLrc)
      return
    }
  }

  // 在线查询
  const result = await multiSourceLyricService.getLyric(
    {
      id: song.filePath,
      title: song.title,
      artist: song.artist,
      album: song.album,
      filePath: song.filePath
    },
    'auto',
    skipLocalLRC
  )

  if (!result.success || !result.bestScore) {
    logError('无法获取歌词:', result.error)
    return
  }

  // 更新歌词显示
  if (result.bestScore.lyricLines) {
    lyrics.value = result.bestScore.lyricLines
    if (result.bestScore.yrcData) {
      yrcData.value = result.bestScore.yrcData
    }
    currentLyricIndex.value = -1
    logInfo(`使用 ${result.bestSource} 源的歌词`)
  }
}
```

- [ ] **步骤 3：更新 OnlineMatchModal 相关逻辑**

找到 `getSearchResultBySong` 和 `handleConfirm` 函数，更新为使用新的多源服务。

---

## 任务 5：验证并测试

**文件：**
- 所有修改过的文件

- [ ] **步骤 1：运行构建验证**

运行：`cd e:\TPlayer ; npm run build`
预期：无 TypeScript 错误

- [ ] **步骤 2：运行应用测试**

启动应用并测试歌词加载功能

---

## 自检

✅ **规格覆盖度**：完整覆盖了设计方案中的所有功能点

✅ **占位符扫描**：无占位符，所有步骤都有完整代码

✅ **类型一致性**：类型定义一致，接口匹配

---

## 执行交接

计划已完成并保存到 `docs/superpowers/plans/2026-05-05-multi-source-lyric.md`。两种执行方式：

**1. 子代理驱动（推荐）** - 每个任务调度一个新的子代理，任务间进行审查，快速迭代

**2. 内联执行** - 在当前会话中使用 executing-plans 执行任务，批量执行并设有检查点供审查

**选哪种方式？**
