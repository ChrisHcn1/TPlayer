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
    toastStore.warning('歌单还是空的')
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
    title: '从歌单移除',
    message: `确定从「${playlist.value.name}」移除选中的 ${selectedIds.value.length} 首吗？`,
    confirmText: '移除',
    danger: true,
  })
  if (!confirmed) return
  for (const id of selectedIds.value) {
    await playlistStore.removeTrack(playlistId, id)
  }
  clear()
  toastStore.success('已从歌单移除')
}

async function renamePlaylist(): Promise<void> {
  if (!playlist.value) return
  const name = await dialogStore.prompt({
    title: '重命名歌单',
    initialValue: playlist.value.name,
    placeholder: '歌单名称',
    validate: (value) => (value.trim().length === 0 ? '请输入歌单名称' : null),
  })
  if (!name || name === playlist.value.name) return
  await playlistStore.rename(playlistId, name)
  toastStore.success('歌单已重命名')
}

async function deletePlaylist(): Promise<void> {
  if (!playlist.value) return
  const confirmed = await dialogStore.confirm({
    title: '删除歌单',
    message: `确定删除歌单「${playlist.value.name}」吗？歌单内曲目不会从曲库移除。`,
    confirmText: '删除',
    danger: true,
  })
  if (!confirmed) return
  await playlistStore.remove(playlistId)
  toastStore.success('歌单已删除')
  router.push('/playlists')
}
</script>

<template>
  <section class="view">
    <template v-if="playlist">
      <DetailHero
        icon="list-music"
        :title="playlist.name"
        :subtitle="playlist.description || (playlist.isSystem ? '系统歌单（只读）' : '')"
        :meta="`${tracks.length} 首`"
        :badge="playlist.isSystem ? '系统' : ''"
      >
        <template #actions>
          <BaseButton size="md" variant="primary" @click="playAll">
            <Icon name="play" :size="15" />
            播放
          </BaseButton>
          <BaseButton size="md" variant="subtle" title="随机播放" @click="shuffleAll">
            <Icon name="shuffle" :size="15" />
          </BaseButton>
          <template v-if="!playlist.isSystem">
            <BaseButton size="md" variant="ghost" title="重命名" @click="renamePlaylist">
              <Icon name="edit" :size="16" />
            </BaseButton>
            <BaseButton size="md" variant="ghost" title="删除歌单" @click="deletePlaylist">
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
        empty-title="歌单还是空的"
        empty-description="在曲目右键菜单选择“添加到歌单”即可加入"
      />
      <SelectionBar
        v-if="!playlist.isSystem"
        :count="count"
        :all-favorited="allFavorited"
        remove-title="从歌单移除"
        @play="play"
        @add-playlist="addToPlaylist"
        @favorite="favorite"
        @remove="removeFromPlaylist"
        @clear="clear"
      />
    </template>

    <EmptyState v-else icon="list-music" title="歌单不存在" description="它可能已被删除">
      <BaseButton size="sm" variant="subtle" @click="router.push('/playlists')">
        <Icon name="back" :size="14" />
        返回歌单
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
