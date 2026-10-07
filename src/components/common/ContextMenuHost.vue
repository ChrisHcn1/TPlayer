<script setup lang="ts">
/**
 * 上下文菜单宿主（DESIGN §4.17）：全局唯一，挂载于 App。
 * 视口边缘自动翻转；外点 / Esc / 滚动关闭；方向键导航、Enter 执行。
 * 子菜单独立 fixed 渲染，避免被父菜单滚动裁剪。
 */
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'

import {
  useContextMenu,
  type MenuItem,
} from '../../composables/useContextMenu'
import Icon from './Icon.vue'

const MENU_WIDTH = 208

const { state, activeIndex, openSubmenuKey, close, select, setActiveIndex } = useContextMenu()

const rootRef = ref<HTMLElement | null>(null)
const submenuStyle = ref<Record<string, string>>({})
/** 当前展开子菜单的父项（直接持有 items，模板渲染用） */
const submenuParent = ref<MenuItem | null>(null)

const positionStyle = computed(() => ({
  left: `${state.x}px`,
  top: `${state.y}px`,
}))

const enabledIndices = computed(() =>
  state.items
    .map((item, index) => ({ item, index }))
    .filter(({ item }) => !item.divider && !item.disabled)
    .map(({ index }) => index),
)

async function reposition(): Promise<void> {
  await nextTick()
  const el = rootRef.value
  if (!el) return
  const rect = el.getBoundingClientRect()
  const margin = 8
  if (rect.right > window.innerWidth - margin) {
    state.x = Math.max(margin, window.innerWidth - rect.width - margin)
  }
  if (rect.bottom > window.innerHeight - margin) {
    state.y = Math.max(margin, window.innerHeight - rect.height - margin)
  }
}

watch(
  () => state.open,
  (open) => {
    if (open) {
      reposition()
      window.addEventListener('resize', close)
      window.addEventListener('blur', close)
    } else {
      window.removeEventListener('resize', close)
      window.removeEventListener('blur', close)
    }
  },
)

onBeforeUnmount(() => {
  window.removeEventListener('resize', close)
  window.removeEventListener('blur', close)
})

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault()
    close()
    return
  }
  const indices = enabledIndices.value
  if (indices.length === 0) return
  const cursor = indices.indexOf(activeIndex.value)
  if (event.key === 'ArrowDown') {
    event.preventDefault()
    setActiveIndex(indices[(cursor + 1) % indices.length] ?? indices[0])
  } else if (event.key === 'ArrowUp') {
    event.preventDefault()
    const next = cursor <= 0 ? indices.length - 1 : cursor - 1
    setActiveIndex(indices[next] ?? indices[indices.length - 1])
  } else if (event.key === 'Enter' && activeIndex.value >= 0) {
    event.preventDefault()
    select(state.items[activeIndex.value] as MenuItem)
  }
}

/** 悬停父项：打开子菜单并按其屏幕位置定位 */
function hoverEntry(item: MenuItem, index: number, event: MouseEvent): void {
  setActiveIndex(index)
  if (!item.children) return
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect()
  const margin = 8
  const placeLeft = rect.right + MENU_WIDTH > window.innerWidth - margin
  submenuStyle.value = {
    top: `${rect.top}px`,
    ...(placeLeft ? { right: `${window.innerWidth - rect.left + 2}px` } : { left: `${rect.right - 2}px` }),
  }
  submenuParent.value = item
}

