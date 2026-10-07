<script setup lang="ts">
/**
 * 媒体卡片（DESIGN §4.13 网格）：专辑 / 艺术家 / 歌单 / 分轨专辑共用。
 * 无封面素材时使用语义占位色 + 语义图标；悬停浮现播放按钮。
 */
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'

import Icon from '../common/Icon.vue'
import { loadCover } from '../../composables/useTrackCover'

type CardVariant = 'album' | 'artist' | 'playlist' | 'cue'

interface Props {
  variant: CardVariant
  title: string
  subtitle?: string
  /** 角标（如“分轨”/曲目数） */
  badge?: string
  /** 处于播放中上下文（标题染品牌色） */
  active?: boolean
  /** 封面缓存键（album:<id> / artist:<名>）；缺省或无封面时回落变体图标 */
  coverKey?: string | null
}

const props = withDefaults(defineProps<Props>(), {
  subtitle: '',
  badge: '',
  active: false,
  coverKey: null,
})

const emit = defineEmits<{
  (e: 'open'): void
  (e: 'play'): void
  (e: 'context-menu', event: MouseEvent): void
}>()

const variantIcon = computed(() => {
  switch (props.variant) {
    case 'artist':
      return 'user'
    case 'playlist':
      return 'list-music'
    case 'cue':
      return 'layers'
    case 'album':
    default:
      return 'disc'
  }
})

// ---- 封面（进入视口附近才加载，避免整屏卡片同时打 IPC） ----
const rootEl = ref<HTMLElement | null>(null)
const coverUrl = shallowRef<string | null>(null)
/** 是否已进入过视口（之后 key 变化直接重载） */
let armed = false
let loadToken = 0
let observer: IntersectionObserver | null = null

function loadCoverArt(key: string | null | undefined): void {
  const token = ++loadToken
  coverUrl.value = null
  if (!key) return
  void loadCover(key).then((url) => {
    // 重扫/切组产生的过期响应丢弃
    if (token === loadToken) coverUrl.value = url
  })
}

onMounted(() => {
  if (typeof IntersectionObserver === 'undefined') {
    // 环境不支持观察器：直接加载（仍走模块缓存，代价可控）
    armed = true
    loadCoverArt(props.coverKey)
    return
  }
  observer = new IntersectionObserver(
    (entries) => {
      if (!entries.some((entry) => entry.isIntersecting)) return
      armed = true
      loadCoverArt(props.coverKey)
      observer?.disconnect()
      observer = null
    },
    { rootMargin: '300px 0px' },
  )
  if (rootEl.value) observer.observe(rootEl.value)
})

onBeforeUnmount(() => observer?.disconnect())

watch(
  () => props.coverKey,
  (key) => {
    if (armed) loadCoverArt(key)
  },
)
</script>

<template>
  <div
    ref="rootEl"
    class="media-card"
    :class="`media-card--${variant}`"
    tabindex="0"
    @click="emit('open')"
    @dblclick="emit('play')"
    @keydown.enter="emit('open')"
    @contextmenu="emit('context-menu', $event)"
  >
    <div class="media-card__cover">
      <img
        v-if="coverUrl"
        :src="coverUrl"
        alt=""
        draggable="false"
        class="media-card__cover-img"
      />
      <Icon v-else class="media-card__cover-icon" :name="variantIcon" :size="36" />
      <span v-if="badge" class="media-card__badge">{{ badge }}</span>
      <button
        type="button"
        class="media-card__play"
        :aria-label="`播放 ${title}`"
        @click.stop="emit('play')"
      >
        <Icon name="play" :size="18" />
      </button>
    </div>
    <p class="media-card__title" :class="{ 'media-card__title--active': active }">{{ title }}</p>
    <p v-if="subtitle" class="media-card__subtitle">{{ subtitle }}</p>
  </div>
</template>

<style scoped>
.media-card {
  display: block;
  padding: var(--space-3);
  border-radius: var(--radius-md);
  cursor: default;
  transition: background-color var(--transition-colors);
}

.media-card:hover,
.media-card:focus-visible {
  background-color: var(--color-bg-hover);
  outline: none;
}

.media-card__cover {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  aspect-ratio: 1;
  margin-bottom: var(--space-3);
  overflow: hidden;
  background-color: var(--color-cover-placeholder);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-elevation-1);
}

.media-card__cover-img {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  user-select: none;
}

.media-card__cover-icon {
  color: var(--color-text-tertiary);
  opacity: 0.7;
}

.media-card__badge {
  position: absolute;
  top: var(--space-2);
  left: var(--space-2);
  padding: 2px var(--space-2);
  font-size: var(--font-size-2xs, 11px);
  color: var(--color-text-secondary);
  background-color: var(--color-bg-elevated);
  border-radius: var(--radius-full);
}

.media-card__play {
  position: absolute;
  right: var(--space-3);
  bottom: var(--space-3);
  display: flex;
  align-items: center;
  justify-content: center;
  width: 42px;
  height: 42px;
  color: var(--color-on-brand);
  background-color: var(--color-brand-solid);
  border-radius: var(--radius-full);
  box-shadow: var(--shadow-elevation-2);
  opacity: 0;
  transform: translateY(6px);
  transition:
    opacity var(--transition-colors),
    transform var(--transition-colors);
}

.media-card:hover .media-card__play,
.media-card__play:focus-visible {
  opacity: 1;
  transform: translateY(0);
}

.media-card__play:hover {
  background-color: var(--color-brand-hover);
}

.media-card__title {
  margin: 0;
  overflow: hidden;
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-primary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.media-card__title--active {
  color: var(--color-brand);
}

.media-card__subtitle {
  margin: 2px 0 0;
  overflow: hidden;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
