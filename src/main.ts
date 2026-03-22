import { createApp } from 'vue'
import App from './App.vue'

// 尝试使用@vueuse/core的虚拟滚动组件
// 或者使用更简单的方式，直接使用div滚动

// 创建Vue应用实例
const app = createApp(App)

// 挂载应用到DOM
app.mount('#app')

console.log('TPlayer initialized successfully')
