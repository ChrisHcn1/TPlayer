/**
 * 实验室 store
 *
 * 单一事实源：config.json 的 lab 段（经 services/lab 读写）。集中管理各实验功能的
 * **开关**（只有关闭 / 开启两种状态，默认全关），提供 `isEnabled(id)` 供各处判断
 * 某实验功能是否生效。与主设置 / 其他 store 完全解耦。
 *
 * 关于「上线」：功能满足需求后由开发者把它从实验室并入正式版（代码迁移到主项目），
 * 届时它不再是实验室条目、也不再需要开关——所以这里刻意不做「上线 / 废除」状态机。
 */
import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import { LAB_FEATURES } from '../lab/registry'
import { labGet, labSetFeature, type LabState } from '../services/lab'
import { normalizeError } from '../services/ipc'

export interface LabFeatureView {
  id: string
  nameKey: string
  descriptionKey: string
  /** 是否开启（默认 false） */
  enabled: boolean
}

export const useLabStore = defineStore('lab', () => {
  const state = ref<LabState>({ features: {} })
  const loaded = ref(false)
  const error = ref<string | null>(null)

  /** 启动时加载一次实验室配置 */
  async function init(): Promise<void> {
    state.value = await labGet()
    loaded.value = true
  }

  /** 全部功能视图（合并注册表元数据 + 持久化开关） */
  const features = computed<LabFeatureView[]>(() =>
    LAB_FEATURES.map((def) => ({
      id: def.id,
      nameKey: def.nameKey,
      descriptionKey: def.descriptionKey,
      enabled: state.value.features[def.id] ?? def.defaultEnabled,
    })),
  )

  /** 生效中的功能 id 列表 */
  const activeIds = computed(() => features.value.filter((f) => f.enabled).map((f) => f.id))

  /** 某功能是否开启（缺省 false）。各处据此判断是否加载实验功能，确保关闭时零影响。 */
  function isEnabled(id: string): boolean {
    const def = LAB_FEATURES.find((f) => f.id === id)
    return state.value.features[id] ?? def?.defaultEnabled ?? false
  }

  async function setEnabled(id: string, enabled: boolean): Promise<void> {
    // 乐观更新，失败回滚
    const prev = state.value.features[id]
    state.value.features[id] = enabled
    try {
      await labSetFeature(id, enabled)
      error.value = null
    } catch (cause) {
      state.value.features[id] = prev ?? false
      error.value = normalizeError(cause).message
    }
  }

  return {
    state,
    loaded,
    error,
    features,
    activeIds,
    isEnabled,
    init,
    setEnabled,
  }
})
