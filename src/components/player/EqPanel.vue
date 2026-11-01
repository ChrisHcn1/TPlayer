<script setup lang="ts">
/**
 * 均衡器面板（正式版特性）
 *
 * 由 PlayerBar 的均衡器按钮唤起。内容在打开时挂载并加载后端已保存曲线。
 * 组成：
 *  - 总开关 + 输出增益（防削波 / 整体提亮）；
 *  - 实时合成幅频响应曲线（前端按 biquad 公式绘制，与听感一致）；
 *  - 预设场景卡片（覆盖不同使用需求，一键切换 + 平滑过渡）；
 *  - 10 段竖向滑块（拖动即时调音色，松手防抖下发）。
 */
import { computed, nextTick, onMounted, ref, watch } from 'vue'

import {
  applyPreset,
  computeResponseCurve,
  EQ_PRESETS,
  resetEqualizer,
  setBandGain,
  setEqEnabled,
  setOutputGain,
  useEqualizer,
  useEqualizerState,
} from '../../features/equalizer'
import Modal from '../common/Modal.vue'
import Icon from '../common/Icon.vue'
import { t } from '../../i18n'

const { config, preset, error, OUTPUT_GAIN_MIN, OUTPUT_GAIN_MAX } = useEqualizerState()

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ (e: 'close'): void }>()

const canvasRef = ref<HTMLCanvasElement | null>(null)

/** 预设列表（对象转数组） */
const presetList = computed(() =>
  Object.entries(EQ_PRESETS).map(([id, p]) => ({ id, nameKey: p.nameKey })),
)

/** 频段标签：≥1kHz 显示 1k / 16k，更紧凑 */
function bandLabel(freq: number): string {
  return freq >= 1000 ? `${freq / 1000}k` : `${freq}`
}

/** 响应曲线（对数频率 → dB），含输出增益；用于绘制与预览 */
const curve = computed(() => computeResponseCurve(config.value.bands))

function drawCurve(): void {
  const canvas = canvasRef.value
  if (!canvas) return
  const ctx = canvas.getContext('2d')
  if (!ctx) return
  const dpr = window.devicePixelRatio || 1
  const w = canvas.clientWidth
  const h = canvas.clientHeight
  canvas.width = Math.max(1, Math.floor(w * dpr))
  canvas.height = Math.max(1, Math.floor(h * dpr))
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, w, h)

  const dbMin = -15
  const dbMax = 15
  const padX = 6
  const padY = 10
  const plotW = w - padX * 2
  const plotH = h - padY * 2
  const xOf = (hz: number): number =>
    padX + ((Math.log10(hz) - Math.log10(20)) / (Math.log10(20000) - Math.log10(20))) * plotW
  const yOf = (db: number): number => {
    const clamped = Math.max(dbMin, Math.min(dbMax, db))
    return padY + (1 - (clamped - dbMin) / (dbMax - dbMin)) * plotH
  }

  // 网格与刻度（0dB 主中线 + ±6/±12 辅助线）
  ctx.lineWidth = 1
  ctx.font = '10px var(--font-family, system-ui)'
  ctx.textBaseline = 'middle'
  for (const db of [12, 6, 0, -6, -12]) {
    const y = yOf(db)
    ctx.strokeStyle =
      db === 0 ? 'rgba(127,127,127,0.35)' : 'rgba(127,127,127,0.15)'
    ctx.beginPath()
    ctx.moveTo(padX, y)
    ctx.lineTo(w - padX, y)
    ctx.stroke()
  }

  const { freqs, db } = curve.value
  if (freqs.length > 1) {
    // 曲线下填充（品牌色渐变）
    const grad = ctx.createLinearGradient(0, padY, 0, padY + plotH)
    grad.addColorStop(0, 'rgba(0,0,0,0)')
    grad.addColorStop(1, 'rgba(99,102,241,0.18)')
    ctx.beginPath()
    ctx.moveTo(xOf(freqs[0]), yOf(db[0]))
    for (let i = 1; i < freqs.length; i++) ctx.lineTo(xOf(freqs[i]), yOf(db[i]))
    ctx.lineTo(xOf(freqs[freqs.length - 1]), yOf(0))
    ctx.lineTo(xOf(freqs[0]), yOf(0))
    ctx.closePath()
    ctx.fillStyle = grad
    ctx.fill()

    // 曲线描边
    ctx.beginPath()
    ctx.moveTo(xOf(freqs[0]), yOf(db[0]))
    for (let i = 1; i < freqs.length; i++) ctx.lineTo(xOf(freqs[i]), yOf(db[i]))
    ctx.strokeStyle = '#6366f1'
    ctx.lineWidth = 2
    ctx.stroke()
  }
}

onMounted(async () => {
  useEqualizer()
  await nextTick()
  drawCurve()
})

watch(
  () => [config.value.bands, config.value.outputGainDb, props.open],
  async () => {
    await nextTick()
    drawCurve()
  },
  { deep: true },
)

function onToggleEnabled(event: Event): void {
  const el = event.target as HTMLInputElement
  void setEqEnabled(el.checked)
}

function onPreset(id: string): void {
  void applyPreset(id)
}

function onBandInput(index: number, event: Event): void {
  const el = event.target as HTMLInputElement
  setBandGain(index, Number(el.value))
}

function onOutputGain(event: Event): void {
  const el = event.target as HTMLInputElement
  setOutputGain(Number(el.value))
}
</script>

