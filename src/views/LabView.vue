<script setup lang="ts">
/**
 * 实验室面板（设置页内嵌）
 *
 * 每个实验功能只有两种状态：**关闭 / 开启**（拨动开关），默认全部关闭、
 * 与现有功能完全隔离。功能满足需求后由开发者把它从实验室「走出去」并入正式版
 * （代码从 src/lab/features/<id> 迁到主项目、并从 src/lab/registry.ts 移除条目），
 * 届时它不再是实验室条目、也不需要开关。
 */
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import { useLabStore } from '../stores/lab'
import {
  applyPreset,
  resetEqualizer,
  setBandGain,
  setEqEnabled,
  useEqualizer,
  useEqualizerState,
  type EqPresetId,
} from '../lab/features/equalizer'
import {
  ACCENT_PRESETS,
  customAccent,
  backdropMode,
  wallpaperPath,
  wallpaperFit,
  wallpaperSource,
  dailyWallpaper,
  importWallpaper,
  removeWallpaper,
  setWallpaperSource,
  ensureDailyWallpaper,
  clearDailyWallpaper,
  GRADIENT_PRESETS,
  gradientId,
  TEXTURE_PRESETS,
  textureId,
} from '../lab/features/theme-colors'
import type { BackdropMode } from '../services/lab'
import { metaVerifyStatus } from '../services/lab'
import { isTauriRuntime } from '../services/ipc'

const lab = useLabStore()

/** 顶层实验室卡片：自定义壁纸作为「皮肤配色」的子模块，不再单独成卡 */
const topFeatures = computed(() => lab.features.filter((f) => f.id !== 'custom-wallpaper'))

/* ---------- 实验室·均衡器 ---------- */
const { config: eqConfig, preset: eqPreset, EQ_PRESETS } = useEqualizerState()
useEqualizer() // 初始化时读取一次后端已保存的 EQ 配置用于面板回显

/**
 * 统一的功能开关处理：开启 / 关闭某个实验功能。
 * 均衡器额外联动 EQ 启用状态——头部那个拨动开关即「均衡器总开关」，
 * 避免与面板内再叠一个「均衡器 开/关」造成重复、易误解的两层开关。
 */
function onFeatureToggle(f: { id: string }, enabled: boolean): void {
  if (f.id === 'equalizer') {
    setEqEnabled(enabled)
  }
  lab.setEnabled(f.id, enabled)
  if (f.id === 'enhanced-metadata' && enabled) {
    void refreshVerifyStatus()
  }
}

/* ---------- 实验室·元数据增强：批量校验运行状态（重试按单曲进行） ---------- */
const verifyRunning = ref(false)
/** 校验完成事件的解绑句柄 */
let unlistenVerify: UnlistenFn | undefined

async function refreshVerifyStatus(): Promise<void> {
  const s = await metaVerifyStatus()
  verifyRunning.value = s.running
}

/** 预设列表（对象转数组，便于模板 v-for） */
const eqPresetList = computed(() =>
  (Object.entries(EQ_PRESETS) as [EqPresetId, { name: string }][]).map(([id, p]) => ({
    id,
    name: p.name,
  })),
)

// 进入实验室页时回显在线校验状态（失败项可在此重试）；
// 后台校验任务完成时实时刷新（library://meta-verify-done）
onMounted(async () => {
  if (lab.isEnabled('enhanced-metadata')) await refreshVerifyStatus()
  if (isTauriRuntime()) {
    unlistenVerify = await listen('library://meta-verify-done', () => {
      void refreshVerifyStatus()
    })
  }
})
onUnmounted(() => {
  unlistenVerify?.()
})

/** 频段标签：≥1kHz 显示为 1k / 12k，更紧凑 */
function eqBandLabel(freq: number): string {
  return freq >= 1000 ? `${freq / 1000}k` : `${freq}`
}

/** 滑块输入（range 的 value 是字符串，需转数字） */
function onEqGainInput(index: number, event: Event): void {
  const el = event.target as HTMLInputElement
  setBandGain(index, Number(el.value))
}

/** 窗口底纹模式选项 */
const BACKDROP_OPTIONS: { mode: BackdropMode; label: string; hint: string; ready: boolean }[] = [
  { mode: 'off', label: '关闭', hint: '恢复不透明窗口（与现状一致）', ready: true },
  { mode: 'solid', label: '纯色基底', hint: '窗口透明 + 主题实底，任何平台可用', ready: true },
  {
    mode: 'native',
    label: '原生质感',
    hint: 'Windows Mica/Acrylic、macOS Vibrancy；不可用时自动降级为纯色',
    ready: true,
  },
  { mode: 'gradient', label: '预设渐变', hint: '5 套渐变，停止色跟随明暗主题', ready: true },
  { mode: 'texture', label: '纹理图案', hint: '网格 / 斜纹 / 圆点 / 噪点（纯内联）', ready: true },
]

