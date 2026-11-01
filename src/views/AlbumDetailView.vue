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
import { t } from '../i18n'

function formatTotal(seconds: number): string {
  const minutes = Math.round(seconds / 60)
  return minutes >= 60
    ? t('albumDetail.hoursMinutes', { hours: Math.floor(minutes / 60), minutes: minutes % 60 })
    : t('albumDetail.minutes', { minutes })
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
        :meta="`${album.year ?? t('albumDetail.unknownYear')} · ${t('common.trackCount', { count: tracks.length })} · ${formatTotal(tracks.reduce((sum, t) => sum + t.duration, 0))}`"
      >
        <template #actions>
          <BaseButton size="md" variant="primary" @click="playAll">
            <Icon name="play" :size="15" />
            {{ t('common.play') }}
          </BaseButton>
          <BaseButton size="md" variant="subtle" :title="t('albumDetail.shuffleTitle')" @click="shuffleAll">
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
        :empty-title="t('albumDetail.emptyTracks')"
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

    <EmptyState
      v-else
      icon="disc"
      :title="t('albumDetail.notFound')"
      :description="t('albumDetail.notFoundDescription')"
    >
      <BaseButton size="sm" variant="subtle" @click="router.push('/albums')">
        <Icon name="back" :size="14" />
        {{ t('albumDetail.backToAlbums') }}
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
