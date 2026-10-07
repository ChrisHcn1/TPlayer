<script setup lang="ts">
/**
 * 空状态占位组件（通用）
 * 统一各页面的“无数据”表现，避免每个视图各写一套。
 */
import Icon from './Icon.vue'

interface Props {
  /** 主文案 */
  title?: string
  /** 补充说明 */
  description?: string
  /** 语义图标名（默认空盒） */
  icon?: string
}

withDefaults(defineProps<Props>(), {
  title: '暂无内容',
  description: '',
  icon: 'inbox',
})
</script>

<template>
  <div class="empty-state">
    <Icon class="empty-state__icon" :name="icon" :size="32" />
    <p class="empty-state__title">{{ title }}</p>
    <p v-if="description" class="empty-state__description">{{ description }}</p>
    <div v-if="$slots.default" class="empty-state__action">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  min-height: 200px;
  padding: var(--space-8) var(--space-4);
  color: var(--color-text-secondary);
  text-align: center;
}

.empty-state__icon {
  color: var(--color-text-tertiary);
}

.empty-state__title {
  margin: 0;
  font-size: var(--font-size-md);
  color: var(--color-text-secondary);
}

.empty-state__description {
  margin: 0;
  font-size: var(--font-size-sm);
  color: var(--color-text-tertiary);
}

.empty-state__action {
  margin-top: var(--space-3);
}
</style>
