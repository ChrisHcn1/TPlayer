# 在线音乐服务实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为 TPlayer 添加在线音乐元数据获取能力（歌词/封面/元数据）

**架构：** 基于前端 TypeScript 服务层实现多音源搜索、歌词获取、封面下载，通过 Tauri FS API 实现本地缓存管理

**技术栈：** TypeScript/Vue 3, music-metadata-browser, Tauri FS API, IndexedDB

---

## 文件结构

### 新增文件

- `src/services/onlineMusicService.ts` - 在线音乐服务核心
- `src/services/cacheService.ts` - 缓存管理服务
- `src-tauri/src/commands_cache.rs` - Rust 缓存命令
- `src/components/Modal/OnlineMatchModal.vue` - 在线匹配对话框
- `src/utils/stringSimilarity.ts` - 字符串相似度算法

### 修改文件

- `src/services/musicDataService.ts` - 集成 onlineMusicService
- `src-tauri/src/main.rs` - 注册缓存管理命令
- `src/components/Modal/EditTagsModal.vue` - 添加在线匹配功能
- `src/components/Player/PlayerLyric/DefaultLyric.vue` - 添加歌词来源显示
- `public/locales/*.json` - 添加新增翻译键

---

## 任务分解

### 任务 1：Rust 缓存管理命令

**文件：**
- 创建：`src-tauri/src/commands_cache.rs`
- 修改：`src-tauri/src/main.rs:77-95`
- 测试：手动测试命令

- [ ] **步骤 1：创建 commands_cache.rs**

```rust
// src-tauri/src/commands_cache.rs
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// 保存文件到缓存目录
#[tauri::command]
pub async fn save_to_cache(
    app_handle: AppHandle,
    category: String,
    filename: String,
    data: Vec<u8>,
) -> Result<String, String> {
    let cache_dir = get_cache_dir_path(&app_handle, &category)?;
    let file_path = cache_dir.join(&filename);
    
    fs::write(&file_path, &data)
        .map_err(|e| format!("写入缓存失败：{}", e))?;
    
    Ok(file_path.to_string_lossy().to_string())
}

/// 从缓存读取文件
#[tauri::command]
pub async fn get_cached_file(
    app_handle: AppHandle,
    category: String,
    filename: String,
) -> Result<Vec<u8>, String> {
    let file_path = get_cache_dir_path(&app_handle, &category)?
        .join(&filename);
    
    fs::read(&file_path)
        .map_err(|e| format!("读取缓存失败：{}", e))
}

/// 获取缓存目录路径
#[tauri::command]
pub async fn get_cache_dir(
    app_handle: AppHandle,
    category: String,
) -> Result<String, String> {
    let cache_dir = get_cache_dir_path(&app_handle, &category)?;
    Ok(cache_dir.to_string_lossy().to_string())
}

/// 清理过期缓存
#[tauri::command]
pub async fn clear_cache(
    app_handle: AppHandle,
    category: String,
    older_than_days: u64,
) -> Result<usize, String> {
    let cache_dir = get_cache_dir_path(&app_handle, &category)?;
    
    if !cache_dir.exists() {
        return Ok(0);
    }
    
    let mut cleaned = 0;
    let now = std::time::SystemTime::now();
    
    if let Ok(entries) = fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age.as_secs() > older_than_days * 86400 {
                            let _ = fs::remove_file(entry.path());
                            cleaned += 1;
                        }
                    }
                }
            }
        }
    }
    
    Ok(cleaned)
}

/// 辅助函数：获取缓存目录路径
fn get_cache_dir_path(app_handle: &AppHandle, category: &str) -> Result<PathBuf, String> {
    let base_dir = app_handle
        .path_resolver()
        .app_data_dir()
        .ok_or("无法获取应用数据目录")?
        .join("cache")
        .join(category);
    
    fs::create_dir_all(&base_dir)
        .map_err(|e| format!("创建缓存目录失败：{}", e))?;
    
    Ok(base_dir)
}
```

- [ ] **步骤 2：在 main.rs 中注册命令**

```rust
// src-tauri/src/main.rs:77
// 在现有的 .invoke_handler() 中添加
mod commands_cache;

.invoke_handler(tauri::generate_handler![
    // ... 现有命令 ...
    commands_cache::save_to_cache,
    commands_cache::get_cached_file,
    commands_cache::get_cache_dir,
    commands_cache::clear_cache,
])
```

- [ ] **步骤 3：编译验证**

运行：`cd src-tauri && cargo check`
预期：无错误

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/commands_cache.rs src-tauri/src/main.rs
git commit -m "feat: 添加缓存管理 Rust 命令"
```

---

### 任务 2：字符串相似度工具

**文件：**
- 创建：`src/utils/stringSimilarity.ts`
- 测试：`src/utils/__tests__/stringSimilarity.test.ts`

- [ ] **步骤 1：创建 stringSimilarity.ts**

```typescript
// src/utils/stringSimilarity.ts

/**
 * 计算两个字符串的 Levenshtein 距离
 */
