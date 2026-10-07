<script setup lang="ts">
/**
 * 全局命令浮层（DESIGN §2.4）：Ctrl/⌘+K 唤起。
 * 分组：曲目 / 专辑（含分轨专辑）/ 艺术家 / 歌单 / 快捷操作；
 * 上下键选择、Enter 执行、Esc 关闭；无命中时 Enter 进入搜索结果页。
 */
import { computed, nextTick, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import { useRouter } from 'vue-router'

import { useCommandPalette } from '../../composables/useCommandPalette'
import { useTheme } from '../../composables/useTheme'
import { useLibraryStore, matchTrackKeyword } from '../../stores/library'
import { usePlaylistStore } from '../../stores/playlist'
import { usePlayerStore } from '../../stores/player'
import type { Album, Artist, Playlist, Track } from '../../types'
import Icon from './Icon.vue'

interface PaletteEntry {
  key: string
  group: string
  label: string
  hint: string
  icon: string
  run: () => void
}

const router = useRouter()
const libraryStore = useLibraryStore()
const playlistStore = usePlaylistStore()
const playerStore = usePlayerStore()
const { theme, toggleTheme } = useTheme()
const { isOpen, close } = useCommandPalette()
const { tracks, albums, cueAlbums, artists } = storeToRefs(libraryStore)
const { playlists } = storeToRefs(playlistStore)

const query = ref('')
const activeIndex = ref(0)
const inputRef = ref<HTMLInputElement | null>(null)

function includes(text: string, key: string): boolean {
  return text.toLowerCase().includes(key)
}

const groups = computed<{ title: string; entries: PaletteEntry[] }[]>(() => {
  const key = query.value.trim().toLowerCase()
  const result: { title: string; entries: PaletteEntry[] }[] = []

  // ---- 曲目 ----
  const matchedTracks = (
    key ? tracks.value.filter((track) => matchTrackKeyword(track, key)) : tracks.value
  ).slice(0, 6)
  if (matchedTracks.length > 0) {
    const matchedIds = matchedTracks.map((track) => track.id)
    result.push({
      title: '曲目',
      entries: matchedTracks.map((track: Track) => ({
        key: `track-${track.id}`,
        group: '曲目',
        label: track.title,
        hint: `${track.artists.join(' / ')} · ${track.album}`,
        icon: 'music-note',
        run: () => {
          void playerStore.playFromContext(track.id, {
            source: 'search',
            sourceId: null,
            trackIds: matchedIds,
          })
        },
      })),
    })
  }

  // ---- 专辑（普通 + 分轨） ----
  const albumEntries: PaletteEntry[] = []
  const pushAlbum = (album: Album, base: string, icon: string) => {
    if (key && !includes(`${album.name} ${album.artists.join(' ')}`, key)) return
    albumEntries.push({
      key: `${base}-${album.id}`,
      group: '专辑',
      label: album.name,
      hint: album.artists.join(' / '),
      icon,
      run: () => {
        if (base === 'cue') router.push(`/cue/${album.id}`)
        else router.push(`/albums/${album.id}`)
      },
    })
  }
  albums.value.slice(0, 4).forEach((album) => pushAlbum(album, 'album', 'disc'))
  cueAlbums.value.slice(0, 3).forEach((album) => pushAlbum(album, 'cue', 'layers'))
  if (albumEntries.length > 0) result.push({ title: '专辑', entries: albumEntries.slice(0, 6) })

  // ---- 艺术家 ----
  const matchedArtists = (key
    ? artists.value.filter((artist) => includes(artist.name, key))
    : artists.value
  ).slice(0, 4)
  if (matchedArtists.length > 0) {
    result.push({
      title: '艺术家',
      entries: matchedArtists.map((artist: Artist) => ({
        key: `artist-${artist.name}`,
        group: '艺术家',
        label: artist.name,
        hint: `${artist.trackCount ?? 0} 首`,
        icon: 'user',
        run: () => router.push(`/artists/${encodeURIComponent(artist.name)}`),
      })),
    })
  }

  // ---- 歌单 ----
  const matchedPlaylists = (key
    ? playlists.value.filter((playlist) => includes(playlist.name, key))
    : playlists.value
  ).slice(0, 4)
  if (matchedPlaylists.length > 0) {
    result.push({
      title: '歌单',
      entries: matchedPlaylists.map((playlist: Playlist) => ({
        key: `playlist-${playlist.id}`,
        group: '歌单',
        label: playlist.name,
        hint: `${playlist.trackIds.length} 首`,
        icon: 'list-music',
        run: () => router.push(`/playlists/${playlist.id}`),
      })),
    })
  }

  // ---- 快捷操作（无关键词时展示常用项；有关键词时按名称过滤） ----
  const actions: PaletteEntry[] = [
    { key: 'go-songs', group: '操作', label: '前往：歌曲', hint: '', icon: 'music-note', run: () => router.push('/songs') },
    { key: 'go-recent', group: '操作', label: '前往：最近播放', hint: '', icon: 'clock', run: () => router.push('/recent') },
    { key: 'go-favorites', group: '操作', label: '前往：我的收藏', hint: '', icon: 'heart', run: () => router.push('/favorites') },
    { key: 'go-converter', group: '操作', label: '前往：格式转换', hint: '', icon: 'swap', run: () => router.push('/converter') },
    { key: 'go-settings', group: '操作', label: '前往：设置', hint: '', icon: 'settings', run: () => router.push('/settings') },
    {
      key: 'toggle-theme',
      group: '操作',
      label: theme.value === 'dark' ? '切换到浅色主题' : '切换到深色主题',
      hint: '',
      icon: theme.value === 'dark' ? 'sun' : 'moon',
      run: () => toggleTheme(),
    },
  ].filter((entry) => !key || includes(entry.label, key))
  if (actions.length > 0) result.push({ title: '快捷操作', entries: actions })

  return result
})

const flatEntries = computed(() => groups.value.flatMap((group) => group.entries))

watch(
  () => isOpen.value,
  async (open) => {
    if (!open) return
    query.value = ''
    activeIndex.value = 0
    await nextTick()
    inputRef.value?.focus()
  },
)

watch(query, () => {
  activeIndex.value = 0
})

function execute(entry: PaletteEntry | undefined): void {
  close()
  if (entry) {
    entry.run()
    return
  }
  // 无直接命中：进入搜索结果页
  const q = query.value.trim()
  if (q) router.push({ path: '/search', query: { q } })
}

function onKeydown(event: KeyboardEvent): void {
  const total = flatEntries.value.length
  if (event.key === 'Escape') {
    event.preventDefault()
    close()
  } else if (event.key === 'ArrowDown') {
    event.preventDefault()
    if (total > 0) activeIndex.value = (activeIndex.value + 1) % total
  } else if (event.key === 'ArrowUp') {
    event.preventDefault()
    if (total > 0) activeIndex.value = (activeIndex.value - 1 + total) % total
  } else if (event.key === 'Enter') {
    event.preventDefault()
    execute(flatEntries.value[activeIndex.value])
  }
}

/** 组内首个条目的全局下标，用于滚动定位与高亮判定 */
function globalIndex(groupIndex: number, entryIndex: number): number {
  let offset = 0
  for (let i = 0; i < groupIndex; i += 1) offset += groups.value[i]?.entries.length ?? 0
  return offset + entryIndex
}
</script>

<template>
  <Teleport to="body">
    <Transition name="palette">
      <div v-if="isOpen" class="palette-overlay" @mousedown.self="close">
        <div class="palette" role="dialog" aria-modal="true" aria-label="全局搜索">
          <div class="palette__input-row">
            <Icon name="search" :size="18" class="palette__input-icon" />
            <input
              ref="inputRef"
              v-model="query"
              class="palette__input"
              type="text"
              placeholder="搜索曲目、专辑、艺术家、歌单…"
              @keydown="onKeydown"
            />
            <kbd class="palette__kbd">Esc</kbd>
          </div>

          <div class="palette__results u-scroll">
            <template v-if="flatEntries.length > 0">
              <section v-for="(group, groupIndex) in groups" :key="group.title" class="palette__group">
                <h3 class="palette__group-title">{{ group.title }}</h3>
                <ul>
                  <li v-for="(entry, entryIndex) in group.entries" :key="entry.key">
                    <button
                      type="button"
                      class="palette__entry"
                      :class="{ 'palette__entry--active': activeIndex === globalIndex(groupIndex, entryIndex) }"
                      @mouseenter="activeIndex = globalIndex(groupIndex, entryIndex)"
                      @click="execute(entry)"
                    >
                      <Icon :name="entry.icon" :size="16" class="palette__entry-icon" />
                      <span class="palette__entry-label">{{ entry.label }}</span>
                      <span v-if="entry.hint" class="palette__entry-hint">{{ entry.hint }}</span>
                    </button>
                  </li>
                </ul>
              </section>
            </template>
            <div v-else class="palette__empty">
              <Icon name="search" :size="20" />
              <p>无匹配结果，按 Enter 查看搜索页</p>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.palette-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal);
  display: flex;
  justify-content: center;
  padding: 12vh var(--space-6) var(--space-6);
  background-color: var(--color-overlay);
  align-items: flex-start;
}

