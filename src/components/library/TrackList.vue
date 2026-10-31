<script setup lang="ts">
/**
 * 曲目列表（DESIGN §4.5 / §4.6）
 *
 * 受控多选（v-model:selectedIds）+ 表头排序 + 双击播放 + 右键批量动作。
 * 播放上下文由 source/sourceId 与当前（排序后）列表构成，保证“下一首”语义一致。
 *
 * 性能：万级曲目使用 RecycleScroller 虚拟滚动，只渲染可视区行；
 * 行高取 --size-row-h（密度档 40/48/56），滚动容器高度实时测量
 * （外层滚动经过大段 hero 时列表高度同步增长，滚走 hero 后列表铺满视口）。
 */
import { computed, nextTick, onActivated, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { RecycleScroller } from 'vue-virtual-scroller'

import type { PlayContext, Track } from '../../types'
import { usePlayerStore } from '../../stores/player'
import { useLibraryStore } from '../../stores/library'
import { useTrackActions } from '../../composables/useTrackActions'
import TrackRow from './TrackRow.vue'
import TrackListHeader, {
  type SortDirection,
  type TrackSortField,
} from './TrackListHeader.vue'
import EmptyState from '../common/EmptyState.vue'

interface Props {
  tracks: Track[]
  /** 播放上下文来源（library/album/artist/playlist/search） */
  source: PlayContext['source']
  sourceId?: string | null
  showArtist?: boolean
  showAlbum?: boolean
  sortable?: boolean
  selectedIds: string[]
  emptyTitle?: string
  emptyDescription?: string
}

const props = withDefaults(defineProps<Props>(), {
  sourceId: null,
  showArtist: true,
  showAlbum: true,
  sortable: true,
  selectedIds: () => [],
  emptyTitle: '暂无曲目',
  emptyDescription: '',
})

const emit = defineEmits<{
  (e: 'update:selectedIds', ids: string[]): void
}>()

const playerStore = usePlayerStore()
const libraryStore = useLibraryStore()
const { openTrackMenu } = useTrackActions()

// ---- 滚动按需补解析 ---------------------------------------------------------
// 「全部歌曲」不再后台整库解析：滚到可见区才补该区普通占位的标签 / 时长，
// 未滚动到的保持轻量占位（文件夹推断的专辑/艺人），从而加载/滚动/播放都不卡。
// debounce 200ms 避免快速滚动时频繁请求；可见区前后各多取 10 行做预取。
let resolveTimer: ReturnType<typeof setTimeout> | null = null

function scheduleVisibleResolve(startIndex: number, endIndex: number): void {
  if (resolveTimer != null) clearTimeout(resolveTimer)
  resolveTimer = setTimeout(() => {
    resolveTimer = null
    const list = sortedTracks.value
    const from = Math.max(0, startIndex - 10)
    const to = Math.min(list.length - 1, endIndex + 10)
    const ids: string[] = []
    for (let i = from; i <= to; i += 1) {
      const id = list[i]?.id
      if (id) ids.push(id)
    }
    void libraryStore.resolveTracks(ids)
  }, 200)
}

/** RecycleScroller 可见区变化（虚拟滚动回调） */
function onVisibleUpdate(startIndex: number, endIndex: number): void {
  scheduleVisibleResolve(startIndex, endIndex)
}

// ---- 虚拟滚动：行高与容器高度 -------------------------------------------
// RecycleScroller 是泛型函数式组件，InstanceType 无法表示，仅声明需要用到的命令式 API
interface ScrollerApi {
  scrollToItem: (index: number, options?: ScrollToOptions) => void
}
const scrollerRef = ref<ScrollerApi | null>(null)
const scrollerEl = ref<HTMLElement | null>(null)
const scrollerHeight = ref(480)
const rowHeight = ref(48)

/** 读取当前密度档行高（--size-row-h，px） */
function readRowHeight(): number {
  const raw = getComputedStyle(document.documentElement)
    .getPropertyValue('--size-row-h')
    .trim()
  const parsed = Number.parseFloat(raw)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : 48
}

/**
 * 测量列表可视高度：以外层滚动容器（.app-shell__content-scroll）底边为界，
 * 减去列表当前距视口顶的距离。外层滚动经过 hero 时会实时重测，
 * 使列表随 hero 滚走而增高，最终铺满视口，不产生空白。
 */
function measure(): void {
  rowHeight.value = readRowHeight()
  const el = scrollerEl.value
  if (!el) return
  const root = el.closest('.app-shell__content-scroll') as HTMLElement | null
  if (!root) {
    scrollerHeight.value = 480
    return
  }
  const rootRect = root.getBoundingClientRect()
  const rect = el.getBoundingClientRect()
  scrollerHeight.value = Math.max(200, Math.floor(rootRect.bottom - rect.top))
}

// 观察器与滚动根引用改为**组件实例级**：TrackList 被多个 keepAlive 视图复用，
// 模块级变量会让后挂载的实例覆盖前者的观察器，前者卸载时又 disconnect 别人的，
// 导致高度测量失效与监听泄漏。
const resizeObserver = ref<ResizeObserver | null>(null)
const densityObserver = ref<MutationObserver | null>(null)
const scrollRoot = ref<HTMLElement | null>(null)

onMounted(() => {
  rowHeight.value = readRowHeight()
  void nextTick(measure)
  scrollRoot.value = scrollerEl.value?.closest('.app-shell__content-scroll') as HTMLElement | null
  // 外层滚动时视口相对位置变化 → 实时调整高度（passive，不影响滚动性能）
  scrollRoot.value?.addEventListener('scroll', measure, { passive: true })
  window.addEventListener('resize', measure)
  resizeObserver.value = new ResizeObserver(() => measure())
  if (scrollRoot.value) resizeObserver.value.observe(scrollRoot.value)
  // 密度档切换（html[data-density]）时同步 item-size
  densityObserver.value = new MutationObserver(() => {
    rowHeight.value = readRowHeight()
    void nextTick(measure)
  })
  densityObserver.value.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-density'],
  })
})

