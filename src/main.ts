/**
 * 应用入口（M0 骨架）
 * 职责：创建 Vue 应用、装配 Pinia 与 Router、挂载根组件。
 * 明确约束：入口层不承载任何业务逻辑，业务一律下沉到 stores / services。
 */
import { createApp } from 'vue'
import { createPinia } from 'pinia'

import App from './App.vue'
import { router } from './router'
import './assets/styles/theme.css'
import './assets/styles/main.css'
// 曲目虚拟列表（RecycleScroller）基础样式
import 'vue-virtual-scroller/dist/vue-virtual-scroller.css'

const app = createApp(App)

app.use(createPinia())
// 路由插件必须在 mount 前注册：App.vue 与各布局组件均依赖 useRoute/useRouter 注入，
// 漏注册会导致 setup 阶段注入缺失并使整站白屏（本轮实测 P0 缺陷）。
app.use(router)

app.mount('#app')