.palette {
  width: 640px;
  max-width: 100%;
  max-height: 60vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background-color: var(--color-bg-elevated);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-elevation-4);
}

.palette__input-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex: 0 0 auto;
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--color-border-subtle);
}

.palette__input-icon {
  flex: 0 0 auto;
  color: var(--color-text-tertiary);
}

.palette__input {
  flex: 1 1 auto;
  min-width: 0;
  font-size: var(--font-size-md);
  color: var(--color-text-primary);
}

.palette__kbd {
  flex: 0 0 auto;
  padding: 2px var(--space-2);
  font-size: var(--font-size-2xs, 11px);
  color: var(--color-text-tertiary);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-xs);
}

.palette__results {
  flex: 1 1 auto;
  min-height: 0;
  padding: var(--space-2);
}

.palette__group + .palette__group {
  margin-top: var(--space-2);
}

.palette__group-title {
  padding: var(--space-1) var(--space-2);
  font-size: var(--font-size-xs);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-tertiary);
}

.palette__entry {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  min-height: 38px;
  padding: 0 var(--space-2);
  text-align: left;
  border-radius: var(--radius-sm);
}

.palette__entry--active {
  background-color: var(--color-bg-selected);
}

.palette__entry-icon {
  flex: 0 0 auto;
  color: var(--color-text-secondary);
}

.palette__entry-label {
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--font-size-sm);
  color: var(--color-text-primary);
}

.palette__entry-hint {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: right;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

.palette__empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-8) 0;
  color: var(--color-text-tertiary);
  font-size: var(--font-size-sm);
}

.palette-enter-active,
.palette-leave-active {
  transition: opacity 0.12s ease;
}

.palette-enter-active .palette,
.palette-leave-active .palette {
  transition:
    opacity 0.16s var(--ease-emphasized, cubic-bezier(0.2, 0, 0, 1)),
    transform 0.16s var(--ease-emphasized, cubic-bezier(0.2, 0, 0, 1));
}

.palette-enter-from,
.palette-leave-to {
  opacity: 0;
}

.palette-enter-from .palette,
.palette-leave-to .palette {
  opacity: 0;
  transform: translateY(-8px) scale(0.99);
}
</style>
