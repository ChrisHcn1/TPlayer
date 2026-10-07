<script setup lang="ts">
/**
 * 侧栏导航（布局组件）
 * DESIGN 依据：§1.1 主导航项清单（曲库组 → 工具组 → 底部固定组）、§4.2 SideBar / NavItem 规格。
 *
 * 数据来源：路由表 meta —— nav（分组）+ titleKey（文案键）+ icon（§3.7 语义图标名）。
 * 菜单项不在本组件硬编码：新增视图只要在路由表声明 meta.nav / meta.titleKey / meta.icon
 * 即自动出现在对应分组（§1.4）；文案一律经 i18n/nav 解析，组件内不出现导航文案字面量。
 *
 * 引用 token（全部取自 assets/styles/theme.css 语义层，组件内不写死色值）：
 *   尺寸 --size-icon-md · --space-1/2/3 · --radius-sm · --radius-full · --line-height-sm
 *   色彩 --color-bg-sidebar/-hover/-active/-selected · --color-text-secondary/-primary
 *        · --color-text-tertiary · --color-text-disabled · --color-brand · --color-border-subtle
 *   其他 --font-size-base · --font-size-2xs · --font-weight-medium · --transition-colors · --shadow-focus
 *
 * M0 未接入项（代码内已标注，不留"可点击但不生效"的假入口）：
 *  1) 图标：§3.7 图标集（icon-{语义}.svg 雪碧）M1 接入，本轮按 §4.2 保留 20px 图标位，
 *     以稳定行高与文本对齐（§3.9.5 防布局跳动）；不使用图片图标与 emoji（§3.7 禁止）。
 *  2) 折叠：折叠状态由布局外壳统一持有（composables/useLayout.ts，§2.3 折叠三原则 1），
 *     本组件只"读取状态 + 触发切换"，不自行维护折叠标志；折叠把手位于侧栏右缘垂直居中；
 *     窗口高度 < 560px 时底部组转图标行属 M1。
 */
import { computed } from 'vue'
import { RouterLink, useRoute, type RouteMeta } from 'vue-router'

import { router } from '../../router'
import { useTheme } from '../../composables/useTheme'
import { useLayout } from '../../composables/useLayout'
import { NAV_GROUPS, resolveGroupLabel, resolveNavLabel } from '../../i18n/nav'
import Icon from '../common/Icon.vue'

interface SideBarItem {
  /** 路由 name：当前项判定与二级父项高亮的依据 */
  name: string
  path: string
  /** 已解析的展示文案 */
  label: string
  /** §3.7 语义图标名（来自路由 meta.icon） */
  icon: string
}

/** meta 为弱类型（RouteMeta 索引签名），逐字段守卫后再使用，避免脏 meta 直接进入渲染 */
function metaString(meta: RouteMeta | undefined, key: string): string | undefined {
  const value = meta?.[key]
  return typeof value === 'string' && value.length > 0 ? value : undefined
}

/**
 * 侧栏项判定（§1.1）：nav 分组 + titleKey + 导航图标，且非二级详情（meta.parent 为空）。
 * 说明：/search 在 §1.4 中归 library 分组，但按 §1.1 不占侧栏项（由标题栏搜索框承载），
 * 其 meta 未声明 icon，因此天然被排除——组件内无需维护例外名单。
 * 分组与组内顺序均取路由表声明顺序，与 §1.1 清单一致。
 */
const sidebarGroups = computed(() =>
  NAV_GROUPS.map((group) => ({
    group,
    label: resolveGroupLabel(group),
    items: router.getRoutes().reduce<SideBarItem[]>((items, record) => {
      if (typeof record.name !== 'string') return items
      if (metaString(record.meta, 'nav') !== group) return items
      if (metaString(record.meta, 'parent') !== undefined) return items
      if (metaString(record.meta, 'icon') === undefined) return items
      const titleKey = metaString(record.meta, 'titleKey')
      if (titleKey === undefined) return items
      const icon = metaString(record.meta, 'icon')
      if (icon === undefined) return items
      items.push({ name: record.name, path: record.path, label: resolveNavLabel(titleKey), icon })
      return items
    }, []),
  })).filter((section) => section.items.length > 0),
)

/** 曲库组 / 工具组随可用高度滚动；底部固定组（§4.2）吸附侧栏底部，不随滚动位移 */
const scrollGroups = computed(() =>
  sidebarGroups.value.filter((section) => section.group !== 'bottom'),
)
const bottomItems = computed(
  () => sidebarGroups.value.find((section) => section.group === 'bottom')?.items ?? [],
)