const importing = ref(false)
const wallpaperMsg = ref('')

async function selectBackdrop(mode: BackdropMode): Promise<void> {
  if (!BACKDROP_OPTIONS.find((o) => o.mode === mode)?.ready) return
  if (mode === 'wallpaper' && wallpaperSource.value === 'local' && !wallpaperPath.value) {
    wallpaperMsg.value = '请先导入一张图片'
    return
  }
  wallpaperMsg.value = ''
  backdropMode.value = mode
  // 进入壁纸模式且为每日来源但图尚未就位：立即拉取
  if (mode === 'wallpaper' && wallpaperSource.value === 'daily' && !wallpaperPath.value) {
    await ensureDailyWallpaper()
  }
}

async function onRefreshDaily(): Promise<void> {
  wallpaperMsg.value = ''
  await ensureDailyWallpaper(1 + Math.floor(Math.random() * 7))
}

async function onClearDaily(): Promise<void> {
  wallpaperMsg.value = ''
  await clearDailyWallpaper()
  wallpaperMsg.value = '已清除每日壁纸缓存'
}

async function onImportWallpaper(): Promise<void> {
  importing.value = true
  wallpaperMsg.value = ''
  try {
    const ok = await importWallpaper()
    if (!ok) {
      wallpaperMsg.value = '未选择图片或导入失败'
      return
    }
    // 导入成功：自动切到壁纸模式，让用户立刻看到效果
    backdropMode.value = 'wallpaper'
    wallpaperMsg.value = ''
  } finally {
    importing.value = false
  }
}

async function onRemoveWallpaper(): Promise<void> {
  await removeWallpaper()
  wallpaperMsg.value = '已移除壁纸'
}

function onFitChange(fit: 'cover' | 'contain'): void {
  wallpaperFit.value = fit
}
</script>

