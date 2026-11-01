<script setup lang="ts">
/**
 * 列表工具条（DESIGN §4.7）：高 --size-toolbar-h。
 * 左：大标题 + 副信息 + 主操作插槽；右：视图工具插槽 + 内联筛选框。
 */
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import { t } from '../../i18n'
import SearchBox from '../common/SearchBox.vue'

interface Props {
  title: string
  subtitle?: string
  keyword?: string
  showFilter?: boolean
  filterPlaceholder?: string
}

const props = withDefaults(defineProps<Props>(), {
  subtitle: '',
  keyword: '',
  showFilter: true,
  filterPlaceholder: '',
})

const filterPlaceholderText = computed(
  () => props.filterPlaceholder || t('list.filterPlaceholder'),
)

const emit = defineEmits<{
  (e: 'update:keyword', value: string): void
  (e: 'submit-keyword', value: string): void
}>()

/**
 * 输入防抖时长（毫秒）。
 *
 * 性能：大曲库下每次按键都会触发一次全量过滤（O(n) 扫描 + 匹配），
 * 逐键执行会阻塞输入；此处停顿时只过滤一次。回车提交立即生效，不等防抖。
 * 对外契约不变：仍然抛出 `update:keyword` / `submit-keyword`，父组件用法无需调整。
 */
const DEBOUNCE_MS = 180

/** 本地即时值：保证输入框回显不丢字（父级 prop 更新滞后时仍显示用户已输入内容） */
const localKeyword = ref(props.keyword)
let timer: ReturnType<typeof setTimeout> | null = null

function cancelTimer(): void {
  if (timer !== null) {
    clearTimeout(timer)
    timer = null
  }
}

function onInput(value: string): void {
  localKeyword.value = value
  cancelTimer()
  timer = setTimeout(() => {
    timer = null
    emit('update:keyword', localKeyword.value)
  }, DEBOUNCE_MS)
}

function onSubmit(value: string): void {
  cancelTimer()
  localKeyword.value = value
  emit('update:keyword', value)
  emit('submit-keyword', value)
}

// 外部 keyword 变化（路由跳转 / 外部清空等）时同步本地值，并作废待发的防抖
watch(
  () => props.keyword,
  (value) => {
    if (value === localKeyword.value) return
    cancelTimer()
    localKeyword.value = value
  },
)

onBeforeUnmount(cancelTimer)
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
        :placeholder="filterPlaceholderText"
        :model-value="localKeyword"
        @update:model-value="onInput"
        @submit="onSubmit"
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
