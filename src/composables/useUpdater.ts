import { ref } from 'vue'

// 后端 updater.rs 返回的更新信息结构
export interface UpdateInfo {
  version: string
  release_notes: string
  download_url: string
  sha256_hash: string
  size: number
  release_date: string
}

// 应用更新流程的共享状态与动作。
// Settings 触发检查 -> App.vue 通过 openUpdateModal 打开 UpdateModal
// -> UpdateProgress 执行 下载/校验/安装 真实流程。
export function useUpdater() {
  const showUpdateModal = ref(false)
  const updateInfo = ref<UpdateInfo | null>(null)
  const currentVersion = ref('')

  const openUpdateModal = (info: UpdateInfo, version: string) => {
    updateInfo.value = info
    currentVersion.value = version
    showUpdateModal.value = true
  }

  const closeUpdateModal = () => {
    showUpdateModal.value = false
  }

  return {
    showUpdateModal,
    updateInfo,
    currentVersion,
    openUpdateModal,
    closeUpdateModal
  }
}
