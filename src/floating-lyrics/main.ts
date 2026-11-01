/**
 * 悬浮歌词窗口入口（独立 WebView）
 *
 * 只挂载精简的 Pinia + i18n，不引入路由与其它业务，保持低开销与独立性。
 */
import { createApp } from 'vue'
import { createPinia } from 'pinia'

import App from './App.vue'
import { initI18n } from '../i18n'
import '../assets/styles/theme.css'

async function bootstrap(): Promise<void> {
  const app = createApp(App)
  app.use(createPinia())
  try {
    await initI18n()
  } catch {
    /* 资源缺失：i18n 内部兜底，启动照常 */
  }
  // 注意：播放 store 的初始化（订阅 playback://event + 拉快照）移到 App.vue 的
  // onMounted 中执行——确保在组件 setup 上下文里调用 usePlayerStore()，避免在此处
  // Pinia 尚未对组件树激活时调用导致初始化失败、窗口整片透明无内容。
  app.mount('#app')
}

void bootstrap()
