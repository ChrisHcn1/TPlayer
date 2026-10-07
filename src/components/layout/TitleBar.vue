<script setup lang="ts">
/**
 * 标题栏（TitleBar）
 *
 * 规格（DESIGN §4.3）：
 *   - 高 --size-titlebar-h，背景 --color-bg-sidebar，底部 1px --color-border-subtle
 *   - 左：Logo（--size-icon-lg）+ 品牌名 "TPlayer Next"
 *   - 中：全局搜索（320 × 28）+ Ctrl K 快捷键提示
 *   - 右：主题切换、设置入口、窗口控制（最小化 / 最大化 / 关闭）
 *   - 非 Tauri 环境：窗口控制整体不渲染（不占位、不引起右侧控件位移）
 *
 * 图标（§3.7）：图标集 M1 接入。本轮所有按钮保留与 SideBar 一致的"图标位"占位，
 * 不使用 emoji、unicode 符号或图片图标；每个按钮的尺寸由自身 CSS 类给定，
 * M1 换成 <svg> 时按钮尺寸不变，不产生布局跳动（§3.9.5）。
 *
 * 拖拽区（§4.3）：整条标题栏为拖拽区（main.css 的 .u-drag-region），
 * 其中的可交互元素所在容器必须显式标记 .u-no-drag，否则按钮点击会被窗口拖拽吞掉。
 *
 * 引用 token：--size-titlebar-h · --size-icon-* · --space-1/2/3 · --radius-sm
 *   --color-bg-sidebar/-input/-hover · --color-text-secondary/-tertiary/-primary/-disabled
 *   --color-border-subtle/-border · --color-brand · --color-danger · --color-on-brand
 *   --color-focus-ring · --font-size-sm/-2xs · --font-weight-semibold · --transition-colors
 */
import { computed } from 'vue'
import { RouterLink } from 'vue-router'

import { useTheme } from '../../composables/useTheme'
import { useCommandPalette } from '../../composables/useCommandPalette'
import { router } from '../../router'
import { resolveNavLabel } from '../../i18n/nav'
import { isTauriRuntime } from '../../services/ipc'
import { windowService } from '../../services/window'
import SearchBox from '../common/SearchBox.vue'
import Icon from '../common/Icon.vue'

const { theme, toggleTheme } = useTheme()
const { open: openCommandPalette } = useCommandPalette()

/** 窗口控制是否渲染：非 Tauri 环境（浏览器调试）不渲染，避免出现无响应按钮（§4.3） */
const hasWindowControls = computed(() => isTauriRuntime())

/** 主题按钮文案：显示"将要切换到的主题"，与 SideBar 底部主题按钮同源同文案 */
const themeActionLabel = computed(() =>
  resolveNavLabel(theme.value === 'dark' ? 'nav.theme-to-light' : 'nav.theme-to-dark'),
)

function handleSearchQuery(query: string) {
  const keyword = query.trim()
  if (keyword.length === 0) return
  void router.push({ name: 'search', query: { q: keyword } })
}

/**
 * 无边框窗口拖拽（§4.3）：WebView2 不识别 CSS app-region，
 * 左键按下标题栏空白处时调用 Tauri startDragging；
 * 落在按钮/链接/输入框/标记 .u-no-drag 的容器上不触发。
 */
function handleDragMouseDown(event: MouseEvent) {
  if (event.button !== 0) return
  const target = event.target as HTMLElement | null
  if (target?.closest('.u-no-drag, button, a, input, [role="button"]')) return
  void windowService.startDragging()
}

/** 双击标题栏空白处：最大化 / 还原（Windows 惯例） */
function handleDragDoubleClick(event: MouseEvent) {
  const target = event.target as HTMLElement | null
  if (target?.closest('.u-no-drag, button, a, input, [role="button"]')) return
  void windowService.toggleMaximize()
}
</script>

