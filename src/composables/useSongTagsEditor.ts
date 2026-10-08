import { ref, type Ref } from 'vue'
import * as mm from 'music-metadata'
import { musicDataService } from '../services/musicDataService'
import { multiSourceLyricService } from '../services/multiSourceLyricService'
import { localStorageService } from '../stores/local'
import { getFileNameWithoutExtension } from '../utils/songDisplay'
import { formatTime, formatTimeForLrc } from '../utils/format'
import { useMessage } from './useMessage'
import type { Song } from '../types/song'

type LogFn = (...args: any[]) => void

interface UseSongTagsEditorOptions {
  // 共享的播放列表与当前歌曲（保存标签时原地更新）
  songs: Ref<Song[]>
  currentSong: Ref<Song | null>
  // 编辑入口由歌曲右键菜单触发，保存/打开后需要关闭菜单
  closeSongMenu: () => void
  logInfo: LogFn
  logError: LogFn
}

// 歌曲标签编辑模态框的表单结构
interface EditTagsForm {
  title: string
  artist: string
  album: string
  year: string
  genre: string
  fileName: string
  albumArtist: string
  trackNumber: string
  discNumber: string
  alia: string
  lyric: string
  cover: string
}

// 在线匹配结果回填字段
interface OnlineMatchData {
  title?: string
  artist?: string
  album?: string
  lyric?: string
  coverUrl?: string
}