export function levenshteinDistance(str1: string, str2: string): number {
  const m = str1.length
  const n = str2.length
  const dp: number[][] = Array(m + 1).fill(null).map(() => Array(n + 1).fill(0))

  for (let i = 0; i <= m; i++) dp[i][0] = i
  for (let j = 0; j <= n; j++) dp[0][j] = j

  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      if (str1[i - 1] === str2[j - 1]) {
        dp[i][j] = dp[i - 1][j - 1]
      } else {
        dp[i][j] = Math.min(
          dp[i - 1][j] + 1,     // 删除
          dp[i][j - 1] + 1,     // 插入
          dp[i - 1][j - 1] + 1  // 替换
        )
      }
    }
  }

  return dp[m][n]
}

/**
 * 计算相似度（0-1 之间）
 */
export function similarity(str1: string, str2: string): number {
  const maxLen = Math.max(str1.length, str2.length)
  if (maxLen === 0) return 1
  
  const distance = levenshteinDistance(str1.toLowerCase(), str2.toLowerCase())
  return 1 - distance / maxLen
}

/**
 * 查找最佳匹配
 */
export function findBestMatch<T extends { title: string; artist?: string }>(
  target: { title: string; artist?: string },
  candidates: T[]
): T | null {
  if (candidates.length === 0) return null

  const targetKey = `${target.title} ${target.artist || ''}`.toLowerCase()
  
  let bestMatch: T | null = null
  let bestScore = 0

  for (const candidate of candidates) {
    const candidateKey = `${candidate.title} ${candidate.artist || ''}`.toLowerCase()
    const score = similarity(targetKey, candidateKey)
    
    if (score > bestScore && score >= 0.6) {
      bestScore = score
      bestMatch = candidate
    }
  }

  return bestMatch
}
```

- [ ] **步骤 2：创建测试文件**

```typescript
// src/utils/__tests__/stringSimilarity.test.ts
import { describe, it, expect } from 'vitest'
import { levenshteinDistance, similarity, findBestMatch } from '../stringSimilarity'

describe('stringSimilarity', () => {
  describe('levenshteinDistance', () => {
    it('相同字符串距离为 0', () => {
      expect(levenshteinDistance('hello', 'hello')).toBe(0)
    })

    it('不同字符串计算正确距离', () => {
      expect(levenshteinDistance('kitten', 'sitting')).toBe(3)
    })

    it('空字符串处理', () => {
      expect(levenshteinDistance('', 'abc')).toBe(3)
    })
  })

  describe('similarity', () => {
    it('相同字符串相似度为 1', () => {
      expect(similarity('test', 'test')).toBe(1)
    })

    it('不同字符串相似度在 0-1 之间', () => {
      const sim = similarity('hello', 'hallo')
      expect(sim).toBeGreaterThan(0)
      expect(sim).toBeLessThan(1)
    })

    it('忽略大小写', () => {
      expect(similarity('Test', 'test')).toBe(1)
    })
  })

  describe('findBestMatch', () => {
    it('找到最佳匹配', () => {
      const target = { title: '周杰伦', artist: '稻香' }
      const candidates = [
        { title: '周杰伦', artist: '稻香' },
        { title: '周杰伦', artist: '青花瓷' }
      ]
      
      const match = findBestMatch(target, candidates)
      expect(match).toBe(candidates[0])
    })

    it('相似度低于阈值返回 null', () => {
      const target = { title: '完全不同的歌', artist: '' }
      const candidates = [
        { title: '周杰伦', artist: '稻香' }
      ]
      
      const match = findBestMatch(target, candidates)
      expect(match).toBeNull()
    })
  })
})
```

- [ ] **步骤 3：运行测试**

运行：`npm run test -- src/utils/stringSimilarity.test.ts`
预期：所有测试通过

- [ ] **步骤 4：Commit**

```bash
git add src/utils/stringSimilarity.ts src/utils/__tests__/stringSimilarity.test.ts
git commit -m "feat: 添加字符串相似度算法"
```

---

### 任务 3：缓存服务层

**文件：**
- 创建：`src/services/cacheService.ts`
- 测试：手动测试

- [ ] **步骤 1：创建 cacheService.ts**

```typescript
// src/services/cacheService.ts
import { invoke } from '@tauri-apps/api/core'

export interface CacheMetadata {
  key: string
  category: 'lyrics' | 'covers' | 'metadata'
  source: string
  size: number
  createdAt: number
  lastAccessedAt: number
  accessCount: number
}

export interface CacheIndex {
  version: number
  lastCleanup: number
  entries: Record<string, CacheMetadata>
}

class CacheService {
  private readonly INDEX_KEY = 'cache_index'
  private readonly MAX_COVERS = 100
  private index: CacheIndex | null = null

  constructor() {
    this.loadIndex()
  }

  private async loadIndex() {
    try {
      const cached = localStorage.getItem(this.INDEX_KEY)
      if (cached) {
        this.index = JSON.parse(cached)
      } else {
        this.index = {
          version: 1,
          lastCleanup: Date.now(),
          entries: {}
        }
      }
    } catch {
      this.index = {
        version: 1,
        lastAccessedAt: Date.now(),
        entries: {}
      }
    }
  }

  private saveIndex() {
    if (this.index) {
      localStorage.setItem(this.INDEX_KEY, JSON.stringify(this.index))
    }
  }

