<template>
  <div v-if="visible" class="modal-overlay" @click.self="close">
    <div class="modal-container">
      <div class="modal-header">
        <h2>{{ t('settings.about') }}</h2>
        <button class="modal-close" @click="close">×</button>
      </div>
      
      <div class="modal-content">
        <div class="about-content">
          <img src="/logo.png" alt="TPlayer Logo" class="about-logo" />
          <h1 class="about-title">TPlayer</h1>
          <p class="about-version">{{ t('about.version') }}: {{ appVersion }}</p>
          <p class="about-description">{{ t('about.description') }}</p>
          
          <div class="about-section">
            <p class="about-developer">{{ t('about.developer') }}: ChrisHcn1</p>
            <a :href="t('about.githubUrl')" target="_blank" class="about-link">{{ t('about.githubRepo') }}</a>
          </div>
          
          <div class="about-section">
            <p class="about-features-title">{{ t('about.featuresTitle') }}</p>
            <ul class="about-features">
              <li>{{ t('about.feature1') }}</li>
              <li>{{ t('about.feature2') }}</li>
              <li>{{ t('about.feature3') }}</li>
              <li>{{ t('about.feature4') }}</li>
              <li>{{ t('about.feature5') }}</li>
              <li>{{ t('about.feature6') }}</li>
            </ul>
          </div>
          
          <p class="about-thanks">{{ t('about.thanks') }}</p>
        </div>
      </div>
      
      <div class="modal-footer">
        <button class="btn btn-primary" @click="close">{{ t('common.close') }}</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, defineProps, defineEmits } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { t } from '../services/i18n'

const props = defineProps({
  visible: {
    type: Boolean,
    default: false
  },
  isBrowser: {
    type: Boolean,
    default: false
  }
})

const emit = defineEmits(['close'])

const appVersion = ref('获取中...')

onMounted(async () => {
  if (props.visible) {
    await loadVersion()
  }
})

// 监听 visible 变化，加载版本号
const loadVersion = async () => {
  try {
    if (props.isBrowser) {
      appVersion.value = 'Web 版本'
    } else {
      const version = await invoke<string>('get_current_version')
      appVersion.value = `v${version}`
    }
  } catch (error) {
    console.error('获取版本号失败:', error)
    appVersion.value = t('about.unknownVersion')
  }
}

const close = () => {
  emit('close')
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
}

.modal-container {
  background-color: #1e1e1e;
  border-radius: 12px;
  width: 600px;
  max-width: 90%;
  max-height: 80vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 20px 24px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

.modal-header h2 {
  margin: 0;
  font-size: 20px;
  color: #ffffff;
}

.modal-close {
  background: none;
  border: none;
  color: #b0b0b0;
  font-size: 28px;
  cursor: pointer;
  padding: 0;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
  transition: all 0.2s;
}

.modal-close:hover {
  background-color: rgba(255, 255, 255, 0.1);
  color: #ffffff;
}

.modal-content {
  padding: 24px;
  overflow-y: auto;
  flex: 1;
}

.about-content {
  text-align: center;
}

.about-logo {
  width: 100px;
  height: 100px;
  border-radius: 16px;
  margin-bottom: 16px;
}

.about-title {
  margin: 0 0 8px 0;
  font-size: 28px;
  color: #4CAF50;
}

.about-version {
  margin: 8px 0;
  color: #b0b0b0;
  font-size: 14px;
}

.about-description {
  margin: 8px 0;
  color: #a0a0a0;
  font-size: 13px;
}

.about-section {
  margin-top: 20px;
  padding-top: 20px;
  border-top: 1px solid rgba(255, 255, 255, 0.1);
}

.about-developer {
  margin: 8px 0;
  color: #b0b0b0;
  font-size: 13px;
}

.about-link {
  color: #4CAF50;
  text-decoration: none;
  font-size: 13px;
  transition: color 0.2s;
}

.about-link:hover {
  color: #66bb6a;
  text-decoration: underline;
}

.about-features-title {
  margin: 15px 0 8px 0;
  color: #b0b0b0;
  font-size: 13px;
  text-align: left;
}

.about-features {
  text-align: left;
  margin: 10px 0;
  padding-left: 20px;
  color: #b0b0b0;
  font-size: 13px;
}

.about-features li {
  margin: 6px 0;
}

.about-thanks {
  margin-top: 20px;
  color: #4CAF50;
  font-size: 14px;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  padding: 16px 24px;
  border-top: 1px solid rgba(255, 255, 255, 0.1);
}

.btn {
  padding: 10px 20px;
  border-radius: 6px;
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s;
  border: none;
}

.btn-primary {
  background-color: #4CAF50;
  color: white;
}

.btn-primary:hover {
  background-color: #45a049;
}
</style>
