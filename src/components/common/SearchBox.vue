<script setup lang="ts">
/**
 * 搜索框（通用组件，纯展示 + 事件抛出）
 *
 * DESIGN 依据：§4.4 SearchBox —— 标题栏内高 28px；视图工具条内取标准控件高。
 * 约束：本组件不直接调用服务层，搜索请求由父级（标题栏 / 视图）转发到路由或 library store。
 *
 * 变体：
 *   titlebar —— 标题栏内使用（§4.3）：高 28px、圆角 --radius-sm，宽度由父容器给定（320px）；
 *   inline   —— 视图工具条内使用（§2.4）：高 --size-control-h、圆角 --radius-md、宽 260px。
 * 尺寸差异只由 variant 决定，调用方不再传尺寸或内联样式。
 */
interface Props {
  modelValue?: string
  placeholder?: string
  variant?: 'titlebar' | 'inline'
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: '',
  placeholder: '搜索曲目 / 专辑 / 歌手',
  variant: 'inline',
})

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
  /** 输入即触发（供即时筛选场景使用） */
  (e: 'search', value: string): void
  /** 回车提交（供标题栏跳转等场景使用） */
  (e: 'submit', value: string): void
}>()

function onInput(event: Event) {
  const value = (event.target as HTMLInputElement).value
  emit('update:modelValue', value)
  emit('search', value)
}

function onSubmit() {
  emit('submit', props.modelValue)
}
</script>

<template>
  <input
    class="search-box"
    :class="`search-box--${variant}`"
    type="search"
    :value="modelValue"
    :placeholder="placeholder"
    @input="onInput"
    @keydown.enter="onSubmit"
  />
</template>

<style scoped>
.search-box {
  width: 100%;
  min-width: 0;
  color: var(--color-text-primary);
  background-color: var(--color-bg-input);
  border: 1px solid var(--color-border-subtle);
  outline: none;
  transition: var(--transition-colors);
}

/* 标题栏变体（§4.3 / §4.4）：高 28px 为规范值，theme.css 未提供同名 token */
.search-box--titlebar {
  height: 28px;
  padding: 0 var(--space-3);
  font-size: var(--font-size-sm);
  border-radius: var(--radius-sm);
}

/* 视图工具条变体（§2.4 / §4.4）：标准控件高、宽 260px */
.search-box--inline {
  width: 260px;
  height: var(--size-control-h);
  padding: 0 var(--space-3);
  font-size: var(--font-size-sm);
  border-radius: var(--radius-md);
}

.search-box::placeholder {
  color: var(--color-text-tertiary);
}

.search-box:hover {
  border-color: var(--color-border);
}

/* 焦点用边框色表达，而非外扩 outline：标题栏仅 36px 高，外扩焦点环会被裁切（§6.2） */
.search-box:focus {
  border-color: var(--color-focus-ring);
}
</style>
