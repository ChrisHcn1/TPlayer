import axios from 'axios';

// 创建axios实例，使用公开的网易云音乐API代理服务
const apiClient = axios.create({
  baseURL: 'https://netease-cloud-music-api-gules.vercel.app',
  timeout: 10000,
  headers: {
    'Content-Type': 'application/json'
  }
});

// 获取歌曲详情
export const songDetail = (ids: number | number[]) => {
  return apiClient({
    url: '/song/detail',
    params: {
      ids: Array.isArray(ids) ? ids.join(',') : ids.toString()
    }
  });
};

// 获取歌曲歌词
export const songLyric = (id: number) => {
  return apiClient({
    url: '/lyric',
    params: {
      id
    }
  });
};

// 获取歌曲歌词（别名函数）
export const fetchLyricById = (id: number) => {
  return songLyric(id);
};

// 本地歌曲文件匹配
export const matchSong = (
  title: string,
  artist: string,
  album: string,
  duration: number,
  md5: string
) => {
  // 使用搜索API作为替代，因为文件匹配API可能需要特殊权限
  return apiClient({
    url: '/search',
    params: {
      keywords: `${title} ${artist}`,
      type: 1,
      limit: 5
    }
  });
};

// 搜索歌曲
export const searchSong = (keyword: string, limit: number = 10) => {
  return apiClient({
    url: '/search',
    params: {
      keywords: keyword,
      type: 1,
      limit: limit
    }
  });
};