  /**
   * 保存歌词到缓存
   */
  async saveLyric(key: string, source: string, data: any): Promise<void> {
    try {
      const dataStr = JSON.stringify(data)
      const encoder = new TextEncoder()
      const bytes = encoder.encode(dataStr)
      
      await invoke<string>('save_to_cache', {
        category: 'lyrics',
        filename: `${key}.json`,
        data: Array.from(bytes)
      })

      this.updateIndex(key, 'lyrics', source, bytes.length)
    } catch (error) {
      console.error('保存歌词缓存失败:', error)
    }
  }

  /**
   * 从缓存读取歌词
   */
  async getLyric(key: string): Promise<any | null> {
    try {
      const bytes = await invoke<number[]>('get_cached_file', {
        category: 'lyrics',
        filename: `${key}.json`
      })

      const decoder = new TextDecoder()
      const dataStr = decoder.decode(new Uint8Array(bytes))
      const data = JSON.parse(dataStr)

      this.updateAccess(key)
      return data
    } catch {
      return null
    }
  }

  /**
   * 保存封面到缓存
   */
  async saveCover(key: string, source: string, data: Uint8Array): Promise<void> {
    try {
      await invoke<string>('save_to_cache', {
        category: 'covers',
        filename: `${key}.jpg`,
        data: Array.from(data)
      })

      this.updateIndex(key, 'covers', source, data.length)
      await this.enforceCoverLimit()
    } catch (error) {
      console.error('保存封面缓存失败:', error)
    }
  }

  /**
   * 从缓存读取封面
   */
  async getCover(key: string): Promise<string | null> {
    try {
      const bytes = await invoke<number[]>('get_cached_file', {
        category: 'covers',
        filename: `${key}.jpg`
      })

      const blob = new Blob([new Uint8Array(bytes)], { type: 'image/jpeg' })
      const url = URL.createObjectURL(blob)

      this.updateAccess(key)
      return url
    } catch {
      return null
    }
  }

  /**
   * 清理过期缓存
   */
  async clearCache(olderThanDays: number = 30): Promise<number> {
    try {
      const cleaned = await invoke<number>('clear_cache', {
        category: 'covers',
        olderThanDays
      })
      
      // 清理索引
      if (this.index) {
        const now = Date.now()
        const threshold = olderThanDays * 86400000
        
        for (const [key, meta] of Object.entries(this.index.entries)) {
          if (now - meta.lastAccessedAt > threshold) {
            delete this.index.entries[key]
          }
        }
        
        this.index.lastCleanup = now
        this.saveIndex()
      }

      return cleaned
    } catch {
      return 0
    }
  }

  private updateIndex(key: string, category: string, source: string, size: number) {
    if (!this.index) return

    const now = Date.now()
    this.index.entries[key] = {
      key,
      category: category as any,
      source,
      size,
      createdAt: now,
      lastAccessedAt: now,
      accessCount: 1
    }

    this.saveIndex()
  }

  private updateAccess(key: string) {
    if (!this.index?.entries[key]) return

    this.index.entries[key].lastAccessedAt = Date.now()
    this.index.entries[key].accessCount++
    this.saveIndex()
  }

  private async enforceCoverLimit() {
    if (!this.index) return

    const coverEntries = Object.entries(this.index.entries)
      .filter(([_, meta]) => meta.category === 'covers')
      .sort((a, b) => a[1].lastAccessedAt - b[1].lastAccessedAt)

    if (coverEntries.length > this.MAX_COVERS) {
      const toDelete = coverEntries.slice(0, coverEntries.length - this.MAX_COVERS)
      
      for (const [key] of toDelete) {
        try {
          await invoke('clear_cache', {
            category: 'covers',
            olderThanDays: 0 // 立即删除
          })
          delete this.index.entries[key]
        } catch {
          // 忽略错误
        }
      }

      this.saveIndex()
    }
  }
}

export const cacheService = new CacheService()
```

- [ ] **步骤 2：Commit**

```bash
git add src/services/cacheService.ts
git commit -m "feat: 添加缓存服务层"
```

---

### 任务 4：在线音乐服务核心

**文件：**
- 创建：`src/services/onlineMusicService.ts`
- 依赖：`src/services/cacheService.ts`, `src/utils/stringSimilarity.ts`

- [ ] **步骤 1：创建 onlineMusicService.ts（第一部分：搜索）**

```typescript
// src/services/onlineMusicService.ts
import { cacheService } from './cacheService'
import { findBestMatch } from '../utils/stringSimilarity'
import { parseLrc, parseSmartLrc, parseQrcLyric, parseTTML, type LyricLine } from './lyricParser'

export interface OnlineSong {
  id: string
  title: string
  artist: string
  album: string
  duration: number
  coverUrl: string
  source: 'qq' | 'netease' | 'amll'
}

export interface LyricResult {
  lrcData: LyricLine[]
  yrcData: LyricLine[]
  source: 'qq' | 'netease' | 'amll'
  hasTranslation: boolean
  hasRomanization: boolean
}

class OnlineMusicService {
  private readonly QQ_MUSIC_API = 'https://api.lxmusic.top/qq'
  private readonly NETEASE_API = 'https://music.163.com/api'
  private readonly AMLL_API = 'https://amll.nyakku.moe/ttml'

  /**
   * 搜索歌曲
   */
  async searchSong(keyword: string): Promise<OnlineSong[]> {
    try {
      const [qqResults, neteaseResults] = await Promise.all([
        this.searchQQMusic(keyword),
        this.searchNetease(keyword)
      ])

      return this.mergeAndDeduplicate([...qqResults, ...neteaseResults])
    } catch (error) {
      console.error('搜索失败:', error)
      return []
    }
  }

