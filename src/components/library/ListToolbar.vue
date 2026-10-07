<script setup lang="ts">
/**
 * 列表工具条（DESIGN §4.7）：高 --size-toolbar-h。
 * 左：大标题 + 副信息 + 主操作插槽；右：视图工具插槽 + 内联筛选框。
 */
import SearchBox from '../common/SearchBox.vue'

interface Props {
  title: string
  subtitle?: string
  keyword?: string
  showFilter?: boolean
  filterPlaceholder?: string
}

withDefaults(defineProps<Props>(), {
  subtitle: '',
  keyword: '',
  showFilter: true,
  filterPlaceholder: '筛选本列表',
})

const emit = defineEmits<{
  (e: 'update:keyword', value: string): void
  (e: 'submit-keyword', value: string): void
}>()
</script>

<template>
  <div class="list-toolbar">
    <div class="list-toolbar__left">
      <h1 class="list-toolbar__title">{{ title }}</h1>
      <span v-if="subtitle" class="list-toolbar__subtitle">{{ subtitle }}</span>
      <div v-if="$slots.actions" class="list-toolbar__actions">
        <slot name="actions" />
      </div>
    </div>

    <div class="list-toolbar__right">
      <slot name="tools" />
      <SearchBox
        v-if="showFilter"
        variant="inline"
        :placeholder="filterPlaceholder"
        :model-value="keyword"
        @update:model-value="emit('update:keyword', $event)"
        @submit="emit('submit-keyword', $event)"
      />
    </div>
  </div>
</template>

<style scoped>
.list-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  min-height: var(--size-toolbar-h);
  padding: var(--space-2) var(--space-5);
}

.list-toolbar__left {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-width: 0;
}

.list-toolbar__title {
  margin: 0;
  font-size: var(--font-size-xl);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
}

.list-toolbar__subtitle {
  flex: 0 0 auto;
  font-size: var(--font-size-sm);
  color: var(--color-text-tertiary);
}

.list-toolbar__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-left: var(--space-2);
}

.list-toolbar__right {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: 0 0 auto;
}
</style>