onBeforeUnmount(() => {
  scrollRoot.value?.removeEventListener('scroll', measure)
  window.removeEventListener('resize', measure)
  resizeObserver.value?.disconnect()
  resizeObserver.value = null
  densityObserver.value?.disconnect()
  densityObserver.value = null
  scrollRoot.value = null
})

const sortField = ref<TrackSortField | null>(null)
const sortDir = ref<SortDirection>('asc')

function onSort(field: TrackSortField): void {
  if (!props.sortable) return
  if (sortField.value === field) {
    sortDir.value = sortDir.value === 'asc' ? 'desc' : 'asc'
  } else {
    sortField.value = field
    sortDir.value = 'asc'
  }
}

/** 排序结果缓存：仅在 tracks 引用 / 排序字段 / 排序方向变化时重排 */
interface SortCache {
  tracksRef: Track[]
  sortField: TrackSortField | null
  sortDir: SortDirection
  result: Track[]
}
const sortCache = ref<SortCache | null>(null)

const sortedTracks = computed<Track[]>(() => {
  if (!sortField.value) return props.tracks

  const cache = sortCache.value
  if (
    cache &&
    cache.tracksRef === props.tracks &&
    cache.sortField === sortField.value &&
    cache.sortDir === sortDir.value
  ) {
    return cache.result
  }

  const field = sortField.value
  const dir = sortDir.value === 'asc' ? 1 : -1

  // 预计算排序键，避免 localeCompare 在 sort 回调中反复调用（O(n log n) → O(n)）
  const keyed = props.tracks.map((track) => {
    let value: string | number
    switch (field) {
      case 'title':
        value = track.title
        break
      case 'artist':
        value = track.artists[0] ?? ''
        break
      case 'album':
        value = track.album
        break
      case 'duration':
        value = track.duration
        break
    }
    return { track, value }
  })

  keyed.sort((a, b) => {
    if (typeof a.value === 'number' && typeof b.value === 'number') {
      return (a.value - b.value) * dir
    }
    return String(a.value).localeCompare(String(b.value), 'zh-Hans-CN') * dir
  })

  const result = keyed.map((k) => k.track)

  sortCache.value = {
    tracksRef: props.tracks,
    sortField: sortField.value,
    sortDir: sortDir.value,
    result,
  }

  return result
})

