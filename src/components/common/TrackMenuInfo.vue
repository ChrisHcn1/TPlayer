<script setup lang="ts">
/**
 * 右键菜单 · 曲目信息头（DESIGN §5.5 内容展示模块）
 *
 * - 挂载时并行拉取完整元数据（libraryService.trackInfo，music-metadata 解析：
 *   格式 / 时长 / 采样率 / 比特率 / 位深 / 声道 / 大小 / 音轨号）与歌词
 *   （lyricsService.load：内嵌 > 同名 .lrc > 在线），菜单即开即用、加载完成后原位填充；
 * - 所有字段 Option 化优雅降级：缺失显示 “—”，封面缺失回落默认图，
 *   无歌词显示占位文案；任一请求失败只降级对应分区，不阻断菜单操作。
 */
import { computed, ref, watch } from 'vue'

import { libraryService } from '../../services/library'
import { lyricsService } from '../../services/lyrics'
import type { Lyrics, TrackInfo } from '../../types'
import Icon from './Icon.vue'
import defaultCover from '../../assets/default-cover.svg'

const props = defineProps<{ trackId: string }>()

const info = ref<TrackInfo | null>(null)
const lyrics = ref<Lyrics | null>(null)
const loading = ref(true)
const failed = ref(false)

async function load(): Promise<void> {
  info.value = null
  lyrics.value = null
  loading.value = true
  failed.value = false
  const [infoResult, lyricsResult] = await Promise.allSettled([
    libraryService.trackInfo(props.trackId),
    lyricsService.load(props.trackId),
  ])
  if (infoResult.status === 'fulfilled') info.value = infoResult.value
  if (lyricsResult.status === 'fulfilled') lyrics.value = lyricsResult.value
  failed.value = infoResult.status === 'rejected'
  loading.value = false
}

watch(() => props.trackId, load, { immediate: true })

/** 歌词纯文本：优先歌词解析链结果（含 .lrc / 在线），回落内嵌歌词 */
const lyricsText = computed<string | null>(() => {
  const lines = lyrics.value?.lines
  if (lines && lines.length > 0) {
    const text = lines.map((line) => line.text).join('\n').trim()
    if (text) return text
  }
  return info.value?.embeddedLyrics ?? null
})

// ---- 格式化工具（缺失一律 “—”） ----
function formatSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '—'
  const units = ['B', 'KB', 'MB', 'GB']
  let i = 0
  let v = bytes
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i += 1
  }
  return `${v.toFixed(v >= 10 || i === 0 ? 0 : 1)} ${units[i]}`
}

function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '—'
  const total = Math.round(seconds)
  const m = Math.floor(total / 60)
  const s = total % 60
  return `${m}:${s.toString().padStart(2, '0')}`
}

function formatBitrate(bps: number | null): string {
  if (bps === null || bps === undefined) return '—'
  return `${Math.round(bps / 1000)} kbps`
}

function formatSampleRate(hz: number | null): string {
  if (hz === null || hz === undefined) return '—'
  if (hz >= 1000) return `${(hz / 1000).toFixed(hz % 1000 === 0 ? 0 : 1)} kHz`
  return `${hz} Hz`
}

function safeFormat<T>(value: T | null | undefined, fn: (v: T) => string, fallback = '—'): string {
  return value === null || value === undefined ? fallback : fn(value)
}
</script>