function runChild(child: MenuItem): void {
  if (child.disabled || child.divider) return
  const action = child.onClick
  close()
  action?.()
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="state.open"
      class="ctx-backdrop"
      @click="close"
      @contextmenu.prevent="close"
    >
      <ul
        ref="rootRef"
        class="context-menu"
        :style="positionStyle"
        role="menu"
        tabindex="-1"
        @keydown="onKeydown"
        @click.stop
        @contextmenu.prevent.stop
      >
        <li v-if="state.title" class="context-menu__title">{{ state.title }}</li>

        <template v-for="(item, index) in state.items" :key="item.key ?? `d-${index}`">
          <li v-if="item.divider" class="context-menu__divider" role="separator"></li>
          <li
            v-else
            class="context-menu__entry"
            :class="{
              'context-menu__entry--active': activeIndex === index,
              'context-menu__entry--disabled': item.disabled,
              'context-menu__entry--parent-open': item.children && openSubmenuKey === item.key,
            }"
            @mouseenter="hoverEntry(item, index, $event)"
          >
            <button
              type="button"
              class="context-menu__item"
              :class="{ 'context-menu__item--danger': item.danger }"
              role="menuitem"
              :disabled="item.disabled"
              @click="select(item)"
            >
              <span class="context-menu__icon">
                <Icon v-if="item.icon" :name="item.icon" :size="16" />
              </span>
              <span class="context-menu__label">{{ item.label }}</span>
              <span v-if="item.shortcut" class="context-menu__shortcut">{{ item.shortcut }}</span>
              <Icon v-if="item.children" name="chevron-right" :size="14" class="context-menu__arrow" />
            </button>
          </li>
        </template>
      </ul>

      <!-- 一层子菜单（添加到歌单等），独立 fixed 定位 -->
      <ul
        v-if="submenuParent?.children && openSubmenuKey === submenuParent.key"
        class="context-menu context-menu--sub"
        :style="submenuStyle"
        role="menu"
        @click.stop
        @contextmenu.prevent.stop
        @mouseleave="submenuParent = null"
      >
        <li v-if="submenuParent.children.length === 0" class="context-menu__empty">（暂无歌单）</li>
        <template v-for="(child, childIndex) in submenuParent.children" :key="child.key ?? `c-${childIndex}`">
          <li v-if="child.divider" class="context-menu__divider" role="separator"></li>
          <li v-else>
            <button
              type="button"
              class="context-menu__item"
              :class="{ 'context-menu__item--danger': child.danger }"
              role="menuitem"
              :disabled="child.disabled"
              @click="runChild(child)"
            >
              <span class="context-menu__icon">
                <Icon v-if="child.icon" :name="child.icon" :size="16" />
              </span>
              <span class="context-menu__label">{{ child.label }}</span>
            </button>
          </li>
        </template>
      </ul>
    </div>
  </Teleport>
</template>

<style scoped>
.ctx-backdrop {
  position: fixed;
  inset: 0;
  z-index: var(--z-dropdown);
  /* 透明遮罩用于截获外点，不可拦截鼠标移动给底层 */
  background: transparent;
}

.context-menu {
  position: fixed;
  width: 208px;
  max-height: 70vh;
  margin: 0;
  padding: var(--space-1);
  overflow-y: auto;
  background-color: var(--color-bg-elevated);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-elevation-3);
}

.context-menu:focus {
  outline: none;
}

.context-menu--sub {
  z-index: calc(var(--z-dropdown) + 1);
}

.context-menu__title {
  padding: var(--space-1) var(--space-3) var(--space-2);
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

.context-menu__divider {
  height: 1px;
  margin: var(--space-1) calc(var(--space-2) * -1);
  background-color: var(--color-border-subtle);
}

.context-menu__empty {
  padding: var(--space-2) var(--space-3);
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

.context-menu__entry {
  position: relative;
  border-radius: var(--radius-sm);
}

.context-menu__entry--active,
.context-menu__entry--parent-open {
  background-color: var(--color-bg-hover);
}

.context-menu__entry--disabled {
  opacity: 0.5;
}

.context-menu__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  min-height: 32px;
  padding: 0 var(--space-2);
  font-size: var(--font-size-sm);
  color: var(--color-text-primary);
  text-align: left;
  border-radius: var(--radius-sm);
}

.context-menu__item--danger {
  color: var(--color-danger);
}

.context-menu__icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--size-icon-md);
  height: var(--size-icon-md);
  flex: 0 0 auto;
  color: var(--color-text-secondary);
}

.context-menu__item--danger .context-menu__icon {
  color: var(--color-danger);
}

.context-menu__label {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.context-menu__shortcut {
  flex: 0 0 auto;
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

.context-menu__arrow {
  flex: 0 0 auto;
  color: var(--color-text-tertiary);
}
</style>
