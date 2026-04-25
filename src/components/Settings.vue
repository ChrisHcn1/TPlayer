<template>
  <div class="settings-container">
    <div class="settings-section">
      <h3>{{ t('settings.playback') }}</h3>
      
      <!-- 交叉淡入淡出设置 -->
      <div class="setting-item">
        <label class="setting-label">
          <input type="checkbox" :checked="localCrossfadeEnabled" @change="handleCrossfadeEnabledChange($event)">
          {{ t('settings.crossfade') }}
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
          {{ t('settings.autoPlayNext') }}
        </label>
      </div>
      
      <!-- 均衡器设置 -->
      <div class="setting-item">
        <label class="setting-label">
          <input type="checkbox" :checked="localEqualizerEnabled" @change="handleEqualizerEnabledChange($event)">
          {{ t('settings.equalizer') }}
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
          {{ t('settings.transcode') }}
        </label>
        <div class="setting-control">
          <span>自动转码不支持的格式</span>
        </div>
      </div>
      
      <div class="setting-item" v-if="localEnableTranscode">
        <label class="setting-label">
          <input type="checkbox" :checked="localForceTranscode" @change="handleForceTranscodeChange($event)">
          {{ t('settings.forceTranscode') }}
        </label>
        <div class="setting-control">
          <span>全部转码为FLAC</span>
        </div>
      </div>
    </div>
    
    <div class="settings-section">
      <h3>{{ t('settings.interface') }}</h3>
      
      <!-- 主题设置 -->
      <div class="setting-item">
        <label class="setting-label">{{ t('settings.theme') }}</label>
        <div class="setting-control">
          <select :value="localTheme" @change="handleThemeChange($event)">
            <option value="dark">{{ t('settings.dark') }}</option>
            <option value="light">{{ t('settings.light') }}</option>
          </select>
        </div>
      </div>
      
      <!-- 语言设置 -->
      <div class="setting-item">
        <label class="setting-label">{{ t('settings.language') }}</label>
        <div class="setting-control">
          <select :value="localLanguage" @change="handleLanguageChange($event)">
            <option value="zh-CN">简体中文</option>
            <option value="zh-TW">繁體中文</option>
            <option value="en-US">English</option>
            <option value="hi-IN">हिन्दी</option>
            <option value="es-ES">Español</option>
            <option value="ar-SA">العربية</option>
            <option value="fr-FR">Français</option>
            <option value="bn-BD">বাংলা</option>
            <option value="ru-RU">Русский</option>
            <option value="pt-PT">Português</option>
            <option value="ms-MY">Bahasa Melayu</option>
            <option value="de-DE">Deutsch</option>
          </select>
        </div>
      </div>
      
      <!-- 歌词显示设置 -->
      <div class="setting-item">
        <label class="setting-label">
          <input type="checkbox" :checked="localShowLyrics" @change="handleShowLyricsChange($event)">
          {{ t('settings.showLyrics') }}
        </label>
      </div>
      
      <!-- 歌词显示位置 -->
      <div class="setting-item" v-if="localShowLyrics">
        <label class="setting-label">{{ t('settings.lyricsPosition') }}</label>
        <div class="setting-control">
          <select :value="localLyricsPosition" @change="handleLyricsPositionChange($event)">
            <option value="bottom">{{ t('settings.bottom') }}</option>
            <option value="top">{{ t('settings.top') }}</option>
          </select>
        </div>
      </div>
      
      <!-- 音乐目录设置（仅浏览器） -->
      <div class="setting-item" v-if="isBrowser">
        <label class="setting-label">音乐目录</label>
        <div class="setting-control">
          <input 
            type="text" 
            :value="localMusicDirectory" 
            @input="handleMusicDirectoryChange($event)"
            placeholder="输入音乐目录路径"
          />
          <button class="btn btn-secondary" @click="browseMusicDirectory">浏览</button>
        </div>
      </div>
      
      <!-- 关于 -->
      <div class="setting-item">
        <label class="setting-label">{{ t('settings.about') }}</label>
        <div class="setting-control">
          <button class="btn btn-secondary" @click="showAbout">{{ t('settings.viewAbout') }}</button>
        </div>
      </div>
    </div>
    
    <div class="settings-actions">
      <button class="btn btn-secondary" @click="cancel">{{ t('common.cancel') }}</button>
      <button class="btn btn-primary" @click="saveSettings">{{ t('common.save') }}</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { t } from '../services/i18n'

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
  language: {
    type: String,
    default: 'zh-CN'
  },
  showLyrics: {
    type: Boolean,
    default: true
  },
  lyricsPosition: {
    type: String,
    default: 'bottom'
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
  },
  musicDirectory: {
    type: String,
    default: ''
  },
  isBrowser: {
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
  'update:language',
  'update:showLyrics',
  'update:lyricsPosition',
  'update:equalizerEnabled',
  'update:currentPreset',
  'update:enableTranscode',
  'update:forceTranscode',
  'update:musicDirectory',
  'browseMusicDirectory',
  'save',
  'cancel'
])