  private async searchQQMusic(keyword: string): Promise<OnlineSong[]> {
    try {
      const response = await fetch(`${this.QQ_MUSIC_API}/search/${encodeURIComponent(keyword)}?page=1&limit=20`)
      if (!response.ok) throw new Error('QQ 音乐搜索失败')

      const data = await response.json()
      return (data.data?.list || []).map((item: any) => ({
        id: item.id,
        title: item.title,
        artist: item.singer?.[0]?.name || '未知艺术家',
        album: item.album?.name || '',
        duration: item.interval || 0,
        coverUrl: item.album?.mid ? `https://y.gtimg.cn/music/photo_new/T002R300x300M000${item.album.mid}.jpg` : '',
        source: 'qq' as const
      }))
    } catch {
      return []
    }
  }

  private async searchNetease(keyword: string): Promise<OnlineSong[]> {
    try {
      const response = await fetch(`${this.NETEASE_API}/search/get/web`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Referer': 'https://music.163.com'
        },
        body: JSON.stringify({ s: keyword, type: 1, page: 1, limit: 20 })
      })

      if (!response.ok) throw new Error('网易云搜索失败')

      const data = await response.json()
      return (data.result?.songs || []).map((item: any) => ({
        id: item.id.toString(),
        title: item.name,
        artist: item.artists?.[0]?.name || '未知艺术家',
        album: item.album?.name || '',
        duration: item.duration || 0,
        coverUrl: item.album?.picUrl || '',
        source: 'netease' as const
      }))
    } catch {
      return []
    }
  }

  private mergeAndDeduplicate(results: OnlineSong[]): OnlineSong[] {
    const map = new Map<string, OnlineSong>()
    
    for (const result of results) {
      const key = `${result.title}-${result.artist}`.toLowerCase()
      if (!map.has(key)) {
        map.set(key, result)
      }
    }

    return Array.from(map.values())
  }
```

- [ ] **步骤 2：添加歌词获取方法**

```typescript
// 续 onlineMusicService.ts

  /**
   * 获取歌词
   */
  async getLyric(songId: string, source: 'qq' | 'netease'): Promise<LyricResult> {
    const cacheKey = `${source}_${songId}`
    
    // 检查缓存
    const cached = await cacheService.getLyric(cacheKey)
    if (cached) {
      return cached
    }

    // 请求 API
    const result = source === 'qq'
      ? await this.getQQMusicLyric(songId)
      : await this.getNeteaseLyric(songId)

    // 保存到缓存
    await cacheService.saveLyric(cacheKey, source, result)

    return result
  }

  private async getQQMusicLyric(songId: string): Promise<LyricResult> {
    try {
      const response = await fetch(`${this.QQ_MUSIC_API}/lyric/${songId}`)
      if (!response.ok) throw new Error('QQ 音乐歌词获取失败')

      const data = await response.json()
      
      const result: LyricResult = {
        lrcData: [],
        yrcData: [],
        source: 'qq',
        hasTranslation: false,
        hasRomanization: false
      }

      // 解析 QRC 逐字歌词
      if (data.qrc) {
        result.yrcData = parseQrcLyric(data.qrc)
      }

      // 解析 LRC 歌词
      if (data.lrc) {
        result.lrcData = parseSmartLrc(data.lrc).lines
        
        // 翻译
        if (data.trans) {
          const transLines = parseSmartLrc(data.trans).lines
          result.lrcData = this.alignLyrics(result.lrcData, transLines, 'translatedLyric')
          result.hasTranslation = true
        }

        // 音译
        if (data.roma) {
          const romaLines = parseSmartLrc(data.roma).lines
          result.lrcData = this.alignLyrics(result.lrcData, romaLines, 'romanLyric')
          result.hasRomanization = true
        }
      }

      return result
    } catch (error) {
      console.error('QQ 音乐歌词获取失败:', error)
      return { lrcData: [], yrcData: [], source: 'qq', hasTranslation: false, hasRomanization: false }
    }
  }

  private async getNeteaseLyric(songId: string): Promise<LyricResult> {
    try {
      const response = await fetch(`${this.NETEASE_API}/song/lyric/v1?id=${songId}`, {
        headers: { 'Referer': 'https://music.163.com' }
      })

      if (!response.ok) throw new Error('网易云歌词获取失败')

      const data = await response.json()
      
      const result: LyricResult = {
        lrcData: [],
        yrcData: [],
        source: 'netease',
        hasTranslation: false,
        hasRomanization: false
      }

      // 逐字歌词
      if (data.yrc?.lyric) {
        result.yrcData = parseSmartLrc(data.yrc.lyric).lines
        
        if (data.ytlrc?.lyric) {
          const transLines = parseSmartLrc(data.ytlrc.lyric).lines
          result.yrcData = this.alignLyrics(result.yrcData, transLines, 'translatedLyric')
          result.hasTranslation = true
        }
      }

      // 普通歌词
      if (data.lrc?.lyric && result.yrcData.length === 0) {
        result.lrcData = parseSmartLrc(data.lrc.lyric).lines
        
        if (data.tlyric?.lyric) {
          const transLines = parseSmartLrc(data.tlyric.lyric).lines
          result.lrcData = this.alignLyrics(result.lrcData, transLines, 'translatedLyric')
          result.hasTranslation = true
        }
      }

      return result
    } catch (error) {
      console.error('网易云歌词获取失败:', error)
      return { lrcData: [], yrcData: [], source: 'netease', hasTranslation: false, hasRomanization: false }
    }
  }

  private alignLyrics(
    base: LyricLine[],
    align: LyricLine[],
    field: 'translatedLyric' | 'romanLyric'
  ): LyricLine[] {
    // 简单的对齐逻辑：按索引匹配
    return base.map((line, i) => ({
      ...line,
      [field]: align[i]?.words?.[0]?.word || ''
    }))
  }
