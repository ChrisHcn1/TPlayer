# 按钮样式统一指南

## 概述

本文档说明了 TPlayer 项目中按钮样式的统一系统，包括颜色主题、按钮类型和使用规范。

## 颜色主题

### CSS 变量

所有按钮颜色通过 CSS 变量定义,支持深色和浅色主题自动切换:

```css
/* 深色主题 */
--btn-primary: #5cb85c;           /* 主按钮颜色 (青绿色) */
--btn-primary-hover: #4aa34a;     /* 主按钮悬停色 */
--btn-secondary: #3a3a3a;         /* 次要按钮颜色 */
--btn-secondary-hover-light: #4a4a4a; /* 次要按钮悬停色 */
--btn-danger: #d9534f;             /* 危险按钮颜色 (柔和红色) */
--btn-danger-hover: #c9302c;      /* 危险按钮悬停色 */
--btn-success: #5cb85c;            /* 成功按钮颜色 (青绿色) */
--btn-success-hover: #4aa34a;      /* 成功按钮悬停色 */
--btn-info: #5bc0de;              /* 信息按钮颜色 (天蓝色) */
--btn-info-hover: #46b8da;         /* 信息按钮悬停色 */

/* 浅色主题自动切换 */
--btn-secondary: #e0e0e0;
--btn-secondary-hover-light: #d0d0d0;
--btn-info: #5cb85c;              /* 浅色主题信息按钮改为绿色以保持一致性 */
```

### 颜色说明

