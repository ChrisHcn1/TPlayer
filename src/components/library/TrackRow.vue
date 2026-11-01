<script setup lang="ts">
/**
 * 曲目行（DESIGN §4.5）
 *
 * 列：选择框 40 · 序号/播放位 40 · 标题（弹性）· 艺术家 2fr · 专辑 2fr · 时长 64 · 更多 40
 * 状态：default / hover / selected / playing（左缘 3px 指示条 + 均衡器）；
 * 交互：双击播放、勾选多选（Shift 连选由 TrackList 处理）、右键菜单、更多按钮。
 */
import { computed, onMounted } from 'vue'

import type { Track } from '../../types'
import { t } from '../../i18n'
import Icon from '../common/Icon.vue'
import AudioQualityBadge from '../common/AudioQualityBadge.vue'
import { classifyQuality, QUALITY_LEVELS } from '../../services/audio-quality'
import { useLyricsMetadata } from '../../lab/features/lyrics-metadata'
import { useMetaRetryStore } from '../../stores/metaRetry'
import { useSettingsStore } from '../../stores/settings'

interface Props {
  track: Track
  /** 基于 1 的展示序号（内部转 index 文本） */
  displayIndex: number
  selected: boolean
  /** 当前曲目 id 等于本行（无论是否在播放） */
  isCurrent: boolean
  isPlaying: boolean
  showArtist?: boolean
  showAlbum?: boolean
  /** 分轨专辑行：展示该轨在整盘中的起止时间提示等（预留） */
}

const props = withDefaults(defineProps<Props>(), {
  showArtist: true,
  showAlbum: true,
})

const emit = defineEmits<{
  (e: 'dblplay', track: Track): void
  (e: 'toggle-select', track: Track, event: MouseEvent): void
  (e: 'row-click', track: Track, event: MouseEvent): void
  (e: 'context-menu', track: Track, event: MouseEvent): void
}>()

// 实验室：歌词边车元数据补全（开启后，曲目渲染时按需回填未知艺术家/专辑）
useLyricsMetadata(() => props.track?.id)

// 实验室·元数据增强：单曲识别记录（列表内展示「已识别」标记）
const metaRetry = useMetaRetryStore()
const retryRecord = computed(() => metaRetry.recordOf(props.track.id))
const retryStatusText = computed(() => {
  const rec = retryRecord.value
  if (!rec) return ''
  return rec.status === 'ok'
    ? t('track.retryOk')
    : rec.status === 'failed'
      ? t('track.retryFailed')
      : t('track.retryUnchanged')
})
/** 标记文案：状态 + 识别时间（HH:MM），完整时间与详情见悬停提示 */
const retryBadgeText = computed(() => {
  const rec = retryRecord.value
  if (!rec) return ''
  const time = rec.at.length >= 16 ? rec.at.slice(11, 16) : rec.at
  return `${retryStatusText.value} ${time}`
})
const retryBadgeTitle = computed(() => {
  const rec = retryRecord.value
  if (!rec) return ''
  return t('track.retryBadgeTitle', {
    time: rec.at,
    status: retryStatusText.value,
    detail: rec.detail,
  })
})
onMounted(() => {
  void metaRetry.ensureLoaded()
})

// 音质分级标识：依据格式 / 比特率 / 采样率 / SACD-DSD 归类；标准音质不展示。
// 受全局「显示音质标识」开关控制，关闭后隐藏（状态已持久化）。
const settingsStore = useSettingsStore()
const quality = computed(() => {
  if (!settingsStore.showAudioQuality) return null
  const q = classifyQuality(props.track)
  return q && q !== 'standard' ? q : null
})
const qualityTitle = computed(() =>
  quality.value ? t(QUALITY_LEVELS[quality.value].descKey) : '',
)