/**
 * 把当前播放行滚动到可见区域。
 * smooth=true 平滑滚动（切歌）；false 即时定位（启动恢复 / KeepAlive 切回）。
 * 返回当前曲目是否在本列表中。
 */
function scrollCurrentIntoView(smooth = true): boolean {
  const id = playerStore.snapshot.trackId
  if (!id) return false
  const index = sortedTracks.value.findIndex((item) => item.id === id)
  if (index < 0) return false
  // 先确保虚拟列表自身高度正确（从 KeepAlive 恢复时），再定位
  measure()
  scrollerRef.value?.scrollToItem(index, { behavior: smooth ? 'smooth' : 'auto' })
  return true
}

// 切歌（自动连播 / 托盘 / 双击列表）：定位到新曲目
watch(
  () => playerStore.snapshot.trackId,
  (id, previousId) => {
    if (!id || id === previousId) return
    void nextTick(() => scrollCurrentIntoView(true))
  },
)

// 启动恢复播放：数据可能晚于组件就绪，数据到达后定位一次即停止
const stopInitialWatch = watch(
  () => sortedTracks.value.length,
  async () => {
    await nextTick()
    if (scrollCurrentIntoView(false)) stopInitialWatch()
  },
)

// 排序变化后虚拟列表重渲染，重新测量并保持当前曲目可见
watch(sortedTracks, async () => {
  await nextTick()
  measure()
})

// 首屏解析：数据首次到达时补第一屏（约一屏行数 + 预取），保证第一屏即有真实
// 标题 / 艺人 / 专辑，而非文件名。只做一次。
const FIRST_SCREEN_ROWS = 40

// 从其他路由切回（KeepAlive 激活）：确保当前曲目可见
onActivated(() => {
  void nextTick(() => scrollCurrentIntoView(false))
  // 切回时补一屏元数据（占位期间切走可能还未解析）
  scheduleVisibleResolve(0, FIRST_SCREEN_ROWS)
})

const stopFirstScreen = watch(
  () => sortedTracks.value.length,
  (len) => {
    if (len === 0) return
    void nextTick(() => scheduleVisibleResolve(0, FIRST_SCREEN_ROWS))
    stopFirstScreen()
  },
)

const selectedSet = computed(() => new Set(props.selectedIds))
const allSelected = computed(
  () => sortedTracks.value.length > 0 && props.selectedIds.length === sortedTracks.value.length,
)
const partialSelected = computed(
  () => props.selectedIds.length > 0 && props.selectedIds.length < sortedTracks.value.length,
)

function updateSelection(ids: string[]): void {
  emit('update:selectedIds', ids)
}

function toggleAll(): void {
  if (allSelected.value) updateSelection([])
  else updateSelection(sortedTracks.value.map((track) => track.id))
}

/** Shift 连选锚点（下标基于 sortedTracks） */
const anchorIndex = ref<number | null>(null)

function toggleOne(track: Track, event: MouseEvent): void {
  const index = sortedTracks.value.findIndex((item) => item.id === track.id)
  const ids = new Set(props.selectedIds)
  if (event.shiftKey && anchorIndex.value !== null && index >= 0) {
    const [from, to] = [anchorIndex.value, index].sort((a, b) => a - b)
    for (let i = from; i <= to; i += 1) {
      const id = sortedTracks.value[i]?.id
      if (id) ids.add(id)
    }
  } else if (ids.has(track.id)) {
    ids.delete(track.id)
  } else {
    ids.add(track.id)
  }
  anchorIndex.value = index
  updateSelection([...ids])
}

