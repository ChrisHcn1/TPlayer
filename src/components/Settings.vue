<template>
  <div class="settings-container">
    <div class="settings-section">
      <h3>播放设置</h3>
      
      <!-- 交叉淡入淡出设置 -->
      <div class="setting-item">
        <label class="setting-label">
          <input type="checkbox" :checked="localCrossfadeEnabled" @change="handleCrossfadeEnabledChange($event)">
          交叉淡入淡出
        </label>
        <div class="setting-control" v-if="localCrossfadeEnabled">
          <span>{{ localCrossfadeDuration }}s</span>
          <input
            type="range"
            min="0"
            max="3"
            step="0.1"
            :value="localCrossfadeDuration"
            @input="handleCrossfadeDurationChange($event)"
          />
        </div>
      </div>
      
      <!-- 自动播放下一首 -->
      <div class="setting-item">
        <label class="setting-label">
          <input type="checkbox" :checked="localAutoPlayNext" @change="handleAutoPlayNextChange($event)">
          自动播放下一首
        </label>
      </div>
      
      <!-- 均衡器设置 -->
      <div class="setting-item">
        <label class="setting-label">
          <input type="checkbox" :checked="localEqualizerEnabled" @change="handleEqualizerEnabledChange($event)">
          均衡器
        </label>
        <div class="setting-control" v-if="localEqualizerEnabled">
          <select :value="localCurrentPreset" @change="handleCurrentPresetChange($event)">
            <option value="flat">平坦</option>
            <option value="rock">摇滚</option>
            <option value="pop">流行</option>
            <option value="jazz">爵士</option>
            <option value="classical">古典</option>
            <option value="electronic">电子</option>
          </select>
        </div>
      </div>
      
      <!-- 转码设置 -->
      <div class="setting-item">
        <label class="setting-label">
          <input type="checkbox" :checked="localEnableTranscode" @change="handleEnableTranscodeChange($event)">
          启用转码
        </label>
        <div class="setting-control">
          <span>自动转码不支持的格式</span>
        </div>
      </div>
      
      <div class="setting-item" v-if="localEnableTranscode">
        <label class="setting-label">
          <input type="checkbox" :checked="localForceTranscode" @change="handleForceTranscodeChange($event)">
          强制转码
        </label>
        <div class="setting-control">
          <span>全部转码为FLAC</span>
        </div>
      </div>
    </div>
    
    <div class="settings-section">
      <h3>界面设置</h3>
      
      <!-- 主题设置 -->
      <div class="setting-item">
        <label class="setting-label">主题</label>
        <div class="setting-control">
          <select :value="localTheme" @change="handleThemeChange($event)">
            <option value="dark">深色</option>
            <option value="light">浅色</option>
          </select>
        </div>
      </div>
      
      <!-- 歌词显示设置 -->
      <div class="setting-item">
        <label class="setting-label">
          <input type="checkbox" :checked="localShowLyrics" @change="handleShowLyricsChange($event)">
          显示歌词
        </label>
      </div>
      
      <!-- 关于 -->
      <div class="setting-item">
        <span class="setting-value">TPlayer V1.0.1-20260322</span>
      </div>
    </div>
    
    <div class="settings-actions">
      <button class="btn btn-secondary" @click="cancel">取消</button>
      <button class="btn btn-primary" @click="saveSettings">保存设置</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'

// 定义props
const props = defineProps({
  crossfadeEnabled: {
    type: Boolean,
    default: false
  },
  crossfadeDuration: {
    type: Number,
    default: 1
  },
  autoPlayNext: {
    type: Boolean,
    default: true
  },
  theme: {
    type: String,
    default: 'dark'
  },
  showLyrics: {
    type: Boolean,
    default: true
  },
  equalizerEnabled: {
    type: Boolean,
    default: false
  },
  currentPreset: {
    type: String,
    default: 'flat'
  },
  enableTranscode: {
    type: Boolean,
    default: true
  },
  forceTranscode: {
    type: Boolean,
    default: false
  }
})

// 定义emit
const emit = defineEmits([
  'update:crossfadeEnabled',
  'update:crossfadeDuration',
  'update:autoPlayNext',
  'update:theme',
  'update:showLyrics',
  'update:equalizerEnabled',
  'update:currentPreset',
  'update:enableTranscode',
  'update:forceTranscode',
  'save',
  'cancel'
])

// 本地状态
const localCrossfadeEnabled = ref(props.crossfadeEnabled)
const localCrossfadeDuration = ref(props.crossfadeDuration)
const localAutoPlayNext = ref(props.autoPlayNext)
const localTheme = ref(props.theme)
const localShowLyrics = ref(props.showLyrics)
const localEqualizerEnabled = ref(props.equalizerEnabled)
const localCurrentPreset = ref(props.currentPreset)
const localEnableTranscode = ref(props.enableTranscode)
const localForceTranscode = ref(props.forceTranscode)