- **主色调 (#5cb85c)**: 柔和的青绿色，比原来的 #4CAF50 更柔和、更现代
- **危险色 (#d9534f)**: 柔和的红色，比原来的 #ff4757 更温和
- **信息色 (#5bc0de)**: 清新的天蓝色，在浅色主题下自动切换为主色调
- **阴影统一**: 所有绿色阴影都使用 rgba(92, 184, 92, ...)，确保色调一致

## 按钮类型

### 1. 基础按钮 (.btn)

所有按钮的统一样式基础:

```html
<button class="btn">默认按钮</button>
```

**特性:**
- 统一的圆角: 6px
- 统一的内边距: 10px 16px
- 统一的过渡效果: 0.2s ease
- 悬停时轻微上移: translateY(-1px)
- 阴影效果: 0 2px 6px rgba(0, 0, 0, 0.15)
- 禁用状态: opacity 0.5

### 2. 主要按钮 (.btn.primary)

用于主要的操作按钮(如"保存"、"确定"等):

```html
<button class="btn primary">保存</button>
```

**样式:** 绿色背景 (#4CAF50)

### 3. 次要按钮 (.btn.secondary)

用于次要的操作按钮(如"取消"、"关闭"等):

```html
<button class="btn secondary">取消</button>
```

**样式:** 深色/浅色背景,带边框

### 4. 危险按钮 (.btn.danger)

用于危险操作(如"删除"等):

```html
<button class="btn danger">删除</button>
```

**样式:** 红色背景 (#ff4757)

### 5. 成功按钮 (.btn.success)

用于成功状态操作:

```html
<button class="btn success">完成</button>
```

**样式:** 绿色背景 (#4CAF50)

### 6. 信息按钮 (.btn.info)

用于信息提示操作:

```html
<button class="btn info">信息</button>
```

**样式:** 深色主题蓝色 (#4a90e2), 浅色主题绿色 (#4CAF50)

### 7. 图标按钮 (.control-btn, .action-btn)

用于只包含图标的按钮:

```html
<button class="control-btn">⏮</button>
<button class="action-btn">❤️</button>
```

**特性:**
- 透明背景
- 悬停时显示背景色
- 圆角: 6px
- 收藏按钮激活时显示红色

### 8. 浮动按钮 (.float-button)

用于浮动在页面上的按钮(如播放控制):

```html
<button class="float-button">▶</button>
```

**样式:** 圆形绿色按钮,直径 48px

### 9. 标签页按钮 (.tab-button)

用于标签页切换:

```html
<button class="tab-button active">标签1</button>
<button class="tab-button">标签2</button>
```

**特性:**
- 透明背景
- 激活时底部绿色边框
- 悬停时颜色变化

### 10. 搜索按钮 (.search-btn)

用于搜索框的搜索图标按钮:

```html
<button class="search-btn">🔍</button>
```

**样式:** 圆角 6px,悬停时背景色

### 11. 关闭按钮 (.close-btn)

用于模态框关闭:

```html
<button class="close-btn">×</button>
```

**样式:** 纯文本按钮,悬停时背景色

## 特殊按钮

### 播放按钮

播放控制区的播放按钮有特殊样式:

```css
.playback-controls .control-btn.play {
  width: 40px;
  height: 40px;
  font-size: 20px;
  background-color: var(--btn-success);
  color: #ffffff;
  border-radius: 50%;
}
```

### 收藏按钮

收藏按钮激活时显示红色:

```css
.action-btn.favorite.active {
  color: var(--btn-danger);
}
```

## 使用规范

### 1. 按钮选择原则

- **主要操作** → `.btn.primary` (绿色)
- **次要操作** → `.btn.secondary` (灰色)
- **危险操作** → `.btn.danger` (红色)
- **成功状态** → `.btn.success` (绿色)
- **信息提示** → `.btn.info` (蓝色/绿色)
- **仅图标** → `.control-btn` 或 `.action-btn`

### 2. 按钮组

对于多个按钮组,建议使用:

```html
<div class="settings-actions">
  <button class="btn secondary">取消</button>
  <button class="btn primary">保存</button>
</div>
```

### 3. 按钮状态

所有按钮都支持以下状态:

```html
<!-- 正常状态 -->
<button class="btn primary">正常</button>

<!-- 禁用状态 -->
<button class="btn primary" disabled>禁用</button>

<!-- 激活状态 (适用于标签页、收藏等) -->
<button class="tab-button active">激活</button>
<button class="action-btn favorite active">已收藏</button>
```

## 主题适配

### 深色主题 (默认)

```css
.tplayer-container {
  --btn-primary: #4CAF50;
  --btn-secondary: #3a3a3a;
  /* ... */
}
```

### 浅色主题

```css
.tplayer-container.light {
  --btn-primary: #4CAF50;
  --btn-secondary: #e0e0e0;
  /* ... */
}
```

按钮会自动适配当前主题,无需额外样式定义。

## 统一性原则

1. **颜色统一**: 所有操作类按钮使用绿色主题色
2. **圆角统一**: 按钮使用 6px 圆角,浮动/播放按钮使用圆角
3. **过渡统一**: 所有按钮使用 0.2s ease 过渡
4. **悬停效果**: 悬停时上移并增加阴影
5. **禁用状态**: 统一使用 opacity 0.5
6. **响应式**: 按钮在悬停和点击时有适当的视觉反馈

## 迁移指南

### 旧代码示例

```css
/* 旧样式 - 硬编码颜色 */
.btn-save {
  background-color: #4a90e2;  /* 蓝色 */
}
.btn-cancel {
  background-color: #3a3a3a;  /* 灰色 */
}
```

### 新代码示例

```css
/* 新样式 - 使用变量 */
.btn-save {
  background-color: var(--btn-success);  /* 绿色,统一主题 */
}
.btn-cancel {
  background-color: var(--btn-secondary-bg);  /* 适配主题 */
}
```

## 组件使用

### App.vue 中使用

```vue
<template>
  <button class="btn primary" @click="save">保存</button>
  <button class="btn secondary" @click="cancel">取消</button>
  <button class="control-btn" @click="play">▶</button>
  <button class="action-btn favorite" :class="{ active: isFavorite }">❤️</button>
</template>
```

### Settings.vue 中使用

```vue
<template>
  <div class="settings-actions">
    <button class="btn secondary" @click="cancel">取消</button>
    <button class="btn primary" @click="saveSettings">保存设置</button>
  </div>
</template>
```

## 未来扩展

如需添加新的按钮类型,请遵循以下步骤:

1. 在 CSS 变量中定义颜色
2. 创建 `.btn.new-type` 类
3. 使用 CSS 变量而不是硬编码颜色
4. 添加悬停和激活状态
5. 确保在深色和浅色主题下都能正常工作

## 总结

通过统一的按钮样式系统,TPlayer 实现了:

- ✅ 一致的颜色主题(绿色为主)
- ✅ 统一的交互效果
- ✅ 主题自适应
- ✅ 易于维护和扩展
- ✅ 良好的用户体验

所有按钮现在都遵循相同的设计语言,提供了一致且专业的用户体验。