```

- [ ] **步骤 3：添加封面获取方法**

```typescript
// 续 onlineMusicService.ts

  /**
   * 获取封面
   */
  async getCover(songId: string, source: 'qq' | 'netease', size: 's' | 'm' | 'l' | 'xl' = 'l'): Promise<string | null> {
    const cacheKey = `${source}_${songId}_${size}`
    
    // 检查缓存
    const cached = await cacheService.getCover(cacheKey)
    if (cached) return cached

    // 获取封面 URL
    const coverUrl = source === 'qq'
      ? await this.getQQMusicCover(songId, size)
      : await this.getNeteaseCover(songId, size)

    if (!coverUrl) return null

    // 下载并保存
    try {
      const response = await fetch(coverUrl)
      const blob = await response.blob()
      const arrayBuffer = await blob.arrayBuffer()
      const uint8Array = new Uint8Array(arrayBuffer)

      await cacheService.saveCover(cacheKey, source, uint8Array)

      return URL.createObjectURL(blob)
    } catch (error) {
      console.error('下载封面失败:', error)
      return coverUrl // 返回原 URL
    }
  }

  private async getQQMusicCover(songId: string, size: 's' | 'm' | 'l' | 'xl'): Promise<string | null> {
    const sizeMap = { s: 150, m: 300, l: 500, xl: 1000 }
    try {
      const response = await fetch(`${this.QQ_MUSIC_API}/cover/${songId}`)
      if (!response.ok) throw new Error('QQ 音乐封面获取失败')

      const data = await response.json()
      const baseUrl = data.coverUrl || ''
      
      // QQ 音乐封面 URL 格式：https://y.gtimg.cn/music/photo_new/T002R{size}x{size}M000{mid}.jpg
      return baseUrl.replace('R300x300', `R${sizeMap[size]}x${sizeMap[size]}`)
    } catch {
      return null
    }
  }

  private async getNeteaseCover(songId: string, size: 's' | 'm' | 'l' | 'xl'): Promise<string | null> {
    const sizeMap = { s: 150, m: 300, l: 500, xl: 1000 }
    try {
      const response = await fetch(`${this.NETEASE_API}/song/detail?ids=${songId}`, {
        headers: { 'Referer': 'https://music.163.com' }
      })

      if (!response.ok) throw new Error('网易云封面获取失败')

      const data = await response.json()
      const song = data.songs?.[0]
      const picUrl = song?.album?.picUrl || ''

      // 网易云封面尺寸参数
      return `${picUrl}?param=${sizeMap[size]}y${sizeMap[size]}`
    } catch {
      return null
    }
  }
```

- [ ] **步骤 4：添加智能匹配方法**

```typescript
// 续 onlineMusicService.ts

  /**
   * 智能匹配本地歌曲
   */
  async matchLocalSong(title: string, artist: string): Promise<OnlineSong | null> {
    const keyword = `${title} ${artist}`.trim()
    const searchResults = await this.searchSong(keyword)
    
    if (searchResults.length === 0) return null

    // 使用相似度算法找到最佳匹配
    const target = { title, artist }
    const bestMatch = findBestMatch(target, searchResults)

    return bestMatch
  }

  /**
   * 获取动态封面
   */
  async getDynamicCover(songId: string): Promise<string | null> {
    try {
      const response = await fetch(`${this.NETEASE_API}/song/dynamic/cover?id=${songId}`, {
        headers: { 'Referer': 'https://music.163.com' }
      })

      if (!response.ok) throw new Error('动态封面获取失败')

      const data = await response.json()
      return data.data?.videoPlayUrl || null
    } catch {
      return null
    }
  }
}

