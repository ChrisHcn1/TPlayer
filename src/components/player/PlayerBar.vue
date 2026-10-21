<script setup lang="ts">
/**
 * 底部播放条（DESIGN §4.9）
 *
 * 左：封面 + 标题/艺术家（可跳详情）+ 收藏；
 * 中：播放模式 / 上一首 / 播放暂停 / 下一首 + 进度条；
 * 右：歌词 / 队列面板开关 + 音量。
 * 状态只读消费 player snapshot，动作经 player store，不直接碰 services。
 */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { storeToRefs } from 'pinia'
import { useRouter } from 'vue-router'

import { usePlayerStore } from '../../stores/player'
import { useLibraryStore } from '../../stores/library'
import { useLayout } from '../../composables/useLayout'
import { useTrackCover } from '../../composables/useTrackCover'
import { useContextMenu, type MenuItem } from '../../composables/useContextMenu'
import type { PlayMode } from '../../types'
import ProgressBar from './ProgressBar.vue'
import VolumeControl from './VolumeControl.vue'
import Icon from '../common/Icon.vue'
import defaultCover from '../../assets/default-cover.svg'

const MODE_META: Record<PlayMode, { icon: string; label: string }> = {
  order: { icon: 'list-order', label: '顺序播放' },
  'repeat-all': { icon: 'repeat', label: '列表循环' },
  'repeat-one': { icon: 'repeat-one', label: '单曲循环' },
  shuffle: { icon: 'shuffle', label: '随机播放' },
}

const MODE_ORDER: PlayMode[] = ['order', 'repeat-all', 'repeat-one', 'shuffle']

const router = useRouter()
const playerStore = usePlayerStore()
const libraryStore = useLibraryStore()
const {
  panelTab,
  isPanelOpen,
  isDesktopLyricsVisible,
  togglePanelTab,
  toggleDesktopLyrics,
  openNowPlaying,
} = useLayout()
const { openAt } = useContextMenu()
const { snapshot, sleepTimer } = storeToRefs(playerStore)
const { coverUrl } = useTrackCover(() => snapshot.value.trackId)

const track = computed(() => libraryStore.getTrack(snapshot.value.trackId))
const hasTrack = computed(() => Boolean(track.value))
const isPlaying = computed(() => snapshot.value.status === 'playing')
const isLoading = computed(() => snapshot.value.status === 'loading')
const isFavorite = computed(() =>
  snapshot.value.trackId ? libraryStore.isFavorite(snapshot.value.trackId) : false,
)

const modeMeta = computed(() => MODE_META[snapshot.value.mode] ?? MODE_META.order)

/** 睡眠定时器预设（分钟） */
const SLEEP_PRESETS = [15, 30, 45, 60, 90]

/** 倍速预设（有效区间 0.25 ~ 4.0） */
const RATE_PRESETS = [0.5, 0.75, 1, 1.25, 1.5, 2]

/** 每秒刷新的当前时刻：驱动剩余时长倒计时显示 */
const nowMs = ref(Date.now())
let nowTimer: ReturnType<typeof setInterval> | null = null
onMounted(() => {
  nowTimer = setInterval(() => {
    nowMs.value = Date.now()
  }, 1000)
})
onBeforeUnmount(() => {
  if (nowTimer !== null) clearInterval(nowTimer)
})

const sleepRemainingMs = computed(() => {
  if (!sleepTimer.value.active || sleepTimer.value.endsAtMs === null) return null
  return Math.max(0, sleepTimer.value.endsAtMs - nowMs.value)
})

const sleepLabel = computed(() => {
  if (!sleepTimer.value.active) return '睡眠定时器'
  if (sleepTimer.value.waitTrackEnd) return '播完当前曲目后停止'
  const remain = sleepRemainingMs.value ?? 0
  const minutes = Math.floor(remain / 60000)
  const seconds = Math.floor((remain % 60000) / 1000)
  return minutes > 0
    ? `剩余 ${minutes}:${String(seconds).padStart(2, '0')} 后暂停`
    : `剩余 ${seconds} 秒后暂停`
})

