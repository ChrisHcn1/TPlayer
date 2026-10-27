/**
 * 实验室·元数据增强：单曲重试记录
 *
 * 完整历史由后端持久化在 app_data_dir/meta_retry_log.json；本 store 只负责：
 *  - 启动后拉取一次日志（曲目列表据此显示「已重试」标记与时间/状态）；
 *  - 单曲重试后即时更新该曲目最新一条记录（不触碰其他曲目）。
 */
import { defineStore } from 'pinia'
import { ref } from 'vue'

import { metaVerifyLog, metaVerifyRetryTrack, type MetaRetryRecord } from '../services/lab'

export const useMetaRetryStore = defineStore('metaRetry', () => {
  /** trackId → 最新一条重试记录 */
  const latest = ref<Record<string, MetaRetryRecord>>({})
  /** 是否已拉取过历史日志（避免重复 IPC） */
  const loaded = ref(false)
  /** 正在重试中的曲目 id（防重复触发） */
  const pending = ref<string[]>([])

  /** 首次使用时拉取完整历史（重复调用无开销） */
  async function ensureLoaded(): Promise<void> {
    if (loaded.value) return
    loaded.value = true
    const list = await metaVerifyLog()
    const map: Record<string, MetaRetryRecord> = { ...latest.value }
    for (const rec of list) map[rec.trackId] = rec
    latest.value = map
  }

  /** 取某曲目最新一条重试记录（未重试过返回 undefined） */
  function recordOf(trackId: string): MetaRetryRecord | undefined {
    return latest.value[trackId]
  }

  /** 对单曲发起重试；结果只更新该曲目的记录 */
  async function retry(trackId: string): Promise<MetaRetryRecord | null> {
    if (pending.value.includes(trackId)) return null
    pending.value = [...pending.value, trackId]
    try {
      const rec = await metaVerifyRetryTrack(trackId)
      if (rec) latest.value = { ...latest.value, [rec.trackId]: rec }
      return rec
    } finally {
      pending.value = pending.value.filter((id) => id !== trackId)
    }
  }

  return { latest, loaded, pending, ensureLoaded, recordOf, retry }
})
