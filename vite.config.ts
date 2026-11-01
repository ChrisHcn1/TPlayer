import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

// TPlayer — 前端构建配置
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
    // 生产构建：强制压缩（esbuild 最小化处理），关闭 sourcemap 以避免暴露源码与调试信息
    minify: 'esbuild',
    sourcemap: false,
    // 强制剔除生产包中的 console / debugger 调用，关闭调试日志输出
    esbuild: {
      drop: ['console', 'debugger'],
    },
    // 多入口：主窗口 + 悬浮歌词独立窗口（各自加载独立 HTML 与精简引导）
    rollupOptions: {
      input: {
        main: fileURLToPath(new URL('./index.html', import.meta.url)),
        floating: fileURLToPath(new URL('./floating-lyrics.html', import.meta.url)),
      },
    },
  },
  server: {
    port: 3101,
    strictPort: true,
    // Rust 编译产物目录文件极多，Vite 默认会递归监听导致耗尽 inotify
    // 观察上限（ENOSPC）。这里显式忽略，前端热更新只监听 src/。
    watch: {
      ignored: ['**/src-tauri/target/**', '**/target/**'],
    },
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
  // Vitest 单元测试配置（共享 Vite 的 alias / resolve）
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./vitest.setup.ts'],
    css: true,
    include: ['src/**/*.{test,spec}.{ts,tsx,vue}'],
  },
})
