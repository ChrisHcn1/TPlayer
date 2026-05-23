<template>
  <div class="progress-modal">
    <div class="progress-content">
      <div class="progress-header">
        <div class="progress-icon" :class="currentStatus">
          <svg v-if="currentStatus === 'downloading'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
            <polyline points="7 10 12 15 17 10"/>
            <line x1="12" y1="15" x2="12" y2="3"/>
          </svg>
          <svg v-else-if="currentStatus === 'verifying'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10"/>
            <polyline points="16 10 10 16 8 14"/>
          </svg>
          <svg v-else-if="currentStatus === 'installing'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 8l3.26-3.26A9.75 9.75 0 0 1 12 3a9 9 0 0 1 9 9z"/>
            <path d="M15 9l-3 3 3 3"/>
          </svg>
          <svg v-else-if="currentStatus === 'completed'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10"/>
            <polyline points="16 10 10 16 8 14"/>
          </svg>
          <svg v-else viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10"/>
            <line x1="12" y1="8" x2="12" y2="12"/>
            <line x1="12" y1="16" x2="12" y2="16"/>
          </svg>
        </div>
        <h3>{{ statusText }}</h3>
      </div>
      
      <div class="progress-bar-container">
        <div class="progress-bar">
          <div class="progress-fill" :style="{ width: progress + '%' }"></div>
          <div class="progress-glow" :style="{ width: progress + '%' }"></div>
        </div>
        <div class="progress-info">
          <span class="progress-percent">{{ progress }}%</span>
          <span class="progress-size">{{ downloadedSize }} / {{ totalSize }}</span>
        </div>
      </div>
      
      <div class="progress-details" v-if="currentStatus === 'downloading'">
        <div class="download-speed">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
          </svg>
          <span>{{ downloadSpeed }}</span>
        </div>
        <div class="download-time">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10"/>
            <polyline points="12 6 12 12 16 14"/>
          </svg>
          <span>{{ remainingTime }}</span>
        </div>
      </div>
      
      <div class="progress-actions">
        <button 
          v-if="currentStatus !== 'completed'" 
          class="btn btn-secondary" 
          @click="handleCancel"
        >
          {{ t('update.cancel') }}
        </button>
        <button 
          v-if="currentStatus === 'completed'" 
          class="btn btn-primary" 
          @click="handleRestart"
        >
          {{ t('update.restart') }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { t } from '../services/i18n'

interface UpdateInfo {
  version: string
  release_notes: string
  download_url: string
  sha256_hash: string
  size: number
  release_date: string
}

const props = defineProps<{
  updateInfo: UpdateInfo | null
}>()

const emit = defineEmits<{
  (e: 'complete'): void
  (e: 'cancel'): void
}>()

const currentStatus = ref<'downloading' | 'verifying' | 'installing' | 'completed' | 'error'>('downloading')
const progress = ref(0)
const downloadedBytes = ref(0)
const totalBytes = ref(props.updateInfo?.size || 0)
const downloadSpeed = ref('0 KB/s')
const remainingTime = ref('--')

const statusText = computed(() => {
  switch (currentStatus.value) {
    case 'downloading':
      return t('update.downloading')
    case 'verifying':
      return t('update.verifying')
    case 'installing':
      return t('update.installing')
    case 'completed':
      return t('update.completed')
    case 'error':
      return t('update.error')
    default:
      return ''
  }
})

const downloadedSize = computed(() => formatSize(downloadedBytes.value))
const totalSize = computed(() => formatSize(totalBytes.value))

const formatSize = (bytes: number): string => {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

const handleProgress = (downloaded: number, total: number) => {
  downloadedBytes.value = downloaded
  totalBytes.value = total
  progress.value = total > 0 ? Math.round((downloaded / total) * 100) : 0
  
  const now = Date.now()
  const speed = calculateSpeed(downloaded, now)
  downloadSpeed.value = speed
  
  if (speed !== '0 KB/s' && progress.value < 100) {
    const remaining = total - downloaded
    const speedBytes = parseSpeed(speed)
    if (speedBytes > 0) {
      const seconds = Math.ceil(remaining / speedBytes)
      remainingTime.value = formatTime(seconds)
    }
  }
}

let lastDownloaded = 0
let lastTime = 0

const calculateSpeed = (downloaded: number, now: number): string => {
  if (lastTime === 0) {
    lastDownloaded = downloaded
    lastTime = now
    return '0 KB/s'
  }
  
  const timeDiff = (now - lastTime) / 1000
  const bytesDiff = downloaded - lastDownloaded
  
  if (timeDiff >= 1) {
    const speedBps = bytesDiff / timeDiff
    const speedKbps = speedBps / 1024
    
    lastDownloaded = downloaded
    lastTime = now
    
    if (speedKbps >= 1024) {
      return (speedKbps / 1024).toFixed(2) + ' MB/s'
    }
    return speedKbps.toFixed(2) + ' KB/s'
  }
  
  return downloadSpeed.value
}

const parseSpeed = (speed: string): number => {
  const match = speed.match(/([\d.]+)\s*(KB|MB)/)
  if (!match) return 0
  
  const value = parseFloat(match[1])
  const unit = match[2]
  
  if (unit === 'MB') {
    return value * 1024 * 1024
  }
  return value * 1024
}

const formatTime = (seconds: number): string => {
  const mins = Math.floor(seconds / 60)
  const secs = seconds % 60
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

const simulateProgress = async () => {
  if (!props.updateInfo) return
  
  try {
    currentStatus.value = 'downloading'
    progress.value = 0
    
    const steps = 100
    const stepDuration = 300
    
    for (let i = 0; i <= steps; i++) {
      await new Promise(resolve => setTimeout(resolve, stepDuration))
      progress.value = i
      downloadedBytes.value = Math.round((i / steps) * props.updateInfo!.size)
      
      const speed = (Math.random() * 10 + 5).toFixed(1) + ' MB/s'
      downloadSpeed.value = speed
      
      const remaining = Math.ceil(((100 - i) * stepDuration) / 1000)
      remainingTime.value = formatTime(remaining)
    }
    
    currentStatus.value = 'verifying'
    progress.value = 100
    
    await new Promise(resolve => setTimeout(resolve, 1500))
    
    currentStatus.value = 'installing'
    await new Promise(resolve => setTimeout(resolve, 2000))
    
    currentStatus.value = 'completed'
  } catch (error) {
    currentStatus.value = 'error'
    console.error('Update failed:', error)
  }
}

const handleCancel = () => {
  emit('cancel')
}

const handleRestart = () => {
  emit('complete')
  window.location.reload()
}

onMounted(() => {
  simulateProgress()
})
</script>

<style scoped>
.progress-modal {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.85);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1001;
  backdrop-filter: blur(8px);
}

.progress-content {
  background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
  border-radius: 20px;
  width: 90%;
  max-width: 450px;
  padding: 32px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.6);
}

.progress-header {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  margin-bottom: 28px;
}

.progress-icon {
  width: 64px;
  height: 64px;
  border-radius: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.3s ease;
}

.progress-icon.downloading {
  background: linear-gradient(135deg, #2196F3, #1976D2);
  color: white;
}

.progress-icon.verifying {
  background: linear-gradient(135deg, #FF9800, #F57C00);
  color: white;
}

.progress-icon.installing {
  background: linear-gradient(135deg, #9C27B0, #7B1FA2);
  color: white;
}

.progress-icon.completed {
  background: linear-gradient(135deg, #4CAF50, #388E3C);
  color: white;
}

.progress-icon.error {
  background: linear-gradient(135deg, #F44336, #D32F2F);
  color: white;
}

.progress-icon svg {
  width: 32px;
  height: 32px;
}

.progress-header h3 {
  margin: 0;
  color: #ffffff;
  font-size: 20px;
  font-weight: 600;
}

.progress-bar-container {
  margin-bottom: 20px;
}

.progress-bar {
  position: relative;
  height: 8px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 4px;
  overflow: hidden;
}

.progress-fill {
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  background: linear-gradient(90deg, #4CAF50, #8BC34A);
  border-radius: 4px;
  transition: width 0.3s ease;
}

.progress-glow {
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  background: linear-gradient(90deg, rgba(76, 175, 80, 0.5), rgba(139, 195, 74, 0.3));
  border-radius: 4px;
  filter: blur(4px);
  transition: width 0.3s ease;
}

.progress-info {
  display: flex;
  justify-content: space-between;
  margin-top: 10px;
}

.progress-percent {
  color: #4CAF50;
  font-weight: 600;
  font-size: 16px;
}

.progress-size {
  color: #888;
  font-size: 13px;
}

.progress-details {
  display: flex;
  justify-content: space-around;
  padding: 16px;
  background: rgba(255, 255, 255, 0.05);
  border-radius: 12px;
  margin-bottom: 24px;
}

.download-speed,
.download-time {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #b0b0b0;
  font-size: 13px;
}

.download-speed svg,
.download-time svg {
  width: 16px;
  height: 16px;
  color: #4CAF50;
}

.progress-actions {
  display: flex;
  justify-content: center;
}

.btn {
  padding: 12px 32px;
  border: none;
  border-radius: 10px;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
}

.btn:hover {
  transform: translateY(-2px);
}

.btn:active {
  transform: translateY(0);
}

.btn-primary {
  background: linear-gradient(135deg, #4CAF50, #388E3C);
  color: white;
  box-shadow: 0 4px 14px 0 rgba(76, 175, 80, 0.39);
}

.btn-primary:hover {
  box-shadow: 0 6px 20px 0 rgba(76, 175, 80, 0.45);
}

.btn-secondary {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
  border: 1px solid rgba(255, 255, 255, 0.2);
}

.btn-secondary:hover {
  background: rgba(255, 255, 255, 0.15);
}
</style>