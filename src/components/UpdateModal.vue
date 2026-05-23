<template>
  <div class="modal-overlay" v-if="visible" @click.self="handleClose">
    <div class="modal-content">
      <div class="modal-header">
        <div class="modal-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2z"/>
            <path d="M9 12l2 2 4-4"/>
            <path d="M12 2v4"/>
          </svg>
        </div>
        <h2>{{ t('update.newVersionAvailable') }}</h2>
        <button class="close-btn" @click="handleClose">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M18 6L6 18M6 6l12 12"/>
          </svg>
        </button>
      </div>
      
      <div class="modal-body" v-if="updateInfo">
        <div class="version-info">
          <div class="version-row">
            <span class="label">{{ t('update.currentVersion') }}</span>
            <span class="current">{{ currentVersion }}</span>
          </div>
          <div class="version-row">
            <span class="label">{{ t('update.newVersion') }}</span>
            <span class="new">{{ updateInfo.version }}</span>
          </div>
        </div>
        
        <div class="release-date">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="3" y="4" width="18" height="18" rx="2" ry="2"/>
            <line x1="16" y1="2" x2="16" y2="6"/>
            <line x1="8" y1="2" x2="8" y2="6"/>
            <line x1="3" y1="10" x2="21" y2="10"/>
          </svg>
          <span>{{ updateInfo.release_date }}</span>
        </div>
        
        <div class="release-notes">
          <h3>{{ t('update.releaseNotes') }}</h3>
          <div class="notes-content">
            <pre>{{ updateInfo.release_notes }}</pre>
          </div>
        </div>
        
        <div class="update-size">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
            <polyline points="17 8 12 3 7 8"/>
            <line x1="12" y1="3" x2="12" y2="15"/>
          </svg>
          <span>{{ formatSize(updateInfo.size) }}</span>
        </div>
      </div>
      
      <div class="modal-footer">
        <button class="btn btn-secondary" @click="handleClose" v-if="!isDownloading">
          {{ t('update.later') }}
        </button>
        <button 
          class="btn btn-primary" 
          @click="handleUpdate"
          :disabled="isDownloading"
        >
          <span v-if="!isDownloading">{{ t('update.updateNow') }}</span>
          <span v-else class="loading-text">
            <svg class="loading-spinner" viewBox="0 0 24 24">
              <circle class="path" cx="12" cy="12" r="10" fill="none" stroke-width="2"/>
            </svg>
            {{ t('update.downloading') }}
          </span>
        </button>
      </div>
    </div>
    
    <UpdateProgress 
      v-if="showProgress" 
      :updateInfo="updateInfo"
      @complete="handleUpdateComplete"
      @cancel="handleUpdateCancel"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { t } from '../services/i18n'
import UpdateProgress from './UpdateProgress.vue'

interface UpdateInfo {
  version: string
  release_notes: string
  download_url: string
  sha256_hash: string
  size: number
  release_date: string
}

const props = defineProps<{
  visible: boolean
  updateInfo: UpdateInfo | null
  currentVersion: string
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'update'): void
}>()

const isDownloading = ref(false)
const showProgress = ref(false)

const handleClose = () => {
  emit('close')
}

const handleUpdate = async () => {
  if (!props.updateInfo) return
  
  isDownloading.value = true
  showProgress.value = true
  emit('update')
}

const handleUpdateComplete = () => {
  isDownloading.value = false
  showProgress.value = false
}

const handleUpdateCancel = () => {
  isDownloading.value = false
  showProgress.value = false
}

const formatSize = (bytes: number): string => {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}
</script>

<style scoped>
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  backdrop-filter: blur(4px);
}

.modal-content {
  background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
  border-radius: 16px;
  width: 90%;
  max-width: 500px;
  max-height: 80vh;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
}

.modal-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 20px 24px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

.modal-icon {
  width: 40px;
  height: 40px;
  background: linear-gradient(135deg, #4CAF50, #8BC34A);
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
}

.modal-icon svg {
  width: 22px;
  height: 22px;
}

.modal-header h2 {
  flex: 1;
  margin: 0;
  color: #ffffff;
  font-size: 18px;
  font-weight: 600;
}

.close-btn {
  background: none;
  border: none;
  color: #888;
  cursor: pointer;
  padding: 8px;
  border-radius: 8px;
  transition: all 0.2s;
}

.close-btn:hover {
  background-color: rgba(255, 255, 255, 0.1);
  color: #fff;
}

.close-btn svg {
  width: 20px;
  height: 20px;
}

.modal-body {
  padding: 20px 24px;
  overflow-y: auto;
  max-height: 40vh;
}

.version-info {
  background: rgba(76, 175, 80, 0.1);
  border-radius: 10px;
  padding: 16px;
  margin-bottom: 16px;
}

.version-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 0;
}

.version-row:first-child {
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

.version-row .label {
  color: #b0b0b0;
  font-size: 14px;
}

.version-row .current {
  color: #888;
  font-size: 14px;
  font-family: monospace;
}

.version-row .new {
  color: #4CAF50;
  font-size: 16px;
  font-weight: 600;
  font-family: monospace;
}

.release-date,
.update-size {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #b0b0b0;
  font-size: 13px;
  margin-bottom: 16px;
}

.release-date svg,
.update-size svg {
  width: 16px;
  height: 16px;
}

.release-notes {
  margin-bottom: 16px;
}

.release-notes h3 {
  color: #fff;
  font-size: 14px;
  font-weight: 600;
  margin-bottom: 10px;
}

.notes-content {
  background: rgba(255, 255, 255, 0.05);
  border-radius: 8px;
  padding: 12px;
  max-height: 150px;
  overflow-y: auto;
}

.notes-content pre {
  margin: 0;
  color: #ccc;
  font-size: 13px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-wrap: break-word;
  font-family: inherit;
}

.modal-footer {
  display: flex;
  gap: 12px;
  padding: 16px 24px;
  border-top: 1px solid rgba(255, 255, 255, 0.1);
  justify-content: flex-end;
}

.btn {
  padding: 10px 20px;
  border: none;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.btn:hover {
  transform: translateY(-1px);
}

.btn:active {
  transform: translateY(0);
}

.btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
  transform: none;
}

.btn-primary {
  background: linear-gradient(135deg, #4CAF50, #388E3C);
  color: white;
  box-shadow: 0 4px 14px 0 rgba(76, 175, 80, 0.39);
}

.btn-primary:hover:not(:disabled) {
  background: linear-gradient(135deg, #43A047, #2E7D32);
  box-shadow: 0 6px 20px 0 rgba(76, 175, 80, 0.45);
}

.btn-secondary {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
  border: 1px solid rgba(255, 255, 255, 0.2);
}

.btn-secondary:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.15);
}

.loading-text {
  display: flex;
  align-items: center;
  gap: 8px;
}

.loading-spinner {
  width: 18px;
  height: 18px;
  animation: spin 1s linear infinite;
}

.loading-spinner .path {
  stroke: currentColor;
  stroke-linecap: round;
  animation: dash 1.5s ease-in-out infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@keyframes dash {
  0% { stroke-dasharray: 1, 150; stroke-dashoffset: 0; }
  50% { stroke-dasharray: 90, 150; stroke-dashoffset: -35; }
  to { stroke-dasharray: 90, 150; stroke-dashoffset: -124; }
}
</style>