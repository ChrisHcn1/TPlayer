<template>
  <div class="audio-converter-modal" v-if="visible" @click.self="close">
    <div class="modal-content">
      <div class="modal-header">
        <h2>{{ t('converter.title') }}</h2>
        <button class="close-btn" @click="close">×</button>
      </div>
      
      <div class="modal-body">
        <!-- 文件选择区域 -->
        <div class="section">
          <label class="section-title">{{ t('converter.sourceFiles') }}</label>
          <div class="file-drop-area" @click="selectFiles" @drop.prevent="handleDrop" @dragover.prevent>
            <div v-if="selectedFiles.length === 0" class="drop-hint">
              <span class="drop-icon">📁</span>
              <p>{{ t('converter.dropFiles') }}</p>
              <p class="drop-sub">{{ t('converter.clickToSelect') }}</p>
            </div>
            <div v-else class="file-list">
              <div v-for="(file, index) in selectedFiles" :key="index" class="file-item">
                <span class="file-name">{{ file.name }}</span>
                <button class="remove-file" @click.stop="removeFile(index)">×</button>
              </div>
            </div>
          </div>
        </div>
        
        <!-- 输出格式选择 -->
        <div class="section">
          <label class="section-title">{{ t('converter.outputFormat') }}</label>
          <div class="format-options">
            <button 
              v-for="format in outputFormats" 
              :key="format.value"
              class="format-btn"
              :class="{ active: selectedFormat === format.value }"
              @click="selectedFormat = format.value"
            >
              <span class="format-icon">{{ format.icon }}</span>
              <span class="format-name">{{ format.name }}</span>
            </button>
          </div>
        </div>
        
        <!-- 质量选择 -->
        <div class="section">
          <label class="section-title">{{ t('converter.quality') }}</label>
          <div class="quality-options">
            <button 
              v-for="quality in qualityLevels" 
              :key="quality.value"
              class="quality-btn"
              :class="{ active: selectedQuality === quality.value }"
              @click="selectedQuality = quality.value"
            >
              <span class="quality-label">{{ quality.label }}</span>
              <span class="quality-desc">{{ quality.description }}</span>
            </button>
          </div>
        </div>
        
        <!-- 输出目录 -->
        <div class="section">
          <label class="section-title">{{ t('converter.outputFolder') }}</label>
          <div class="output-path">
            <input type="text" :value="outputFolder" readonly class="path-input" />
            <button class="browse-btn" @click="selectOutputFolder">{{ t('converter.browse') }}</button>
          </div>
        </div>
        
        <!-- 转换进度 -->
        <div v-if="isConverting" class="section">
          <label class="section-title">{{ t('converter.progress') }}</label>
          <div class="progress-container">
            <div class="progress-bar" :style="{ width: progressPercent + '%' }"></div>
          </div>
          <div class="progress-info">
            <span>{{ currentFile }}</span>
            <span>{{ progressPercent }}%</span>
          </div>
        </div>
        
        <!-- 转换结果 -->
        <div v-if="conversionComplete" class="section">
          <div class="result-message" :class="{ success: successCount > 0, error: errorCount > 0 }">
            <p v-if="successCount > 0">{{ t('converter.successCount', { count: successCount }) }}</p>
            <p v-if="errorCount > 0">{{ t('converter.errorCount', { count: errorCount }) }}</p>
          </div>
        </div>
      </div>
      
      <div class="modal-footer">
        <button class="btn secondary" @click="close">{{ t('buttons.cancel') }}</button>
        <button 
          class="btn primary" 
          :disabled="selectedFiles.length === 0 || isConverting"
          @click="startConversion"
        >
          {{ isConverting ? t('converter.converting') : t('converter.start') }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { t } from '../services/i18n'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

const props = defineProps<{
  visible: boolean
}>()

const emit = defineEmits<{
  (e: 'close'): void
}>()

const selectedFiles = ref<File[]>([])
const selectedFormat = ref('mp3')
const selectedQuality = ref('medium')
const outputFolder = ref('')
const isConverting = ref(false)
const progressPercent = ref(0)
const currentFile = ref('')
const conversionComplete = ref(false)
const successCount = ref(0)
const errorCount = ref(0)

const outputFormats = [
  { value: 'mp3', name: 'MP3', icon: '🎵' },
  { value: 'flac', name: 'FLAC', icon: '🎼' },
  { value: 'wav', name: 'WAV', icon: '🔊' },
  { value: 'aac', name: 'AAC', icon: '🎧' },
  { value: 'ogg', name: 'OGG', icon: '🎶' },
]

const qualityLevels = [
  { value: 'low', label: t('converter.low'), description: t('converter.lowDesc') },
  { value: 'medium', label: t('converter.medium'), description: t('converter.mediumDesc') },
  { value: 'high', label: t('converter.high'), description: t('converter.highDesc') },
]

const close = () => {
  emit('close')
}

const selectFiles = async () => {
  const result = await open({
    multiple: true,
    filters: [
      {
        name: '音频文件',
        extensions: ['mp3', 'flac', 'wav', 'aac', 'ogg', 'm4a', 'ape', 'dsf', 'dff', 'dsd', 'wma']
      }
    ]
  })
  
  if (result) {
    const paths = Array.isArray(result) ? result : [result]
    selectedFiles.value = paths.map((path: string) => ({
      name: path.split('\\').pop() || path.split('/').pop() || path,
      path
    }))
  }
}

const handleDrop = (event: DragEvent) => {
  const files = event.dataTransfer?.files
  if (files) {
    selectedFiles.value = Array.from(files).filter(f => 
      /\.(mp3|flac|wav|aac|ogg|ape|dsd|dff|dsf|wma|m4a)$/i.test(f.name)
    )
  }
}

const removeFile = (index: number) => {
  selectedFiles.value.splice(index, 1)
}

const selectOutputFolder = async () => {
  const result = await open({
    directory: true,
    multiple: false
  })
  if (result) {
    outputFolder.value = Array.isArray(result) ? result[0] : result
  }
}

const getQualitySettings = (format: string, quality: string) => {
  const settings: Record<string, Record<string, { codec: string; bitrate?: string; compression?: string }>> = {
    mp3: {
      low: { codec: 'libmp3lame', bitrate: '128k' },
      medium: { codec: 'libmp3lame', bitrate: '192k' },
      high: { codec: 'libmp3lame', bitrate: '320k' },
    },
    flac: {
      low: { codec: 'flac', compression: '0' },
      medium: { codec: 'flac', compression: '5' },
      high: { codec: 'flac', compression: '8' },
    },
    wav: {
      low: { codec: 'pcm_s16le' },
      medium: { codec: 'pcm_s24le' },
      high: { codec: 'pcm_s32le' },
    },
    aac: {
      low: { codec: 'aac', bitrate: '128k' },
      medium: { codec: 'aac', bitrate: '192k' },
      high: { codec: 'aac', bitrate: '256k' },
    },
    ogg: {
      low: { codec: 'libvorbis', bitrate: '128k' },
      medium: { codec: 'libvorbis', bitrate: '192k' },
      high: { codec: 'libvorbis', bitrate: '256k' },
    },
  }
  return settings[format]?.[quality] || settings[format]?.medium
}

const startConversion = async () => {
  if (selectedFiles.value.length === 0) return
  
  if (!outputFolder.value) {
    const result = await invoke('select_output_folder')
    if (!result) return
    outputFolder.value = result as string
  }
  
  isConverting.value = true
  conversionComplete.value = false
  successCount.value = 0
  errorCount.value = 0
  progressPercent.value = 0
  
  const totalFiles = selectedFiles.value.length
  
  for (let i = 0; i < totalFiles; i++) {
    const file = selectedFiles.value[i]
    currentFile.value = file.name
    
    try {
      const settings = getQualitySettings(selectedFormat.value, selectedQuality.value)
      await invoke('convert_audio', {
        inputPath: file.path,
        outputFolder: outputFolder.value,
        outputFormat: selectedFormat.value,
        codec: settings.codec,
        bitrate: settings.bitrate,
        compression: settings.compression,
      })
      successCount.value++
    } catch (error) {
      console.error('转换失败:', error)
      errorCount.value++
    }
    
    progressPercent.value = ((i + 1) / totalFiles) * 100
  }
  
  isConverting.value = false
  conversionComplete.value = true
  currentFile.value = ''
}
</script>

<style scoped>
.audio-converter-modal {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-content {
  background: #1a1a1a;
  border-radius: 12px;
  width: 90%;
  max-width: 500px;
  max-height: 90vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid #333;
}

.modal-header h2 {
  margin: 0;
  font-size: 18px;
  color: #fff;
}

.close-btn {
  background: none;
  border: none;
  color: #999;
  font-size: 24px;
  cursor: pointer;
  padding: 0;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.close-btn:hover {
  color: #fff;
}

.modal-body {
  flex: 1;
  padding: 20px;
  overflow-y: auto;
}

.section {
  margin-bottom: 20px;
}

.section-title {
  display: block;
  margin-bottom: 10px;
  font-size: 14px;
  color: #999;
}

.file-drop-area {
  border: 2px dashed #444;
  border-radius: 8px;
  padding: 30px;
  text-align: center;
  cursor: pointer;
  transition: border-color 0.3s;
}

.file-drop-area:hover,
.file-drop-area.dragover {
  border-color: #4a90d9;
}

.drop-hint {
  color: #666;
}

.drop-icon {
  font-size: 48px;
  display: block;
  margin-bottom: 10px;
}

.drop-sub {
  font-size: 12px;
  margin-top: 5px;
}

.file-list {
  max-height: 150px;
  overflow-y: auto;
}

.file-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  background: #2a2a2a;
  border-radius: 4px;
  margin-bottom: 5px;
}

.file-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: #fff;
}

.remove-file {
  background: #333;
  border: none;
  color: #999;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-left: 10px;
}

.remove-file:hover {
  background: #ff4444;
  color: #fff;
}

.format-options {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 8px;
}

.format-btn {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 12px 8px;
  background: #2a2a2a;
  border: 2px solid transparent;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.3s;
}

.format-btn:hover {
  background: #333;
}

.format-btn.active {
  border-color: #4a90d9;
  background: rgba(74, 144, 217, 0.1);
}

.format-icon {
  font-size: 24px;
  margin-bottom: 4px;
}

.format-name {
  font-size: 12px;
  color: #fff;
}

.quality-options {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 10px;
}

.quality-btn {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 12px;
  background: #2a2a2a;
  border: 2px solid transparent;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.3s;
}

.quality-btn:hover {
  background: #333;
}

.quality-btn.active {
  border-color: #4a90d9;
  background: rgba(74, 144, 217, 0.1);
}

.quality-label {
  font-size: 14px;
  font-weight: bold;
  color: #fff;
  margin-bottom: 4px;
}

.quality-desc {
  font-size: 11px;
  color: #666;
  text-align: center;
}

.output-path {
  display: flex;
  gap: 10px;
}

.path-input {
  flex: 1;
  padding: 10px 12px;
  background: #2a2a2a;
  border: 1px solid #444;
  border-radius: 6px;
  color: #fff;
  font-size: 13px;
}

.browse-btn {
  padding: 10px 20px;
  background: #4a90d9;
  border: none;
  border-radius: 6px;
  color: #fff;
  cursor: pointer;
  font-size: 13px;
}

.browse-btn:hover {
  background: #3a80c9;
}

.progress-container {
  height: 8px;
  background: #333;
  border-radius: 4px;
  overflow: hidden;
  margin-bottom: 10px;
}

.progress-bar {
  height: 100%;
  background: linear-gradient(90deg, #4a90d9, #6aafed);
  transition: width 0.3s;
}

.progress-info {
  display: flex;
  justify-content: space-between;
  color: #999;
  font-size: 12px;
}

.result-message {
  padding: 12px 16px;
  border-radius: 6px;
  text-align: center;
}

.result-message.success {
  background: rgba(76, 175, 80, 0.1);
  color: #4caf50;
}

.result-message.error {
  background: rgba(244, 67, 54, 0.1);
  color: #f44336;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding: 16px 20px;
  border-top: 1px solid #333;
}

.btn {
  padding: 10px 24px;
  border-radius: 6px;
  font-size: 14px;
  cursor: pointer;
  border: none;
  transition: background 0.3s;
}

.btn.secondary {
  background: #333;
  color: #fff;
}

.btn.secondary:hover {
  background: #444;
}

.btn.primary {
  background: #4a90d9;
  color: #fff;
}

.btn.primary:hover:not(:disabled) {
  background: #3a80c9;
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
