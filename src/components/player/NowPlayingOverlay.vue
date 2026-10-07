<script setup lang="ts">
/**
 * 封面放大层（Now Playing）—— 沉浸式"正在播放"
 *
 * 封面铺满整个窗口（object-fit: cover + 暗角渐变保证文字可读）；
 * 底部区域展示 上一句 / 当前句（高亮大字）/ 下一句，随播放进度实时切换；
 * 强调色（主播放按钮、当前句辉光）取自当前封面主色（--color-cover-accent，
 * 未开启取色或无封面时回落品牌色）。
 *
 * Esc、关闭按钮退出；不持久化、不影响右侧歌词面板。
 */
import { computed, onBeforeUnmount, onMounted, watch } from 'vue'
import { storeToRefs } from 'pinia'

import { usePlayerStore } from '../../stores/player'
import { useLibraryStore } from '../../stores/library'
import { useLyricsStore } from '../../stores/lyrics'
import { useLayout } from '../../composables/useLayout'
import { useTrackCover } from '../../composables/useTrackCover'
import Icon from '../common/Icon.vue'
import defaultCover from '../../assets/default-cover.svg'

const playerStore = usePlayerStore()
const libraryStore = useLibraryStore()
const lyricsStore = useLyricsStore()
const { closeNowPlaying } = useLayout()
const { snapshot } = storeToRefs(playerStore)

const { coverUrl } = useTrackCover(() => snapshot.value.trackId)
/** 背景图（封面链未命中时使用内置默认封面）；作为 :key 触发切歌淡入 */
const bgSrc = computed(() => coverUrl.value ?? defaultCover)

const track = computed(() => libraryStore.getTrack(snapshot.value.trackId))
const isPlaying = computed(() => snapshot.value.status === 'playing')

// 歌词加载与播放条歌词条共用同一 store（store 内对同曲目去重），此处兜底触发
watch(
  () => snapshot.value.trackId,
  (trackId) => {
    if (trackId) void lyricsStore.load(trackId)
  },
  { immediate: true },
)

/** 当前/上/下三句（随 positionMs 响应式重算，250ms 一次的进度事件驱动） */
const lyricTrio = computed(() => {
  const lines = lyricsStore.lyrics?.lines ?? []
  const active = lyricsStore.activeLineIndexAt(snapshot.value.positionMs)
  if (active < 0) {
    return { prev: null as string | null, current: null, next: lines[0]?.text ?? null }
  }
  return {
    prev: active > 0 ? lines[active - 1].text : null,
    current: lines[active]?.text ?? null,
    next: active + 1 < lines.length ? lines[active + 1].text : null,
  }
})

const hasLyrics = computed(() => lyricTrio.value.current !== null || lyricTrio.value.next !== null)

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') closeNowPlaying()
  else if (event.key === ' ') {
    event.preventDefault()
    void playerStore.toggle()
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <Teleport to="body">
    <div class="now-overlay" role="dialog" aria-label="正在播放">
      <!-- 铺满窗口的封面背景（切歌时淡入） -->
      <img
        :key="bgSrc"
        class="now-overlay__bg"
        :src="bgSrc"
        alt=""
        draggable="false"
      />
      <!-- 暗角 + 封面主色淡染，保证任意封面上的文字可读性 -->
      <div class="now-overlay__scrim" aria-hidden="true"></div>

      <button
        type="button"
        class="now-overlay__close"
        aria-label="关闭封面视图（Esc）"
        title="关闭（Esc）"
        @click="closeNowPlaying"
      >
        <Icon name="x" :size="20" />
      </button>

      <!-- 底部信息区：三句歌词 + 曲目 + 控制 -->
      <div class="now-overlay__dock">
        <div class="now-overlay__lyrics" aria-live="polite">
          <p v-if="lyricTrio.prev" class="now-overlay__line now-overlay__line--prev">
            {{ lyricTrio.prev }}
          </p>
          <p :key="lyricTrio.current ?? lyricTrio.next" class="now-overlay__line now-overlay__line--current">
            {{ hasLyrics ? (lyricTrio.current ?? lyricTrio.next) : '暂无歌词' }}
          </p>
          <p v-if="lyricTrio.next && lyricTrio.current" class="now-overlay__line now-overlay__line--next">
            {{ lyricTrio.next }}
          </p>
        </div>

        <div class="now-overlay__meta">
          <p class="now-overlay__title">{{ track?.title ?? '未在播放' }}</p>
          <p class="now-overlay__artist">{{ track?.artists.join(' / ') ?? '—' }}</p>
        </div>

        <div class="now-overlay__controls">
          <button
            type="button"
            class="now-overlay__btn"
            aria-label="上一首"
            :disabled="!track"
            @click="playerStore.previous()"
          >
            <Icon name="previous" :size="26" />
          </button>
          <button
            type="button"
            class="now-overlay__btn now-overlay__btn--primary"
            :aria-label="isPlaying ? '暂停' : '播放'"
            :disabled="!track"
            @click="playerStore.toggle()"
          >
            <Icon :name="isPlaying ? 'pause' : 'play'" :size="30" />
          </button>
          <button
            type="button"
            class="now-overlay__btn"
            aria-label="下一首"
            :disabled="!track"
            @click="playerStore.next()"
          >
            <Icon name="next" :size="26" />
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.now-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal);
  overflow: hidden;
  color: #fff;
  background-color: #11141a;
  animation: now-overlay-in 0.22s ease-out;
}