function openSleepMenu(event: MouseEvent): void {
  const items: MenuItem[] = [
    ...SLEEP_PRESETS.map((minutes) => ({
      key: `sleep-${minutes}`,
      label: `${minutes} 分钟后暂停`,
      onClick: () => void playerStore.setSleepTimer(minutes * 60_000, false),
    })),
    {
      key: 'sleep-track-end',
      label: '播完当前曲目后停止',
      onClick: () => void playerStore.setSleepTimer(0, true),
    },
  ]
  if (sleepTimer.value.active) {
    items.push({
      key: 'sleep-cancel',
      label: '取消定时',
      onClick: () => void playerStore.cancelSleepTimer(),
    })
  }
  openAt(event.clientX, event.clientY, items, '睡眠定时器')
}

/** 格式化倍速：1 → "1.0x"，0.75 → "0.75x"，2 → "2.0x" */
function formatRate(r: number): string {
  return `${r.toFixed(r === Math.round(r) ? 1 : 2).replace(/\.?0+$/, '')}x`
}

function openRateMenu(event: MouseEvent): void {
  const items: MenuItem[] = RATE_PRESETS.map((rate) => ({
    key: `rate-${rate}`,
    label: formatRate(rate),
    icon: Math.abs(snapshot.value.rate - rate) < 0.001 ? 'check' : undefined,
    onClick: () => void playerStore.setRate(rate),
  }))
  openAt(event.clientX, event.clientY, items, '播放速度')
}

function onSeek(positionMs: number): void {
  void playerStore.seek(positionMs)
}

function onToggle(): void {
  if (!hasTrack.value) return
  void playerStore.toggle()
}

async function toggleFavorite(): Promise<void> {
  if (track.value) await libraryStore.toggleFavorite(track.value.id)
}

function openModeMenu(event: MouseEvent): void {
  const items: MenuItem[] = MODE_ORDER.map((mode) => ({
    key: mode,
    label: MODE_META[mode].label,
    icon: snapshot.value.mode === mode ? 'check' : MODE_META[mode].icon,
    onClick: () => void playerStore.setMode(mode),
  }))
  openAt(event.clientX, event.clientY, items, '播放模式')
}

function openAlbum(): void {
  if (!track.value?.albumId) return
  const id = track.value.albumId
  if (libraryStore.cueAlbums.some((album) => album.id === id)) router.push(`/cue/${id}`)
  else if (libraryStore.sacdAlbums.some((album) => album.id === id)) router.push(`/cue/${id}`)
  else router.push(`/albums/${id}`)
}

function openArtist(): void {
  const artist = track.value?.artists[0]
  if (artist) router.push(`/artists/${encodeURIComponent(artist)}`)
}
</script>

