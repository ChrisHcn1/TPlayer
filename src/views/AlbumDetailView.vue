<script setup lang="ts">
/**
 * 专辑详情（/albums/:id）：专辑信息 + 该专辑曲目表（隐藏艺术家列）。
 */
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useLibraryStore } from '../stores/library'
import { usePlayerStore } from '../stores/player'
import { useTrackActions } from '../composables/useTrackActions'
import { useTrackSelection } from '../composables/useTrackSelection'
import { useGroupCover } from '../composables/useTrackCover'
import type { PlayContext } from '../types'
import DetailHero from '../components/library/DetailHero.vue'
import TrackList from '../components/library/TrackList.vue'
import SelectionBar from '../components/library/SelectionBar.vue'
import BaseButton from '../components/common/BaseButton.vue'
import Icon from '../components/common/Icon.vue'
import EmptyState from '../components/common/EmptyState.vue'

function formatTotal(seconds: number): string {
  const minutes = Math.round(seconds / 60)
  return minutes >= 60
    ? `${Math.floor(minutes / 60)} 小时 ${minutes % 60} 分钟`
    : `${minutes} 分钟`
}

const route = useRoute()
const router = useRouter()
const libraryStore = useLibraryStore()
const playerStore = usePlayerStore()
const { playTracks } = useTrackActions()

const albumId = String(route.params.id ?? '')
const album = computed(() => libraryStore.albumById(albumId))
const tracks = computed(() => libraryStore.tracksOfAlbum(albumId))
// 头部大封面：专辑聚合封面（albumId 原文即带 album: 前缀；组内回退 + 后端两级缓存）
const { coverUrl } = useGroupCover(() => albumId)

const buildContext = (): PlayContext => ({
  source: 'album',
  sourceId: albumId,
  trackIds: tracks.value.map((track) => track.id),
})

const { selectedIds, count, allFavorited, clear, play, favorite, remove, addToPlaylist } =
  useTrackSelection(tracks, buildContext)

function playAll(): void {
  playTracks(tracks.value, { playContext: buildContext() })
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
        icon="disc"
        :cover-url="coverUrl"
        :title="album.name"
        :subtitle="album.artists.join(' / ')"
        :meta="`${album.year ?? '未知年份'} · ${tracks.length} 首 · ${formatTotal(tracks.reduce((sum, t) => sum + t.duration, 0))}`"
      >
        <template #actions>
          <BaseButton size="md" variant="primary" @click="playAll">
            <Icon name="play" :size="15" />
            播放
          </BaseButton>
          <BaseButton size="md" variant="subtle" title="随机播放整张专辑" @click="shuffleAll">
            <Icon name="shuffle" :size="15" />
          </BaseButton>
        </template>
      </DetailHero>

      <TrackList
        :tracks="tracks"
        source="album"
        :source-id="albumId"
        :show-artist="false"
        v-model:selected-ids="selectedIds"
        empty-title="专辑暂无曲目"
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

    <EmptyState v-else icon="disc" title="专辑不存在" description="它可能已被移除，请返回专辑列表">
      <BaseButton size="sm" variant="subtle" @click="router.push('/albums')">
        <Icon name="back" :size="14" />
        返回专辑
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
</style>
