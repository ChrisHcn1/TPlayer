<script setup lang="ts">
/**
 * 右面板 · 歌词页签
 *
 * - watch 当前 trackId 自动加载歌词；当前行由 lyricsStore.activeLineIndexAt 计算；
 * - 当前行变化时 smooth scroll 居中（§4.10）；点击某行 seek 到该行；
 * - 无词：空态 + 禁用的"在线匹配"（在线匹配 M3 落地）；
 * - 底部偏移校正：±0.5s，经 lyricsStore.setOffset 持久化。
 */
import { computed, nextTick, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'

import { useLyricsStore } from '../../stores/lyrics'
import { usePlayerStore } from '../../stores/player'
import { useLibraryStore } from '../../stores/library'
import { useSettingsStore } from '../../stores/settings'
import Icon from '../common/Icon.vue'

const OFFSET_STEP_MS = 500

const lyricsStore = useLyricsStore()
const playerStore = usePlayerStore()
const libraryStore = useLibraryStore()
const settingsStore = useSettingsStore()
const { snapshot } = storeToRefs(playerStore)
const { lyrics, status } = storeToRefs(lyricsStore)
const { online } = storeToRefs(settingsStore)

/** 在线匹配可用：设置中已开启在线模块（歌词链本地未命中时后端自动在线匹配） */
const onlineEnabled = computed(() => online.value.enabled)

/** 手动触发在线匹配：强制重新走完整歌词链（内嵌 → 边车 → 在线） */
function matchOnline(): void {
  if (!snapshot.value.trackId) return
  void lyricsStore.load(snapshot.value.trackId, true)
}

const lineRefs = ref<HTMLElement[]>([])
const lastActiveIndex = ref(-1)

const track = computed(() => libraryStore.getTrack(snapshot.value.trackId))
const activeIndex = computed(() =>
  lyrics.value?.synced ? lyricsStore.activeLineIndexAt(snapshot.value.positionMs) : -1,
)

watch(
  () => snapshot.value.trackId,
  (trackId) => {
    lastActiveIndex.value = -1
    lineRefs.value = []
    if (trackId) void lyricsStore.load(trackId)
    else lyricsStore.clear()
  },
  { immediate: true },
)

watch(activeIndex, async (index) => {
  if (index === lastActiveIndex.value || index < 0) return
  lastActiveIndex.value = index
  await nextTick()
  lineRefs.value[index]?.scrollIntoView({ behavior: 'smooth', block: 'center' })
})

function seekToLine(timeMs: number): void {
  if (!lyrics.value?.synced) return
  void playerStore.seek(timeMs + lyrics.value.offsetMs)
}

function adjustOffset(deltaMs: number): void {
  if (!lyrics.value) return
  const next = lyrics.value.offsetMs + deltaMs
  void lyricsStore.setOffset(Math.max(-10_000, Math.min(10_000, next)))
}

const offsetText = computed(() => {
  const value = lyrics.value?.offsetMs ?? 0
  if (value === 0) return '0.0s'
  const seconds = value / 1000
  return `${seconds > 0 ? '+' : ''}${seconds.toFixed(1)}s`
})

function setLineRef(el: Element | null, index: number): void {
  if (el) lineRefs.value[index] = el as HTMLElement
}
</script>

<template>
  <div class="lyrics-panel">
    <!-- 曲目信息头 -->
    <div v-if="track" class="lyrics-panel__head">
      <div class="lyrics-panel__cover" aria-hidden="true">
        <Icon name="music-note" :size="20" />
      </div>
      <p class="lyrics-panel__title" :title="track.title">{{ track.title }}</p>
      <p class="lyrics-panel__artist">{{ track.artists.join(' / ') }}</p>
    </div>

    <!-- 加载中 -->
    <div v-if="status === 'loading'" class="lyrics-panel__state">
      <Icon name="loader" :size="22" spin />
      <p>歌词加载中…</p>
    </div>

    <!-- 无歌词 -->
    <div v-else-if="status === 'empty'" class="lyrics-panel__state">
      <Icon name="lyrics" :size="26" />
      <p>暂无歌词</p>
      <button
        v-if="onlineEnabled"
        type="button"
        class="lyrics-panel__retry"
        @click="matchOnline"
      >
        <Icon name="refresh" :size="15" />
        在线匹配
      </button>
      <p v-else class="lyrics-panel__hint">可在「设置 → 在线模块」开启后自动匹配在线歌词</p>
    </div>

    <!-- 加载失败 -->
    <div v-else-if="status === 'error'" class="lyrics-panel__state">
      <Icon name="alert-triangle" :size="26" />
      <p>歌词加载失败</p>
      <button
        v-if="snapshot.trackId"
        type="button"
        class="lyrics-panel__retry"
        @click="lyricsStore.load(snapshot.trackId)"
      >
        <Icon name="refresh" :size="15" />
        重试
      </button>
    </div>

    <!-- 歌词正文 -->
    <div v-else-if="lyrics" class="lyrics-panel__lines" :class="{ 'lyrics-panel__lines--plain': !lyrics.synced }">
      <button
        v-for="(line, index) in lyrics.lines"
        :key="index"
        :ref="(el) => setLineRef(el as Element | null, index)"
        type="button"
        class="lyrics-panel__line"
        :class="{ 'lyrics-panel__line--active': lyrics.synced && index === activeIndex }"
        @click="seekToLine(line.timeMs)"
      >
        {{ line.text }}
      </button>
    </div>

    <!-- 空闲（未播放） -->
    <div v-else class="lyrics-panel__state">
      <Icon name="music-off" :size="26" />
      <p>播放曲目后显示歌词</p>
    </div>

    <!-- 偏移校正 -->
    <div v-if="lyrics" class="lyrics-panel__offset">
      <button type="button" title="歌词提前 0.5 秒" @click="adjustOffset(-OFFSET_STEP_MS)">−0.5s</button>
      <span class="u-tabular" title="歌词偏移">{{ offsetText }}</span>
      <button type="button" title="歌词延后 0.5 秒" @click="adjustOffset(OFFSET_STEP_MS)">+0.5s</button>
    </div>
  </div>
</template>

<style scoped>
.lyrics-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.lyrics-panel__head {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  padding: var(--space-2) var(--space-2) var(--space-4);
}

.lyrics-panel__cover {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--size-cover-sm);
  height: var(--size-cover-sm);
  margin-bottom: var(--space-3);
  color: var(--color-text-tertiary);
  background-color: var(--color-cover-placeholder);
  border-radius: var(--radius-sm);
}

