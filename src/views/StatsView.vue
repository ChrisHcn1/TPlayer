<script setup lang="ts">
/**
 * 收听报告（/stats）
 *
 * 数据源：stats store（本地播放统计，localStorage 持久化，不上传）。
 * 视图负责聚合展示：总播放时长 / 播放曲目数 / 播放次数 / 最近播放、
 * TOP 曲目 / TOP 艺术家 / 最常听时段（24 小时分布），并支持导出分享卡片 PNG。
 *
 * 样式约束：只引用 theme.css 语义 token，禁止字面量色值 / 像素。
 * 入口：设置 → 播放与媒体 → 收听报告（由"工具"页签独立而来；/stats 独立页保留兼容）。
 */
import { computed, ref, withDefaults, defineProps } from 'vue'
import { useRouter } from 'vue-router'

import { t } from '../i18n'
import { useStatsStore } from '../stores/stats'
import { useToastStore } from '../stores/toast'
import { exportShareCardPng, formatDuration } from '../services/shareCard'
import BaseButton from '../components/common/BaseButton.vue'
import Icon from '../components/common/Icon.vue'
import EmptyState from '../components/common/EmptyState.vue'

// embedded：作为设置页"收听报告"页签内嵌展示时隐藏页头（标题/返回按钮由设置页承载）；
// 作为 /stats 独立页展示时保留完整页头。默认独立页行为。
withDefaults(defineProps<{ embedded?: boolean }>(), {
  embedded: false,
})

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
      toastStore.success(t('stats.exportDone', { path: result.path }))
    } else if (result.ok) {
      toastStore.info(t('stats.exportDownloadStarted'))
    } else if (!result.cancelled) {
      toastStore.error(
        result.path ? t('stats.exportFailedWith', { path: result.path }) : t('stats.exportFailed'),
      )
    }
  } finally {
    exporting.value = false
  }
}
</script>

<template>
  <section class="view" :class="{ 'view--embedded': embedded }">
    <div v-if="!embedded" class="view__header">
      <div class="view__heading">
        <h1 class="view__title">{{ t('stats.title') }}</h1>
        <p class="view__subtitle">{{ t('stats.subtitle') }}</p>
      </div>
      <div class="view__actions">
        <BaseButton
          size="sm"
          variant="primary"
          :disabled="!hasStats || exporting"
          @click="onExport"
        >
          <Icon name="download" :size="14" />
          {{ exporting ? t('stats.exporting') : t('stats.exportShareCard') }}
        </BaseButton>
        <BaseButton size="sm" @click="router.push('/settings')">
          <Icon name="back" :size="14" />
          {{ t('stats.backToSettings') }}
        </BaseButton>
      </div>
    </div>

    <EmptyState
      v-if="!hasStats"
      icon="clock"
      :title="t('stats.emptyTitle')"
      :description="t('stats.emptyDesc')"
    />

    <template v-else>
      <div class="stats-grid">
        <div class="stat-card stat-card--accent">
          <div class="stat-card__value">{{ formatDuration(totalPlayMs) }}</div>
          <div class="stat-card__label">{{ t('stats.totalDuration') }}</div>
        </div>
        <div class="stat-card">
          <div class="stat-card__value">{{ playedTrackCount }}</div>
          <div class="stat-card__label">{{ t('stats.playedTracks') }}</div>
        </div>
        <div class="stat-card">
          <div class="stat-card__value">{{ totalPlayCount }}</div>
          <div class="stat-card__label">{{ t('stats.totalPlayCount') }}</div>
        </div>
        <div class="stat-card">
          <div class="stat-card__value stat-card__value--sm">{{ formatLastPlayed(lastPlayedAt) }}</div>
          <div class="stat-card__label">{{ t('nav.recent') }}</div>
        </div>
      </div>

      <div class="stats-cols">
        <section class="panel">
          <h2 class="panel__title">{{ t('stats.topTracks') }}</h2>
          <ol class="rank-list">
            <li v-for="(item, index) in topTracks" :key="item.trackId" class="rank-row">
              <span class="rank-badge">{{ index + 1 }}</span>
              <div class="rank-row__main">
                <div class="rank-row__title">{{ item.title }}</div>
                <div class="rank-row__meta">
                  {{ item.artists.length > 0 ? item.artists.join(' / ') : t('stats.unknownArtist') }}
                </div>
              </div>
              <span class="rank-row__count">{{ t('stats.playsWithDuration', { count: item.playCount, duration: formatDuration(item.totalMs) }) }}</span>
            </li>
          </ol>
        </section>

        <section class="panel">
          <h2 class="panel__title">{{ t('stats.topArtists') }}</h2>
          <ol class="rank-list">
            <li v-for="(item, index) in topArtists" :key="item.name" class="rank-row">
              <span class="rank-badge">{{ index + 1 }}</span>
              <div class="rank-row__main">
                <div class="rank-row__title">{{ item.name }}</div>
                <div class="rank-row__meta">{{ t('stats.artistsAggregate') }}</div>
              </div>
              <span class="rank-row__count">{{ t('stats.plays', { count: item.playCount }) }}</span>
            </li>
          </ol>
        </section>
      </div>

      <section class="panel">
        <h2 class="panel__title">
          {{ t('stats.peakHours') }}
          <span class="panel__hint">{{ t('stats.peakHint', { hour: peakHour, next: peakHour + 1 }) }}</span>
        </h2>
        <div class="hour-chart" role="img" :aria-label="t('stats.chartAria', { hour: peakHour })">
          <div
            v-for="bar in hourBars"
            :key="bar.hour"
            class="hour-chart__bar"
            :class="{ 'hour-chart__bar--peak': bar.isPeak, 'hour-chart__bar--empty': bar.count === 0 }"
            :style="bar.count > 0 ? { height: `${bar.pct}%` } : undefined"
            :title="t('stats.barTip', { hour: bar.hour, count: bar.count })"
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

/* 作为设置页页签内嵌展示：不再强占整页高度，由设置内容区自行滚动 */
.view--embedded {
  min-height: 0;
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
  min-width: 0;
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
  /* 时长 / 日期等带单位或空格的数值在窄卡片内换行，避免溢出被相邻卡片遮盖 */
  white-space: normal;
  overflow-wrap: anywhere;
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