// 歌曲标签编辑：表单状态、在线匹配、本地元数据读取、封面选择与持久化。
// 播放列表与当前歌曲由容器（App.vue）持有并注入，本 composable 只原地修改，
// 不创建第二份歌曲状态。
export function useSongTagsEditor(options: UseSongTagsEditorOptions) {
  const { songs, currentSong, closeSongMenu, logInfo, logError } = options
  const { showSuccess, showError, showWarning, showInfo } = useMessage()

  const showEditTagsModal = ref(false)
  const showOnlineMatchModal = ref(false)
  const editTagsForm = ref<EditTagsForm>({
    title: '',
    artist: '',
    album: '',
    year: '',
    genre: '',
    fileName: '',
    albumArtist: '',
    trackNumber: '',
    discNumber: '',
    alia: '',
    lyric: '',
    cover: ''
  })
  const songToEdit = ref<Song | null>(null)

  const editSongTags = async (song: Song) => {
    songToEdit.value = song
    // 提取文件名
    const fileName = getFileNameWithoutExtension(song.path)

    // 尝试动态读取歌词
    let lyric = song.lyric || ''
    if (!lyric) {
      logInfo('【编辑标签】歌曲对象中没有歌词，尝试动态读取')
      try {
        const { readTextFile } = await import('@tauri-apps/plugin-fs')
        const lyricPath = song.path.replace(/\.[^/.]+$/, '.lrc')
        logInfo('【编辑标签】尝试读取歌词文件:', lyricPath)
        lyric = await readTextFile(lyricPath)
        logInfo('【编辑标签】成功读取歌词文件，长度:', lyric.length)
      } catch (e) {
        logInfo('【编辑标签】读取歌词文件失败:', e)
      }
    }

    editTagsForm.value = {
      title: song.title || '',
      artist: song.artist || '',
      album: song.album || '',
      year: song.year || '',
      genre: song.genre || '',
      fileName: fileName,
      albumArtist: '',
      trackNumber: song.isCueTrack ? (song.trackNumber || (song as any).track_number || '').toString() : '',
      discNumber: '',
      alia: '',
      lyric: lyric,
      cover: song.cover || ''
    }

    // 如果是CUE track，添加开始和结束时间信息到备注或其他字段
    if (song.isCueTrack) {
      logInfo('CUE track信息:', {
        trackNumber: song.trackNumber || (song as any).track_number,
        startTime: song.startTime,
        endTime: song.endTime
      })
    }
    logInfo('编辑歌曲标签:', song.title, '封面:', song.cover ? '有' : '无', '歌词:', lyric ? '有' : '无')
    showEditTagsModal.value = true
    closeSongMenu()
  }

  // 关闭编辑标签模态框
  const closeEditTagsModal = () => {
    showEditTagsModal.value = false
    songToEdit.value = null
  }

  const copyPath = () => {
    if (songToEdit.value?.path) {
      navigator.clipboard.writeText(songToEdit.value.path)
        .then(() => {
          showSuccess('路径已复制到剪贴板')
        })
        .catch(err => {
          logError('复制失败:', err)
          showError('复制失败，请手动复制')
        })
    }
  }

  // 从本地文件读取元数据
  const readLocalMetadata = async () => {
    try {
      if (!songToEdit.value?.path) return

      logInfo('【本地元数据】开始读取本地音频文件元数据')
      logInfo('【本地元数据】文件路径:', songToEdit.value.path)

      // 使用music-metadata读取文件
      const metadata = await mm.parseFile(songToEdit.value.path)

      logInfo('【本地元数据】读取成功，格式:', metadata.format.container)
      logInfo('【本地元数据】音频编码:', metadata.format.codec)
      logInfo('【本地元数据】时长:', metadata.format.duration?.toFixed(2), '秒')

      // 更新表单数据
      if (metadata.common.title && !editTagsForm.value.title) {
        editTagsForm.value.title = metadata.common.title
        logInfo('【本地元数据】更新标题:', metadata.common.title)
      }

      if (metadata.common.artist && !editTagsForm.value.artist) {
        editTagsForm.value.artist = metadata.common.artist
        logInfo('【本地元数据】更新艺术家:', metadata.common.artist)
      }

      if (metadata.common.album && !editTagsForm.value.album) {
        editTagsForm.value.album = metadata.common.album
        logInfo('【本地元数据】更新专辑:', metadata.common.album)
      }

      if (metadata.common.year && !editTagsForm.value.year) {
        editTagsForm.value.year = metadata.common.year.toString()
        logInfo('【本地元数据】更新年份:', metadata.common.year)
      }

      if (metadata.common.genre && metadata.common.genre.length > 0 && !editTagsForm.value.genre) {
        editTagsForm.value.genre = metadata.common.genre.join(', ')
        logInfo('【本地元数据】更新流派:', editTagsForm.value.genre)
      }

      showSuccess('从本地文件读取元数据成功')
    } catch (error) {
      logError('【本地元数据】读取失败:', error)
      showError('读取本地元数据失败，请检查文件格式是否支持')
    }
  }

  // 在线查找歌词
  const fetchLyric = async () => {
    try {
      if (!songToEdit.value) {
        logInfo('【在线歌词】没有歌曲可编辑')
        showWarning('没有歌曲可编辑')
        return
      }

      logInfo('【在线歌词】开始在线查找歌词')

      const title = editTagsForm.value.title || songToEdit.value.title
      const artist = editTagsForm.value.artist || songToEdit.value.artist
      const album = editTagsForm.value.album || songToEdit.value.album

      if (!title || !artist) {
        logInfo('【在线歌词】歌曲标题或艺术家为空')
        showWarning('请先填写歌曲标题和艺术家信息')
        return
      }

      logInfo('【在线歌词】搜索:', { title, artist, album })

      // 使用多源歌词服务获取歌词
      let result
      try {
        result = await multiSourceLyricService.getLyric({
          id: songToEdit.value.id,
          title,
          artist,
          album,
          filePath: songToEdit.value.path
        }, 'manual', true)
      } catch (apiError) {
        logError('【在线歌词】API调用失败:', apiError)
        showError('网络请求失败，请检查网络连接')
        return
      }

      logInfo('【在线歌词】多源服务返回结果:', result)

      if (!result.success || !result.bestScore) {
        logInfo('【在线歌词】未找到歌词')
        showInfo('未找到歌词，请尝试修改搜索信息后重试')
        return
      }

      // 将歌词转换为LRC格式
      let lrcContent = ''
      if (result.bestScore.lyricLines && result.bestScore.lyricLines.length > 0) {
        lrcContent = result.bestScore.lyricLines.map(line => {
          // 结构化歌词行：startTime 为毫秒，转秒后格式化
          const time = line.startTime / 1000
          const text = line.words.map(w => w.word).join('')
          return `[${formatTime(time)}]${text}`
        }).join('\n')
      } else if (result.bestScore.lyricText) {
        // 如果有原始的lyricText，直接使用
        lrcContent = result.bestScore.lyricText
      }

      if (!lrcContent) {
        logInfo('【在线歌词】歌词内容为空')
        showInfo('找到歌词但内容为空，请尝试其他来源')
        return
      }

      editTagsForm.value.lyric = lrcContent

      logInfo('【在线歌词】获取成功，来源:', result.bestSource, '歌词行数:', result.bestScore.lyricLines?.length || 0)

      showSuccess('获取歌词成功')
    } catch (error) {
      logError('【在线歌词】获取失败:', error)
      showError('获取歌词失败，请检查网络连接后重试')
    }
  }

  // 获取封面
  const fetchCover = async () => {
    try {
      if (!songToEdit.value) {
        logInfo('【在线封面】没有歌曲可编辑')
        showWarning('没有歌曲可编辑')
        return
      }

      logInfo('【在线封面】开始在线查找封面')

      const title = editTagsForm.value.title || songToEdit.value.title
      const artist = editTagsForm.value.artist || songToEdit.value.artist
      const keyword = `${title} ${artist}`.trim()

      if (!keyword) {
        logInfo('【在线封面】关键词为空')
        showWarning('请先填写歌曲标题和艺术家信息')
        return
      }

      logInfo('【在线封面】搜索关键词:', keyword)

      // 使用新的音乐数据服务获取封面
      let result
      try {
        result = await musicDataService.getSongInfoWithLyric(keyword)
      } catch (apiError) {
        logError('【在线封面】API调用失败:', apiError)
        showError('网络请求失败，请检查网络连接')
        return
      }

      logInfo('【在线封面】API返回结果:', result)

      if (!result || !result.song) {
        logInfo('【在线封面】未找到匹配的歌曲')
        showInfo('未找到匹配的歌曲，请修改歌曲信息后重试')
        return
      }

      if (!result.song.coverUrl) {
        logInfo('【在线封面】找到歌曲但没有封面')
        showInfo('找到歌曲但未找到封面')
        return
      }

      logInfo('【在线封面】获取封面成功:', result.song.coverUrl)

      // 下载封面并转换为Base64
      try {
        const coverBase64 = await musicDataService.getCoverAsBase64(result.song.coverUrl)

        if (coverBase64) {
          editTagsForm.value.cover = coverBase64
          logInfo('【在线封面】封面已转换为Base64，长度:', coverBase64.length)
        } else {
          editTagsForm.value.cover = result.song.coverUrl
          logInfo('【在线封面】使用原始封面URL')
        }
      } catch (coverError) {
        logError('【在线封面】下载封面失败:', coverError)
        // 即使下载失败，也尝试使用URL
        editTagsForm.value.cover = result.song.coverUrl
        logInfo('【在线封面】使用原始封面URL作为备选')
      }

      // 保存动态封面URL
      if (result.song.dynamicCoverUrl) {
        songToEdit.value.dynamicCoverUrl = result.song.dynamicCoverUrl
        logInfo('【在线封面】获取动态封面成功:', result.song.dynamicCoverUrl)
      }

      showSuccess('获取封面成功')
    } catch (error) {
      logError('【在线封面】获取失败:', error)
      showError('获取封面失败，请检查网络连接后重试')
    }
  }

  // 打开在线匹配对话框
  const openOnlineMatch = () => {
    if (!songToEdit.value) return
    showOnlineMatchModal.value = true
  }

  // 处理在线匹配结果
  const handleOnlineMatchApply = (data: OnlineMatchData) => {
    if (!songToEdit.value) return

    if (data.title) editTagsForm.value.title = data.title
    if (data.artist) editTagsForm.value.artist = data.artist
    if (data.album) editTagsForm.value.album = data.album
    if (data.lyric) {
      try {
        const lyricData = JSON.parse(data.lyric)
        if (lyricData.lrcData && Array.isArray(lyricData.lrcData)) {
          const lrcLines = lyricData.lrcData.map((line: any) => {
            const time = formatTimeForLrc(line.startTime)
            const text = line.words?.map((w: any) => w.word).join('') || ''
            return `[${time}]${text}`
          }).join('\n')
          editTagsForm.value.lyric = lrcLines
        } else {
          editTagsForm.value.lyric = data.lyric
        }
      } catch {
        editTagsForm.value.lyric = data.lyric
      }
    }
    if (data.coverUrl) {
      editTagsForm.value.cover = data.coverUrl
    }

    showOnlineMatchModal.value = false
  }

  // 自动匹配标签
  const autoMatchTags = async () => {
    try {
      if (!songToEdit.value) {
        logInfo('【自动匹配】没有歌曲可编辑')
        showWarning('没有歌曲可编辑')
        return
      }

      logInfo('【自动匹配】开始自动匹配标签')

      const title = editTagsForm.value.title || songToEdit.value.title
      const artist = editTagsForm.value.artist || songToEdit.value.artist
      const keyword = `${title} ${artist}`.trim()

      if (!keyword) {
        logInfo('【自动匹配】关键词为空')
        showWarning('请先填写歌曲标题和艺术家信息')
        return
      }

      logInfo('【自动匹配】搜索关键词:', keyword)

      // 使用新的音乐数据服务获取歌曲信息
      let result
      try {
        result = await musicDataService.getSongInfoWithLyric(keyword)
      } catch (apiError) {
        logError('【自动匹配】API调用失败:', apiError)
        showError('网络请求失败，请检查网络连接')
        return
      }

      logInfo('【自动匹配】API返回结果:', result)

      if (!result || !result.song) {
        logInfo('【自动匹配】未找到匹配的歌曲')
        showInfo('未找到匹配的歌曲，请修改歌曲信息后重试')
        return
      }

      logInfo('【自动匹配】找到歌曲:', result.song.title, '-', result.song.artist)

      let matchedCount = 0

      // 更新元数据
      if (result.song.title && !editTagsForm.value.title) {
        editTagsForm.value.title = result.song.title
        matchedCount++
        logInfo('【自动匹配】更新标题:', result.song.title)
      }

      if (result.song.artist && !editTagsForm.value.artist) {
        editTagsForm.value.artist = result.song.artist
        matchedCount++
        logInfo('【自动匹配】更新艺术家:', result.song.artist)
      }

      if (result.song.album && !editTagsForm.value.album) {
        editTagsForm.value.album = result.song.album
        matchedCount++
        logInfo('【自动匹配】更新专辑:', result.song.album)
      }

      // 获取封面
      if (result.song.coverUrl && !editTagsForm.value.cover) {
        try {
          // 下载封面并转换为Base64
          const coverBase64 = await musicDataService.getCoverAsBase64(result.song.coverUrl)
          if (coverBase64) {
            editTagsForm.value.cover = coverBase64
            matchedCount++
            logInfo('【自动匹配】更新封面成功')
          }
        } catch (coverError) {
          logError('【自动匹配】下载封面失败:', coverError)
        }
      }

      // 保存动态封面URL
      if (result.song.dynamicCoverUrl) {
        songToEdit.value.dynamicCoverUrl = result.song.dynamicCoverUrl
        logInfo('【自动匹配】获取动态封面:', result.song.dynamicCoverUrl)
      }

      // 获取歌词
      const lyricData = result.lyricData
      if (lyricData && (lyricData.lrcData.length > 0 || lyricData.yrcData.length > 0) && !editTagsForm.value.lyric) {
        try {
          const lyricLines = lyricData.lrcData.length > 0 ? lyricData.lrcData : lyricData.yrcData

          // 转换为LRC格式
          const lrcContent = lyricLines.map(line => {
            const text = line.words.map(w => w.word).join('')
            return `[${formatTime(line.startTime)}]${text}`
          }).join('\n')

          editTagsForm.value.lyric = lrcContent
          matchedCount++
          logInfo('【自动匹配】更新歌词，行数:', lyricLines.length)
        } catch (lyricError) {
          logError('【自动匹配】处理歌词失败:', lyricError)
        }
      }

      logInfo('【自动匹配】自动匹配完成，共匹配', matchedCount, '项')

      if (matchedCount > 0) {
        showSuccess(`自动匹配完成，共匹配 ${matchedCount} 项`)
      } else {
        showInfo('已找到歌曲，但没有新的信息可以匹配（可能已有完整信息）')
      }
    } catch (error) {
      logError('【自动匹配】自动匹配失败:', error)
      showError('自动匹配失败，请检查网络连接后重试')
    }
  }

  // 更换封面
  const changeCover = async () => {
    try {
      // 这里可以实现文件选择功能
      // 由于是Tauri应用，可以使用dialog插件
      const { open } = await import('@tauri-apps/plugin-dialog')
      const { readFile } = await import('@tauri-apps/plugin-fs')

      const selected = await open({
        multiple: false,
        filters: [
          {
            name: 'Image files',
            extensions: ['jpg', 'jpeg', 'png', 'gif', 'bmp']
          }
        ]
      })

      logInfo('选择的文件:', selected)

      // 处理返回值（可能是字符串或数组）
      let filePath: string | null = null
      if (typeof selected === 'string') {
        filePath = selected
      } else if (Array.isArray(selected) && (selected as any[]).length > 0) {
        filePath = (selected as any[])[0]
      }

      if (filePath) {
        logInfo('读取文件:', filePath)
        // 读取文件并转换为base64
        const content = await readFile(filePath)
        logInfo('文件内容长度:', content.length)
        // 使用更安全的方式转换为base64，避免栈溢出
        const bytes = new Uint8Array(content)
        let binary = ''
        const len = bytes.byteLength
        for (let i = 0; i < len; i++) {
          binary += String.fromCharCode(bytes[i])
        }
        const base64 = btoa(binary)
        // 根据文件扩展名确定 MIME 类型
        const ext = filePath.split('.').pop()?.toLowerCase() || 'jpg'
        const mimeType = ext === 'png' ? 'image/png' :
                        ext === 'gif' ? 'image/gif' :
                        ext === 'bmp' ? 'image/bmp' : 'image/jpeg'
        editTagsForm.value.cover = `data:${mimeType};base64,${base64}`
        logInfo('封面已设置，长度:', editTagsForm.value.cover.length)
      } else {
        logInfo('未选择文件')
      }
    } catch (error) {
      logError('选择封面失败:', error)
      showError('选择封面失败，请重试')
    }
  }

  // 保存歌曲标签
  const saveSongTags = async () => {
    if (!songToEdit.value) return

    try {
      // 验证歌词内容
      if (editTagsForm.value.lyric && editTagsForm.value.lyric.length > 100000) {
        showWarning('歌词内容过长，请精简后重试')
        return
      }

      // 更新歌曲信息
      const updatedSong: Song = {
        ...songToEdit.value,
        ...editTagsForm.value
      } as Song

      // 找到并更新歌曲列表中的歌曲
      const index = songs.value.findIndex(s => s.id === songToEdit.value?.id)
      if (index !== -1) {
        songs.value[index] = updatedSong as Song
      }

      // 如果是当前播放的歌曲，也更新当前歌曲信息
      if (currentSong.value?.id === songToEdit.value?.id) {
        currentSong.value = updatedSong as Song
      }

      // 保存到本地存储（确保类型兼容）
      const songsToSave = songs.value.map(song => ({
        ...song,
        startTime: song.startTime ? String(song.startTime) : undefined,
        endTime: song.endTime ? String(song.endTime) : undefined
      })) as import('../stores/local').Song[]
      await localStorageService.saveSongs(songsToSave)

      showSuccess('标签编辑成功')
      closeEditTagsModal()
    } catch (error) {
      logError('保存标签失败:', error)
      // 提供更详细的错误信息
      if (error instanceof Error) {
        showError(`保存标签失败: ${error.message}\n请检查歌词内容是否过大或包含特殊字符`)
      } else {
        showError('保存标签失败，请重试')
      }
    }
  }

  return {
    // 状态
    showEditTagsModal,
    showOnlineMatchModal,
    editTagsForm,
    songToEdit,
    // 动作
    editSongTags,
    closeEditTagsModal,
    copyPath,
    readLocalMetadata,
    fetchLyric,
    fetchCover,
    openOnlineMatch,
    handleOnlineMatchApply,
    autoMatchTags,
    changeCover,
    saveSongTags
  }
}