<template>
  <div class="lab">
    <div class="lab__intro">
      实验室模式：更多功能正在酝酿中，敬请期待。实验室功能默认关闭、与现有功能隔离，开启后即可体验；成熟后并入正式版。
    </div>

    <div v-for="f in topFeatures" :key="f.id" class="lab__card">
      <div class="lab__head">
        <div class="lab__meta">
          <div class="lab__name">{{ f.name }}</div>
          <div class="lab__desc">{{ f.description }}</div>
        </div>

        <!-- 关闭 / 开启（Switch，与 SettingsView 原开关一致） -->
        <label class="lab__switch">
          <input
            type="checkbox"
            class="switch"
            :checked="f.enabled"
            @change="onFeatureToggle(f, ($event.target as HTMLInputElement).checked)"
          />
        </label>
      </div>

      <div v-if="f.id === 'theme-colors' && f.enabled" class="lab__theme">
        <div class="lab__group">
          <div class="lab__group-title">强调色</div>
          <div class="lab__swatches">
            <button
              v-for="p in ACCENT_PRESETS"
              :key="p.value"
              class="lab__swatch"
              :class="{ 'lab__swatch--active': customAccent === p.value }"
              :style="{ background: p.value }"
              :title="p.name"
              @click="customAccent = p.value"
            />
          </div>
          <label class="lab__custom">
            自定义
            <input type="color" v-model="customAccent" />
          </label>
        </div>

        <div class="lab__group">
          <div class="lab__group-title">窗口底纹</div>
          <div class="lab__modes">
            <button
              v-for="o in BACKDROP_OPTIONS"
              :key="o.mode"
              class="lab__mode"
              :class="{
                'lab__mode--active': backdropMode === o.mode,
                'lab__mode--todo': !o.ready,
              }"
              :disabled="!o.ready"
              :title="o.hint"
              @click="selectBackdrop(o.mode)"
            >
              {{ o.label }}{{ o.ready ? '' : '（开发中）' }}
            </button>
          </div>
          <p class="lab__hint">
            {{ BACKDROP_OPTIONS.find((o) => o.mode === backdropMode)?.hint ?? '' }}
          </p>

          <!-- 预设渐变：选择渐变方案 -->
          <div v-if="backdropMode === 'gradient'" class="lab__presets">
            <button
              v-for="g in GRADIENT_PRESETS"
              :key="g.id"
              class="lab__preset"
              :class="{ 'lab__preset--active': gradientId === g.id }"
              :title="g.name"
              :style="{ background: g.css }"
              @click="gradientId = g.id"
            >
              <span class="lab__preset-label">{{ g.name }}</span>
            </button>
          </div>

          <!-- 纹理图案：选择纹理样式 -->
          <div v-if="backdropMode === 'texture'" class="lab__presets">
            <button
              v-for="t in TEXTURE_PRESETS"
              :key="t.id"
              class="lab__preset lab__preset--chip"
              :class="{ 'lab__preset--active': textureId === t.id }"
              :title="t.name"
              @click="textureId = t.id"
            >
              {{ t.name }}
            </button>
          </div>
        </div>
        <!-- 自定义壁纸：皮肤配色下的子模块 -->
        <div class="lab__submodule">
          <div class="lab__head">
            <div class="lab__meta">
              <div class="lab__name">自定义壁纸</div>
            </div>
            <label class="lab__switch">
              <input
                type="checkbox"
                class="switch"
                :checked="lab.isEnabled('custom-wallpaper')"
                @change="lab.setEnabled('custom-wallpaper', ($event.target as HTMLInputElement).checked)"
              />
            </label>
          </div>

          <div v-if="lab.isEnabled('custom-wallpaper')" class="lab__wallpaper-panel">
            <p class="lab__hint">
              开启后应用窗口壁纸。可选择「本地图片」上传，或「每日 Bing」自动更换壁纸。
            </p>

            <div class="lab__wallpaper-row">
              <span class="lab__group-title">来源</span>
              <div class="lab__radio lab__radio--sm">
                <label class="lab__radio-item" :class="{ 'lab__radio-item--on': wallpaperSource === 'local' }">
                  <input
                    type="radio"
                    name="lab-wallpaper-source"
                    :checked="wallpaperSource === 'local'"
                    @change="setWallpaperSource('local')"
                  />
                  <span>本地图片</span>
                </label>
                <label class="lab__radio-item" :class="{ 'lab__radio-item--on': wallpaperSource === 'daily' }">
                  <input
                    type="radio"
                    name="lab-wallpaper-source"
                    :checked="wallpaperSource === 'daily'"
                    @change="setWallpaperSource('daily')"
                  />
                  <span>每日 Bing</span>
                </label>
              </div>
            </div>

            <!-- 本地图片来源 -->
            <template v-if="wallpaperSource === 'local'">
              <div class="lab__wallpaper-row">
                <button class="lab__btn" :disabled="importing" @click="onImportWallpaper">
                  {{ importing ? '导入中…' : wallpaperPath ? '更换图片' : '导入图片' }}
                </button>
                <button v-if="wallpaperPath" class="lab__btn lab__btn--danger" @click="onRemoveWallpaper">
                  移除
                </button>
                <span class="lab__wallpaper-state">
                  {{ wallpaperPath ? '已导入' : '未导入' }}
                </span>
              </div>
              <div v-if="wallpaperPath" class="lab__wallpaper-row">
                <span class="lab__group-title">铺放</span>
                <div class="lab__radio lab__radio--sm">
                  <label class="lab__radio-item" :class="{ 'lab__radio-item--on': wallpaperFit === 'cover' }">
                    <input
                      type="radio"
                      name="lab-wallpaper-fit"
                      :checked="wallpaperFit === 'cover'"
                      @change="onFitChange('cover')"
                    />
                    <span>铺满</span>
                  </label>
                  <label class="lab__radio-item" :class="{ 'lab__radio-item--on': wallpaperFit === 'contain' }">
                    <input
                      type="radio"
                      name="lab-wallpaper-fit"
                      :checked="wallpaperFit === 'contain'"
                      @change="onFitChange('contain')"
                    />
                    <span>适应</span>
                  </label>
                </div>
              </div>
            </template>

            <!-- 每日 Bing 来源 -->
            <template v-else>
              <div class="lab__wallpaper-row">
                <button class="lab__btn" @click="onRefreshDaily">换一张</button>
                <button class="lab__btn" @click="onClearDaily">清除缓存</button>
                <button class="lab__btn lab__btn--danger" @click="onRemoveWallpaper">
                  停止每日
                </button>
              </div>
              <p v-if="dailyWallpaper" class="lab__hint">
                {{ dailyWallpaper.title ? `${dailyWallpaper.title} · ` : '' }}{{ dailyWallpaper.copyright || '每日自动更新' }}
              </p>
              <p class="lab__hint">每天自动更新壁纸，断网时沿用上次图片。</p>
            </template>

            <p v-if="wallpaperMsg" class="lab__hint lab__warn">{{ wallpaperMsg }}</p>
          </div>
        </div>
      </div>

      <!-- 均衡器：开启时显示预设 + 10 段滑块 -->
      <div v-else-if="f.id === 'equalizer' && f.enabled" class="lab__eq">
        <div class="lab__group">
          <div class="lab__group-title">
            预设
            <button class="lab__link" type="button" @click="resetEqualizer()">复位为原声</button>
          </div>
          <div class="lab__presets">
            <button
              v-for="p in eqPresetList"
              :key="p.id"
              type="button"
              class="lab__preset lab__preset--chip"
              :class="{ 'lab__preset--active': eqPreset === p.id }"
              @click="applyPreset(p.id)"
            >
              {{ p.name }}
            </button>
          </div>
        </div>

        <div class="lab__group">
          <div class="lab__group-title">
            频段增益
            <span class="lab__hint">拖动滑块即时调整音色</span>
          </div>
          <div class="lab__bands">
            <div v-for="(b, i) in eqConfig.bands" :key="b.freq" class="lab__band">
              <input
                class="lab__slider"
                type="range"
                min="-15"
                max="15"
                step="0.5"
                :value="b.gainDb"
                :aria-label="`${b.freq} Hz 增益`"
                @input="onEqGainInput(i, $event)"
              />
              <div class="lab__band-freq">{{ eqBandLabel(b.freq) }}</div>
              <div class="lab__band-gain">{{ b.gainDb.toFixed(1) }}</div>
            </div>
          </div>
        </div>
      </div>

      <div v-else-if="f.id === 'enhanced-metadata' && f.enabled" class="lab__group">
        <div class="lab__wallpaper-row">
          <span class="lab__group-title">在线校验</span>
          <span class="lab__hint">{{ verifyRunning ? '后台校验中…' : '空闲' }}</span>
        </div>
        <p class="lab__hint">
          只补全缺失信息，不会覆盖你已标注的内容；识别有误可右键「识别元数据」单曲纠正。
        </p>
      </div>

      <!-- 未实现的占位功能提示（已实现但无需面板的功能已排除） -->
      <div
        v-else-if="
          f.id !== 'theme-colors' &&
          f.id !== 'equalizer' &&
          f.id !== 'enhanced-metadata' &&
          f.id !== 'cue-sacd-convert'
        "
        class="lab__wip"
      >
        功能开发中，敬请期待。
      </div>
    </div>
  </div>
