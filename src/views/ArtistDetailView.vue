<script setup lang="ts">
/**
 * 艺术家详情（/artists/:id，id 为 encodeURIComponent(name)）：
 * 艺术家信息 + 该艺术家的全部曲目（保留专辑列）。
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

const route = useRoute()
const router = useRouter()
const libraryStore = useLibraryStore()
const playerStore = usePlayerStore()
const { playTracks } = useTrackActions()

const artistName = decodeURIComponent(String(route.params.id ?? ''))
const artist = computed(() => libraryStore.artistByName(artistName))
const tracks = computed(() => libraryStore.tracksOfArtist(artistName))
// 头部大封面：艺术家聚合封面（名下曲目专辑封面回退，后端两级缓存）
const { coverUrl } = useGroupCover(() => `artist:${artistName}`)

const buildContext = (): PlayContext => ({
  source: 'artist',
  sourceId: artistName,
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
    <template v-if="artist">
      <DetailHero
        icon="user"
        :cover-url="coverUrl"
        :title="artist.name"
        :meta="t('common.artistMeta', { tracks: artist.trackCount, albums: artist.albumCount })"
      >
        <template #actions>
          <BaseButton size="md" variant="primary" @click="playAll">
            <Icon name="play" :size="15" />
            {{ t('common.play') }}
          </BaseButton>
          <BaseButton size="md" variant="subtle" :title="t('common.shuffle')" @click="shuffleAll">
            <Icon name="shuffle" :size="15" />
          </BaseButton>
        </template>
      </DetailHero>

      <TrackList
        :tracks="tracks"
        source="artist"
        :source-id="artistName"
        v-model:selected-ids="selectedIds"
        :empty-title="t('artistDetail.emptyTracks')"
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
      icon="user"
      :title="t('artistDetail.notFound')"
      :description="t('artistDetail.notFoundDescription')"
    >
      <BaseButton size="sm" variant="subtle" @click="router.push('/artists')">
        <Icon name="back" :size="14" />
        {{ t('artistDetail.backToArtists') }}
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
