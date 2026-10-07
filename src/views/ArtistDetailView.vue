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
        :meta="`${artist.trackCount} 首 · ${artist.albumCount} 张专辑`"
      >
        <template #actions>
          <BaseButton size="md" variant="primary" @click="playAll">
            <Icon name="play" :size="15" />
            播放
          </BaseButton>
          <BaseButton size="md" variant="subtle" title="随机播放" @click="shuffleAll">
            <Icon name="shuffle" :size="15" />
          </BaseButton>
        </template>
      </DetailHero>

      <TrackList
        :tracks="tracks"
        source="artist"
        :source-id="artistName"
        v-model:selected-ids="selectedIds"
        empty-title="艺术家暂无曲目"
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

    <EmptyState v-else icon="user" title="艺术家不存在" description="请返回艺术家列表重新选择">
      <BaseButton size="sm" variant="subtle" @click="router.push('/artists')">
        <Icon name="back" :size="14" />
        返回艺术家
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
