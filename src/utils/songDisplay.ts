// 显示函数只依赖这 4 个字段，使用最小结构类型以避免耦合
// 具体的 Song 定义（App.vue / stores/local 各自的接口）均结构兼容
interface SongDisplayInfo {
  title: string
  artist: string
  album: string
  path: string
}

// 常见音频文件扩展名，用于判断标题是否只是一个扩展名
const AUDIO_EXTENSIONS = ['mp3', 'flac', 'wav', 'aac', 'ogg', 'm4a', 'ape', 'dsd', 'dts', 'wma', 'opus']

// 从路径中提取不含扩展名的文件名（兼容 Windows 与 Unix 分隔符）
export function getFileNameWithoutExtension(path: string): string {
  // 提取文件名
  const fileName = path.split('\\').pop()?.split('/').pop() || ''
  // 移除后缀名
  const lastDotIndex = fileName.lastIndexOf('.')
  if (lastDotIndex > 0) {
    return fileName.substring(0, lastDotIndex)
  }
  return fileName
}

// 从文件名中提取艺术家和专辑信息
export function extractInfoFromFileName(fileName: string): { artist: string; album: string } {
  // 常见格式：艺术家 - 歌曲名
  // 或者：艺术家 - 专辑 - 歌曲名
  const parts = fileName.split('-').map(part => part.trim())

  if (parts.length >= 2) {
    return {
      artist: parts[0],
      album: parts.length >= 3 ? parts[1] : ''
    }
  }

  return {
    artist: '',
    album: ''
  }
}

// 获取显示的歌曲标题
export function getDisplayTitle(song: SongDisplayInfo): string {
  // 检查标题是否只是一个文件扩展名
  if (song.title && AUDIO_EXTENSIONS.includes(song.title.toLowerCase())) {
    // 如果标题只是扩展名，使用文件名（不含后缀）
    return getFileNameWithoutExtension(song.path)
  }

  // 去掉标题后面的时间信息（格式：::开始时间::结束时间）
  let displayTitle = song.title || getFileNameWithoutExtension(song.path)
  const parts = displayTitle.split('::')
  if (parts.length >= 3) {
    // 如果有至少3个部分，说明包含时间信息，只保留第一部分
    displayTitle = parts[0]
  }

  return displayTitle
}

// 获取显示的艺术家名称
export function getDisplayArtist(song: SongDisplayInfo): string {
  if (song.artist && song.artist !== '未知艺术家') {
    return song.artist
  }

  // 尝试从文件名中提取艺术家信息
  const fileName = getFileNameWithoutExtension(song.path)
  const info = extractInfoFromFileName(fileName)

  return info.artist || '未知艺术家'
}

// 获取显示的专辑名称
export function getDisplayAlbum(song: SongDisplayInfo): string {
  if (song.album && song.album !== '未知专辑') {
    return song.album
  }

  // 尝试从文件名中提取专辑信息
  const fileName = getFileNameWithoutExtension(song.path)
  const info = extractInfoFromFileName(fileName)

  return info.album || '未知专辑'
}