const route = useRoute()
const { theme, toggleTheme } = useTheme()
// 折叠状态与切换方法均来自布局外壳（§2.3 折叠三原则 1）：本组件不持有折叠标志
const { isSidebarCollapsed, toggleSidebar } = useLayout()

/** 当前项判定：命中自身，或命中其二级详情（详情路由的 meta.parent 指向本项，§1.4） */
function isCurrent(name: string): boolean {
  return route.name === name || metaString(route.meta, 'parent') === name
}

/** 主题切换按钮显示"将要切换到的主题"（§4.2 底部固定组；文案与状态均来自 useTheme 单一事实源） */
const themeActionLabel = computed(() =>
  resolveNavLabel(theme.value === 'dark' ? 'nav.theme-to-light' : 'nav.theme-to-dark'),
)
const themeActionText = computed(() => `${resolveNavLabel('nav.theme')}：${themeActionLabel.value}`)

/**
 * 折叠把手文案（§2.3 折叠三原则 1：状态由外壳统一持有）
 * 文案随状态切换：展开态显示"折叠"，折叠态显示"展开"，用于 aria-label / title。
 */
const collapseActionLabel = computed(() =>
  resolveNavLabel(isSidebarCollapsed.value ? 'nav.expand' : 'nav.collapse'),
)
const collapseHint = computed(() =>
  resolveNavLabel(isSidebarCollapsed.value ? 'nav.expand-hint' : 'nav.collapse-hint'),
)
</script>

<template>
  <nav
    class="sidebar"
    :class="{ 'sidebar--collapsed': isSidebarCollapsed }"
    aria-label="主导航"
  >
    <!--
      折叠把手：骑在侧栏与内容区的分隔线上（垂直居中的小圆钮），
      不占底部导航位；状态仍由外壳统一持有（§2.3）。
    -->
    <button
      type="button"
      class="sidebar__collapse-handle"
      :aria-label="collapseActionLabel"
      :title="collapseHint"
      :aria-expanded="!isSidebarCollapsed"
      @click="toggleSidebar"
    >
      <Icon
        :name="isSidebarCollapsed ? 'chevron-right' : 'chevron-left'"
        :size="15"
      />
    </button>

    <!-- 曲库组 / 工具组：内容超出时仅本区域滚动，底部固定组不随滚动位移（§2.3） -->
    <div class="sidebar__scroll u-scroll">
      <section v-for="section in scrollGroups" :key="section.group" class="sidebar__group">
        <h2 class="sidebar__group-title">{{ section.label }}</h2>
        <ul class="sidebar__list">
          <li v-for="item in section.items" :key="item.name">
            <RouterLink
              class="sidebar__item"
              :class="{ 'sidebar__item--current': isCurrent(item.name) }"
              :aria-current="isCurrent(item.name) ? 'page' : undefined"
              :to="item.path"
            >
              <Icon class="sidebar__icon" :name="item.icon" :size="20" />
              <span class="sidebar__label">{{ item.label }}</span>
            </RouterLink>
          </li>
        </ul>
      </section>
    </div>

    <!--
      底部固定组（§1.1 第 9 项 + §4.2）：设置项来自路由 nav=bottom，
      另加主题切换与折叠按钮；折叠按钮固定在底部，与设置同级（§2.3 折叠三原则 1）
    -->
    <div class="sidebar__footer">
      <RouterLink
        v-for="item in bottomItems"
        :key="item.name"
        class="sidebar__item"
        :class="{ 'sidebar__item--current': isCurrent(item.name) }"
        :aria-current="isCurrent(item.name) ? 'page' : undefined"
        :to="item.path"
      >
        <Icon class="sidebar__icon" :name="item.icon" :size="20" />
        <span class="sidebar__label">{{ item.label }}</span>
      </RouterLink>

      <button
        type="button"
        class="sidebar__item"
        :aria-label="themeActionText"
        :title="themeActionText"
        @click="toggleTheme"
      >
        <Icon class="sidebar__icon" :name="theme === 'dark' ? 'sun' : 'moon'" :size="20" />
        <span class="sidebar__label">{{ themeActionLabel }}</span>
      </button>
    </div>
  </nav>
</template>

<style scoped>
.sidebar {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  padding: var(--space-3) var(--space-2);
  background-color: var(--color-bg-sidebar);
  /* 封面皮肤：右上角极淡主色染（--color-cover-wash 默认透明） */
  background-image: radial-gradient(
    130% 70% at 90% 0%,
    var(--color-cover-wash, transparent),
    transparent 62%
  );
  /* 与内容区的分隔线由布局壳（App.vue 的 .app-shell__sidebar）绘制，此处不重复画线 */
}