<template>
  <Modal :open="open" :title="t('player.eq')" size="lg" @close="emit('close')">
    <div class="eq">
      <!-- 顶部：总开关 + 输出增益 -->
      <div class="eq__top">
        <label class="eq__switch">
          <input type="checkbox" :checked="config.enabled" @change="onToggleEnabled" />
          <span class="eq__switch-track"><span class="eq__switch-thumb" /></span>
          <span class="eq__switch-label">{{ t('player.eqEnable') }}</span>
        </label>

        <div class="eq__output">
          <span class="eq__output-label">{{ t('player.eqOutputGain') }}</span>
          <input
            class="eq__output-slider"
            type="range"
            :min="OUTPUT_GAIN_MIN"
            :max="OUTPUT_GAIN_MAX"
            step="0.5"
            :value="config.outputGainDb"
            :aria-label="t('player.eqOutputGainAria', { db: config.outputGainDb })"
            @input="onOutputGain"
          />
          <span class="eq__output-value">{{ config.outputGainDb.toFixed(1) }} dB</span>
        </div>
      </div>

      <!-- 响应曲线 -->
      <div class="eq__curve">
        <canvas ref="canvasRef" class="eq__canvas" />
      </div>

      <!-- 预设场景 -->
      <div class="eq__section-title">{{ t('player.eqPresets') }}</div>
      <div class="eq__presets">
        <button
          v-for="p in presetList"
          :key="p.id"
          type="button"
          class="eq__preset"
          :class="{ 'eq__preset--active': preset === p.id }"
          @click="onPreset(p.id)"
        >
          {{ t(p.nameKey) }}
        </button>
      </div>

      <!-- 10 段滑块 -->
      <div class="eq__section-title">{{ t('player.eqBands') }}</div>
      <div class="eq__bands">
        <div v-for="(b, i) in config.bands" :key="b.freq" class="eq__band">
          <input
            class="eq__slider"
            type="range"
            min="-15"
            max="15"
            step="0.5"
            :value="b.gainDb"
            :aria-label="t('player.eqBandGainAria', { freq: b.freq })"
            @input="onBandInput(i, $event)"
          />
          <div class="eq__band-freq">{{ bandLabel(b.freq) }}</div>
          <div class="eq__band-gain">{{ b.gainDb.toFixed(1) }}</div>
        </div>
      </div>

      <p v-if="error" class="eq__error">{{ error }}</p>

      <div class="eq__footer">
        <button type="button" class="eq__reset" @click="resetEqualizer()">
          <Icon name="refresh" :size="14" />
          {{ t('player.eqReset') }}
        </button>
        <span class="eq__hint">{{ t('player.eqHint') }}</span>
      </div>
    </div>
  </Modal>
</template>

<style scoped>
.eq {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.eq__top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  flex-wrap: wrap;
}

/* 开关 */
.eq__switch {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  cursor: pointer;
  user-select: none;
}
.eq__switch input {
  position: absolute;
  opacity: 0;
  width: 0;
  height: 0;
}
.eq__switch-track {
  position: relative;
  width: 38px;
  height: 22px;
  border-radius: 999px;
  background-color: var(--color-bg-hover);
  transition: background-color 0.15s ease;
}
.eq__switch-thumb {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background-color: #fff;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
  transition: transform 0.15s ease;
}
.eq__switch input:checked + .eq__switch-track {
  background-color: var(--color-brand);
}
.eq__switch input:checked + .eq__switch-track .eq__switch-thumb {
  transform: translateX(16px);
}
.eq__switch-label {
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
}

/* 输出增益 */
.eq__output {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
.eq__output-label {
  font-size: var(--font-size-sm);
  color: var(--color-text-tertiary);
}
.eq__output-slider {
  width: 120px;
  accent-color: var(--color-brand);
}
.eq__output-value {
  width: 52px;
  font-size: var(--font-size-sm);
  font-variant-numeric: tabular-nums;
  color: var(--color-text-secondary);
}

/* 曲线 */
.eq__curve {
  height: 140px;
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-md);
  background-color: var(--color-bg);
  overflow: hidden;
}
.eq__canvas {
  width: 100%;
  height: 100%;
  display: block;
}

.eq__section-title {
  font-size: var(--font-size-xs);
  font-weight: var(--font-weight-semibold);
  letter-spacing: 0.04em;
  color: var(--color-text-tertiary);
  text-transform: uppercase;
}

/* 预设 */
.eq__presets {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}
.eq__preset {
  padding: 6px 12px;
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  background-color: var(--color-bg);
  border: 1px solid var(--color-border-subtle);
  border-radius: 999px;
  cursor: pointer;
  transition:
    background-color 0.12s ease,
    border-color 0.12s ease,
    color 0.12s ease;
}
.eq__preset:hover {
  border-color: var(--color-border-strong);
  color: var(--color-text-primary);
}
.eq__preset--active {
  background-color: var(--color-brand);
  border-color: var(--color-brand);
  color: #fff;
}

/* 频段滑块：竖向 */
.eq__bands {
  display: flex;
  justify-content: space-between;
  gap: var(--space-1);
}
.eq__band {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  flex: 1 1 0;
}
.eq__slider {
  /* 竖向滑块仅用标准的 writing-mode，避免已废弃的 appearance: slider-vertical 告警 */
  writing-mode: vertical-lr;
  direction: rtl;
  width: 6px;
  height: 130px;
  accent-color: var(--color-brand);
  cursor: pointer;
}
.eq__band-freq {
  font-size: 10px;
  color: var(--color-text-tertiary);
}
.eq__band-gain {
  font-size: 10px;
  font-variant-numeric: tabular-nums;
  color: var(--color-text-secondary);
}

.eq__error {
  margin: 0;
  font-size: var(--font-size-sm);
  color: var(--color-danger);
}

.eq__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  flex-wrap: wrap;
}
.eq__reset {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  background-color: var(--color-bg);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-sm);
  cursor: pointer;
}
.eq__reset:hover {
  border-color: var(--color-border-strong);
  color: var(--color-text-primary);
}
.eq__hint {
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}
</style>