<template>
  <div class="player-bar">
    <!-- 左：当前曲目 -->
    <div class="now-playing">
      <button
        type="button"
        class="now-playing__cover"
        title="放大封面 / 歌词"
        aria-label="放大封面并查看歌词"
        :disabled="!hasTrack"
        @click="openNowPlaying"
      >
        <!-- 有曲目：真实封面（内嵌/边车/在线），整条链未命中时回落内置默认封面 -->
        <img
          v-if="hasTrack"
          class="now-playing__cover-img"
          :src="coverUrl ?? defaultCover"
          alt=""
          draggable="false"
        />
        <Icon v-else name="music-note" :size="20" />
      </button>
      <div class="now-playing__text">
        <p
          v-if="track"
          class="now-playing__title"
          :class="{ 'now-playing__title--active': true }"
          title="打开专辑"
          @click="openAlbum"
        >
          {{ track.title }}
        </p>
        <p v-else class="now-playing__title now-playing__title--idle">未在播放</p>
        <p v-if="track" class="now-playing__artist" @click="openArtist">
          {{ track.artists.join(' / ') }}
        </p>
      </div>
      <button
        v-if="track"
        type="button"
        class="now-playing__favorite"
        :class="{ 'now-playing__favorite--on': isFavorite }"
        :aria-label="isFavorite ? '取消收藏' : '收藏'"
        :title="isFavorite ? '取消收藏（F）' : '收藏（F）'"
        @click="toggleFavorite"
      >
        <Icon :name="isFavorite ? 'heart-filled' : 'heart'" :size="18" />
      </button>
    </div>

    <!-- 中：传输 + 进度 -->
    <div class="transport">
      <div class="transport__buttons">
        <button
          type="button"
          class="transport__mode"
          :aria-label="modeMeta.label"
          :title="modeMeta.label"
          @click="openModeMenu($event)"
        >
          <Icon :name="modeMeta.icon" :size="17" />
        </button>
        <button
          type="button"
          class="transport__skip"
          aria-label="上一首"
          title="上一首"
          :disabled="!hasTrack"
          @click="playerStore.previous()"
        >
          <Icon name="previous" :size="20" />
        </button>
        <button
          type="button"
          class="transport__play"
          :aria-label="isPlaying ? '暂停' : '播放'"
          :disabled="!hasTrack && !isLoading"
          @click="onToggle"
        >
          <Icon v-if="isLoading" name="loader" :size="20" spin />
          <Icon v-else :name="isPlaying ? 'pause' : 'play'" :size="20" />
        </button>
        <button
          type="button"
          class="transport__skip"
          aria-label="下一首"
          title="下一首"
          :disabled="!hasTrack"
          @click="playerStore.next()"
        >
          <Icon name="next" :size="20" />
        </button>
      </div>
      <div class="transport__timeline">
        <ProgressBar
          :position-ms="snapshot.positionMs"
          :duration-ms="snapshot.durationMs"
          :disabled="!hasTrack"
          @seek="onSeek"
        />
      </div>
    </div>

    <!-- 右：桌面歌词 / 面板 + 音量 -->
    <div class="player-bar__side">
      <button
        type="button"
        class="side-btn"
        :class="{ 'side-btn--active': isDesktopLyricsVisible }"
        aria-label="实时歌词条"
        :title="isDesktopLyricsVisible ? '收起列表上方实时歌词' : '在列表上方显示实时歌词'"
        @click="toggleDesktopLyrics"
      >
        <Icon name="captions" :size="18" />
      </button>
      <button
        type="button"
        class="side-btn"
        :class="{ 'side-btn--active': isPanelOpen && panelTab === 'lyrics' }"
        aria-label="歌词面板"
        title="歌词面板（Ctrl+U）"
        @click="togglePanelTab('lyrics')"
      >
        <Icon name="lyrics" :size="18" />
      </button>
      <button
        type="button"
        class="side-btn"
        :class="{ 'side-btn--active': isPanelOpen && panelTab === 'queue' }"
        aria-label="播放队列"
        title="队列（Ctrl+Shift+U）"
        @click="togglePanelTab('queue')"
      >
        <Icon name="queue" :size="18" />
      </button>
      <button
        type="button"
        class="side-btn"
        :class="{ 'side-btn--active': sleepTimer.active }"
        aria-label="睡眠定时器"
        :title="sleepLabel"
        @click="openSleepMenu($event)"
      >
        <Icon name="moon" :size="18" />
      </button>
      <button
        type="button"
        class="side-btn side-btn--rate"
        :class="{ 'side-btn--active': Math.abs(snapshot.rate - 1) > 0.001 }"
        aria-label="播放速度"
        :title="`播放速度 ${formatRate(snapshot.rate)}`"
        @click="openRateMenu($event)"
      >
        <span class="side-btn__rate-label">{{ formatRate(snapshot.rate) }}</span>
      </button>
      <VolumeControl />
    </div>
  </div>
</template>

