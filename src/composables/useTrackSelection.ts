/**
 * 列表选择与批量动作绑定（DESIGN §4.8）
 *
 * 各曲目型视图共用：传入“当前展示曲目”的响应式引用与播放上下文构造器，
 * 返回 v-model 所需的 selectedIds 与 SelectionBar 的全部动作。
 */
import { computed, ref, type ComputedRef, type Ref } from 'vue'

import type { PlayContext, Track } from '../types'
import { useLibraryStore } from '../stores/library'
import { useTrackActions } from './useTrackActions'

export function useTrackSelection(
  displayedTracks: Ref<Track[]> | ComputedRef<Track[]>,
  buildContext: () => PlayContext,
) {
  const libraryStore = useLibraryStore()
  const { playTracks, toggleFavorite, removeFromLibrary, openAddToPlaylistMenu } =
    useTrackActions()

  const selectedIds = ref<string[]>([])

  const selectedTracks = computed(() => {
    const ids = new Set(selectedIds.value)
    return displayedTracks.value.filter((track) => ids.has(track.id))
  })

  const count = computed(() => selectedIds.value.length)

  const allFavorited = computed(
    () =>
      selectedTracks.value.length > 0 &&
      selectedTracks.value.every((track) => libraryStore.isFavorite(track.id)),
  )

  function clear(): void {
    selectedIds.value = []
  }

  function play(): void {
    if (selectedTracks.value.length === 0) return
    playTracks(selectedTracks.value, { playContext: buildContext() })
    clear()
  }

  function favorite(): void {
    void toggleFavorite(selectedTracks.value)
  }

  async function remove(): Promise<void> {
    if (selectedTracks.value.length === 0) return
    await removeFromLibrary(selectedTracks.value)
    clear()
  }

  function addToPlaylist(event: MouseEvent): void {
    openAddToPlaylistMenu(event, selectedTracks.value)
  }

  return {
    selectedIds,
    selectedTracks,
    count,
    allFavorited,
    clear,
    play,
    favorite,
    remove,
    addToPlaylist,
  }
}
