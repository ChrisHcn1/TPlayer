import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

// TPlayer Next — 前端构建配置
// 约定：base 使用相对路径以适配 Tauri 打包后的本地加载
export default defineConfig({
  plugins: [vue()],
  base: './',
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  build: {
    outDir: 'dist',
    target: 'es2021',
    sourcemap: false,
  },
  server: {
    port: 3100,
    strictPort: true,
  },
  // Tauri 侧包在浏览器预览中虽不会真正执行，但动态 import 首次被发现时
  // Vite 会重新预构建并整页 reload；启动时统一预打包，避免会话中刷新打断状态。
  optimizeDeps: {
    include: [
      '@tauri-apps/api',
      '@tauri-apps/plugin-dialog',
      '@tauri-apps/plugin-fs',
    ],
  },
  clearScreen: false,
})
