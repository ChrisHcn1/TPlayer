<script setup lang="ts">
/**
 * 收听报告（/stats）
 *
 * 数据源：stats store（本地播放统计，localStorage 持久化，不上传）。
 * 视图负责聚合展示：总播放时长 / 播放曲目数 / 播放次数 / 最近播放、
 * TOP 曲目 / TOP 艺术家 / 最常听时段（24 小时分布），并支持导出分享卡片 PNG。
 *
 * 样式约束：只引用 theme.css 语义 token，禁止字面量色值 / 像素。
 * 入口：设置 → 工具 → 收听报告（工具组不占侧栏，与转换器同策略）。
 */
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'

import { useStatsStore } from '../stores/stats'
import { useToastStore } from '../stores/toast'
import { exportShareCardPng, formatDuration } from '../services/shareCard'
import BaseButton from '../components/common/BaseButton.vue'
import Icon from '../components/common/Icon.vue'
import EmptyState from '../components/common/EmptyState.vue'

const router = useRouter()
const statsStore = useStatsStore()
const toastStore = useToastStore()

const exporting = ref(false)

const hasStats = computed(() => statsStore.playedTrackCount > 0)
const totalPlayMs = computed(() => statsStore.totalPlayMs)
const totalPlayCount = computed(() => statsStore.totalPlayCount)
const playedTrackCount = computed(() => statsStore.playedTrackCount)
const lastPlayedAt = computed(() => statsStore.lastPlayedAt)
const topTracks = computed(() => statsStore.topTracks(5))
const topArtists = computed(() => statsStore.topArtists(5))
const peakHour = computed(() => statsStore.peakHour)

/** 24 小时柱状数据（最常听时段分布；峰值小时高亮） */
const hourBars = computed(() => {
  const counts = statsStore.snapshot.hourCounts
  const max = Math.max(1, ...counts)
  return counts.map((count, hour) => ({
    hour,
    count,
    pct: count > 0 ? Math.max(4, Math.round((count / max) * 100)) : 0,
    isPeak: count > 0 && hour === peakHour.value,
  }))
})

