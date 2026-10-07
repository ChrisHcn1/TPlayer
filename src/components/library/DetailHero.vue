<script setup lang="ts">
/**
 * 详情页头部（专辑 / 艺术家 / 歌单 / 分轨通用）：
 * 大封面占位 + 标题 + 元信息 + 操作按钮插槽。
 */
import Icon from '../common/Icon.vue'

interface Props {
  icon: string
  title: string
  /** 主元信息行（艺术家 / 描述） */
  subtitle?: string
  /** 次行（年份 · 曲目数 · 时长） */
  meta?: string
  badge?: string
  /** 代表曲目封面 data URL（专辑/艺术家）；缺省时显示变体图标占位 */
  coverUrl?: string | null
}

withDefaults(defineProps<Props>(), {
  subtitle: '',
  meta: '',
  badge: '',
  coverUrl: null,
})
</script>

<template>
  <header class="detail-hero">
    <div class="detail-hero__cover">
      <img v-if="coverUrl" :src="coverUrl" alt="" draggable="false" class="detail-hero__img" />
      <Icon v-else :name="icon" :size="64" />
      <span v-if="badge" class="detail-hero__badge">{{ badge }}</span>
    </div>
    <div class="detail-hero__info">
      <h1 class="detail-hero__title">{{ title }}</h1>
      <p v-if="subtitle" class="detail-hero__subtitle">{{ subtitle }}</p>
      <p v-if="meta" class="detail-hero__meta">{{ meta }}</p>
      <div v-if="$slots.actions" class="detail-hero__actions">
        <slot name="actions" />
      </div>
    </div>
  </header>
</template>

<style scoped>
.detail-hero {
  display: flex;
  align-items: flex-end;
  gap: var(--space-5);
  padding: var(--space-4) 0 var(--space-5);
}

.detail-hero__cover {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 0 0 auto;
  width: var(--size-cover-lg);
  height: var(--size-cover-lg);
  overflow: hidden;
  color: var(--color-text-tertiary);
  background-color: var(--color-cover-placeholder);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-elevation-2);
}

.detail-hero__img {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  user-select: none;
}

.detail-hero__badge {
  position: absolute;
  top: var(--space-3);
  left: var(--space-3);
  padding: 2px var(--space-2);
  font-size: var(--font-size-2xs, 11px);
  color: var(--color-text-secondary);
  background-color: var(--color-bg-elevated);
  border-radius: var(--radius-full);
}

.detail-hero__info {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
  padding-bottom: var(--space-2);
}

.detail-hero__title {
  margin: 0;
  font-size: var(--font-size-2xl);
  font-weight: var(--font-weight-semibold);
  line-height: 1.2;
  color: var(--color-text-primary);
}

.detail-hero__subtitle {
  margin: 0;
  font-size: var(--font-size-md);
  color: var(--color-text-secondary);
}

.detail-hero__meta {
  margin: 0;
  font-size: var(--font-size-sm);
  color: var(--color-text-tertiary);
}

.detail-hero__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-top: var(--space-3);
}
</style>
