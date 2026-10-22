<script setup lang="ts">
/**
 * 分轨专辑详情（/cue/:id）：CUE 整盘与 SACD ISO 共用此详情。
 *
 * 两者均为"整盘镜像拆出的虚拟多轨专辑"，UI 同构；仅角标 / 图标与文案按
 * 专辑类型（isSacd）区分，播放链路由引擎按 is_cue / is_sacd 自动路由。
 */
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useLibraryStore } from '../stores/library'
import { usePlayerStore } from '../stores/player'
import { useTrackActions } from '../composables/useTrackActions'
import { useTrackSelection } from '../composables/useTrackSelection'
import { useGroupCover } from '../composables/useTrackCover'
import type { PlayContext, Track } from '../types'
import DetailHero from '../components/library/DetailHero.vue'
import TrackList from '../components/library/TrackList.vue'
import SelectionBar from '../components/library/SelectionBar.vue'
import BaseButton from '../components/common/BaseButton.vue'
import Icon from '../components/common/Icon.vue'
import EmptyState from '../components/common/EmptyState.vue'

const route = useRoute()
const router = useRouter()
const libraryStore = useLibraryStore()
const playerStore = usePlayerStore()
const { playTracks } = useTrackActions()

const cueId = String(route.params.id ?? '')
// CUE 与 SACD 虚拟专辑统一走 /cue/:id，这里在两集合里查找
const album = computed(
  () =>
    libraryStore.cueAlbums.find((item) => item.id === cueId) ??
    libraryStore.sacdAlbums.find((item) => item.id === cueId),
)
/** 专辑全部虚拟分轨（SACD 含立体声 + 5.1 两套，同曲目一一对应） */
const tracks = computed(() => libraryStore.tracksOfAlbum(cueId))
const { coverUrl } = useGroupCover(() => cueId)

const isSacd = computed(() => album.value?.isSacd === true)
const badge = computed(() => (isSacd.value ? 'SACD' : 'CUE'))
const heroIcon = computed(() => (isSacd.value ? 'disc' : 'layers'))

/**
 * SACD 区域切换：ISO 同时含 2ch 立体声区与 5.1 环绕区，
 * 两套曲目标题只差 `[5.1]` 后缀，混在同一列表里按标题排序会交错成
 * "Argentina / Argentina [5.1] / Comfort Zone / …"，看起来像重复加载。
 * 这里默认只显示立体声区，按需切到 5.1，列表与「播放整盘」都只作用于当前区。
 */
const areaTab = ref<'stereo' | 'multich'>('stereo')
const isMultichTrack = (t: Track): boolean =>
  String(t.sacdArea ?? '').toLowerCase() === 'multich'
const hasMultich = computed(() => tracks.value.some(isMultichTrack))
const displayTracks = computed(() => {
  if (!isSacd.value || !hasMultich.value) return tracks.value
  return tracks.value.filter((t) => isMultichTrack(t) === (areaTab.value === 'multich'))
})
const multichCount = computed(
  () => tracks.value.filter(isMultichTrack).length,
)
const heroMeta = computed(() => {
  if (!isSacd.value || !hasMultich.value) return `${tracks.value.length} 个分轨`
  return `${displayTracks.value.length} 个分轨 · ${areaTab.value === 'multich' ? '5.1 环绕' : '立体声'}`
})

const buildContext = (): PlayContext => ({
  source: 'album',
  sourceId: cueId,
  trackIds: displayTracks.value.map((track) => track.id),
})

const { selectedIds, count, allFavorited, clear, play, favorite, remove, addToPlaylist } =
  useTrackSelection(displayTracks, buildContext)

function playAll(): void {
  playTracks(displayTracks.value, { playContext: buildContext() })
}

async function shuffleAll(): Promise<void> {
  await playerStore.setMode('shuffle')
  playAll()
}
</script>

<template>
  <section class="view">
    <template v-if="album">
      <DetailHero
        :icon="heroIcon"
        :badge="badge"
        :cover-url="coverUrl"
        :title="album.name"
        :subtitle="album.artists.join(' / ')"
        :meta="heroMeta"
      >
        <template #actions>
          <BaseButton size="md" variant="primary" @click="playAll">
            <Icon name="play" :size="15" />
            播放整盘
          </BaseButton>
          <BaseButton size="md" variant="subtle" title="随机播放" @click="shuffleAll">
            <Icon name="shuffle" :size="15" />
          </BaseButton>
          <div v-if="hasMultich" class="area-switch" role="tablist" aria-label="音轨版本">
            <button
              type="button"
              role="tab"
              :aria-selected="areaTab === 'stereo'"
              :class="{ active: areaTab === 'stereo' }"
              @click="areaTab = 'stereo'"
            >
              立体声
            </button>
            <button
              type="button"
              role="tab"
              :aria-selected="areaTab === 'multich'"
              :class="{ active: areaTab === 'multich' }"
              @click="areaTab = 'multich'"
            >
              5.1 环绕
              <span class="area-switch__count">{{ multichCount }}</span>
            </button>
          </div>
        </template>
      </DetailHero>

      <TrackList
        :tracks="displayTracks"
        source="album"
        :source-id="cueId"
        v-model:selected-ids="selectedIds"
        empty-title="整盘暂无分轨"
      />
      <SelectionBar
        :count="count"
        :all-favorited="allFavorited"
        @play="play"
        @add-playlist="addToPlaylist"
        @favorite="favorite"
        @remove="remove"
        @clear="clear"
      />
    </template>

    <EmptyState v-else :icon="heroIcon" title="分轨专辑不存在" description="它可能已被移除，请返回分轨列表">
      <BaseButton size="sm" variant="subtle" @click="router.push('/cue')">
        <Icon name="back" :size="14" />
        返回分轨专辑
      </BaseButton>
    </EmptyState>
  </section>
</template>

<style scoped>
.view {
  display: flex;
  flex-direction: column;
  min-height: 100%;
  color: var(--color-text-primary);
}

/* SACD 区域切换（立体声 / 5.1 环绕） */
.area-switch {
  display: inline-flex;
  align-items: center;
  padding: 3px;
  gap: 2px;
  border-radius: 999px;
  background: var(--color-bg-elevated, rgba(255, 255, 255, 0.06));
  border: 1px solid var(--color-border-subtle, rgba(255, 255, 255, 0.08));
}

.area-switch button {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 5px 12px;
  border: 0;
  border-radius: 999px;
  background: transparent;
  color: var(--color-text-secondary);
  font-size: 12.5px;
  line-height: 1.2;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}

.area-switch button:hover {
  color: var(--color-text-primary);
}

.area-switch button.active {
  background: var(--color-bg-hover, rgba(255, 255, 255, 0.12));
  color: var(--color-text-primary);
  font-weight: 600;
}

.area-switch__count {
  font-size: 11px;
  opacity: 0.6;
}
</style>
