<script setup lang="ts">
/**
 * 右面板（RightPanel）—— 歌词 / 队列
 *
 * DESIGN 依据：§2.6 右面板（可开合，列宽在 0 ↔ --size-panel-w 间切换）、§4.10 RightPanel 规格。
 *
 * 职责边界与状态来源：
 *  - 组件为纯渲染层：开合状态与当前页签由布局外壳（composables/useLayout.ts）持有，
 *    不与 SideBar / App 争夺状态，也不使用 v-model 回写父级（§2.3 折叠三原则 1 的同源原则）；
 *  - 组件内部不自行隐藏：关闭时由外壳把网格列宽设为 0（§2.1 硬性规则 3），
 *    若面板需要临时不可见，调用方用 v-show（保留实例）而非 v-if（避免丢滚动与焦点）；
 *  - 内容（歌词 / 队列）为骨架占位：M1 由 lyrics / playlist store 注入，
 *    本轮只保证结构与尺寸稳定（§3.9.5 防布局跳动），不引入假数据与假交互。
 *
 * 引用 token（全部取自 theme.css 语义层）：
 *   尺寸 --size-header-h · --size-icon-sm/-md · --size-cover-lg · --space-1/2/3/4
 *       · --radius-sm · --line-height-sm/-2xs · --size-scrollbar
 *   色彩 --color-bg-sidebar/-elevated/-hover/-selected/-active/-input · --color-text-primary
 *       · --color-text-secondary/-tertiary/-disabled · --color-border-subtle/-border
 *       · --color-brand · --color-cover-placeholder · --color-focus-ring
 *   其他 --font-size-xs/-2xs · --font-weight-medium/-semibold · --transition-colors
 */
import { computed } from 'vue'

import { useLayout, type PanelTab } from '../../composables/useLayout'
import { resolveNavLabel } from '../../i18n/nav'
import LyricsPanel from '../panel/LyricsPanel.vue'
import QueuePanel from '../panel/QueuePanel.vue'
import Icon from '../common/Icon.vue'

const { panelTab, setPanelTab, closePanel } = useLayout()

/** 页签顺序按 §4.10 固定为「歌词 / 队列」 */
const tabs: ReadonlyArray<{ tab: PanelTab; labelKey: string }> = [
  { tab: 'lyrics', labelKey: 'nav.panel-lyrics' },
  { tab: 'queue', labelKey: 'nav.panel-queue' },
]

const closeLabel = computed(() => resolveNavLabel('nav.panel-close'))
const currentTabLabel = computed(() =>
  resolveNavLabel(panelTab.value === 'lyrics' ? 'nav.panel-lyrics' : 'nav.panel-queue'),
)
</script>

<template>
  <aside class="right-panel" aria-label="歌词与队列">
    <!-- 页签条（§4.10：高 --size-header-h，右侧为关闭按钮） -->
    <div class="right-panel__tabs" role="tablist" :aria-label="resolveNavLabel('nav.panel-label')">
      <button
        v-for="entry in tabs"
        :key="entry.tab"
        type="button"
        class="right-panel__tab"
        role="tab"
        :class="{ 'right-panel__tab--active': panelTab === entry.tab }"
        :aria-selected="panelTab === entry.tab"
        :tabindex="panelTab === entry.tab ? 0 : -1"
        @click="setPanelTab(entry.tab)"
      >
        {{ resolveNavLabel(entry.labelKey) }}
      </button>

      <span class="right-panel__tabs-spacer"></span>

      <button
        type="button"
        class="right-panel__close"
        :aria-label="closeLabel"
        :title="closeLabel"
        @click="closePanel"
      >
        <Icon name="close" :size="16" />
      </button>
    </div>

    <!-- 面板主体：长度固定，内容超出时仅本区域滚动（§2.1 硬性规则 1）；
         具体滚动容器在各面板内部（歌词逐行居中 / 队列长列表） -->
    <div class="right-panel__body" role="tabpanel" :aria-label="currentTabLabel">
      <LyricsPanel v-if="panelTab === 'lyrics'" />
      <QueuePanel v-else />
    </div>
  </aside>
</template>

<style scoped>
.right-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  overflow: hidden;
  background-color: var(--color-bg-sidebar);
  /* 边框分区，不用阴影（§2.1 硬性规则 2；阴影只用于 PlayerBar） */
  border-left: 1px solid var(--color-border-subtle);
}

.right-panel__tabs {
  display: flex;
  align-items: stretch;
  flex: 0 0 auto;
  height: var(--size-header-h);
  border-bottom: 1px solid var(--color-border-subtle);
}

.right-panel__tab {
  display: flex;
  align-items: center;
  padding: 0 var(--space-3);
  font-size: var(--font-size-xs);
  font-weight: var(--font-weight-medium);
  line-height: var(--line-height-sm);
  color: var(--color-text-tertiary);
  background-color: transparent;
  border: none;
  border-bottom: 2px solid transparent;
  cursor: pointer;
  transition: var(--transition-colors);
}

.right-panel__tab:hover {
  color: var(--color-text-secondary);
}

.right-panel__tab--active {
  color: var(--color-text-primary);
  border-bottom-color: var(--color-brand);
}

.right-panel__tab:focus-visible {
  outline: none;
  box-shadow: inset 0 0 0 2px var(--color-focus-ring);
}

.right-panel__tabs-spacer {
  flex: 1 1 auto;
}

.right-panel__close {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 0 0 auto;
  width: var(--size-header-h);
  color: var(--color-text-tertiary);
  background-color: transparent;
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: var(--transition-colors);
}

.right-panel__close:hover {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}

.right-panel__close:focus-visible {
  outline: none;
  box-shadow: inset 0 0 0 2px var(--color-focus-ring);
}

/* 关闭按钮图标位：§3.7 图标集 M1 接入，本轮保留 16px 占位 */
.right-panel__close-icon {
  display: block;
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
}

.right-panel__body {
  flex: 1 1 auto;
  min-height: 0;
  padding: var(--space-3) var(--space-2);
}
</style>