/* 折叠把手：骑在右侧分隔线上的小圆钮，垂直居中 */
.sidebar__collapse-handle {
  position: absolute;
  top: 50%;
  right: -13px;
  z-index: 10;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  color: var(--color-text-tertiary);
  background-color: var(--color-bg-elevated);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-full);
  box-shadow: var(--shadow-elevation-2);
  cursor: pointer;
  transform: translateY(-50%);
  transition: color 0.15s ease, border-color 0.15s ease, transform 0.15s ease;
}

.sidebar__collapse-handle:hover {
  color: var(--color-brand);
  border-color: var(--color-brand);
}

.sidebar__collapse-handle:focus-visible {
  outline: none;
  box-shadow: var(--shadow-focus);
}

.sidebar__scroll {
  flex: 1 1 auto;
  min-height: 0;
  /* 滚动槽位常驻：长列表滚动时文本不横向跳动（§3.9.2） */
  scrollbar-gutter: stable;
}

.sidebar__group + .sidebar__group {
  margin-top: var(--space-2);
  padding-top: var(--space-2);
  border-top: 1px solid var(--color-border-subtle);
}

.sidebar__group-title {
  padding: 0 var(--space-3);
  font-size: var(--font-size-2xs);
  font-weight: var(--font-weight-medium);
  line-height: var(--line-height-2xs);
  color: var(--color-text-tertiary);
}

.sidebar__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin-top: var(--space-1);
}

.sidebar__footer {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin-top: var(--space-2);
  padding-top: var(--space-2);
  border-top: 1px solid var(--color-border-subtle);
}

.sidebar__item {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  /* 行盒高取 --line-height-sm（20px，与图标等高）：项高 = 20 + --space-2 × 2 = 36px（§4.2） */
  padding: var(--space-2) var(--space-3);
  font-size: var(--font-size-base);
  line-height: var(--line-height-sm);
  color: var(--color-text-secondary);
  text-align: left;
  text-decoration: none;
  background-color: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  transition: var(--transition-colors);
}

.sidebar__item:not(:disabled):hover {
  color: var(--color-text-primary);
  background-color: var(--color-bg-hover);
}

.sidebar__item:not(:disabled):active {
  background-color: var(--color-bg-active);
}

.sidebar__item:focus-visible {
  /* 键盘焦点环统一走 --shadow-focus；不删除 :focus-visible 本身，键盘可达性红线（§6.2） */
  outline: none;
  box-shadow: var(--shadow-focus);
}

/* 当前项（含二级详情页对父项的高亮，§1.4）：选中背景 + 品牌色文本 + 左侧 3px 指示条（§4.2） */
.sidebar__item--current,
.sidebar__item--current:not(:disabled):hover {
  color: var(--color-brand);
  background-color: var(--color-bg-selected);
}

.sidebar__item--current::before {
  content: '';
  position: absolute;
  inset: 0 auto 0 0;
  /* 3px 为 §4.2 / §3.9.6 规定的指示条宽度，theme.css 未提供对应 token */
  width: 3px;
  background-color: var(--color-brand);
  border-radius: var(--radius-full);
}

.sidebar__item:disabled {
  /* 禁用态：文字色由 main.css 的 button:disabled 统一给出，此处只补光标与背景（§4.2） */
  background-color: transparent;
  cursor: not-allowed;
}

/* 图标位：§3.7 图标集（icon-{语义}.svg）M1 接入，本轮仅占位以稳定行高与文本对齐（§3.9.5） */
.sidebar__icon {
  flex: 0 0 auto;
  width: var(--size-icon-md);
  height: var(--size-icon-md);
}

.sidebar__label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* --------------------------------------------------------------------------
   折叠态（§2.3 / §4.2）：列宽由外壳切至 --size-sidebar-collapsed（64px），
   本组件只负责"仅保留图标位与无障碍标签"
   -------------------------------------------------------------------------- */
.sidebar--collapsed .sidebar__item {
  justify-content: center;
  padding-inline: var(--space-2);
}

.sidebar--collapsed .sidebar__label,
.sidebar--collapsed .sidebar__group-title {
  /* 视觉隐藏而非 display:none —— 屏幕阅读器仍能读出项名与分组名（§6.4） */
  position: absolute;
  width: 1px;
  height: 1px;
  margin: -1px;
  padding: 0;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
  border: 0;
}
</style>