// 事件处理函数
const handleCrossfadeEnabledChange = (event: Event) => {
  const target = event.target as HTMLInputElement
  if (target) {
    emit('update:crossfadeEnabled', target.checked)
  }
}

const handleCrossfadeDurationChange = (event: Event) => {
  const target = event.target as HTMLInputElement
  if (target) {
    emit('update:crossfadeDuration', parseFloat(target.value))
  }
}

const handleAutoPlayNextChange = (event: Event) => {
  const target = event.target as HTMLInputElement
  if (target) {
    emit('update:autoPlayNext', target.checked)
  }
}

const handleEqualizerEnabledChange = (event: Event) => {
  const target = event.target as HTMLInputElement
  if (target) {
    emit('update:equalizerEnabled', target.checked)
  }
}

const handleCurrentPresetChange = (event: Event) => {
  const target = event.target as HTMLSelectElement
  if (target) {
    emit('update:currentPreset', target.value)
  }
}

const handleEnableTranscodeChange = (event: Event) => {
  const target = event.target as HTMLInputElement
  if (target) {
    emit('update:enableTranscode', target.checked)
  }
}

const handleForceTranscodeChange = (event: Event) => {
  const target = event.target as HTMLInputElement
  if (target) {
    emit('update:forceTranscode', target.checked)
  }
}

const handleThemeChange = (event: Event) => {
  const target = event.target as HTMLSelectElement
  if (target) {
    emit('update:theme', target.value)
  }
}

const handleShowLyricsChange = (event: Event) => {
  const target = event.target as HTMLInputElement
  if (target) {
    emit('update:showLyrics', target.checked)
  }
}

// 保存设置
const saveSettings = () => {
  emit('save')
}

// 取消
const cancel = () => {
  emit('cancel')
}
</script>

<style scoped>
.settings-container {
  padding: 15px;
  max-width: 900px;
  margin: 0 auto;
  display: grid;
  grid-template-columns: 1fr;
  gap: 15px;
}

.settings-section {
  padding: 12px;
  background-color: rgba(255, 255, 255, 0.05);
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.1);
}

h3 {
  margin-bottom: 8px;
  color: var(--text-primary, #ffffff);
  font-size: 14px;
  font-weight: 600;
  padding-bottom: 6px;
  border-bottom: 2px solid rgba(76, 175, 80, 0.3);
}

.setting-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
  padding: 6px 0;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.setting-item:last-child {
  border-bottom: none;
  margin-bottom: 0;
}

.setting-label {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-primary, #ffffff);
  cursor: pointer;
  font-size: 14px;
  min-width: 120px;
}

.setting-control {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 1;
}

.setting-control input[type="range"] {
  width: 120px;
  height: 4px;
  background: #3a3a3a;
  border-radius: 2px;
  outline: none;
  -webkit-appearance: none;
}

.setting-control input[type="range"]::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 14px;
  height: 14px;
  background: #4CAF50;
  border-radius: 50%;
  cursor: pointer;
  transition: transform 0.1s;
}

.setting-control input[type="range"]::-webkit-slider-thumb:hover {
  transform: scale(1.2);
}

.setting-control input[type="range"]::-moz-range-thumb {
  width: 14px;
  height: 14px;
  background: #4CAF50;
  border-radius: 50%;
  cursor: pointer;
  border: none;
}

.setting-control select {
  padding: 6px 10px;
  background-color: var(--bg-secondary, #2a2a2a);
  color: var(--text-primary, #ffffff);
  border: 1px solid var(--border-color, rgba(255, 255, 255, 0.2));
  border-radius: 4px;
  font-size: 14px;
  cursor: pointer;
  outline: none;
}

.setting-control select:hover {
  border-color: rgba(76, 175, 80, 0.5);
}

.setting-control select:focus {
  border-color: #4CAF50;
}

.setting-control span {
  color: #b0b0b0;
  font-size: 13px;
  min-width: 40px;
}

.setting-value {
  color: #b0b0b0;
  font-size: 13px;
}

.btn {
  padding: 10px 20px;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-primary {
  background-color: #4CAF50;
  color: #ffffff;
}

.btn-primary:hover {
  background-color: #45a049;
}

.btn-secondary {
  background-color: var(--btn-secondary-bg, rgba(255, 255, 255, 0.1));
  color: var(--text-primary, #ffffff);
  border: 1px solid var(--border-color, rgba(255, 255, 255, 0.2));
}

.btn-secondary:hover {
  background-color: var(--btn-secondary-hover, rgba(255, 255, 255, 0.15));
}

.settings-actions {
  grid-column: 1 / -1;
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  margin-top: 15px;
  padding-top: 10px;
  border-top: 1px solid var(--border-color, rgba(255, 255, 255, 0.1));
}

.btn {
  padding: 8px 16px;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 14px;
  transition: all 0.2s;
}

.btn-primary {
  background-color: #4CAF50;
  color: white;
}

.btn-primary:hover {
  background-color: #45a049;
}

</style>