// 本地状态
const localCrossfadeEnabled = ref(props.crossfadeEnabled)
const localCrossfadeDuration = ref(props.crossfadeDuration)
const localAutoPlayNext = ref(props.autoPlayNext)
const localTheme = ref(props.theme)
const localLanguage = ref(props.language)
const localShowLyrics = ref(props.showLyrics)
const localLyricsPosition = ref(props.lyricsPosition)
const localEqualizerEnabled = ref(props.equalizerEnabled)
const localCurrentPreset = ref(props.currentPreset)
const localEnableTranscode = ref(props.enableTranscode)
const localForceTranscode = ref(props.forceTranscode)
const localMusicDirectory = ref(props.musicDirectory)

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

const handleLyricsPositionChange = (event: Event) => {
  const target = event.target as HTMLSelectElement
  if (target) {
    emit('update:lyricsPosition', target.value)
  }
}

const handleLanguageChange = (event: Event) => {
  const target = event.target as HTMLSelectElement
  if (target) {
    emit('update:language', target.value)
  }
}

const handleMusicDirectoryChange = (event: Event) => {
  const target = event.target as HTMLInputElement
  if (target) {
    emit('update:musicDirectory', target.value)
  }
}

const browseMusicDirectory = () => {
  emit('browseMusicDirectory')
}

// 保存设置
const saveSettings = () => {
  emit('save')
}

// 取消
const cancel = () => {
  emit('cancel')
}

// 显示关于对话框
const showAbout = () => {
  const aboutContent = `
    <div style="text-align: center; padding: 20px;">
      <img src="/logo.png" alt="TPlayer Logo" style="width: 80px; height: 80px; border-radius: 12px; margin-bottom: 15px;" />
      <h2 style="margin: 0 0 10px 0; color: #4CAF50;">TPlayer</h2>
      <p style="margin: 5px 0; color: #b0b0b0; font-size: 14px;">版本: V1.0.2-20260329</p>
      <p style="margin: 5px 0; color: #b0b0b0; font-size: 14px;">一款现代化的桌面音乐播放器</p>
      <div style="margin-top: 20px; border-top: 1px solid rgba(255,255,255,0.1); padding-top: 15px;">
        <p style="margin: 5px 0; color: #b0b0b0; font-size: 13px;">开发者: ChrisHcn1</p>
        <p style="margin: 5px 0; color: #b0b0b0; font-size: 13px;">
          <a href="https://github.com/ChrisHcn1/TPlayer" target="_blank" style="color: #4CAF50; text-decoration: none;">GitHub仓库</a>
        </p>
        <p style="margin: 15px 0 5px 0; color: #b0b0b0; font-size: 13px;">功能特性:</p>
        <ul style="text-align: left; margin: 10px 0; padding-left: 20px; color: #b0b0b0; font-size: 13px;">
          <li>支持多种音频格式播放</li>
          <li>音频转码功能</li>
          <li>均衡器控制</li>
          <li>歌词显示</li>
          <li>主题切换</li>
          <li>多语言支持</li>
        </ul>
      </div>
      <p style="margin-top: 15px; color: #4CAF50; font-size: 14px;">感谢您使用 TPlayer！</p>
    </div>
  `
  
  // 使用alert显示关于信息（简单实现）
  alert(aboutContent.replace(/<[^>]*>/g, '\n').replace(/\n+/g, '\n').trim())
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

.version-info {
  display: flex;
  align-items: center;
  gap: 10px;
}

.version-logo {
  width: 20px;
  height: 20px;
  border-radius: 4px;
  object-fit: cover;
}

.btn {
  padding: 10px 16px;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.btn:hover {
  transform: translateY(-1px);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.15);
}

.btn:active {
  transform: translateY(0);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  transform: none !important;
  box-shadow: none !important;
}

.btn-primary {
  background-color: var(--btn-success, #5cb85c);
  color: #ffffff;
}

.btn-primary:hover {
  background-color: var(--btn-success-hover, #4aa34a);
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

</style>