function onRowClick(track: Track, event: MouseEvent): void {
  if (event.ctrlKey || event.metaKey) {
    toggleOne(track, event)
    return
  }
  if (event.shiftKey) {
    toggleOne(track, event)
    return
  }
  // 已有选择时，普通点击收敛为单选；无选择时不打断浏览
  if (props.selectedIds.length > 0 && !selectedSet.value.has(track.id)) {
    anchorIndex.value = sortedTracks.value.findIndex((item) => item.id === track.id)
    updateSelection([track.id])
  }
}

function playContextFor(): PlayContext {
  return {
    source: props.source,
    sourceId: props.sourceId,
    trackIds: sortedTracks.value.map((track) => track.id),
  }
}

function onDblPlay(track: Track): void {
  const index = sortedTracks.value.findIndex((item) => item.id === track.id)
  void playerStore.playFromContext(track.id, { ...playContextFor(), startIndex: index })
  updateSelection([])
}

function onContextMenu(track: Track, event: MouseEvent): void {
  let targets: Track[]
  if (selectedSet.value.has(track.id)) {
    targets = sortedTracks.value.filter((item) => selectedSet.value.has(item.id))
  } else {
    updateSelection([track.id])
    targets = [track]
  }
  openTrackMenu(event, targets, {
    playContext: playContextFor(),
    playlistId: props.source === 'playlist' ? (props.sourceId ?? undefined) : undefined,
  })
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key !== 'Escape' || props.selectedIds.length === 0) return
  const target = event.target as HTMLElement | null
  if (target && ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)) return
  updateSelection([])
}
</script>

<template>
  <div class="track-list" @keydown="onKeydown">
    <TrackListHeader
      :show-artist="showArtist"
      :show-album="showAlbum"
      :all-selected="allSelected"
      :partial="partialSelected"
      :sort-field="sortField"
      :sort-dir="sortDir"
      :track-count="sortedTracks.length"
      @toggle-all="toggleAll"
      @sort="onSort"
    />

    <div v-if="sortedTracks.length > 0" ref="scrollerEl" class="track-list__scroller-host">
      <RecycleScroller
        ref="scrollerRef"
        :items="sortedTracks"
        :item-size="rowHeight"
        key-field="id"
        :buffer="rowHeight * 4"
        class="track-list__scroller"
        role="grid"
        :style="{ height: `${scrollerHeight}px` }"
        @update="onVisibleUpdate"
      >
        <template #default="{ item: track, index }">
          <div class="track-list__item">
            <TrackRow
              :track="track"
              :display-index="index + 1"
              :selected="selectedSet.has(track.id)"
              :is-current="playerStore.snapshot.trackId === track.id"
              :is-playing="
                playerStore.snapshot.trackId === track.id &&
                playerStore.snapshot.status === 'playing'
              "
              :show-artist="showArtist"
              :show-album="showAlbum"
              @dblplay="onDblPlay(track)"
              @toggle-select="toggleOne"
              @row-click="onRowClick"
              @context-menu="onContextMenu"
            />
          </div>
        </template>
      </RecycleScroller>
    </div>

    <EmptyState
      v-else
      :title="emptyTitle"
      :description="emptyDescription"
    />
  </div>
</template>

<style scoped>
.track-list {
  min-height: 100%;
}

/* 虚拟滚动容器自身承担滚动（高度由 JS 按视口实时测量） */
.track-list__scroller-host {
  margin-top: var(--space-1);
  min-width: 0;
}

.track-list__item {
  list-style: none;
}

/* 虚拟行严格按密度档行高渲染，避免 item-size 与实际高度错位 */
.track-list__item :deep(.track-row) {
  height: var(--size-row-h);
}
</style>