@keyframes now-overlay-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

.now-overlay__bg {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  animation: now-bg-in 0.45s ease-out;
}

@keyframes now-bg-in {
  from {
    opacity: 0;
    transform: scale(1.03);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

.now-overlay__scrim {
  position: absolute;
  inset: 0;
  pointer-events: none;
  /* 顶部（关闭按钮）与底部（歌词/控制）压暗，中部仅轻染；再叠一层封面主色 */
  background:
    linear-gradient(to top, rgba(6, 8, 12, 0.92) 0%, rgba(6, 8, 12, 0.55) 34%, rgba(6, 8, 12, 0.12) 62%, rgba(6, 8, 12, 0.5) 100%),
    var(--color-cover-accent-soft, transparent);
}

.now-overlay__close {
  position: absolute;
  top: var(--space-5);
  right: var(--space-5);
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  color: rgba(255, 255, 255, 0.82);
  background: rgba(0, 0, 0, 0.28);
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: var(--radius-full);
  transition: background-color 0.15s ease, color 0.15s ease;
}

.now-overlay__close:hover {
  color: #fff;
  background: rgba(0, 0, 0, 0.5);
}

/* ---- 底部信息坞 ---- */
.now-overlay__dock {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  z-index: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-8) clamp(var(--space-6), 8vw, 120px) calc(var(--space-8) + env(safe-area-inset-bottom, 0px));
  text-align: center;
}

.now-overlay__lyrics {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  width: min(900px, 92vw);
  min-height: 7.5em;
  justify-content: flex-end;
}

.now-overlay__line {
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-shadow: 0 2px 18px rgba(0, 0, 0, 0.55);
}

.now-overlay__line--prev,
.now-overlay__line--next {
  font-size: clamp(var(--font-size-sm), 1.6vw, var(--font-size-lg));
  color: rgba(255, 255, 255, 0.58);
}

.now-overlay__line--current {
  font-size: clamp(var(--font-size-xl), 3.4vw, 40px);
  font-weight: var(--font-weight-bold);
  line-height: 1.25;
  /* 封面皮肤开启时用歌词色，未取色时回落白色（大封面上可读性最好） */
  color: var(--color-cover-lyrics, #fff);
  animation: now-line-in 0.3s ease-out;
}

@keyframes now-line-in {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.now-overlay__meta {
  width: min(720px, 90vw);
}

.now-overlay__title {
  margin: 0;
  font-size: clamp(var(--font-size-md), 1.8vw, var(--font-size-xl));
  font-weight: var(--font-weight-semibold);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.now-overlay__artist {
  margin: 4px 0 0;
  font-size: var(--font-size-sm);
  color: rgba(255, 255, 255, 0.66);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.now-overlay__controls {
  display: flex;
  align-items: center;
  gap: var(--space-6);
}

.now-overlay__btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 54px;
  height: 54px;
  color: rgba(255, 255, 255, 0.88);
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: var(--radius-full);
  transition: color 0.15s ease, background-color 0.15s ease, transform 0.1s ease;
}

.now-overlay__btn:hover:not(:disabled) {
  color: #fff;
  background: rgba(255, 255, 255, 0.2);
}

.now-overlay__btn:active:not(:disabled) {
  transform: scale(0.94);
}

.now-overlay__btn:disabled {
  opacity: 0.35;
}

.now-overlay__btn--primary {
  width: 68px;
  height: 68px;
  color: #fff;
  /* 皮肤跟随封面：取色未开启时回落品牌蓝 */
  background-color: var(--color-cover-accent, var(--color-brand-solid, #2f6feb));
  border-color: transparent;
  box-shadow: 0 10px 30px var(--color-cover-accent-glow, rgba(47, 111, 235, 0.45));
}

.now-overlay__btn--primary:hover:not(:disabled) {
  color: #fff;
  background-color: var(--color-cover-accent, var(--color-brand-solid, #2f6feb));
  filter: brightness(1.1);
}

/* 矮窗口：压缩间距，歌词区不再占固定高度，避免遮挡封面主体 */
@media (max-height: 620px) {
  .now-overlay__dock {
    gap: var(--space-2);
    padding-top: var(--space-5);
    padding-bottom: var(--space-5);
  }

  .now-overlay__lyrics {
    min-height: 0;
  }
}
</style>
