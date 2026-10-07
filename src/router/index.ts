/**
 * 路由入口（M0 骨架，对齐 docs/DESIGN.md §1.4 路由清单）
 *
 * 约定：
 * 1. 侧栏导航由 meta.nav（library / tools / bottom）+ meta.titleKey（i18n 键）驱动，
 *    菜单项不在组件内硬编码；新增视图无需改侧栏代码。
 * 2. 二级（详情）路由用 meta.parent 指向所属一级导航项，用于父项高亮与"返回"目标。
 * 3. meta.icon 为 §1.1 / §3.7 的语义图标名（icon-{语义}.svg）：声明了 icon 的视图才占侧栏项，
 *    因此 /search（由标题栏搜索框承载，§1.1）不声明 icon，天然不出现在侧栏。
 * 4. keepAlive 仅声明缓存意图，<KeepAlive> 的实际装配在应用外壳层完成。
 * 5. hash 模式：避免 tauri:// 自定义协议下的 history 兜底问题。
 * 6. 视图组件全部懒加载，避免首屏把全部页面打进主包。
 *
 * 文案约定：路由只提供 titleKey（i18n 键），展示文案统一由 src/i18n/nav 解析。
 * M0 早期为兼容尚未迁移的 SideBar 曾在一级路由保留 meta.title（展示名），现侧栏已改为
 * meta.nav + meta.titleKey 驱动，该兼容字段已删除——避免路由与组件两处维护同一文案。
 */
import { createRouter, createWebHashHistory, type RouteRecordRaw } from 'vue-router'

/**
 * 侧栏导航分组（DESIGN §1.4）：分组键与顺序定义在 src/i18n/nav（导航文案映射模块，
 * 同时提供分组标题与词条解析），此处转出类型供其他模块引用，避免同一字面量联合两处维护。
 */
export type { NavGroup } from '../i18n/nav'

const routes: RouteRecordRaw[] = [
  /* ---- 根路径重定向（§1.4：/ → /songs） ---- */
  { path: '/', redirect: '/songs' },

  /* ---- 曲库分组（nav = library） ---- */
  {
    path: '/songs',
    name: 'songs',
    component: () => import('../views/SongsView.vue'),
    meta: {
      nav: 'library',
      titleKey: 'nav.songs',
      icon: 'music-note',
      keepAlive: true,
    },
  },
  {
    path: '/recent',
    name: 'recent',
    component: () => import('../views/RecentView.vue'),
    meta: {
      nav: 'library',
      titleKey: 'nav.recent',
      icon: 'clock',
      keepAlive: true,
    },
  },
  {
    path: '/favorites',
    name: 'favorites',
    component: () => import('../views/FavoritesView.vue'),
    meta: {
      nav: 'library',
      titleKey: 'nav.favorites',
      icon: 'heart',
      keepAlive: true,
    },
  },
  {
    path: '/artists',
    name: 'artists',
    component: () => import('../views/ArtistsView.vue'),
    meta: {
      nav: 'library',
      titleKey: 'nav.artists',
      icon: 'user',
      keepAlive: true,
    },
  },
  {
    path: '/artists/:id',
    name: 'artist-detail',
    component: () => import('../views/ArtistDetailView.vue'),
    meta: { nav: 'library', titleKey: 'nav.artists', parent: 'artists', keepAlive: false },
  },
  {
    path: '/albums',
    name: 'albums',
    component: () => import('../views/AlbumsView.vue'),
    meta: {
      nav: 'library',
      titleKey: 'nav.albums',
      icon: 'disc',
      keepAlive: true,
    },
  },
  {
    path: '/albums/:id',
    name: 'album-detail',
    component: () => import('../views/AlbumDetailView.vue'),
    meta: { nav: 'library', titleKey: 'nav.albums', parent: 'albums' },
  },
  {
    path: '/cue',
    name: 'cue',
    component: () => import('../views/CueAlbumsView.vue'),
    meta: {
      nav: 'library',
      titleKey: 'nav.cue',
      icon: 'layers',
      keepAlive: true,
    },
  },
  {
    path: '/cue/:id',
    name: 'cue-detail',
    component: () => import('../views/CueDetailView.vue'),
    meta: { nav: 'library', titleKey: 'nav.cue', parent: 'cue' },
  },
  {
    path: '/playlists',
    name: 'playlists',
    component: () => import('../views/PlaylistsView.vue'),
    meta: {
      nav: 'library',
      titleKey: 'nav.playlists',
      icon: 'list-music',
      keepAlive: true,
    },
  },
  {
    path: '/playlists/:id',
    name: 'playlist-detail',
    component: () => import('../views/PlaylistDetailView.vue'),
    meta: { nav: 'library', titleKey: 'nav.playlists', parent: 'playlists' },
  },
  {
    path: '/search',
    name: 'search',
    component: () => import('../views/SearchView.vue'),
    // 结果页支持 ?q= 直达（§1.4 query.q）；搜索由标题栏承载、不占侧栏项，故不声明 icon（§1.1）
    meta: { nav: 'library', titleKey: 'nav.search', keepAlive: false },
  },

  /* ---- 工具（不占侧栏项：入口在设置页"工具"页签，命令面板亦可直达） ---- */
  {
    path: '/converter',
    name: 'converter',
    component: () => import('../views/ConverterView.vue'),
    meta: { titleKey: 'nav.converter' },
  },

  /* ---- 底部固定分组（nav = bottom） ---- */
  {
    path: '/settings',
    name: 'settings',
    component: () => import('../views/SettingsView.vue'),
    meta: { nav: 'bottom', titleKey: 'nav.settings', icon: 'settings' },
  },
  {
    path: '/settings/:tab',
    name: 'settings-tab',
    component: () => import('../views/SettingsView.vue'),
    // tab 取值：playback / appearance / lyrics / online / update / about（§1.4、§2.4 图 2-4-5）
    meta: { nav: 'bottom', titleKey: 'nav.settings', parent: 'settings' },
  },

  /* ---- 兜底：未知路径回到曲库首页 ---- */
  { path: '/:pathMatch(.*)*', redirect: '/songs' },
]

export const router = createRouter({
  // 桌面端使用 hash 模式，避免 tauri:// 自定义协议下的 history 兜底问题
  history: createWebHashHistory(),
  routes,
})

export default router