export const onlineMusicService = new OnlineMusicService()
```

- [ ] **步骤 5：Commit**

```bash
git add src/services/onlineMusicService.ts
git commit -m "feat: 实现在线音乐服务核心"
```

---

### 任务 5：本地元数据读取

**文件：**
- 创建：`src/services/localMetadataService.ts`
- 依赖：`music-metadata-browser`

- [ ] **步骤 1：安装依赖**

```bash
npm install music-metadata-browser
```

- [ ] **步骤 2：创建 localMetadataService.ts**

```typescript
// src/services/localMetadataService.ts
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
```

- [ ] **步骤 3：Commit**

```bash
git add src/services/localMetadataService.ts
git commit -m "feat: 添加本地元数据读取服务"
```

---

### 任务 6：在线匹配对话框 UI

**文件：**
- 创建：`src/components/Modal/OnlineMatchModal.vue`
- 修改：`public/locales/zh-CN.json` (添加翻译键)

- [ ] **步骤 1：创建 OnlineMatchModal.vue**

```vue
<template>
  <div class="online-match-modal" @click.self="$emit('close')">
    <div class="modal-content">
      <div class="modal-header">
        <h2>{{ t('modal.onlineMatch') }}</h2>
        <button class="close-btn" @click="$emit('close')">×</button>
      </div>

      <!-- 搜索框 -->
      <div class="search-section">
        <input
          v-model="searchKeyword"
          :placeholder="t('modal.searchPlaceholder')"
          @keyup.enter="handleSearch"
          class="search-input"
        />
        <button @click="handleSearch" :disabled="searching" class="search-btn">
          {{ searching ? t('common.searching') : t('common.search') }}
        </button>
      </div>

      <!-- 搜索结果 -->
      <div v-if="searchResults.length > 0" class="search-results">
        <div
          v-for="result in searchResults"
          :key="result.id"
          :class="['result-item', { selected: selectedResult?.id === result.id }]"
          @click="selectedResult = result"
        >
          <img :src="result.coverUrl" class="result-cover" :alt="result.title" />
          <div class="result-info">
            <div class="result-title">{{ result.title }}</div>
            <div class="result-artist">{{ result.artist }}</div>
            <div class="result-album">{{ result.album }}</div>
          </div>
          <div class="result-source">{{ getSourceText(result.source) }}</div>
        </div>
      </div>

      <!-- 操作按钮 -->
      <div class="action-buttons" v-if="selectedResult">
        <button @click="handleGetLyrics" :disabled="gettingLyric" class="action-btn">
          {{ gettingLyric ? t('modal.getting') : t('modal.getLyrics') }}
        </button>
        <button @click="handleGetCover" :disabled="gettingCover" class="action-btn">
          {{ gettingCover ? t('modal.getting') : t('modal.getCover') }}
        </button>
        <button @click="handleApplyAll" class="action-btn primary">
          {{ t('modal.applyAll') }}
        </button>
      </div>

      <!-- 预览区域 -->
      <div class="preview-section" v-if="lyricPreview || coverPreview">
        <div v-if="lyricPreview" class="lyric-preview">
          <h3>{{ t('modal.lyricPreview') }}</h3>
          <div class="lyric-content">{{ lyricPreview }}</div>
        </div>
        <div v-if="coverPreview" class="cover-preview">
          <h3>{{ t('modal.coverPreview') }}</h3>
          <img :src="coverPreview" :alt="t('modal.cover')" />
        </div>
      </div>

      <!-- 底部按钮 -->
      <div class="modal-footer">
        <button @click="$emit('close')" class="btn secondary">{{ t('common.cancel') }}</button>
        <button @click="handleConfirm" :disabled="!selectedResult" class="btn primary">
          {{ t('common.confirm') }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { onlineMusicService, type OnlineSong, type LyricResult } from '@/services/onlineMusicService'

interface Props {
  currentTitle?: string
  currentArtist?: string
}

const props = withDefaults(defineProps<Props>(), {
  currentTitle: '',
  currentArtist: ''
})

const emit = defineEmits<{
  close: []
  apply: [data: { title?: string; artist?: string; album?: string; lyric?: string; coverUrl?: string }]
}>()

const { t } = useI18n()

const searchKeyword = ref('')
const searching = ref(false)
const searchResults = ref<OnlineSong[]>([])
const selectedResult = ref<OnlineSong | null>(null)

const gettingLyric = ref(false)
const gettingCover = ref(false)
const lyricPreview = ref('')
const coverPreview = ref('')

const lyricData = ref<LyricResult | null>(null)

// 初始化时自动搜索
searchKeyword.value = `${props.currentTitle} ${props.currentArtist}`.trim()
if (searchKeyword.value) {
  handleSearch()
}

async function handleSearch() {
  if (!searchKeyword.value.trim()) return
  
  searching.value = true
  try {
    searchResults.value = await onlineMusicService.searchSong(searchKeyword.value)
  } catch (error) {
    console.error('搜索失败:', error)
    alert(t('modal.searchFailed'))
  } finally {
    searching.value = false
  }
}

function getSourceText(source: string): string {
  const sourceMap: Record<string, string> = {
    qq: 'QQ 音乐',
    netease: '网易云',
    amll: 'Apple Music'
  }
  return sourceMap[source] || source
}

async function handleGetLyrics() {
  if (!selectedResult.value) return
  
  gettingLyric.value = true
  try {
    lyricData.value = await onlineMusicService.getLyric(
      selectedResult.value.id,
      selectedResult.value.source
    )
    
    // 预览第一句歌词
    const firstLine = lyricData.value.lrcData[0]?.words?.[0]?.word || 
                      lyricData.value.yrcData[0]?.words?.[0]?.word || ''
    lyricPreview.value = firstLine ? `${firstLine}...` : t('modal.noLyricFound')
  } catch (error) {
    console.error('获取歌词失败:', error)
    alert(t('modal.getLyricFailed'))
  } finally {
    gettingLyric.value = false
  }
}

async function handleGetCover() {
  if (!selectedResult.value) return
  
  gettingCover.value = true
  try {
    const coverUrl = await onlineMusicService.getCover(
      selectedResult.value.id,
      selectedResult.value.source,
      'l'
    )
    
    if (coverUrl) {
      coverPreview.value = coverUrl
    } else {
      coverPreview.value = ''
      alert(t('modal.noCoverFound'))
    }
  } catch (error) {
    console.error('获取封面失败:', error)
    alert(t('modal.getCoverFailed'))
  } finally {
    gettingCover.value = false
  }
}

async function handleApplyAll() {
  if (!selectedResult.value) return
  
  emit('apply', {
    title: selectedResult.value.title,
    artist: selectedResult.value.artist,
    album: selectedResult.value.album,
    lyric: lyricData.value ? JSON.stringify(lyricData.value) : undefined,
    coverUrl: coverPreview.value || undefined
  })
}

function handleConfirm() {
  handleApplyAll()
  emit('close')
}
</script>

<style scoped>
.online-match-modal {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-content {
  background: var(--bg-primary, #1a1a1a);
  border-radius: 12px;
  padding: 24px;
  width: 90%;
  max-width: 600px;
  max-height: 80vh;
  overflow-y: auto;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.close-btn {
  background: none;
  border: none;
  color: var(--text-primary, #fff);
  font-size: 24px;
  cursor: pointer;
}

.search-section {
  display: flex;
  gap: 10px;
  margin-bottom: 20px;
}

.search-input {
  flex: 1;
  padding: 10px;
  border: 1px solid var(--border-color, rgba(255, 255, 255, 0.2));
  border-radius: 6px;
  background: var(--bg-secondary, #2a2a2a);
  color: var(--text-primary, #fff);
}

.search-btn {
  padding: 10px 20px;
  background: var(--btn-primary, #4CAF50);
  color: #fff;
  border: none;
  border-radius: 6px;
  cursor: pointer;
}

.search-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.search-results {
  max-height: 300px;
  overflow-y: auto;
  margin-bottom: 20px;
}

.result-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.2s;
}

.result-item:hover {
  background: var(--bg-secondary, #2a2a2a);
}

.result-item.selected {
  background: var(--btn-primary, rgba(76, 175, 80, 0.2));
}

.result-cover {
  width: 50px;
  height: 50px;
  border-radius: 4px;
  object-fit: cover;
}

.result-info {
  flex: 1;
}

.result-title {
  font-weight: 600;
  margin-bottom: 4px;
}

.result-artist,
.result-album {
  font-size: 13px;
  color: var(--text-secondary, #999);
}

.result-source {
  font-size: 12px;
  padding: 4px 8px;
  background: var(--bg-secondary, #2a2a2a);
  border-radius: 4px;
}

.action-buttons {
  display: flex;
  gap: 10px;
  margin-bottom: 20px;
}

.action-btn {
  flex: 1;
  padding: 10px;
  background: var(--bg-secondary, #2a2a2a);
  color: var(--text-primary, #fff);
  border: 1px solid var(--border-color, rgba(255, 255, 255, 0.2));
  border-radius: 6px;
  cursor: pointer;
}

.action-btn.primary {
  background: var(--btn-primary, #4CAF50);
  border-color: var(--btn-primary, #4CAF50);
}

.action-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.preview-section {
  margin-bottom: 20px;
}

.lyric-preview,
.cover-preview {
  background: var(--bg-secondary, #2a2a2a);
  padding: 16px;
  border-radius: 8px;
  margin-bottom: 10px;
}

.lyric-content {
  white-space: pre-wrap;
  max-height: 100px;
  overflow-y: auto;
}

.cover-preview img {
  max-width: 200px;
  border-radius: 8px;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}
</style>
```

- [ ] **步骤 2：添加翻译键**

```json
// public/locales/zh-CN.json
{
  "modal": {
    "onlineMatch": "在线匹配",
    "searchPlaceholder": "搜索歌曲...",
    "searching": "搜索中...",
    "getting": "获取中...",
    "getLyrics": "获取歌词",
    "getCover": "获取封面",
    "applyAll": "应用全部",
    "lyricPreview": "歌词预览",
    "coverPreview": "封面预览",
    "noLyricFound": "未找到歌词",
    "noCoverFound": "未找到封面",
    "searchFailed": "搜索失败",
    "getLyricFailed": "获取歌词失败",
    "getCoverFailed": "获取封面失败"
  }
}
```

- [ ] **步骤 3：Commit**

```bash
git add src/components/Modal/OnlineMatchModal.vue public/locales/zh-CN.json
git commit -m "feat: 实现在线匹配对话框 UI"
```

---

### 任务 7：集成到标签编辑对话框

**文件：**
- 修改：`src/components/Modal/EditTagsModal.vue`
- 修改：`src/services/musicDataService.ts`

- [ ] **步骤 1：在 EditTagsModal.vue 中添加在线匹配按钮**

```vue
<!-- src/components/Modal/EditTagsModal.vue -->
<!-- 在适当位置添加 -->

<div class="online-match-section">
  <button @click="showOnlineMatch = true" class="online-match-btn">
    {{ t('modal.onlineMatchTags') }}
  </button>
  
  <!-- 在线匹配对话框 -->
  <OnlineMatchModal
    v-if="showOnlineMatch"
    :current-title="localSong.title"
    :current-artist="localSong.artist"
    @close="showOnlineMatch = false"
    @apply="handleOnlineMatchApply"
  />
</div>

<script setup lang="ts">
import OnlineMatchModal from './OnlineMatchModal.vue'

const showOnlineMatch = ref(false)

function handleOnlineMatchApply(data: {
  title?: string
  artist?: string
  album?: string
  lyric?: string
  coverUrl?: string
}) {
  if (data.title) localSong.value.title = data.title
  if (data.artist) localSong.value.artist = data.artist
  if (data.album) localSong.value.album = data.album
  if (data.lyric) {
    // 解析歌词数据
    try {
      const lyricData = JSON.parse(data.lyric)
      // 保存到本地歌曲对象
      localSong.value.lyric = lyricData
    } catch {
      localSong.value.lyric = data.lyric
    }
  }
  if (data.coverUrl) {
    localSong.value.coverUrl = data.coverUrl
  }
}
</script>
```

- [ ] **步骤 2：Commit**

```bash
git add src/components/Modal/EditTagsModal.vue
git commit -m "feat: 集成在线匹配到标签编辑对话框"
```

---

### 任务 8：歌词来源显示

**文件：**
- 修改：`src/components/Player/PlayerLyric/DefaultLyric.vue`

- [ ] **步骤 1：添加歌词来源显示**

```vue
<!-- DefaultLyric.vue 底部添加 -->

<div class="lyric-source-indicator">
  <span class="source-badge" v-if="lyricSource">
    {{ getSourceText(lyricSource) }}
  </span>
  <button @click="showSourceSelector" class="switch-source-btn" v-if="hasMultipleSources">
    切换源
  </button>
</div>

<script setup lang="ts">
const lyricSource = ref<'qq' | 'netease' | 'amll' | 'local'>('local')

function getSourceText(source: string): string {
  const sourceMap: Record<string, string> = {
    qq: 'QQ 音乐歌词',
    netease: '网易云歌词',
    amll: 'Apple Music TTML',
    local: '本地歌词'
  }
  return sourceMap[source] || source
}

function showSourceSelector() {
  // 实现源选择逻辑
  console.log('切换歌词源')
}
</script>

<style scoped>
.lyric-source-indicator {
  position: absolute;
  bottom: 10px;
  right: 10px;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}

.source-badge {
  padding: 4px 8px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 4px;
  color: var(--text-secondary, #999);
}

.switch-source-btn {
  background: none;
  border: 1px solid var(--border-color, rgba(255, 255, 255, 0.2));
  color: var(--text-primary, #fff);
  padding: 4px 8px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
}
</style>
```

- [ ] **步骤 2：Commit**

```bash
git add src/components/Player/PlayerLyric/DefaultLyric.vue
git commit -m "feat: 添加歌词来源显示"
```

---

### 任务 9：测试与优化

**文件：**
- 测试：手动测试所有功能

- [ ] **步骤 1：测试搜索功能**

1. 打开标签编辑对话框
2. 点击"在线匹配标签"
3. 输入歌曲名和艺术家
4. 验证搜索结果正确显示

- [ ] **步骤 2：测试歌词获取**

1. 选择搜索结果
2. 点击"获取歌词"
3. 验证歌词预览显示
4. 检查歌词格式正确

- [ ] **步骤 3：测试封面获取**

1. 选择搜索结果
2. 点击"获取封面"
3. 验证封面预览显示
4. 检查封面质量

- [ ] **步骤 4：测试应用功能**

1. 获取歌词和封面后
2. 点击"应用全部"
3. 验证数据正确应用到编辑框

- [ ] **步骤 5：测试缓存**

1. 第一次获取歌词/封面
2. 关闭对话框重新打开
3. 再次获取相同歌曲
4. 验证从缓存加载（速度更快）

- [ ] **步骤 6：性能优化**

- 添加搜索防抖（300ms）
- 添加加载状态提示
- 优化大图加载（渐进式）
- 添加错误重试机制

- [ ] **步骤 7：Commit**

```bash
git add .
git commit -m "test: 完成功能测试和优化"
```

---

### 任务 10：文档和清理

**文件：**
- 更新：`README.md`
- 清理：删除临时文件

- [ ] **步骤 1：更新 README**

```markdown
## 功能特性

### 在线音乐服务

- ✅ 支持 QQ 音乐、网易云音乐、Apple Music TTML 多音源搜索
- ✅ 智能歌词匹配（支持 LRC/QRC/TTML/YRC 格式）
- ✅ 高清封面获取（多尺寸可选）
- ✅ 本地缓存管理（避免重复请求）
- ✅ 本地文件元数据读取
```

- [ ] **步骤 2：清理临时文件**

```bash
# 删除开发过程中的临时文件
git clean -fd
```

- [ ] **步骤 3：最终 Commit**

```bash
git add README.md
git commit -m "docs: 更新在线音乐服务文档"
```

---

## 完成检查

- [ ] 所有任务完成
- [ ] 所有测试通过
- [ ] 代码已格式化
- [ ] 文档已更新
- [ ] 无 TypeScript 错误
- [ ] 无 Rust 编译错误
- [ ] 功能手动测试通过

---

**计划完成！** 现在可以选择执行方式：

**1. 子代理驱动（推荐）** - 每个任务调度一个新的子代理，任务间进行审查，快速迭代

**2. 内联执行** - 在当前会话中使用 executing-plans 执行任务，批量执行并设有检查点

**选哪种方式？**