<template>
  <div class="track-menu-info">
    <!-- 加载中：占位骨架，菜单保持可交互 -->
    <div v-if="loading" class="track-menu-info__state">
      <Icon name="loader" :size="18" spin />
      <span>曲目信息加载中…</span>
    </div>

    <!-- 元数据加载失败：降级提示（动作菜单仍可用） -->
    <div v-else-if="failed || !info" class="track-menu-info__state">
      <Icon name="info" :size="18" />
      <span>曲目信息不可用</span>
    </div>

    <!-- 正常渲染 -->
    <template v-else>
      <!-- 封面 + 标题 / 艺术家 / 专辑 -->
      <div class="track-menu-info__head">
        <img
          :src="info.cover ?? defaultCover"
          class="track-menu-info__cover"
          :alt="info.title"
          draggable="false"
        />
        <div class="track-menu-info__head-text">
          <p class="track-menu-info__title" :title="info.title">{{ info.title }}</p>
          <p class="track-menu-info__artists" :title="info.artists.join(' / ')">
            {{ info.artists.join(' / ') || '—' }}
          </p>
          <p class="track-menu-info__album" :title="info.album">
            {{ info.album || '—' }}<template v-if="info.year"> · {{ info.year }}</template>
          </p>
        </div>
      </div>

      <!-- 完整元数据网格 -->
      <dl class="track-menu-info__grid">
        <div class="track-menu-info__cell">
          <dt>格式</dt>
          <dd>{{ info.format ? info.format.toUpperCase() : '—' }}</dd>
        </div>
        <div class="track-menu-info__cell">
          <dt>时长</dt>
          <dd>{{ formatDuration(info.duration) }}</dd>
        </div>
        <div class="track-menu-info__cell">
          <dt>采样率</dt>
          <dd>{{ formatSampleRate(info.sampleRate) }}</dd>
        </div>
        <div class="track-menu-info__cell">
          <dt>比特率</dt>
          <dd>{{ formatBitrate(info.bitrate) }}</dd>
        </div>
        <div class="track-menu-info__cell">
          <dt>位深</dt>
          <dd>{{ safeFormat(info.bitDepth, (n) => `${n} bit`) }}</dd>
        </div>
        <div class="track-menu-info__cell">
          <dt>声道</dt>
          <dd>{{ safeFormat(info.channels, (n) => (n === 1 ? '单声道' : n === 2 ? '立体声' : `${n} 声道`)) }}</dd>
        </div>
        <div class="track-menu-info__cell">
          <dt>大小</dt>
          <dd>{{ formatSize(info.fileSize) }}</dd>
        </div>
        <div class="track-menu-info__cell">
          <dt>音轨</dt>
          <dd>{{ safeFormat(info.trackNumber, (n) => String(n)) }}</dd>
        </div>
      </dl>

      <!-- 文件位置路径 -->
      <div class="track-menu-info__path" :title="info.filePath">
        <Icon name="folder" :size="13" />
        <span>{{ info.filePath }}</span>
      </div>

      <!-- 歌词（歌词链结果优先，回落内嵌） -->
      <div class="track-menu-info__lyrics">
        <p class="track-menu-info__lyrics-title">
          <Icon name="lyrics" :size="13" />
          <span>歌词</span>
        </p>
        <pre v-if="lyricsText" class="track-menu-info__lyrics-body">{{ lyricsText }}</pre>
        <p v-else class="track-menu-info__lyrics-empty">未找到歌词</p>
      </div>
    </template>
  </div>
</template>

<style scoped>
.track-menu-info {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-2);
  border-radius: var(--radius-sm);
  background-color: var(--color-bg-hover, rgba(255, 255, 255, 0.02));
}

.track-menu-info__state {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  min-height: 56px;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

/* ---- 头部 ---- */
.track-menu-info__head {
  display: flex;
  gap: var(--space-2);
  min-width: 0;
}

.track-menu-info__cover {
  width: 56px;
  height: 56px;
  flex: 0 0 auto;
  object-fit: cover;
  border-radius: var(--radius-xs);
  background-color: var(--color-cover-placeholder);
  box-shadow: 0 0 0 1px var(--color-border-subtle);
}

.track-menu-info__head-text {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 2px;
}

.track-menu-info__title {
  margin: 0;
  overflow: hidden;
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track-menu-info__artists,
.track-menu-info__album {
  margin: 0;
  overflow: hidden;
  font-size: var(--font-size-xs);
  color: var(--color-text-secondary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track-menu-info__album {
  color: var(--color-text-tertiary);
}

/* ---- 元数据网格 ---- */
.track-menu-info__grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-1) var(--space-3);
  margin: 0;
}

.track-menu-info__cell {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-2);
  min-width: 0;
}

.track-menu-info__cell dt {
  flex: 0 0 auto;
  font-size: var(--font-size-2xs, 11px);
  color: var(--color-text-tertiary);
}

.track-menu-info__cell dd {
  flex: 1 1 auto;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  font-size: var(--font-size-xs);
  color: var(--color-text-primary);
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ---- 文件路径 ---- */
.track-menu-info__path {
  display: flex;
  align-items: flex-start;
  gap: var(--space-1);
  font-family: Consolas, 'Segoe UI', monospace;
  font-size: 11px;
  line-height: 1.5;
  color: var(--color-text-secondary);
  word-break: break-all;
}

.track-menu-info__path svg {
  flex: 0 0 auto;
  margin-top: 2px;
}

/* ---- 歌词 ---- */
.track-menu-info__lyrics {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.track-menu-info__lyrics-title {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  margin: 0;
  font-size: var(--font-size-2xs, 11px);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-tertiary);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.track-menu-info__lyrics-body {
  margin: 0;
  max-height: 160px;
  overflow-y: auto;
  padding: var(--space-1) var(--space-2);
  font-family: inherit;
  font-size: var(--font-size-xs);
  line-height: var(--line-height-sm, 1.6);
  white-space: pre-wrap;
  color: var(--color-text-secondary);
  background-color: var(--color-bg-sidebar, transparent);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-xs);
  scrollbar-width: thin;
}

.track-menu-info__lyrics-empty {
  margin: 0;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}
</style>