</template>

<style scoped>
.lab {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.lab__intro {
  color: var(--color-text-secondary, #9aa4b2);
  font-size: 13px;
  line-height: 1.5;
}
.lab__card {
  border: 1px solid var(--color-border, rgba(255, 255, 255, 0.08));
  border-radius: 10px;
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.lab__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}
.lab__name {
  font-weight: 600;
}
.lab__desc {
  color: var(--color-text-secondary, #9aa4b2);
  font-size: 12px;
  margin-top: 4px;
}
/* 关闭 / 开启（Switch，与 SettingsView 一致） */
.lab__switch {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  margin-top: 2px;
}

/* 开关（与 SettingsView .switch 保持一致） */
.switch {
  position: relative;
  flex: 0 0 auto;
  width: 38px;
  height: 22px;
  margin: 0;
  appearance: none;
  -webkit-appearance: none;
  background-color: var(--color-bg-hover, rgba(255, 255, 255, 0.06));
  border: 1px solid var(--color-border, rgba(255, 255, 255, 0.14));
  border-radius: 999px;
  cursor: pointer;
  transition: background-color 0.2s ease;
}
.switch::after {
  content: '';
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  background-color: var(--color-text-inverse, #fff);
  border-radius: 999px;
  transition: transform 0.2s ease;
}
.switch:checked {
  background-color: var(--color-brand-solid, #7a95ff);
  border-color: var(--color-brand-solid, #7a95ff);
}
.switch:checked::after {
  transform: translateX(16px);
}
.switch:disabled {
  cursor: not-allowed;
}
.switch:focus-visible {
  outline: 2px solid var(--color-focus-ring, #7a95ff);
  outline-offset: 2px;
}
.lab__theme {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.lab__submodule {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-top: 4px;
  padding-top: 14px;
  border-top: 1px dashed var(--color-border, rgba(255, 255, 255, 0.12));
}
.lab__group {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.lab__group-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-secondary, #9aa4b2);
}
.lab__modes {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.lab__mode {
  font-size: 12px;
  padding: 6px 12px;
  border-radius: 999px;
  border: 1px solid var(--color-border, rgba(255, 255, 255, 0.14));
  background: transparent;
  color: var(--color-text-primary, #fff);
  cursor: pointer;
}
.lab__mode:hover:not(:disabled) {
  background: var(--color-brand-subtle, rgba(122, 149, 255, 0.12));
}
.lab__mode--active {
  border-color: var(--color-brand, #7a95ff);
  color: var(--color-brand, #7a95ff);
}
.lab__mode--todo {
  opacity: 0.45;
  cursor: not-allowed;
}
.lab__hint {
  margin: 0;
  font-size: 12px;
  color: var(--color-text-secondary, #9aa4b2);
}
.lab__warn {
  color: #fbbf24;
}
.lab__swatches {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.lab__swatch {
  width: 26px;
  height: 26px;
  border-radius: 50%;
  border: 2px solid transparent;
  cursor: pointer;
  padding: 0;
}
.lab__swatch--active {
  border-color: var(--color-text-primary, #fff);
}
.lab__custom {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--color-text-secondary, #9aa4b2);
}
.lab__wip {
  font-size: 12px;
  color: var(--color-text-secondary, #9aa4b2);
}
.lab__wallpaper {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-top: 2px;
}
.lab__wallpaper-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.lab__wallpaper-state {
  font-size: 12px;
  color: var(--color-text-secondary, #9aa4b2);
}
.lab__radio--sm .lab__radio-item {
  padding: 3px 10px;
  font-size: 12px;
}
.lab__btn {
  font-size: 12px;
  padding: 5px 12px;
  border-radius: 6px;
  border: 1px solid var(--color-border, rgba(255, 255, 255, 0.14));
  background: transparent;
  color: var(--color-text-primary, #fff);
  cursor: pointer;
}
.lab__btn:hover:not(:disabled) {
  background: var(--color-brand-subtle, rgba(122, 149, 255, 0.12));
}
.lab__btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.lab__btn--danger {
  color: #f87171;
  border-color: rgba(248, 113, 113, 0.4);
}
.lab__presets {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.lab__preset {
  position: relative;
  width: 84px;
  height: 46px;
  border-radius: 8px;
  border: 2px solid transparent;
  cursor: pointer;
  overflow: hidden;
  background-color: var(--color-bg-base, #1a1d24);
}
.lab__preset--active {
  border-color: var(--color-brand, #7a95ff);
}
.lab__preset-label {
  position: absolute;
  left: 6px;
  bottom: 4px;
  font-size: 11px;
  font-weight: 600;
  color: #fff;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.6);
}
.lab__preset--chip {
  width: auto;
  height: auto;
  min-width: 56px;
  padding: 7px 12px;
  font-size: 12px;
  background: transparent;
  color: var(--color-text-primary, #fff);
}
.lab__preset--chip .lab__preset-label {
  position: static;
}

/* 实验室·均衡器 */
.lab__eq {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.lab__link {
  margin-left: 8px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--color-primary, #5b8cff);
  font-size: 12px;
  cursor: pointer;
}
.lab__link:hover {
  text-decoration: underline;
}
.lab__bands {
  display: flex;
  gap: 10px;
  /* 10 段竖向并排；容器过窄时横向滚动，避免滑块被挤压变形 */
  overflow-x: auto;
  padding: 4px 0 6px;
}
.lab__band {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  /* 等分宽度，窄屏下不低于可拖动宽度 */
  flex: 1 1 0;
  min-width: 32px;
}
.lab__slider {
  /*
   * 竖向滑块（EQ 惯例：向上拖 = 提升增益）。
   *
   * `writing-mode` 是标准做法：它把 range 的行内轴转为垂直方向。默认行内轴
   * 自上而下，即**最小值在上**，与 EQ 惯例相反，故再用 `direction: rtl` 反转
   * 行内轴，使**最大值在上**。
   *
   * 另附 `slider-vertical`（WebKit / Blink 的历史接口）作兜底：两种机制在
   * 同一引擎下取向一致，不会冲突。
   */
  writing-mode: vertical-lr;
  direction: rtl;
  -webkit-appearance: slider-vertical;
  appearance: slider-vertical;
  width: 22px;
  height: 150px;
  accent-color: var(--color-primary, #5b8cff);
  cursor: pointer;
}
.lab__band-freq {
  font-size: 11px;
  color: var(--color-text-secondary, #9aa4b2);
}
.lab__band-gain {
  font-size: 11px;
  /* 等宽数字：拖动时数值不跳动 */
  font-variant-numeric: tabular-nums;
  color: var(--color-text-secondary, #9aa4b2);
}


</style>
