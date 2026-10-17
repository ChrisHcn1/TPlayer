<script setup lang="ts">
/**
 * 右面板 · 曲目信息页签
 *
 * - 监听当前 trackId，异步拉取完整元数据（libraryService.trackInfo）；
 * - 基础 / 技术 / 存储 / 内嵌歌词分组展示，所有字段均 Option 化以便优雅降级；
 * - 封面解析失败或无内嵌歌词时仅隐藏对应分区，整页不崩溃。
 */
import { computed, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'

import { usePlayerStore } from '../../stores/player'
import { libraryService } from '../../services/library'
import type { TrackInfo } from '../../types'
import Icon from '../common/Icon.vue'
import defaultCover from '../../assets/default-cover.svg'

const playerStore = usePlayerStore()
const { snapshot } = storeToRefs(playerStore)

const info = ref<TrackInfo | null>(null)
const loading = ref(false)
const errorMsg = ref<string | null>(null)

const trackId = computed(() => snapshot.value.trackId)

watch(
  trackId,
  async (id) => {
    info.value = null
    errorMsg.value = null
    if (!id) return
    loading.value = true
    try {
      info.value = await libraryService.trackInfo(id)
    } catch (cause) {
      errorMsg.value = cause instanceof Error ? cause.message : String(cause)
    } finally {
      loading.value = false
    }
  },
  { immediate: true },
)

// ---- 格式化工具 ----
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
  <div class="track-info">
    <!-- 加载中 -->
    <div v-if="loading" class="track-info__state">
      <Icon name="loader" :size="22" spin />
      <p>曲目信息加载中…</p>
    </div>

    <!-- 无曲目 -->
    <div v-else-if="!trackId" class="track-info__state">
      <Icon name="music-off" :size="26" />
      <p>播放曲目后显示详情</p>
    </div>

    <!-- 加载失败 -->
    <div v-else-if="errorMsg" class="track-info__state">
      <Icon name="alert-triangle" :size="26" />
      <p>曲目信息加载失败</p>
      <p class="track-info__hint">{{ errorMsg }}</p>
    </div>

    <!-- 曲目不存在于曲库 -->
    <div v-else-if="!info" class="track-info__state">
      <Icon name="file-question" :size="26" />
      <p>曲目未找到</p>
    </div>

    <!-- 正常渲染 -->
    <template v-else>
      <!-- 封面 + 基本信息 -->
      <section class="track-info__head">
        <img
          :src="info.cover ?? defaultCover"
          class="track-info__cover"
          :alt="info.title"
          draggable="false"
        />
        <div class="track-info__head-text">
          <h2 class="track-info__title" :title="info.title">{{ info.title }}</h2>
          <p class="track-info__artists">{{ info.artists.join(' / ') }}</p>
          <p class="track-info__album" :title="info.album">
            {{ info.album }}
            <template v-if="info.year"> · {{ info.year }}</template>
          </p>
        </div>
      </section>

      <!-- 技术参数 -->
      <section class="track-info__section">
        <h3 class="track-info__section-title">音频</h3>
        <dl class="track-info__grid">
          <div class="track-info__cell">
            <dt>格式</dt>
            <dd>{{ info.format.toUpperCase() || '—' }}</dd>
          </div>
          <div class="track-info__cell">
            <dt>时长</dt>
            <dd>{{ formatDuration(info.duration) }}</dd>
          </div>
          <div class="track-info__cell">
            <dt>采样率</dt>
            <dd>{{ formatSampleRate(info.sampleRate) }}</dd>
          </div>
          <div class="track-info__cell">
            <dt>比特率</dt>
            <dd>{{ formatBitrate(info.bitrate) }}</dd>
          </div>
          <div class="track-info__cell">
            <dt>位深</dt>
            <dd>{{ safeFormat(info.bitDepth, (n) => `${n} bit`) }}</dd>
          </div>
          <div class="track-info__cell">
            <dt>声道</dt>
            <dd>{{ safeFormat(info.channels, (n) => (n === 1 ? '单声道' : n === 2 ? '立体声' : `${n} 声道`)) }}</dd>
          </div>
        </dl>
      </section>

      <!-- 存储信息 -->
      <section class="track-info__section">
        <h3 class="track-info__section-title">文件</h3>
        <dl class="track-info__grid">
          <div class="track-info__cell track-info__cell--full">
            <dt>路径</dt>
            <dd class="track-info__path" :title="info.filePath">{{ info.filePath }}</dd>
          </div>
          <div class="track-info__cell">
            <dt>大小</dt>
            <dd>{{ formatSize(info.fileSize) }}</dd>
          </div>
          <div class="track-info__cell">
            <dt>音轨</dt>
            <dd>{{ safeFormat(info.trackNumber, (n) => String(n)) }}</dd>
          </div>
          <div v-if="info.replaygainDb !== null || info.replaygainPeak !== null" class="track-info__cell track-info__cell--full">
            <dt>ReplayGain</dt>
            <dd class="track-info__rg">
              <template v-if="info.replaygainDb !== null">增益 {{ info.replaygainDb.toFixed(2) }} dB</template>
              <template v-if="info.replaygainPeak !== null">峰值 {{ info.replaygainPeak.toFixed(6) }}</template>
            </dd>
          </div>
        </dl>
      </section>

      <!-- 内嵌歌词 -->
      <section v-if="info.embeddedLyrics" class="track-info__section">
        <h3 class="track-info__section-title">内嵌歌词</h3>
        <pre class="track-info__lyrics">{{ info.embeddedLyrics }}</pre>
      </section>
    </template>
  </div>
</template>

<style scoped>
.track-info {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow-y: auto;
  gap: var(--space-4);
  padding: var(--space-2) var(--space-2) var(--space-4);
  scrollbar-width: thin;
}

.track-info__state {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  padding: var(--space-6) var(--space-3);
  color: var(--color-text-tertiary);
  font-size: var(--font-size-xs);
  text-align: center;
}

.track-info__hint {
  margin: 0;
  font-size: var(--font-size-2xs, 11px);
  color: var(--color-text-disabled);
  max-width: 100%;
  word-break: break-all;
}

/* ---- 头部 ---- */
.track-info__head {
  display: flex;
  gap: var(--space-3);
  padding: var(--space-2);
}

.track-info__cover {
  width: var(--size-cover-md, 88px);
  height: var(--size-cover-md, 88px);
  flex: 0 0 auto;
  object-fit: cover;
  border-radius: var(--radius-sm);
  background-color: var(--color-cover-placeholder);
  box-shadow: 0 0 0 1px var(--color-border-subtle);
}

.track-info__head-text {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding-top: var(--space-1);
}

.track-info__title {
  margin: 0;
  overflow: hidden;
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track-info__artists {
  margin: 0;
  overflow: hidden;
  font-size: var(--font-size-xs);
  color: var(--color-text-secondary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track-info__album {
  margin: 0;
  overflow: hidden;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ---- Section ---- */
.track-info__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-2);
  border-radius: var(--radius-sm);
  background-color: var(--color-bg-hover, rgba(255, 255, 255, 0.02));
}

.track-info__section-title {
  margin: 0;
  font-size: var(--font-size-2xs, 11px);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-tertiary);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

/* ---- Grid ---- */
.track-info__grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-2) var(--space-4);
  margin: 0;
}

.track-info__cell {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.track-info__cell--full {
  grid-column: 1 / -1;
}

.track-info__cell dt {
  font-size: var(--font-size-2xs, 11px);
  color: var(--color-text-tertiary);
  line-height: 1;
}

.track-info__cell dd {
  margin: 0;
  font-size: var(--font-size-xs);
  color: var(--color-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track-info__path {
  word-break: break-all;
  white-space: normal;
  font-family: Consolas, 'Segoe UI', monospace;
  font-size: 11px;
  color: var(--color-text-secondary);
}

.track-info__rg {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3);
  font-size: var(--font-size-xs);
  color: var(--color-text-secondary);
}

/* ---- 内嵌歌词 ---- */
.track-info__lyrics {
  margin: 0;
  padding: var(--space-2);
  max-height: 240px;
  overflow-y: auto;
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
</style>