<style scoped>
.player-bar {
  display: grid;
  grid-template-columns: minmax(200px, 1fr) minmax(0, 560px) minmax(200px, 1fr);
  align-items: center;
  gap: var(--space-4);
  height: 100%;
  padding: 0 var(--space-4);
  background-color: var(--color-bg-elevated);
  /* 封面皮肤：右缘极淡主色染（--color-cover-wash 默认透明） */
  background-image: radial-gradient(
    70% 220% at 100% 0%,
    var(--color-cover-wash, transparent),
    transparent 60%
  );
}

/* ---- 当前曲目 ---- */
.now-playing {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-width: 0;
}

.now-playing__cover {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--size-cover-md);
  height: var(--size-cover-md);
  flex: 0 0 auto;
  padding: 0;
  overflow: hidden;
  color: var(--color-text-tertiary);
  background: var(--color-cover-placeholder);
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  /* 封面取色开启时，封面周围晕染当前曲目主色（--color-cover-accent-glow 由 useCoverTheme 注入） */
  box-shadow: 0 0 0 1px var(--color-border-subtle),
    0 4px 16px -4px var(--color-cover-accent-glow, transparent);
  transition: transform 0.12s ease, box-shadow 0.12s ease;
}

.now-playing__cover:hover:not(:disabled) {
  transform: scale(1.05);
  box-shadow: var(--shadow-elevation-2);
}

.now-playing__cover:disabled {
  cursor: default;
}

.now-playing__cover-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.now-playing__text {
  flex: 1 1 auto;
  min-width: 0;
}

.now-playing__title {
  margin: 0;
  overflow: hidden;
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-primary);
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}

.now-playing__title--active {
  color: var(--color-brand);
}

.now-playing__title--active:hover {
  color: var(--color-brand-hover);
}

.now-playing__title--idle {
  color: var(--color-text-tertiary);
  font-weight: var(--font-weight-regular);
  cursor: default;
}

.now-playing__artist {
  margin: 2px 0 0;
  overflow: hidden;
  font-size: var(--font-size-xs);
  color: var(--color-text-secondary);
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}

.now-playing__artist:hover {
  color: var(--color-text-primary);
}

.now-playing__favorite {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-h);
  height: var(--size-control-h);
  color: var(--color-text-tertiary);
  border-radius: var(--radius-sm);
}

.now-playing__favorite:hover {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}

.now-playing__favorite--on {
  color: var(--color-brand);
}

/* ---- 传输区 ---- */
.transport {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-1);
  min-width: 0;
}

.transport__buttons {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.transport__mode,
.transport__skip {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-h);
  height: var(--size-control-h);
  color: var(--color-text-secondary);
  border-radius: var(--radius-sm);
}

.transport__mode:hover,
.transport__skip:hover:not(:disabled) {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}

.transport__skip:disabled {
  color: var(--color-text-disabled);
}

.transport__play {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  color: var(--color-on-brand);
  background-color: var(--color-brand-solid);
  border-radius: var(--radius-full);
}

.transport__play:hover:not(:disabled) {
  background-color: var(--color-brand-hover);
}

.transport__play:disabled {
  background-color: var(--color-bg-hover);
  color: var(--color-text-disabled);
}

.transport__timeline {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1px;
  align-self: stretch;
  min-width: 0;
}

.transport__timeline :deep(.progress) {
  align-self: stretch;
}

/* ---- 右侧 ---- */
.player-bar__side {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-1);
  min-width: 0;
}

.side-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-h);
  height: var(--size-control-h);
  color: var(--color-text-secondary);
  border-radius: var(--radius-sm);
}

.side-btn:hover {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}

.side-btn--active {
  color: var(--color-brand);
}

/* 倍速按钮：与 icon 按钮同尺寸，文字 label 居中 */
.side-btn--rate {
  min-width: 44px;
  padding: 0 var(--space-1);
}

.side-btn__rate-label {
  font-size: 11px;
  font-weight: var(--font-weight-semibold, 600);
  line-height: 1;
  letter-spacing: 0.02em;
}
</style>