const durationText = computed(() => {
  const totalSeconds = Math.round(props.track.duration)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes}:${seconds.toString().padStart(2, '0')}`
})

function onCheckboxClick(event: MouseEvent): void {
  emit('toggle-select', props.track, event)
}
</script>

<template>
  <div
    class="track-row"
    :class="{
      'track-row--selected': selected,
      'track-row--current': isCurrent,
      'track-row--playing': isPlaying,
      'track-row--no-artist': !showArtist,
    }"
    role="row"
    @click="emit('row-click', track, $event)"
    @dblclick="emit('dblplay', track)"
    @contextmenu="emit('context-menu', track, $event)"
  >
    <span class="track-row__indicator" aria-hidden="true"></span>

    <div class="track-row__check">
      <button
        type="button"
        class="track-row__checkbox"
        :class="{ 'track-row__checkbox--checked': selected }"
        role="checkbox"
        :aria-checked="selected"
        :aria-label="selected ? t('track.deselect') : t('track.select')"
        :tabindex="selected ? 0 : -1"
        @click.stop="onCheckboxClick($event)"
      >
        <Icon v-if="selected" name="check" :size="12" />
      </button>
    </div>

    <div class="track-row__index">
      <span class="track-row__index-num u-tabular">{{ displayIndex }}</span>
      <Icon name="play" :size="14" class="track-row__index-play" />
      <span v-if="isCurrent" class="track-row__equalizer" aria-hidden="true">
        <span></span><span></span><span></span>
      </span>
    </div>

    <div class="track-row__title">
      <span class="track-row__title-text" :class="{ 'track-row__title-text--current': isCurrent }">
        {{ track.title }}
      </span>
      <AudioQualityBadge v-if="quality" :level="quality" :title="qualityTitle" class="track-row__quality" />
      <span
        v-if="retryRecord"
        class="track-row__retry-badge"
        :class="`track-row__retry-badge--${retryRecord.status}`"
        :title="retryBadgeTitle"
        >{{ retryBadgeText }}</span
      >
    </div>

    <div v-if="showArtist" class="track-row__artist">
      <span class="u-truncate">{{ track.artists.join(' / ') }}</span>
    </div>

    <div v-if="showAlbum" class="track-row__album">
      <span class="u-truncate">{{ track.album }}</span>
    </div>

    <div class="track-row__duration u-tabular">{{ durationText }}</div>

    <div class="track-row__actions">
      <button
        type="button"
        class="track-row__more"
        :aria-label="t('track.moreActions')"
        @click.stop="emit('context-menu', track, $event)"
      >
        <Icon name="more-horizontal" :size="16" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.track-row {
  position: relative;
  display: grid;
  grid-template-columns: 40px 40px minmax(0, 3fr) minmax(0, 2fr) minmax(0, 2fr) 64px 40px;
  align-items: center;
  min-height: var(--size-row-h);
  padding: 0 var(--space-2) 0 var(--space-1);
  border-radius: var(--radius-sm);
  cursor: default;
}

.track-row--no-artist {
  grid-template-columns: 40px 40px minmax(0, 3fr) minmax(0, 2fr) 64px 40px;
}

/* 左缘播放指示条 */
.track-row__indicator {
  position: absolute;
  left: 0;
  top: 50%;
  width: 3px;
  height: 0;
  border-radius: var(--radius-full);
  background-color: var(--color-brand);
  transform: translateY(-50%);
  transition: height var(--transition-colors);
}

.track-row--current .track-row__indicator {
  height: 16px;
}

.track-row:hover {
  background-color: var(--color-bg-hover);
}

.track-row--selected {
  background-color: var(--color-bg-selected);
}

.track-row--selected:hover {
  background-color: var(--color-bg-active);
}

/* 选择框 */
.track-row__check,
.track-row__index {
  display: flex;
  align-items: center;
  justify-content: center;
}

.track-row__checkbox {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  color: transparent;
  background: transparent;
  border: 1px solid var(--color-border-strong);
  border-radius: var(--radius-xs);
  opacity: 0;
  transition:
    opacity var(--transition-colors),
    background-color var(--transition-colors),
    border-color var(--transition-colors);
}

.track-row:hover .track-row__checkbox,
.track-row--selected .track-row__checkbox,
.track-row__checkbox:focus-visible {
  opacity: 1;
}

.track-row__checkbox--checked {
  opacity: 1;
  color: var(--color-on-brand);
  background-color: var(--color-brand-solid);
  border-color: var(--color-brand-solid);
}

/* 序号 / 悬停播放 / 均衡器三态切换 */
.track-row__index {
  position: relative;
  color: var(--color-text-tertiary);
  font-size: var(--font-size-sm);
}

.track-row__index-play {
  position: absolute;
  color: var(--color-text-primary);
  opacity: 0;
}

.track-row:hover .track-row__index-num,
.track-row--current .track-row__index-num {
  opacity: 0;
}

.track-row:hover .track-row__index-play {
  opacity: 1;
}

.track-row--current .track-row__index-play {
  opacity: 0;
}

.track-row__title-text--current {
  color: var(--color-brand);
}

.track-row--current .track-row__index {
  color: var(--color-brand);
}

/* 均衡器（播放中） */
.track-row__equalizer {
  position: absolute;
  display: flex;
  align-items: flex-end;
  gap: 2px;
  height: 14px;
}

.track-row__equalizer span {
  width: 3px;
  height: 100%;
  background-color: var(--color-brand);
  border-radius: 1px;
  animation: eq-bounce 0.9s ease-in-out infinite;
  transform-origin: bottom;
}

.track-row__equalizer span:nth-child(1) {
  animation-delay: -0.4s;
}
.track-row__equalizer span:nth-child(2) {
  animation-delay: -0.2s;
}
.track-row__equalizer span:nth-child(3) {
  animation-delay: -0.6s;
}

.track-row:not(.track-row--playing) .track-row__equalizer span {
  animation-play-state: paused;
  transform: scaleY(0.4);
}

@keyframes eq-bounce {
  0%,
  100% {
    transform: scaleY(0.35);
  }
  50% {
    transform: scaleY(1);
  }
}

/* 文本列 */
.track-row__title {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: var(--space-2);
  min-width: 0;
  padding-right: var(--space-2);
}

.track-row__title-text,
.track-row__artist span,
.track-row__album span {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track-row__title-text {
  flex: 0 1 auto;
  min-width: 0;
  font-size: var(--font-size-sm);
  color: var(--color-text-primary);
}

/* 音质标识紧贴标题，随标题长短自适应，不再被拉伸到列右缘 */
.track-row__quality {
  align-self: center;
}

/* 单曲识别标记（悬停查看识别时间 / 状态 / 变更详情） */
.track-row__retry-badge {
  flex: none;
  padding: 0 5px;
  font-size: 10px;
  line-height: 15px;
  white-space: nowrap;
  color: var(--color-text-secondary);
  border: 1px solid var(--color-border-strong);
  border-radius: var(--radius-xs);
  cursor: help;
}

.track-row__retry-badge--ok {
  color: var(--color-brand);
  border-color: var(--color-brand);
}

.track-row__retry-badge--failed {
  color: var(--color-danger);
  border-color: var(--color-danger);
}

.track-row__artist,
.track-row__album {
  min-width: 0;
  padding-right: var(--space-3);
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
}

.track-row__duration {
  font-size: var(--font-size-sm);
  color: var(--color-text-tertiary);
  text-align: right;
}

/* 更多按钮 */
.track-row__actions {
  display: flex;
  align-items: center;
  justify-content: center;
}

.track-row__more {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-h);
  height: var(--size-control-h);
  color: var(--color-text-secondary);
  border-radius: var(--radius-sm);
  opacity: 0;
  transition: opacity var(--transition-colors);
}

.track-row:hover .track-row__more,
.track-row--selected .track-row__more {
  opacity: 1;
}

.track-row__more:hover {
  color: var(--color-text-primary);
  background-color: var(--color-bg-active);
}
</style>