<template>
  <header
    class="title-bar u-drag-region"
    role="banner"
    @mousedown="handleDragMouseDown"
    @dblclick="handleDragDoubleClick"
  >
    <!-- 左：Logo + 品牌名 -->
    <div class="title-bar__left">
      <img src="/logo.svg" alt="TPlayer Next" class="title-bar__logo" />
      <span class="title-bar__brand">TPlayer Next</span>
    </div>

    <!-- 中：全局搜索（可交互区域，需排除窗口拖拽）；聚焦/点击即唤起 Ctrl+K 命令浮层 -->
    <div
      class="title-bar__center u-no-drag"
      role="button"
      aria-label="全局搜索（Ctrl K）"
      title="全局搜索（Ctrl K）"
      @click="openCommandPalette"
      @focusin="openCommandPalette"
    >
      <SearchBox
        variant="titlebar"
        placeholder="搜索曲目 / 专辑 / 歌手"
        @submit="handleSearchQuery"
      />
      <span class="title-bar__shortcut" aria-hidden="true">Ctrl K</span>
    </div>

    <!-- 右：主题切换 + 设置 + 窗口控制（可交互区域，需排除窗口拖拽） -->
    <div class="title-bar__right u-no-drag">
      <button
        type="button"
        class="title-bar__btn"
        :aria-label="themeActionLabel"
        :title="themeActionLabel"
        @click="toggleTheme"
      >
        <Icon :name="theme === 'dark' ? 'sun' : 'moon'" :size="16" />
      </button>

      <RouterLink class="title-bar__btn" to="/settings" :aria-label="resolveNavLabel('nav.settings')" :title="resolveNavLabel('nav.settings')">
        <Icon name="settings" :size="16" />
      </RouterLink>

      <template v-if="hasWindowControls">
        <button
          type="button"
          class="title-bar__btn title-bar__btn--window"
          aria-label="最小化"
          title="最小化"
          @click="windowService.minimize()"
        >
          <Icon name="minus" :size="16" />
        </button>
        <button
          type="button"
          class="title-bar__btn title-bar__btn--window"
          aria-label="最大化"
          title="最大化"
          @click="windowService.toggleMaximize()"
        >
          <Icon name="square" :size="14" />
        </button>
        <button
          type="button"
          class="title-bar__btn title-bar__btn--window title-bar__btn--close"
          aria-label="关闭"
          title="关闭"
          @click="windowService.close()"
        >
          <Icon name="close" :size="16" />
        </button>
      </template>
    </div>
  </header>
</template>

<style scoped>
.title-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  height: var(--size-titlebar-h);
  padding: 0 var(--space-2) 0 var(--space-3);
  background-color: var(--color-bg-sidebar);
  /* 封面皮肤：右上角极淡主色染（--color-cover-wash 默认透明） */
  background-image: radial-gradient(
    120% 240% at 88% 0%,
    var(--color-cover-wash, transparent),
    transparent 60%
  );
  border-bottom: 1px solid var(--color-border-subtle);
}

.title-bar__left {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.title-bar__logo {
  width: var(--size-icon-lg);
  height: var(--size-icon-lg);
  flex-shrink: 0;
}

.title-bar__brand {
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-semibold);
  letter-spacing: var(--font-letter-spacing-tight);
  color: var(--color-text-secondary);
  white-space: nowrap;
}

/* 搜索区：宽度按 §4.3 固定 320px，两侧空间不足时允许收缩 */
.title-bar__center {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: 0 1 320px;
  min-width: 0;
}

.title-bar__shortcut {
  flex: 0 0 auto;
  font-size: var(--font-size-2xs);
  line-height: var(--line-height-2xs);
  color: var(--color-text-tertiary);
  white-space: nowrap;
}

.title-bar__right {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  flex: 0 0 auto;
}

.title-bar__btn {
  display: flex;
  align-items: center;
  justify-content: center;
  /* 尺寸由按钮自身给定：M1 换图标集时保证按钮尺寸不变（§3.9.5） */
  width: 32px;
  height: 28px;
  color: var(--color-text-tertiary);
  background-color: transparent;
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: var(--transition-colors);
}

.title-bar__btn:hover {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}

/* 键盘焦点环内收，避免被标题栏边缘裁切（§6.2 焦点可见） */
.title-bar__btn:focus-visible {
  outline: none;
  box-shadow: inset 0 0 0 2px var(--color-focus-ring);
}

/* 窗口控制（§4.3）：各 46px 宽、占满标题栏高度，无圆角，贴合窗口边缘 */
.title-bar__btn--window {
  width: 46px;
  height: var(--size-titlebar-h);
  border-radius: 0;
}

.title-bar__btn--close:hover {
  color: var(--color-on-brand);
  background-color: var(--color-danger);
}

/* 图标位：§3.7 图标集 M1 接入，本轮仅占位（16px 见方） */
.title-bar__icon {
  display: block;
  flex: 0 0 auto;
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
}

@media (prefers-reduced-motion: reduce) {
  .title-bar__btn {
    transition: none;
  }
}
</style>