/** 最近播放时间（Unix 秒 → "YYYY-MM-DD HH:mm"） */
function formatLastPlayed(unix: number | null): string {
  if (!unix) return '—'
  const d = new Date(unix * 1000)
  const pad = (n: number): string => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(
    d.getHours(),
  )}:${pad(d.getMinutes())}`
}

/** 导出分享卡片 PNG（Tauri 保存对话框；浏览器预览降级为下载） */
async function onExport(): Promise<void> {
  if (exporting.value) return
  exporting.value = true
  try {
    const result = await exportShareCardPng(statsStore.buildShareCardData())
    if (result.ok && result.path) {
      toastStore.success(`分享卡片已导出：${result.path}`)
    } else if (result.ok) {
      toastStore.info('分享卡片已开始下载（浏览器预览环境）')
    } else if (!result.cancelled) {
      toastStore.error(result.path ? `导出失败：${result.path}` : '导出失败，请重试')
    }
  } finally {
    exporting.value = false
  }
}
</script>

<template>
  <section class="view">
    <div class="view__header">
      <div class="view__heading">
        <h1 class="view__title">收听报告</h1>
        <p class="view__subtitle">本地播放统计：次数 / 时长 / 榜单 / 时段，仅存于本机，不上传</p>
      </div>
      <div class="view__actions">
        <BaseButton
          size="sm"
          variant="primary"
          :disabled="!hasStats || exporting"
          @click="onExport"
        >
          <Icon name="download" :size="14" />
          {{ exporting ? '导出中…' : '导出分享卡片' }}
        </BaseButton>
        <BaseButton size="sm" @click="router.push('/settings')">
          <Icon name="back" :size="14" />
          返回设置
        </BaseButton>
      </div>
    </div>

    <EmptyState
      v-if="!hasStats"
      icon="clock"
      title="还没有收听数据"
      description="播放过曲目后，这里会统计播放次数、累计时长与收听榜单"
    />

    <template v-else>
      <div class="stats-grid">
        <div class="stat-card stat-card--accent">
          <div class="stat-card__value">{{ formatDuration(totalPlayMs) }}</div>
          <div class="stat-card__label">累计播放时长</div>
        </div>
        <div class="stat-card">
          <div class="stat-card__value">{{ playedTrackCount }}</div>
          <div class="stat-card__label">播放曲目数</div>
        </div>
        <div class="stat-card">
          <div class="stat-card__value">{{ totalPlayCount }}</div>
          <div class="stat-card__label">累计播放次数</div>
        </div>
        <div class="stat-card">
          <div class="stat-card__value stat-card__value--sm">{{ formatLastPlayed(lastPlayedAt) }}</div>
          <div class="stat-card__label">最近播放</div>
        </div>
      </div>

      <div class="stats-cols">
        <section class="panel">
          <h2 class="panel__title">TOP 曲目</h2>
          <ol class="rank-list">
            <li v-for="(item, index) in topTracks" :key="item.trackId" class="rank-row">
              <span class="rank-badge">{{ index + 1 }}</span>
              <div class="rank-row__main">
                <div class="rank-row__title">{{ item.title }}</div>
                <div class="rank-row__meta">
                  {{ item.artists.length > 0 ? item.artists.join(' / ') : '未知艺术家' }}
                </div>
              </div>
              <span class="rank-row__count">{{ item.playCount }} 次 · {{ formatDuration(item.totalMs) }}</span>
            </li>
          </ol>
        </section>

        <section class="panel">
          <h2 class="panel__title">TOP 艺术家</h2>
          <ol class="rank-list">
            <li v-for="(item, index) in topArtists" :key="item.name" class="rank-row">
              <span class="rank-badge">{{ index + 1 }}</span>
              <div class="rank-row__main">
                <div class="rank-row__title">{{ item.name }}</div>
                <div class="rank-row__meta">按曲目播放量聚合</div>
              </div>
              <span class="rank-row__count">{{ item.playCount }} 次</span>
            </li>
          </ol>
        </section>
      </div>

      <section class="panel">
        <h2 class="panel__title">
          最常听时段
          <span class="panel__hint">{{ peakHour }} 点（{{ peakHour }}:00 - {{ peakHour + 1 }}:00）</span>
        </h2>
        <div class="hour-chart" role="img" :aria-label="`24 小时收听分布，峰值 ${peakHour} 点`">
          <div
            v-for="bar in hourBars"
            :key="bar.hour"
            class="hour-chart__bar"
            :class="{ 'hour-chart__bar--peak': bar.isPeak, 'hour-chart__bar--empty': bar.count === 0 }"
            :style="bar.count > 0 ? { height: `${bar.pct}%` } : undefined"
            :title="`${bar.hour} 点：${bar.count} 次`"
          />
        </div>
      </section>
    </template>
  </section>
</template>

<style scoped>
.view {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-height: 100%;
  color: var(--color-text-primary);
}

.view__header {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: var(--space-4);
}

.view__title {
  margin: 0;
  font-size: var(--font-size-2xl);
  font-weight: var(--font-weight-bold);
  line-height: var(--line-height-2xl);
}

.view__subtitle {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-sm);
  color: var(--color-text-tertiary);
}

.view__actions {
  display: flex;
  gap: var(--space-2);
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--space-3);
}

.stat-card {
  background: var(--color-bg-surface);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-lg);
  padding: var(--space-4);
}

.stat-card__value {
  font-size: var(--font-size-2xl);
  font-weight: var(--font-weight-bold);
  line-height: var(--line-height-2xl);
  color: var(--color-text-primary);
  white-space: nowrap;
}

.stat-card__value--sm {
  font-size: var(--font-size-md);
  line-height: var(--line-height-md);
  padding-top: var(--space-1);
}

.stat-card--accent .stat-card__value {
  color: var(--color-brand);
}

.stat-card__label {
  margin-top: var(--space-1);
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

.stats-cols {
  display: grid;
  grid-template-columns: 3fr 2fr;
  gap: var(--space-3);
}

.panel {
  background: var(--color-bg-surface);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-lg);
  padding: var(--space-4);
}

.panel__title {
  margin: 0 0 var(--space-3);
  font-size: var(--font-size-md);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-md);
}

.panel__hint {
  margin-left: var(--space-3);
  font-size: var(--font-size-xs);
  font-weight: var(--font-weight-regular);
  color: var(--color-text-tertiary);
}

.rank-list {
  margin: 0;
  padding: 0;
  list-style: none;
}

.rank-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) 0;
}

.rank-row + .rank-row {
  border-top: 1px solid var(--color-border-subtle);
}

.rank-badge {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  flex: 0 0 auto;
  border-radius: var(--radius-full);
  background: var(--color-brand-subtle);
  color: var(--color-brand);
  font-size: var(--font-size-xs);
  font-weight: var(--font-weight-bold);
}

.rank-row__main {
  flex: 1;
  min-width: 0;
}

.rank-row__title {
  font-size: var(--font-size-base);
  font-weight: var(--font-weight-medium);
  line-height: var(--line-height-base);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.rank-row__meta {
  margin-top: 2px;
  font-size: var(--font-size-xs);
  line-height: var(--line-height-xs);
  color: var(--color-text-tertiary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.rank-row__count {
  flex: 0 0 auto;
  font-size: var(--font-size-sm);
  line-height: var(--line-height-sm);
  color: var(--color-text-secondary);
}

.hour-chart {
  display: flex;
  align-items: flex-end;
  gap: var(--space-1);
  height: var(--space-16);
  margin-top: var(--space-3);
}

.hour-chart__bar {
  flex: 1;
  min-width: 2px;
  border-radius: var(--radius-xs) var(--radius-xs) 0 0;
  background: var(--color-brand-subtle);
}

.hour-chart__bar--peak {
  background: var(--color-brand);
}

.hour-chart__bar--empty {
  background: transparent;
}
</style>
