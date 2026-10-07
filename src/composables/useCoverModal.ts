import { ref, nextTick, onUnmounted, type Ref } from 'vue'
import type { Song } from '../types/song'

type LogFn = (...args: any[]) => void

interface UseCoverModalOptions {
  // 判断是否有当前歌曲（无歌曲时不打开）
  currentSong: Ref<Song | null>
  // 模态框打开渲染完成后的回调（容器侧用于滚动到当前歌词）
  onOpened: () => void
  logInfo: LogFn
}

// 封面模态框的窗口行为：开关、全屏切换、标题栏拖动。
// 歌词滚动（依赖歌词行 DOM refs 与当前歌词索引）留在容器侧，经 onOpened 注入。
// document 全局监听的注册与清理全部内聚在本 composable 中。
export function useCoverModal(options: UseCoverModalOptions) {
  const { currentSong, onOpened, logInfo } = options

  const showCoverModal = ref(false)
  const isCoverModalFullscreen = ref(false)
  const coverModalPosition = ref<{ left: string; top: string; transform?: string }>({
    left: '50%',
    top: '50%',
    transform: 'translate(-50%, -50%)'
  })
  // 模板字符串 ref（ref="coverModalContent"），需在容器 setup 作用域解构暴露
  const coverModalContent = ref<HTMLElement | null>(null)

  let isDraggingCoverModal = false
  let dragStartX = 0
  let dragStartY = 0
  let modalStartX = 0
  let modalStartY = 0

  const resetPosition = () => {
    coverModalPosition.value = { left: '50%', top: '50%', transform: 'translate(-50%, -50%)' }
  }

  const openCoverModal = () => {
    if (currentSong.value) {
      showCoverModal.value = true
      logInfo('打开封面模态框')

      // 打开后等待模态框完全渲染，然后滚动到当前歌词
      setTimeout(() => {
        nextTick(() => {
          onOpened()
        })
      }, 100)
    }
  }

  // 关闭封面模态框
  const closeCoverModal = () => {
    showCoverModal.value = false
    isCoverModalFullscreen.value = false
    resetPosition()
    logInfo('关闭封面模态框')
  }

  // 切换封面模态框全屏状态
  const toggleCoverModalFullscreen = () => {
    isCoverModalFullscreen.value = !isCoverModalFullscreen.value
    // 全屏与退出全屏都重置为居中位置
    resetPosition()
    logInfo('封面模态框全屏状态:', isCoverModalFullscreen.value)
  }

  // 开始拖动封面模态框
  const startDragCoverModal = (e: MouseEvent) => {
    if (isCoverModalFullscreen.value) return // 全屏时不允许拖动

    isDraggingCoverModal = true
    dragStartX = e.clientX
    dragStartY = e.clientY

    // 获取当前位置
    const rect = coverModalContent.value?.getBoundingClientRect()
    if (rect) {
      modalStartX = rect.left
      modalStartY = rect.top
    }

    // 添加全局鼠标事件监听
    document.addEventListener('mousemove', onDragCoverModal)
    document.addEventListener('mouseup', stopDragCoverModal)

    logInfo('开始拖动封面模态框')
  }

  // 拖动中
  const onDragCoverModal = (e: MouseEvent) => {
    if (!isDraggingCoverModal) return

    const deltaX = e.clientX - dragStartX
    const deltaY = e.clientY - dragStartY

    const newX = modalStartX + deltaX
    const newY = modalStartY + deltaY

    coverModalPosition.value = {
      left: `${newX}px`,
      top: `${newY}px`,
      transform: 'none'
    }
  }

  // 停止拖动
  const stopDragCoverModal = () => {
    isDraggingCoverModal = false
    document.removeEventListener('mousemove', onDragCoverModal)
    document.removeEventListener('mouseup', stopDragCoverModal)
    logInfo('停止拖动封面模态框')
  }

  // 组件卸载时清理可能残留的全局监听（与原容器 onUnmounted 行为一致）
  onUnmounted(() => {
    document.removeEventListener('mousemove', onDragCoverModal)
    document.removeEventListener('mouseup', stopDragCoverModal)
  })

  return {
    showCoverModal,
    isCoverModalFullscreen,
    coverModalPosition,
    coverModalContent,
    openCoverModal,
    closeCoverModal,
    toggleCoverModalFullscreen,
    startDragCoverModal
  }
}
