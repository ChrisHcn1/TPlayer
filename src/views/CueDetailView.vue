<script setup lang="ts">
/**
 * 分轨专辑详情（/cue/:id）：整盘信息 + 分轨曲目表（保留艺术家列，整盘可能多人）。
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

const cueId = String(route.params.id ?? '')
const album = computed(() => libraryStore.cueAlbums.find((item) => item.id === cueId))
const tracks = computed(() => libraryStore.tracksOfAlbum(cueId))
const { coverUrl } = useGroupCover(() => cueId)

const buildContext = (): PlayContext => ({
  source: 'album',
  sourceId: cueId,
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
        icon="layers"
        badge="CUE"
        :cover-url="coverUrl"
        :title="album.name"
        :subtitle="album.artists.join(' / ')"
        :meta="`${tracks.length} 个分轨`"
      >
        <template #actions>
          <BaseButton size="md" variant="primary" @click="playAll">
            <Icon name="play" :size="15" />
            播放整盘
          </BaseButton>
          <BaseButton size="md" variant="subtle" title="随机播放" @click="shuffleAll">
            <Icon name="shuffle" :size="15" />
          </BaseButton>
        </template>
      </DetailHero>

      <TrackList
        :tracks="tracks"
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

    <EmptyState v-else icon="layers" title="分轨专辑不存在" description="它可能已被移除，请返回分轨列表">
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
</style>
