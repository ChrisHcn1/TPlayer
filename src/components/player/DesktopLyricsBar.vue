<script setup lang="ts">
/**
 * 桌面实时歌词条（播放列表上方）
 *
 * 与右侧 LyricsPanel 相互独立：
 *  - 本组件常驻于主内容区顶部（v-show 控制开合），随播放进度实时高亮当前歌词行；
 *  - 右面板歌词页签的开合不影响本组件，反之亦然；
 *  - 本组件常驻加载歌词（watch trackId），同时承担"面板未挂载时也能拿到歌词"的责任。
 *
 * 点击歌词正文可打开右侧歌词面板（完整歌词 / 在线匹配 / 偏移校正）。
 */
import { computed, watch } from 'vue'
import { storeToRefs } from 'pinia'

import { usePlayerStore } from '../../stores/player'
import { useLyricsStore } from '../../stores/lyrics'
import { useLayout } from '../../composables/useLayout'
import Icon from '../common/Icon.vue'

const playerStore = usePlayerStore()
const lyricsStore = useLyricsStore()
const { togglePanelTab, toggleDesktopLyrics } = useLayout()
const { snapshot } = storeToRefs(playerStore)
const { lyrics } = storeToRefs(lyricsStore)

// 常驻加载歌词：右面板歌词页签关闭/卸载时，本歌词条仍随切歌实时更新
watch(
  () => snapshot.value.trackId,
  (trackId) => {
    if (trackId) void lyricsStore.load(trackId)
    else lyricsStore.clear()
  },
  { immediate: true },
)

/** 当前播放位置对应的歌词行下标 */
const activeIndex = computed(() =>
  lyrics.value?.synced ? lyricsStore.activeLineIndexAt(snapshot.value.positionMs) : -1,
)

/** 当前歌词行文本 */
const currentLine = computed(() =>
  activeIndex.value >= 0 ? lyrics.value?.lines[activeIndex.value]?.text ?? '' : '',
)

/** 下一行歌词（右侧弱化预览，没有则为空） */
const nextLine = computed(() => {
  if (activeIndex.value < 0) return ''
  return lyrics.value?.lines[activeIndex.value + 1]?.text ?? ''
})

/** 占位文案：未播放 / 无同步歌词 */
const placeholder = computed(() => {
  if (!snapshot.value.trackId) return '未在播放'
  if (!lyrics.value) return '暂无歌词，点击打开歌词面板在线匹配'
  if (!lyrics.value.synced) return '当前为静态歌词，暂无逐字时间轴'
  return ''
})

function openLyricsPanel(): void {
  togglePanelTab('lyrics')
}
</script>

<template>
  <div class="desktop-lyrics" :class="{ 'desktop-lyrics--idle': !currentLine }">
    <!-- 左：标识 -->
    <div class="desktop-lyrics__label">
      <Icon name="lyrics" :size="15" />
      <span>实时歌词</span>
    </div>

    <!-- 中：当前歌词行（点击打开右侧歌词面板） -->
    <button
      type="button"
      class="desktop-lyrics__main"
      title="打开歌词面板"
      @click="openLyricsPanel"
    >
      <template v-if="currentLine">
        <span :key="currentLine" class="desktop-lyrics__current">{{ currentLine }}</span>
        <span v-if="nextLine" class="desktop-lyrics__next">{{ nextLine }}</span>
      </template>
      <span v-else class="desktop-lyrics__placeholder">{{ placeholder }}</span>
    </button>

    <!-- 右：收起歌词条（再次打开用播放条右侧的字幕按钮） -->
    <button
      type="button"
      class="desktop-lyrics__close"
      aria-label="收起实时歌词"
      title="收起实时歌词"
      @click="toggleDesktopLyrics"
    >
      <Icon name="chevron-down" :size="17" />
    </button>
  </div>
</template>

<style scoped>
.desktop-lyrics {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--space-4);
  flex: 0 0 auto;
  min-height: 44px;
  padding: 0 var(--space-4);
  border-bottom: 1px solid var(--color-border-subtle);
  background-color: var(--color-bg-surface);
}

.desktop-lyrics__label {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--color-text-tertiary);
  font-size: var(--font-size-xs);
  white-space: nowrap;
}

.desktop-lyrics__main {
  display: flex;
  align-items: baseline;
  justify-content: center;
  gap: var(--space-4);
  min-width: 0;
  padding: var(--space-1) var(--space-3);
  border-radius: var(--radius-sm);
}

.desktop-lyrics__main:hover {
  background-color: var(--color-bg-hover);
}

.desktop-lyrics__current {
  min-width: 0;
  overflow: hidden;
  font-size: var(--font-size-md);
  font-weight: var(--font-weight-semibold);
  /* 封面皮肤开启时用歌词色（近白高亮），并加强调色辉光与彩色按钮/进度拉开 */
  color: var(--color-cover-lyrics, var(--color-brand));
  text-shadow: 0 0 14px var(--color-cover-accent-glow, transparent);
  text-overflow: ellipsis;
  white-space: nowrap;
  /* 切句时的淡入上移（key 变化重新触发） */
  animation: desktop-lyrics-in 0.25s ease;
}

.desktop-lyrics__next {
  flex: 0 1 auto;
  max-width: 280px;
  overflow: hidden;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.desktop-lyrics__placeholder {
  overflow: hidden;
  font-size: var(--font-size-sm);
  color: var(--color-text-tertiary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.desktop-lyrics--idle .desktop-lyrics__main:hover {
  background-color: transparent;
}

.desktop-lyrics__close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-h);
  height: var(--size-control-h);
  color: var(--color-text-tertiary);
  border-radius: var(--radius-sm);
}

.desktop-lyrics__close:hover {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}

@keyframes desktop-lyrics-in {
  from {
    opacity: 0;
    transform: translateY(4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