.lyrics-panel__title {
  margin: 0;
  max-width: 100%;
  overflow: hidden;
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.lyrics-panel__artist {
  margin: 2px 0 0;
  max-width: 100%;
  overflow: hidden;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.lyrics-panel__state {
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

.lyrics-panel__lines {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: 40% var(--space-2);
  scrollbar-width: thin;
}

.lyrics-panel__lines--plain {
  gap: var(--space-3);
}

.lyrics-panel__line {
  padding: var(--space-1) var(--space-2);
  font-size: var(--font-size-sm);
  line-height: var(--line-height-sm);
  color: var(--color-text-tertiary);
  background: transparent;
  border: none;
  border-radius: var(--radius-xs);
  cursor: pointer;
  transition: color var(--transition-colors);
}

.lyrics-panel__line:hover {
  color: var(--color-text-secondary);
}

.lyrics-panel__line--active {
  font-size: var(--font-size-md);
  font-weight: var(--font-weight-semibold);
  /* 封面皮肤开启时用歌词色（近白/近墨高亮），与彩色按钮/进度一眼区分 */
  color: var(--color-cover-lyrics, var(--color-brand));
}

.lyrics-panel__online,
.lyrics-panel__retry {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  font-size: var(--font-size-xs);
  color: var(--color-text-secondary);
  background-color: transparent;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-full);
}

.lyrics-panel__retry:hover {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}

.lyrics-panel__online {
  color: var(--color-text-disabled);
  cursor: not-allowed;
}

.lyrics-panel__hint {
  margin: 0;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

.lyrics-panel__offset {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  padding-top: var(--space-3);
  border-top: 1px solid var(--color-border-subtle);
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

.lyrics-panel__offset button {
  padding: var(--space-1) var(--space-2);
  color: var(--color-text-secondary);
  border-radius: var(--radius-xs);
}

.lyrics-panel__offset button:hover {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}
</style>
