import { createApp } from 'vue'
import App from './App.vue'

console.log('Main.ts starting...')

try {
  const app = createApp(App)
  console.log('Vue app created')
  app.mount('#app')
  console.log('Vue app mounted')
} catch (error) {
  console.error('Error starting app:', error)
}
