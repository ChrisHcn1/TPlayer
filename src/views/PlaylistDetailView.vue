<script setup lang="ts">
/**
 * 歌单详情（/playlists/:id）：歌单元信息 + 歌单曲目表。
 * 用户歌单支持重命名 / 删除 / 从歌单移除曲目；系统歌单只读。
 */
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { usePlaylistStore } from '../stores/playlist'
import { useLibraryStore } from '../stores/library'
import { usePlayerStore } from '../stores/player'
import { useDialogStore } from '../stores/dialog'
import { useToastStore } from '../stores/toast'
import { useTrackActions } from '../composables/useTrackActions'
import { useTrackSelection } from '../composables/useTrackSelection'
import type { PlayContext, Track } from '../types'
import DetailHero from '../components/library/DetailHero.vue'
import TrackList from '../components/library/TrackList.vue'
import SelectionBar from '../components/library/SelectionBar.vue'
import BaseButton from '../components/common/BaseButton.vue'
import Icon from '../components/common/Icon.vue'
import EmptyState from '../components/common/EmptyState.vue'
import { t } from '../i18n'

const route = useRoute()
const router = useRouter()
const playlistStore = usePlaylistStore()
const libraryStore = useLibraryStore()
const playerStore = usePlayerStore()
const dialogStore = useDialogStore()
const toastStore = useToastStore()
const { playTracks } = useTrackActions()

const playlistId = String(route.params.id ?? '')
const playlist = computed(() => playlistStore.byId(playlistId))

const tracks = computed<Track[]>(() =>
  playlist.value
    ? playlist.value.trackIds
        .map((id) => libraryStore.getTrack(id))
        .filter((track): track is Track => Boolean(track))
    : [],
)

const buildContext = (): PlayContext => ({
  source: 'playlist',
  sourceId: playlistId,
  trackIds: tracks.value.map((track) => track.id),
})

const { selectedIds, count, allFavorited, clear, play, favorite, addToPlaylist } = useTrackSelection(
  tracks,
  buildContext,
)

function playAll(): void {
  if (tracks.value.length === 0) {
    toastStore.warning(t('common.playlistEmpty'))
    return
  }
  playTracks(tracks.value, { playContext: buildContext() })
}

async function shuffleAll(): Promise<void> {
  if (tracks.value.length === 0) return
  await playerStore.setMode('shuffle')
  playAll()
}

/** 多选浮条的“移除”：从歌单移除（不影响曲库） */
async function removeFromPlaylist(): Promise<void> {
  if (!playlist.value || selectedIds.value.length === 0) return
  const confirmed = await dialogStore.confirm({
    title: t('playlistDetail.removeFromPlaylist'),
    message: t('playlistDetail.removeMessage', {
      name: playlist.value.name,
      count: selectedIds.value.length,
    }),
    confirmText: t('common.remove'),
    danger: true,
  })
  if (!confirmed) return
  for (const id of selectedIds.value) {
    await playlistStore.removeTrack(playlistId, id)
  }
  clear()
  toastStore.success(t('playlistDetail.removed'))
}

async function renamePlaylist(): Promise<void> {
  if (!playlist.value) return
  const name = await dialogStore.prompt({
    title: t('playlists.rename'),
    initialValue: playlist.value.name,
    placeholder: t('playlists.namePlaceholder'),
    validate: (value) => (value.trim().length === 0 ? t('playlists.nameRequired') : null),
  })
  if (!name || name === playlist.value.name) return
  await playlistStore.rename(playlistId, name)
  toastStore.success(t('playlists.renamed'))
}

async function deletePlaylist(): Promise<void> {
  if (!playlist.value) return
  const confirmed = await dialogStore.confirm({
    title: t('playlists.delete'),
    message: t('playlists.deleteMessage', { name: playlist.value.name }),
    confirmText: t('common.delete'),
    danger: true,
  })
  if (!confirmed) return
  await playlistStore.remove(playlistId)
  toastStore.success(t('playlists.deleted'))
  router.push('/playlists')
}
</script>

<template>
  <section class="view">
    <template v-if="playlist">
      <DetailHero
        icon="list-music"
        :title="playlist.name"
        :subtitle="playlist.description || (playlist.isSystem ? t('playlistDetail.systemReadonly') : '')"
        :meta="t('common.trackCount', { count: tracks.length })"
        :badge="playlist.isSystem ? t('playlists.systemBadge') : ''"
      >
        <template #actions>
          <BaseButton size="md" variant="primary" @click="playAll">
            <Icon name="play" :size="15" />
            {{ t('common.play') }}
          </BaseButton>
          <BaseButton size="md" variant="subtle" :title="t('common.shuffle')" @click="shuffleAll">
            <Icon name="shuffle" :size="15" />
          </BaseButton>
          <template v-if="!playlist.isSystem">
            <BaseButton size="md" variant="ghost" :title="t('playlistDetail.rename')" @click="renamePlaylist">
              <Icon name="edit" :size="16" />
            </BaseButton>
            <BaseButton size="md" variant="ghost" :title="t('playlists.delete')" @click="deletePlaylist">
              <Icon name="trash-2" :size="16" />
            </BaseButton>
          </template>
        </template>
      </DetailHero>

      <TrackList
        :tracks="tracks"
        source="playlist"
        :source-id="playlistId"
        :show-artist="true"
        v-model:selected-ids="selectedIds"
        :empty-title="t('common.playlistEmpty')"
        :empty-description="t('playlistDetail.emptyDescription')"
      />
      <SelectionBar
        v-if="!playlist.isSystem"
        :count="count"
        :all-favorited="allFavorited"
        :remove-title="t('playlistDetail.removeFromPlaylist')"
        @play="play"
        @add-playlist="addToPlaylist"
        @favorite="favorite"
        @remove="removeFromPlaylist"
        @clear="clear"
      />
    </template>

    <EmptyState
      v-else
      icon="list-music"
      :title="t('playlistDetail.notFound')"
      :description="t('playlistDetail.notFoundDescription')"
    >
      <BaseButton size="sm" variant="subtle" @click="router.push('/playlists')">
        <Icon name="back" :size="14" />
        {{ t('playlistDetail.backToPlaylists') }}
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
