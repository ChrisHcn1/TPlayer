<template>
  <div class="online-match-modal" @click.self="$emit('close')">
    <div class="modal-content">
      <div class="modal-header">
        <h2>{{ t('modal.onlineMatch') }}</h2>
        <button class="close-btn" @click="$emit('close')">×</button>
      </div>

      <!-- 搜索框 -->
      <div class="search-section">
        <input
          v-model="searchKeyword"
          :placeholder="t('modal.searchPlaceholder')"
          @keyup.enter="handleSearch"
          class="search-input"
        />
        <button @click="handleSearch" :disabled="searching" class="search-btn">
          {{ searching ? t('modal.searching') : t('common.search') }}
        </button>
      </div>

      <!-- 搜索结果 -->
      <div v-if="searchResults.length > 0" class="search-results">
        <div
          v-for="result in searchResults"
          :key="result.id"
          :class="['result-item', { selected: selectedResult?.id === result.id }]"
          @click="selectedResult = result"
        >
          <img :src="result.coverUrl" class="result-cover" :alt="result.title" @error="handleCoverError" />
          <div class="result-info">
            <div class="result-title">{{ result.title }}</div>
            <div class="result-artist">{{ result.artist }}</div>
            <div class="result-album">{{ result.album }}</div>
          </div>
          <div class="result-source">{{ getSourceText(result.source) }}</div>
        </div>
      </div>

      <!-- 空状态 -->
      <div v-else-if="searched && searchResults.length === 0" class="empty-state">
        {{ t('modal.noResults') }}
      </div>

      <!-- 操作按钮 -->
      <div class="action-buttons" v-if="selectedResult">
        <button @click="handleGetLyrics" :disabled="gettingLyric" class="action-btn">
          {{ gettingLyric ? t('modal.getting') : t('modal.getLyrics') }}
        </button>
        <button @click="handleGetCover" :disabled="gettingCover" class="action-btn">
          {{ gettingCover ? t('modal.getting') : t('modal.getCover') }}
        </button>
        <button @click="handleApplyAll" :disabled="!hasDataToApply" class="action-btn primary">
          {{ t('modal.applyAll') }}
        </button>
      </div>

      <!-- 预览区域 -->
      <div class="preview-section" v-if="lyricPreview || coverPreview">
        <div v-if="lyricPreview" class="lyric-preview">
          <h3>{{ t('modal.lyricPreview') }}</h3>
          <div class="lyric-content">{{ lyricPreview }}</div>
        </div>
        <div v-if="coverPreview" class="cover-preview">
          <h3>{{ t('modal.coverPreview') }}</h3>
          <img :src="coverPreview" :alt="t('modal.cover')" @error="handleCoverError" />
        </div>
      </div>

      <!-- 底部按钮 -->
      <div class="modal-footer">
        <button @click="$emit('close')" class="btn secondary">{{ t('common.cancel') }}</button>
        <button @click="handleConfirm" :disabled="!selectedResult" class="btn primary">
          {{ t('common.confirm') }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { t } from '../services/i18n'
import { onlineMusicService, type OnlineSong, type LyricResult } from '../services/onlineMusicService'

interface Props {
  currentTitle?: string
  currentArtist?: string
}

const props = withDefaults(defineProps<Props>(), {
  currentTitle: '',
  currentArtist: ''
})

const emit = defineEmits<{
  close: []
  apply: [data: { title?: string; artist?: string; album?: string; lyric?: string; coverUrl?: string }]
}>()

const searchKeyword = ref('')
const searching = ref(false)
const searchResults = ref<OnlineSong[]>([])
const selectedResult = ref<OnlineSong | null>(null)
const searched = ref(false)

const gettingLyric = ref(false)
const gettingCover = ref(false)
const lyricPreview = ref('')
const coverPreview = ref('')

const lyricData = ref<LyricResult | null>(null)
const hasDataToApply = computed(() => {
  return lyricData.value || coverPreview.value || selectedResult.value
})

// 初始化时自动搜索
if (props.currentTitle || props.currentArtist) {
  searchKeyword.value = `${props.currentTitle} ${props.currentArtist}`.trim()
  handleSearch()
}

function handleCoverError(e: Event) {
  const img = e.target as HTMLImageElement
  img.src = '/logo.png' // 使用默认图片
}

async function handleSearch() {
  if (!searchKeyword.value.trim()) return
  
  searching.value = true
  searched.value = false
  try {
    searchResults.value = await onlineMusicService.searchSong(searchKeyword.value)
    searched.value = true
    if (searchResults.value.length > 0) {
      selectedResult.value = searchResults.value[0]
    }
  } catch (error) {
    console.error('搜索失败:', error)
    alert(t('modal.searchFailed'))
  } finally {
    searching.value = false
  }
}

function getSourceText(source: string): string {
  const sourceMap: Record<string, string> = {
    qq: 'QQ 音乐',
    netease: '网易云',
    amll: 'Apple Music'
  }
  return sourceMap[source] || source
}

async function handleGetLyrics() {
  if (!selectedResult.value) return
  
  gettingLyric.value = true
  try {
    lyricData.value = await onlineMusicService.getLyric(
      selectedResult.value.id,
      selectedResult.value.source
    )
    
    // 预览第一句歌词
    const firstLine = lyricData.value?.lrcData?.[0]?.words?.[0]?.word || 
                      lyricData.value?.yrcData?.[0]?.words?.[0]?.word || ''
    lyricPreview.value = firstLine ? `${firstLine}...` : t('modal.noLyricFound')
  } catch (error) {
    console.error('获取歌词失败:', error)
    alert(t('modal.getLyricFailed'))
  } finally {
    gettingLyric.value = false
  }
}

async function handleGetCover() {
  if (!selectedResult.value) return
  
  gettingCover.value = true
  try {
    const coverUrl = await onlineMusicService.getCover(
      selectedResult.value.id,
      selectedResult.value.source,
      'l'
    )
    
    if (coverUrl) {
      coverPreview.value = coverUrl
    } else {
      coverPreview.value = ''
      alert(t('modal.noCoverFound'))
    }
  } catch (error) {
    console.error('获取封面失败:', error)
    alert(t('modal.getCoverFailed'))
  } finally {
    gettingCover.value = false
  }
}

function handleApplyAll() {
  if (!selectedResult.value) return
  
  emit('apply', {
    title: selectedResult.value.title,
    artist: selectedResult.value.artist,
    album: selectedResult.value.album,
    lyric: lyricData.value ? JSON.stringify(lyricData.value) : undefined,
    coverUrl: coverPreview.value || undefined
  })
}

function handleConfirm() {
  handleApplyAll()
  emit('close')
}
</script>

<style scoped>
.online-match-modal {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-content {
  background: var(--bg-primary, #1a1a1a);
  border-radius: 12px;
  padding: 24px;
  width: 90%;
  max-width: 600px;
  max-height: 80vh;
  overflow-y: auto;
  color: var(--text-primary, #fff);
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.modal-header h2 {
  margin: 0;
  font-size: 20px;
}

.close-btn {
  background: none;
  border: none;
  color: var(--text-primary, #fff);
  font-size: 24px;
  cursor: pointer;
  padding: 0;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.search-section {
  display: flex;
  gap: 10px;
  margin-bottom: 20px;
}

.search-input {
  flex: 1;
  padding: 10px;
  border: 1px solid var(--border-color, rgba(255, 255, 255, 0.2));
  border-radius: 6px;
  background: var(--bg-secondary, #2a2a2a);
  color: var(--text-primary, #fff);
  font-size: 14px;
}

.search-input:focus {
  outline: none;
  border-color: var(--btn-primary, #4CAF50);
}

.search-btn {
  padding: 10px 20px;
  background: var(--btn-primary, #4CAF50);
  color: #fff;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
}

.search-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.search-results {
  max-height: 300px;
  overflow-y: auto;
  margin-bottom: 20px;
}

.result-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.2s;
  margin-bottom: 8px;
}

.result-item:hover {
  background: var(--bg-secondary, #2a2a2a);
}

.result-item.selected {
  background: var(--btn-primary, rgba(76, 175, 80, 0.2));
  border: 1px solid var(--btn-primary, #4CAF50);
}

.result-cover {
  width: 50px;
  height: 50px;
  border-radius: 4px;
  object-fit: cover;
  flex-shrink: 0;
}

.result-info {
  flex: 1;
  min-width: 0;
}

.result-title {
  font-weight: 600;
  margin-bottom: 4px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.result-artist,
.result-album {
  font-size: 13px;
  color: var(--text-secondary, #999);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.result-source {
  font-size: 12px;
  padding: 4px 8px;
  background: var(--bg-secondary, #2a2a2a);
  border-radius: 4px;
  flex-shrink: 0;
}

.empty-state {
  text-align: center;
  padding: 40px 20px;
  color: var(--text-secondary, #999);
  font-size: 14px;
}

.action-buttons {
  display: flex;
  gap: 10px;
  margin-bottom: 20px;
}

.action-btn {
  flex: 1;
  padding: 10px;
  background: var(--bg-secondary, #2a2a2a);
  color: var(--text-primary, #fff);
  border: 1px solid var(--border-color, rgba(255, 255, 255, 0.2));
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
}

.action-btn.primary {
  background: var(--btn-primary, #4CAF50);
  border-color: var(--btn-primary, #4CAF50);
}

.action-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.preview-section {
  margin-bottom: 20px;
}

.lyric-preview,
.cover-preview {
  background: var(--bg-secondary, #2a2a2a);
  padding: 16px;
  border-radius: 8px;
  margin-bottom: 10px;
}

.lyric-preview h3,
.cover-preview h3 {
  margin: 0 0 10px 0;
  font-size: 14px;
  color: var(--text-secondary, #999);
}

.lyric-content {
  white-space: pre-wrap;
  max-height: 100px;
  overflow-y: auto;
  font-size: 13px;
  line-height: 1.5;
}

.cover-preview img {
  max-width: 200px;
  border-radius: 8px;
  display: block;
  margin: 0 auto;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.btn {
  padding: 10px 20px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
  border: none;
}

.btn.secondary {
  background: var(--bg-secondary, #2a2a2a);
  color: var(--text-primary, #fff);
}

.btn.primary {
  background: var(--btn-primary, #4CAF50);
  color: #fff;
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
