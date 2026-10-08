<template>
  <!-- 启动画面 -->
  <div v-if="isLoading" class="splash-screen">
    <div class="splash-content">
      <img src="/logo.png" alt="TPlayer Logo" class="splash-logo" />
      <h1 class="splash-title">TPlayer</h1>
      <p class="splash-slogan">让音乐触动心灵</p>
      <div class="splash-loading">
        <div class="loading-spinner"></div>
        <span>加载中...</span>
      </div>
    </div>
  </div>
  
  <div id="app" class="tplayer-container" :class="{ 'light': theme === 'light' }" v-if="!isLoading">
    <!-- 顶部信息栏 -->
    <header class="top-bar" data-tauri-drag-region>
      <div class="app-logo" data-tauri-drag-region="false">
        <img src="/logo.png" alt="TPlayer Logo" class="logo-image" />
        <h1>TPlayer</h1>
      </div>
      <div class="window-controls" data-tauri-drag-region="false">
        <button class="control-btn minimize" @click="minimizeWindow" :title="t('buttons.minimize')">−</button>
        <button class="control-btn maximize" @click="toggleMaximizeWindow" :title="t('buttons.maximize')">□</button>
        <button class="control-btn close" @click="closeWindow" :title="t('buttons.close')">×</button>
      </div>
    </header>
    
    <!-- 主内容区 -->
    <main class="main-content">
      <!-- 左侧边栏 -->
      <aside class="sidebar" :class="{ 'collapsed': !sidebarVisible }">
        <div class="sidebar-header">
          <button class="toggle-btn" @click="toggleSidebar" :title="t('buttons.toggleSidebar')">
            {{ sidebarVisible ? '◀' : '▶' }}
          </button>
          <h2>{{ t('playlist.title') }}</h2>
        </div>
        <nav class="sidebar-nav">
          <ul>
            <li class="nav-item active" @click="switchFilter('all')">
              <span class="nav-icon">🎵</span>
              <span class="nav-text">{{ t('playlist.allSongs') }}</span>
            </li>
            <li class="nav-item" @click="switchFilter('favorites')">
              <span class="nav-icon">❤️</span>
              <span class="nav-text">{{ t('playlist.favorites') }}</span>
            </li>
            <li class="nav-item" @click="switchFilter('artists')">
              <span class="nav-icon">👤</span>
              <span class="nav-text">{{ t('playlist.artists') }}</span>
            </li>
            <li class="nav-item" @click="switchFilter('albums')">
              <span class="nav-icon">💽</span>
              <span class="nav-text">{{ t('playlist.albums') }}</span>
            </li>
            <li class="nav-item" @click="switchFilter('cue')" v-if="cueAlbums.length > 0">
              <span class="nav-icon">📀</span>
              <span class="nav-text">{{ t('playlist.cueAlbums') }}</span>
              <span class="nav-badge">{{ cueAlbums.length }}</span>
            </li>
            <li class="nav-item" @click="showAudioConverter = true" title="音频转换">
              <span class="nav-icon">🔄</span>
              <span class="nav-text">{{ t('converter.title', '音频转换') }}</span>
            </li>
          </ul>
        </nav>
        <div class="sidebar-playlists" v-if="playlists.length > 0">
          <div class="sidebar-section-title">{{ t('playlist.myPlaylists') }}</div>
          <ul>
            <li
              v-for="pl in playlists"
              :key="pl.id"
              class="nav-item"
              :class="{ active: currentFilter === 'playlist' && selectedPlaylistId === pl.id }"
              @click="openPlaylist(pl.id)"
            >
              <span class="nav-icon">📂</span>
              <span class="nav-text">{{ pl.name }}</span>
            </li>
          </ul>
        </div>
        <div class="sidebar-footer">
          <button class="btn primary" @click="handleCreatePlaylist" :title="t('playlist.createPlaylist')">
            + {{ t('playlist.createPlaylist') }}
          </button>
        </div>
      </aside>
      
      <!-- 右侧内容区 -->
      <section class="content-area" :class="{ 'sidebar-collapsed': !sidebarVisible }">
        <!-- 过滤控制区 -->
        <div class="filter-controls">
          <div class="filter-header">
            <!-- 左侧区块：标题和统计信息 -->
            <div class="filter-header-left">
              <h2>{{ currentFilterText }}</h2>
              <div class="playlist-info">
                {{ filteredSongs.length }} {{ t('playlist.songs') }} • {{ t('playlist.totalDuration') }}: {{ totalDurationText }}
              </div>
            </div>
            
            <!-- 中间区块：歌词显示 -->
            <div class="filter-header-center">
              <div v-if="currentSong && showLyrics && lyricsPosition === 'top'" class="current-lyric-display">
                <span v-if="lyrics.length > 0 && currentLyricIndex >= 0" class="current-lyric">
                  {{ lyrics[currentLyricIndex].text }}
                </span>
                <span v-else class="no-lyric">
                  {{ t('playlist.noLyrics') }}
                </span>
              </div>
            </div>
            
            <!-- 右侧区块：操作按钮 -->
            <div class="filter-header-right">
              <button class="btn primary" @click="scanMusic" :title="t('playlist.scanMusic')">
                📁 {{ t('playlist.scanMusic') }}
              </button>
            </div>
          </div>
          <!-- 搜索框 -->
          <div class="search-box">
            <input 
              type="text" 
              v-model="searchQuery" 
              :placeholder="t('playlist.searchPlaceholder')"
              @input="handleSearch"
            />
            <button class="search-btn" @click="handleSearch">🔍</button>
          </div>
        </div>

        <!-- 选择工具栏（有选择时显示） -->
        <div class="selection-toolbar" v-if="selectedSongIds.size > 0">
          <div class="selection-toolbar-left">
            <span class="selection-count">
              {{ selectedSongIds.size }} {{ t('selection.selected') }}
            </span>
          </div>
          <div class="selection-toolbar-right">
            <button class="selection-action-btn" @click="addSelectedToPlaylist($event)" :title="t('selection.addToPlaylist')">
              <span class="btn-icon">+</span>
              <span class="btn-text">{{ t('selection.addToPlaylist') }}</span>
            </button>
            <button class="selection-action-btn" @click="playSelectedSongs" :title="t('selection.playAll')">
              <span class="btn-icon">▶</span>
              <span class="btn-text">{{ t('selection.playAll') }}</span>
            </button>
            <button class="selection-action-btn danger" @click="deleteSelectedSongs" :title="t('selection.delete')">
              <span class="btn-icon">🗑</span>
              <span class="btn-text">{{ t('selection.delete') }}</span>
            </button>
            <button class="selection-action-btn" @click="clearSelection" :title="t('selection.clear')">
              <span class="btn-icon">✕</span>
              <span class="btn-text">{{ t('selection.clear') }}</span>
            </button>
          </div>
        </div>

        <!-- 歌曲列表 -->
        <div class="song-list-container" ref="songListContainer">
          <!-- 悬浮控制按钮 -->
          <div class="playlist-float-buttons" >
            <button 
              class="float-button" 
              @click="scrollToTop"
              :title="t('playlist.backToTop')"
              v-if="showScrollTopButton"
            >
              ↑
            </button>
            <button 
              class="float-button" 
              @click="scrollToCurrentSong"
              :title="t('playlist.jumpToCurrent')"
              :disabled="!currentSong"
              v-if="showJumpToCurrentButton && currentSong"
            >
              ⚪
            </button>
          </div>
          
          <div v-if="songs.length === 0" class="empty-state">
            <div class="empty-icon">🎵</div>
            <p>{{ t('playlist.noSongs') }}</p>
            <p class="empty-hint">{{ t('playlist.clickToScan') }}</p>
          </div>
          
          <!-- 艺术家视图 - 双栏布局 -->
          <div v-else-if="currentFilter === 'artists'" class="artists-view">
            <div class="artists-sidebar">
              <div 
                v-for="artist in artistsList" 
                :key="artist.name"
                class="artist-item"
                :class="{ 'active': selectedArtist === artist.name }"
                @click="selectedArtist = artist.name"
              >
                <div class="artist-name">{{ artist.name }}</div>
                <div class="artist-count">{{ artist.count }} {{ t('playlist.songs') }}</div>
              </div>
            </div>
            <div class="artists-content">
              <div v-if="!selectedArtist" class="empty-selection">
                <p>{{ t('playlist.selectArtist') }}</p>
              </div>
              <div v-else class="song-list">
                <!-- 表头 -->
                <table class="songs-table table-header">
                  <thead>
                    <tr>
                      <th class="col-index">#</th>
                      <th class="col-title">{{ t('playlist.titleHeader') }}</th>
                      <th class="col-album">{{ t('playlist.albumHeader') }}</th>
                      <th class="col-duration">{{ t('playlist.durationHeader') }}</th>
                      <th class="col-actions">{{ t('playlist.actionsHeader') }}</th>
                    </tr>
                  </thead>
                </table>
                <!-- 歌曲列表 -->
                <div class="song-list">
                  <div
                    v-for="(item, index) in filteredSongs"
                    :key="item.id"
                    class="song-row"
                    :class="{ 'active': item.id === currentSong?.id, 'selected': isSongSelected(item) }"
                    @click="handleSongRowClick(item, $event)"
                    @contextmenu.prevent="openSongMenu(item, $event)"
                  >
                    <span class="col-checkbox" v-if="isSelectionMode || selectedSongIds.size > 0">
                      <input
                        type="checkbox"
                        :checked="isSongSelected(item)"
                        @click.stop
                        @change="toggleSongSelection(item)"
                      />
                    </span>
                    <span class="col-index">{{ index + 1 }}</span>
                    <span class="col-title">
                      <div class="song-title" :title="getDisplayTitle(item)">
                        {{ getDisplayTitle(item) }}
                      </div>
                      <div class="song-info" :title="getDisplayAlbum(item)">{{ getDisplayAlbum(item) }}</div>
                    </span>
                    <span class="col-album" :title="getDisplayAlbum(item)">{{ getDisplayAlbum(item) }}</span>
                    <span class="col-duration">{{ item.duration }}</span>
                    <span class="col-actions">
                      <button
                        class="action-btn favorite"
                        @click.stop="toggleFavorite(item)"
                        :class="{ 'active': item.isFavorite }"
                        :title="t('buttons.favorite')"
                      >
                        ♥
                      </button>
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>
          
          <!-- 专辑视图 - 双栏布局 -->
          <div v-else-if="currentFilter === 'albums'" class="albums-view">
            <div class="albums-sidebar">
              <div 
                v-for="album in albumsList" 
                :key="album.name + album.artist"
                class="album-item"
                :class="{ 'active': selectedAlbum === `${album.name} - ${album.artist}` }"
                @click="selectedAlbum = `${album.name} - ${album.artist}`"
              >
                <div class="album-name">{{ album.name }}</div>
                <div class="album-artist">{{ album.artist }}</div>
                <div class="album-count">{{ album.count }} {{ t('playlist.songs') }}</div>
              </div>
            </div>
            <div class="albums-content">
              <div v-if="!selectedAlbum" class="empty-selection">
                <p>{{ t('playlist.selectAlbum') }}</p>
              </div>
              <div v-else class="song-list">
                <!-- 表头 -->
                <table class="songs-table table-header">
                  <thead>
                    <tr>
                      <th class="col-index">#</th>
                      <th class="col-title">{{ t('playlist.titleHeader') }}</th>
                      <th class="col-artist">{{ t('playlist.artistHeader') }}</th>
                      <th class="col-duration">{{ t('playlist.durationHeader') }}</th>
                      <th class="col-actions">{{ t('playlist.actionsHeader') }}</th>
                    </tr>
                  </thead>
                </table>
                <!-- 歌曲列表 -->
                <div class="song-list">
                  <div
                    v-for="(item, index) in filteredSongs"
                    :key="item.id"
                    class="song-row"
                    :class="{ 'active': item.id === currentSong?.id }"
                    @click="playSong(item)"
                    @contextmenu.prevent="openSongMenu(item, $event)"
                  >
                    <span class="col-index">{{ index + 1 }}</span>
                    <span class="col-title">
                      <div class="song-title" :title="getDisplayTitle(item)">
                        {{ getDisplayTitle(item) }}
                      </div>
                      <div class="song-info" :title="getDisplayArtist(item)">{{ getDisplayArtist(item) }}</div>
                    </span>
                    <span class="col-artist" :title="getDisplayArtist(item)">{{ getDisplayArtist(item) }}</span>
                    <span class="col-duration">{{ item.duration }}</span>
                    <span class="col-actions">
                      <button
                        class="action-btn favorite"
                        @click.stop="toggleFavorite(item)"
                        :class="{ 'active': item.isFavorite }"
                        :title="t('buttons.favorite')"
                      >
                        ♥
                      </button>
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>
          
          <!-- 普通歌曲列表 -->
          <div v-else class="song-list">
            <!-- 表头 -->
            <table class="songs-table table-header">
              <thead>
                    <tr>
                      <th class="col-index">#</th>
                      <th class="col-title">{{ t('playlist.titleHeader') }}</th>
                      <th class="col-artist">{{ t('playlist.artistHeader') }}</th>
                      <th class="col-album">{{ t('playlist.albumHeader') }}</th>
                      <th class="col-duration">{{ t('playlist.durationHeader') }}</th>
                      <th class="col-actions">{{ t('playlist.actionsHeader') }}</th>
                    </tr>
                  </thead>
            </table>
            
            <!-- 普通滚动列表 -->
            <div class="song-list">
              <div v-if="currentFilter === 'playlist' && filteredSongs.length === 0" class="playlist-empty-hint">
                🎶 {{ t('playlist.emptyPlaylistHint') }}
              </div>
              <div
                v-for="(item, index) in filteredSongs"
                :key="item.id"
                class="song-row"
                :class="{ 'active': item.id === currentSong?.id }"
                @click="playSong(item)"
                @contextmenu.prevent="openSongMenu(item, $event)"
              >
                <span class="col-index">{{ index + 1 }}</span>
                <span class="col-title">
                  <div class="song-title" :title="getDisplayTitle(item)">
                    {{ getDisplayTitle(item) }}
                  </div>
                </span>
                <span class="col-artist" :title="getDisplayArtist(item)">{{ getDisplayArtist(item) }}</span>
                <span class="col-album" :title="getDisplayAlbum(item)">{{ getDisplayAlbum(item) }}</span>
                <span class="col-duration">{{ item.duration }}</span>
                <span class="col-actions">
                  <button
                    class="action-btn favorite"
                    @click.stop="toggleFavorite(item)"
                    :class="{ 'active': item.isFavorite }"
                    title="收藏"
                  >
                    ♥
                  </button>
                </span>
              </div>
            </div>
          </div>
          
          <!-- CUE专辑视图 - 双栏布局 -->
          <div v-if="currentFilter === 'cue'" class="albums-view">
            <div class="albums-sidebar">
              <div 
                v-for="album in cueAlbums"
                :key="album.filePath"
                class="album-item"
                :class="{ 'active': selectedCueAlbum?.filePath === album.filePath }"
                @click="selectCueAlbum(album)"
              >
                <div class="album-name">{{ album.title || t('playlist.unknownAlbum') }}</div>
                <div class="album-artist">{{ album.performer || t('playlist.unknownArtist') }}</div>
                <div class="album-count">{{ (album as any).tracks?.length || 0 }} {{ t('playlist.songs') }}</div>
              </div>
            </div>
            <div class="albums-content">
              <div v-if="!selectedCueAlbum" class="empty-selection">
                <p>{{ t('playlist.selectCueAlbum') }}</p>
              </div>
              <div v-else class="song-list">
                <!-- 表头 -->
                <table class="songs-table table-header">
                  <thead>
                    <tr>
                      <th class="col-index">#</th>
                      <th class="col-title">{{ t('playlist.titleHeader') }}</th>
                      <th class="col-artist">{{ t('playlist.artistHeader') }}</th>
                      <th class="col-duration">{{ t('playlist.durationHeader') }}</th>
                      <th class="col-actions">{{ t('playlist.actionsHeader') }}</th>
                    </tr>
                  </thead>
                </table>
                <!-- 歌曲列表 -->
                <div class="song-list">
                  <div
                    v-for="(item, index) in getCueAlbumTracks(selectedCueAlbum.filePath)"
                    :key="item.id"
                    class="song-row"
                    :class="{ 'active': item.id === currentSong?.id }"
                    @click="playCueTrackInApp(item)"
                  >
                    <span class="col-index">{{ index + 1 }}</span>
                    <span class="col-title">
                      <div class="song-title" :title="item.title">
                        {{ getDisplayTitle(item as unknown as Song) }}
                      </div>
                      <div class="song-info cue-badge">CUE Track</div>
                    </span>
                    <span class="col-artist" :title="item.artist">{{ item.artist }}</span>
                    <span class="col-duration">{{ item.duration }}</span>
                    <span class="col-actions">
                      <button
                        class="action-btn favorite"
                        @click.stop="toggleFavorite(item as unknown as Song)"
                        :class="{ 'active': favorites.includes(item.id) }"
                        :title="t('buttons.favorite')"
                      >
                        ♥
                      </button>
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
        
        <!-- 均衡器面板 -->
        <div class="equalizer-panel" :class="{ 'visible': equalizerVisible }">
          <div class="equalizer-header">
            <h3>{{ t('settings.equalizer') }}</h3>
            <button class="close-btn" @click="toggleEqualizer" :title="t('common.close')">×</button>
          </div>
          <div class="equalizer-content">
            <div class="presets">
              <select v-model="currentPreset" @change="applyPreset">
                <option value="flat">Flat</option>
                <option value="rock">Rock</option>
                <option value="pop">Pop</option>
                <option value="jazz">Jazz</option>
                <option value="classical">Classical</option>
                <option value="electronic">Electronic</option>
              </select>
            </div>
            <div class="bands">
              <div v-for="(_, index) in equalizerBands" :key="index" class="band">
                <label>{{ getBandLabel(index) }}</label>
                <input 
                  type="range" 
                  min="-12" 
                  max="12" 
                  step="0.5" 
                  v-model.number="equalizerBands[index]"
                  @input="updateEqualizer"
                />
                <span>{{ equalizerBands[index] }} dB</span>
              </div>
            </div>
          </div>
        </div>
      </section>
    </main>
    
    <!-- 底部播放控制栏 -->
    <footer class="player-controls" :class="{ 'expanded': showFullControls }">
      <div class="player-left">
        <div class="current-song">
          <div class="song-cover" v-if="currentSong" @click="openCoverModal">
            <!-- 动态封面 -->
            <video 
              v-if="currentSong.dynamicCoverUrl" 
              :src="currentSong.dynamicCoverUrl" 
              autoplay 
              loop 
              muted 
              class="dynamic-cover"
            />
            <!-- 静态封面 -->
            <img v-else-if="currentSong.cover" :src="currentSong.cover" alt="封面" />
            <div v-else class="cover-placeholder">🎵</div>
          </div>
          <div class="song-info">
            <h3 v-if="currentSong" ref="titleElement" :class="{ 'long-text': isTextLong('title') }">
              <span class="ellipsis-text">{{ getDisplayTitle(currentSong) }}</span>
            </h3>
            <p v-if="currentSong" ref="artistElement" :class="{ 'long-text': isTextLong('artist') }">
              <span class="ellipsis-text">{{ getDisplayArtist(currentSong) }} - {{ getDisplayAlbum(currentSong) }}</span>
            </p>
            <p v-else class="no-song">{{ t('playlist.noSongSelected') }}</p>
          </div>
        </div>
      </div>
      <div class="player-center">
        <div class="playback-controls">
          <button class="control-btn" @click="changePlaybackMode" :title="t('buttons.playbackMode')">
            <img :src="playbackModeImage" alt="播放模式" class="control-icon" />
          </button>
          <button class="control-btn" @click="playPrevious" :title="t('buttons.previous')">
            <img src="/last-track-button_23ee-fe0f.png" alt="上一首" class="control-icon" />
          </button>
          <button class="control-btn play" @click="togglePlayback" :title="t('buttons.playPause')">
            <img :src="isPlaying ? '/pause-button_23f8-fe0f.png' : '/play-button_25b6-fe0f.png'" alt="播放/暂停" class="control-icon" />
          </button>
          <button class="control-btn" @click="playNext" :title="t('buttons.next')">
            <img src="/next-track-button_23ed-fe0f.png" alt="下一首" class="control-icon" />
          </button>
        </div>
        <div class="progress-bar">
          <div class="progress-info">
            <span>{{ formattedCurrentPosition }}</span>
            <span>{{ currentSong?.duration || '0:00' }}</span>
          </div>
          <input
            type="range"
            min="0"
            max="100"
            step="0.1"
            v-model.number="progress"
            @input="handleSeeking"
            @change="seek"
          />
        </div>
        
        <!-- 歌词显示区域 -->
        <div v-if="showLyrics && lyricsPosition !== 'top'" class="lyrics-display" :class="{ 'has-lyrics': lyrics.length > 0 }">
          <div v-if="lyrics.length === 0" class="lyrics-placeholder">
            {{ t('playlist.noLyrics') }}
          </div>
          <div v-else class="lyrics-container">
            <div 
              v-if="currentLyricIndex >= 0 && currentLyricIndex < lyrics.length"
              class="lyric-line active"
              :data-time="lyrics[currentLyricIndex].time"
              :data-index="currentLyricIndex"
            >
              {{ lyrics[currentLyricIndex].text }}
            </div>
          </div>
        </div>
      </div>
      
      <div class="player-right">
        <div class="player-right-top">
          <div class="volume-control">
            <button class="control-btn" @click="toggleMute" :title="t('buttons.mute')">
              {{ isMuted ? '🔇' : '🔊' }}
            </button>
            <input 
              type="range" 
              min="0" 
              max="100" 
              step="1" 
              v-model.number="volume"
              @input="updateVolume"
            />
          </div>
          
          <button class="control-btn" @click="showLyrics = !showLyrics" :title="t('buttons.toggleLyrics')">
            🎵
          </button>
          <button class="control-btn" @click="showSettingsModal = true" :title="t('buttons.settings')">
            ⚙️
          </button>
        </div>
        
        <!-- 下一首歌曲信息 - 仅当正在播放时显示 -->
        <div class="next-song-info" v-if="currentSong && nextSong">
          <div class="next-song-label">{{ t('playlist.nextSong') }}</div>
          <div class="next-song-title" :title="getDisplayTitle(nextSong)">
            {{ getDisplayTitle(nextSong) }}
          </div>
          <div class="next-song-artist" :title="getDisplayArtist(nextSong)">
            {{ getDisplayArtist(nextSong) }}
          </div>
          <button class="skip-next-btn" @click="skipNextSong" :title="t('playlist.skip') + ' ' + t('playlist.nextSong')">
            {{ t('playlist.skip') }} ⏭
          </button>
        </div>
      </div>
    </footer>
    
    <!-- 歌曲菜单 -->
    <div v-if="showSongMenu" class="song-menu" :style="menuPosition">
      <ul>
        <li @click="playSong(selectedSong!)">{{ t('menu.play') }}</li>
        <li @click="openAddToPlaylistMenu(selectedSong!, $event)">{{ t('menu.addToPlaylist') }}</li>
        <li @click="toggleFavorite(selectedSong!)">
          {{ selectedSong?.isFavorite ? t('menu.removeFromFavorites') : t('menu.addToFavorites') }}
        </li>
        <li @click="editSongTags(selectedSong!)">{{ t('menu.editTags') }}</li>
        <li @click="deleteSong(selectedSong!)" class="danger">{{ t('menu.delete') }}</li>
      </ul>
    </div>

    <!-- 加入歌单弹层：点选已有歌单或新建 -->
    <div v-if="showAddToPlaylistMenu" class="song-menu add-to-playlist-menu" :style="addMenuPosition">
      <ul>
        <li v-if="playlists.length === 0" class="menu-hint">{{ t('playlist.noPlaylists') }}</li>
        <li v-for="pl in playlists" :key="pl.id" @click="addToPlaylist(pl.id)">
          <span class="menu-item-icon">📂</span>{{ pl.name }}
        </li>
        <li class="menu-separator"></li>
        <li class="menu-new-item" @click="createPlaylistAndAdd">
          <span class="menu-item-icon">＋</span>{{ t('menu.newPlaylist') }}
        </li>
      </ul>
    </div>
    
    <!-- 编辑歌曲标签模态框 -->
    <div v-if="showEditTagsModal" class="modal-overlay" @click="closeEditTagsModal">
      <div class="modal-content edit-tags-modal" @click.stop>
        <div class="modal-header">
          <h3>{{ t('modal.editTagsTitle') }}</h3>
          <button class="close-btn" @click="closeEditTagsModal">×</button>
        </div>
        <div class="modal-body">
          <!-- 快速操作栏 -->
          <div class="quick-actions">
            <button class="quick-btn" @click="readLocalMetadata">
              📖 {{ t('modal.readLocalMetadata') }}
            </button>
            <button class="quick-btn" @click="autoMatchTags">
              🏷️ {{ t('modal.matchFromFilename') }}
            </button>
            <button class="quick-btn" @click="openOnlineMatch">
              🌐 {{ t('modal.onlineMatchTags') }}
            </button>
          </div>
          
          <!-- 标签页 -->
          <div class="tabs">
            <div class="tab-buttons">
              <button 
                class="tab-button" 
                :class="{ active: activeTab === 'info' }"
                @click="activeTab = 'info'"
              >
                📝 {{ t('modal.basicInfo') }}
              </button>
              <button 
                class="tab-button" 
                :class="{ active: activeTab === 'lyric' }"
                @click="activeTab = 'lyric'"
              >
                🎵 {{ t('modal.lyrics') }}
              </button>
              <button 
                class="tab-button" 
                :class="{ active: activeTab === 'cover' }"
                @click="activeTab = 'cover'"
              >
                🖼️ {{ t('modal.cover') }}
              </button>
            </div>
            
            <!-- 基本信息标签页 -->
            <div v-show="activeTab === 'info'" class="tab-content">
              <div class="form-row">
                <div class="form-group">
                  <label>{{ t('modal.fileName') }}</label>
                  <input type="text" v-model="editTagsForm.fileName" disabled>
                </div>
              </div>
              <div class="form-row two-col">
                <div class="form-group">
                  <label>{{ t('modal.title') }}</label>
                  <input type="text" v-model="editTagsForm.title" :placeholder="t('modal.enterTitle')">
                </div>
                <div class="form-group">
                  <label>{{ t('modal.artist') }}</label>
                  <input type="text" v-model="editTagsForm.artist" :placeholder="t('modal.enterArtist')">
                </div>
              </div>
              <div class="form-row two-col">
                <div class="form-group">
                  <label>{{ t('modal.album') }}</label>
                  <input type="text" v-model="editTagsForm.album" :placeholder="t('modal.enterAlbum')">
                </div>
                <div class="form-group">
                  <label>{{ t('modal.albumArtist') }}</label>
                  <input type="text" v-model="editTagsForm.albumArtist" :placeholder="t('modal.enterAlbumArtist')">
                </div>
              </div>
              <div class="form-row two-col">
                <div class="form-group">
                  <label>{{ t('modal.genre') }}</label>
                  <input type="text" v-model="editTagsForm.genre" :placeholder="t('modal.enterGenre')">
                </div>
                <div class="form-group">
                  <label>{{ t('modal.year') }}</label>
                  <input type="text" v-model="editTagsForm.year" :placeholder="t('modal.enterYear')">
                </div>
              </div>
              <div class="form-row three-col">
                <div class="form-group">
                  <label>{{ t('modal.trackNumber') }}</label>
                  <input type="text" v-model="editTagsForm.trackNumber" :placeholder="t('modal.enterTrackNumber')">
                </div>
                <div class="form-group">
                  <label>{{ t('modal.discNumber') }}</label>
                  <input type="text" v-model="editTagsForm.discNumber" :placeholder="t('modal.enterDiscNumber')">
                </div>
                <div class="form-group">
                  <label>{{ t('modal.alia') }}</label>
                  <input type="text" v-model="editTagsForm.alia" :placeholder="t('modal.enterAlia')">
                </div>
              </div>
              
              <!-- CUE信息区域 -->
              <div v-if="songToEdit && songToEdit.isCueTrack" class="cue-info-section">
                <h4>📀 {{ t('modal.cueInfo') }}</h4>
                <div class="form-row two-col">
                  <div class="form-group">
                    <label>{{ t('modal.startTime') }}</label>
                    <input type="number" v-model="songToEdit.startTime" :placeholder="t('modal.enterStartTime')">
                  </div>
                  <div class="form-group">
                    <label>{{ t('modal.endTime') }}</label>
                    <input type="number" v-model="songToEdit.endTime" :placeholder="t('modal.enterEndTime')">
                  </div>
                </div>
                <div v-if="(songToEdit as any).cueInfo" class="cue-info-text">
                  <pre>{{ (songToEdit as any).cueInfo }}</pre>
                </div>
              </div>
              
              <div class="form-row">
                <div class="form-group">
                  <label>{{ t('modal.path') }}</label>
                  <div class="input-with-button">
                    <input type="text" :value="songToEdit?.path" disabled>
                    <button class="copy-btn" @click="copyPath">{{ t('modal.copy') }}</button>
                  </div>
                </div>
              </div>
            </div>
            
            <!-- 歌词标签页 -->
            <div v-show="activeTab === 'lyric'" class="tab-content">
              <div class="form-group">
                <label>{{ t('modal.lyrics') }}</label>
                <textarea 
                  v-model="editTagsForm.lyric" 
                  placeholder="[00:00.00] 歌词内容" 
                  rows="12"
                ></textarea>
              </div>
              <div class="lyric-actions">
                <button class="action-btn primary" @click="fetchLyric">
                  🔍 {{ t('modal.getLyrics') }}
                </button>
              </div>
            </div>
            
            <!-- 封面标签页 -->
            <div v-show="activeTab === 'cover'" class="tab-content">
              <div class="cover-section">
                <div class="cover-preview" @click="changeCover">
                  <img v-if="editTagsForm.cover" :src="editTagsForm.cover" alt="封面">
                  <div v-else class="cover-placeholder">
                    <div class="placeholder-icon">📷</div>
                    {{ t('modal.clickToChangeCover') }}
                  </div>
                </div>
                <div class="cover-actions">
                  <button class="action-btn" @click="changeCover">
                    📁 {{ t('modal.selectCover') }}
                  </button>
                  <button class="action-btn" @click="fetchCover">
                    🔍 {{ t('modal.getCover') }}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn-cancel" @click="closeEditTagsModal">{{ t('common.cancel') }}</button>
          <button class="btn-save" @click="saveSongTags">{{ t('common.save') }}</button>
        </div>
      </div>
    </div>
    
    <!-- 设置模态框 -->
    <div v-if="showSettingsModal" class="modal-overlay" @click="showSettingsModal = false">
      <div class="modal-content settings-modal" @click.stop>
        <div class="modal-header">
          <h2>{{ t('common.settings') }}</h2>
          <button class="close-btn" @click="showSettingsModal = false">×</button>
        </div>
        <div class="modal-body">
          <Settings
            v-model:crossfadeEnabled="crossfadeEnabled"
            v-model:crossfadeDuration="crossfadeDuration"
            v-model:autoPlayNext="autoPlayNext"
            v-model:theme="theme"
            v-model:language="language"
            v-model:musicDirectory="musicDirectory"
            v-model:showLyrics="showLyrics"
            v-model:lyricsPosition="lyricsPosition"
            v-model:equalizerEnabled="equalizerVisible"
            v-model:currentPreset="currentPreset"
            v-model:enableTranscode="enableTranscode"
            v-model:forceTranscode="forceTranscode"
            :isBrowser="isBrowser"
            @browseMusicDirectory="browseMusicDirectory"
            @checkUpdate="handleCheckUpdate"
            @save="showSettingsModal = false"
            @cancel="showSettingsModal = false"
          />
        </div>
      </div>
    </div>
    
    <!-- 封面放大模态框 -->
    <div v-if="showCoverModal" class="modal-overlay cover-modal-overlay" @click="closeCoverModal">
      <div 
        class="cover-modal-content" 
        :class="{ 'fullscreen': isCoverModalFullscreen, 'windowed': !isCoverModalFullscreen }"
        @click.stop
        ref="coverModalContent"
        :style="coverModalPosition"
      >
        <!-- 拖动标题栏 -->
        <div 
          class="cover-modal-header" 
          @mousedown="startDragCoverModal"
          @dblclick="toggleCoverModalFullscreen"
        >
          <span class="cover-modal-drag-hint">双击全屏 / 拖动移动</span>
          <div class="cover-modal-controls">
            <button class="cover-modal-btn" @click="toggleCoverModalFullscreen" :title="t('buttons.fullscreen')">
              {{ isCoverModalFullscreen ? '📱' : '📺' }}
            </button>
            <button class="cover-modal-btn" @click="closeCoverModal" :title="t('buttons.close')">✕</button>
          </div>
        </div>
        
        <div class="cover-modal-background" :style="coverBackgroundStyle"></div>
        <div class="cover-modal-body">
          <div class="cover-modal-left">
            <div class="cover-modal-image">
              <img v-if="currentSong?.cover" :src="currentSong.cover" alt="封面" />
              <div v-else class="cover-modal-placeholder">🎵</div>
            </div>
            <div class="cover-modal-info">
              <h2 class="cover-modal-title">{{ getDisplayTitle(currentSong!) }}</h2>
              <p class="cover-modal-artist">{{ getDisplayArtist(currentSong!) }}</p>
              <p class="cover-modal-album">{{ getDisplayAlbum(currentSong!) }}</p>
            </div>
          </div>
          <div class="cover-modal-right">
            <div v-if="lyrics.length > 0" class="cover-modal-lyrics" ref="coverLyricsContainer">
              <div
                v-for="(line, index) in lyrics"
                :key="index"
                :ref="(el) => { if (el) coverLyricLineRefs[index] = el }"
                class="cover-lyric-line"
                :class="{ 'active': index === currentLyricIndex }"
              >
                {{ line.text }}
              </div>
            </div>
            <div v-else class="cover-modal-no-lyrics">
              {{ t('playlist.noLyrics') }}
            </div>
          </div>
        </div>
      </div>
    </div>
    
    <!-- 在线匹配对话框 -->
    <OnlineMatchModal
      v-if="showOnlineMatchModal"
      :current-title="songToEdit?.title || ''"
      :current-artist="songToEdit?.artist || ''"
      @close="showOnlineMatchModal = false"
      @apply="handleOnlineMatchApply"
    />
    
    <!-- 音频转换器 -->
    <AudioConverter
      v-if="showAudioConverter"
      :visible="showAudioConverter"
      @close="showAudioConverter = false"
    />

    <!-- 更新模态框 -->
    <UpdateModal
      :visible="showUpdateModal"
      :updateInfo="updateInfo"
      :currentVersion="currentVersion"
      @close="closeUpdateModal"
      @update="closeUpdateModal"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { isTauri } from '@tauri-apps/api/core'
import { localStorageService, type Playlist } from './stores/local'
import { multiSourceLyricService } from './services/multiSourceLyricService'
import { useSongTagsEditor } from './composables/useSongTagsEditor'
import { useCoverModal } from './composables/useCoverModal'
import { useWindowControls } from './composables/useWindowControls'
import { useSongContextMenu } from './composables/useSongContextMenu'
import { useLibraryActions } from './composables/useLibraryActions'
import { useSongSelection } from './composables/useSongSelection'
import Settings from './components/Settings.vue'
import UpdateModal from './components/UpdateModal.vue'
import OnlineMatchModal from './components/OnlineMatchModal.vue'
import AudioConverter from './components/AudioConverter.vue'
import { i18nService, t } from './services/i18n'
import type { Song } from './types/song'
import {
  cueAlbums,
  cueTracks,
  selectedCueAlbum,
  selectCueAlbum,
  getCueAlbumTracks,
  scanCueFiles
} from './composables/useCue'
import { useUpdater } from './composables/useUpdater'
import { getDisplayTitle, getDisplayArtist, getDisplayAlbum } from './utils/songDisplay'
import { toSimpleLyricLines, parseLyrics, type SimpleLyricLine } from './utils/lyrics'
import { getBandLabel, getEqPreset } from './utils/equalizer'
import { isBrowserScannableAudio } from './utils/fileTypes'
import { needsFFplayEngine } from './constants/playbackFormats'
import { exists } from '@tauri-apps/plugin-fs'
import { logInfo, logError, logDebug } from './utils/logger'
// RecycleScroller组件通过VueVirtualScroller插件注册

// 日志函数已移至 utils/logger.ts

// Song 类型已移至 types/song.ts

// 歌词行类型已统一为 utils/lyrics.ts 的 SimpleLyricLine

// FFplay 调用结果类型
interface FFplayResult {
  duration?: number
  format?: string
  sample_rate?: number
  channels?: number
  bit_rate?: number
  bit_depth?: number
  [key: string]: any
}

// 状态管理
const sidebarVisible = ref(true)
const equalizerVisible = ref(false)
const showFullControls = ref(false)
const showSettingsModal = ref(false)
const showAudioConverter = ref(false)
// 更新流程状态（由 useUpdater composable 管理）
const { showUpdateModal, updateInfo, currentVersion, openUpdateModal, closeUpdateModal } = useUpdater()
const activeTab = ref('info')
const isLoading = ref(true)

// 歌词相关状态
const lyrics = ref<SimpleLyricLine[]>([])
const currentLyricIndex = ref(-1)
const showLyrics = ref(true)
const lyricsPosition = ref<'top' | 'bottom'>('bottom')
const yrcData = ref<any[]>([])

// 主题相关状态
const theme = ref<'dark' | 'light'>('dark')

// 语言相关状态
const language = ref('zh-CN')

// 音乐目录设置（仅浏览器）
const musicDirectory = ref('')

// 环境检测 - 使用Tauri v2的isTauri()函数
const checkIsBrowser = async () => {
  try {
    const isTauriEnv = await isTauri()
    console.log('【环境检测】isTauri():', isTauriEnv)
    return !isTauriEnv
  } catch (error) {
    console.log('【环境检测】isTauri()调用失败，认为是浏览器环境:', error)
    return true
  }
}
const isBrowser = ref(true) // 默认为浏览器环境，等待异步检测完成

// 播放设置
const crossfadeEnabled = ref(false)
const crossfadeDuration = ref(1) // 默认为1秒，范围0-3秒
const autoPlayNext = ref(true) // 自动播放下一首
const enableTranscode = ref(true) // 启用转码
const forceTranscode = ref(false) // 强制转码
const playbackStartTime = ref(Date.now()) // 开始播放的时间
const pauseStartTime = ref<number | null>(null) // 开始暂停的时间
const pausedDuration = ref(0) // 累计暂停的时间
const audioElement = ref<HTMLAudioElement | null>(null) // 前端音频元素

// 时间更新事件处理器
let timeupdateHandler: ((this: HTMLAudioElement, ev: Event) => any) | null = null

// 歌曲相关
const songs = ref<Song[]>([])
const currentSong = ref<Song | null>(null)
const currentPosition = ref(0)
const progress = ref(0)
const isPlaying = ref(false)

// FFplay播放器状态
const isFFplayPlaying = ref(false)
const ffplayDuration = ref(0)
const ffplayPosition = ref(0)
const ffplayVolume = ref(1.0)
let ffplayStatusInterval: number | null = null

// Seek防抖机制
let seekDebounceTimer: number | null = null
let pendingSeekPosition: number | null = null
let seekInProgress: boolean = false  // Seek进行中标志，防止状态轮询覆盖位置
let seekCompleteTimer: number | null = null  // seek 完成后延迟清除标志的定时器

// 防抖Seek函数（200毫秒延迟）
const debouncedSeek = async (position: number) => {
  if (seekDebounceTimer) {
    clearTimeout(seekDebounceTimer)
  }
  
  pendingSeekPosition = position
  seekInProgress = true  // 标记Seek开始
  
  seekDebounceTimer = window.setTimeout(async () => {
    if (pendingSeekPosition !== null && isFFplayPlaying.value && currentSong.value) {
      try {
        logInfo(`【FFplay】执行防抖Seek: ${pendingSeekPosition}秒`)
        const result = await invoke('seek_ffplay', {
          path: currentSong.value.path,
          position: pendingSeekPosition
        }) as any
        
        if (result && result.success !== false) {
          playbackStartTime.value = Date.now() - (pendingSeekPosition * 1000)
          currentPosition.value = pendingSeekPosition
          const totalSeconds = ffplayDuration.value || 1
          progress.value = Math.min((pendingSeekPosition / totalSeconds) * 100, 100)
          logInfo('【SEEK】FFplay防抖seek完成: currentPosition=', pendingSeekPosition, 's')
        }
      } catch (error) {
        logError('【SEEK】FFplay防抖Seek失败:', error)
      } finally {
        pendingSeekPosition = null
        seekDebounceTimer = null
        // 延迟500ms后清除Seek进行中标志，给FFplay状态更新留出时间
        seekCompleteTimer = window.setTimeout(() => {
          seekInProgress = false
          seekCompleteTimer = null
        }, 500)
      }
    } else {
      // 如果条件不满足，也要清除标志
      pendingSeekPosition = null
      seekDebounceTimer = null
      seekInProgress = false
    }
  }, 200)
}

// 浏览器环境下的文件对象存储
const browserFileMap = new Map<string, File>()

// 启用日志输出以便调试
const enableLogs = (logInfoFn: (...args: any[]) => void, logErrorFn: (...args: any[]) => void, logDebugFn: (...args: any[]) => void, browserFileMapRef: Map<string, File>): void => {
  // 只在浏览器环境下设置全局变量（且window必须是可写的普通对象）
  try {
    if (typeof window !== 'undefined' && window !== null && typeof window === 'object') {
      const win = window as any
      // 测试window是否可写
      try {
        win.testProp = 'test'
        delete win.testProp
        // 如果上面没出错，说明window可以设置属性
        win.logInfo = logInfoFn
        win.logError = logErrorFn
        win.logDebug = logDebugFn
        win.browserFileMap = browserFileMapRef
      } catch (e) {
        // window可能是只读的proxy，跳过设置
        console.warn('window对象不可写，跳过全局变量设置')
      }
    }
  } catch (e) {
    console.warn('设置全局日志变量失败:', e)
  }
}

// 调用以启用全局日志
enableLogs(logInfo, logError, logDebug, browserFileMap)

// updateProgress调用计数
let updateProgressCallCount = 0

// 使用computed来格式化当前时间，确保响应式更新
const formattedCurrentPosition = computed(() => {
  const position = currentPosition.value
  const mins = Math.floor(position / 60)
  const secs = Math.floor(position % 60)
  const result = `${mins}:${secs.toString().padStart(2, '0')}`

  // 每5秒输出一次，确认computed被调用
  if (updateProgressCallCount % 25 === 0) {
    logInfo('formattedCurrentPosition computed被调用:', {
      position,
      mins,
      secs,
      result
    })
  }

  return result
})
const playbackMode = ref<'order' | 'random' | 'repeat'>('order')
const isMuted = ref(false)
const previousVolume = ref(80)
const volume = ref(80)
const isSeeking = ref(false) // 标记用户是否正在拖动进度条

// 随机播放时预先确定的下一首歌曲索引
const randomNextIndex = ref<number | null>(null)

// 歌单相关
const playlists = ref<Playlist[]>([])
const favorites = ref<string[]>([])

// 过滤和搜索
const currentFilter = ref<'all' | 'favorites' | 'artists' | 'albums' | 'cue' | 'playlist'>('all')
const searchQuery = ref('')
const selectedArtist = ref<string>('')
const selectedAlbum = ref<string>('')
const selectedPlaylistId = ref<string>('')

// 均衡器
const currentPreset = ref('flat')
const equalizerBands = ref<number[]>([0, 0, 0, 0, 0, 0, 0, 0, 0, 0])

// 计算属性
const currentFilterText = computed(() => {
  // 依赖语言和翻译状态，确保语言切换时重新计算
  i18nService.getCurrentLanguage()
  if (currentFilter.value === 'playlist') {
    return playlists.value.find(p => p.id === selectedPlaylistId.value)?.name || t('playlist.title')
  }
  const filters = {
    all: t('playlist.allSongs'),
    favorites: t('playlist.favorites'),
    artists: t('playlist.artists'),
    albums: t('playlist.albums'),
    cue: 'CUE专辑'
  } as const
  return filters[currentFilter.value as 'all' | 'favorites' | 'artists' | 'albums' | 'cue']
})

// 艺术家列表
const artistsList = computed(() => {
  const artists = new Map<string, { name: string; count: number }>()
  songs.value.forEach(song => {
    const artistName = getDisplayArtist(song)
    if (artists.has(artistName)) {
      artists.get(artistName)!.count++
    } else {
      artists.set(artistName, { name: artistName, count: 1 })
    }
  })
  return Array.from(artists.values()).sort((a, b) => a.name.localeCompare(b.name, 'zh-CN'))
})

// 专辑列表
const albumsList = computed(() => {
  const albums = new Map<string, { name: string; artist: string; count: number }>()
  songs.value.forEach(song => {
    const albumName = getDisplayAlbum(song)
    const artistName = getDisplayArtist(song)
    const key = `${albumName} - ${artistName}`
    if (albums.has(key)) {
      albums.get(key)!.count++
    } else {
      albums.set(key, { name: albumName, artist: artistName, count: 1 })
    }
  })
  return Array.from(albums.values()).sort((a, b) => a.name.localeCompare(b.name, 'zh-CN'))
})

const filteredSongs = computed(() => {
  let result = [...songs.value]
  
  // 应用过滤
  if (currentFilter.value === 'favorites') {
    result = result.filter(song => favorites.value.includes(song.id))
  } else if (currentFilter.value === 'artists' && selectedArtist.value) {
    result = result.filter(song => getDisplayArtist(song) === selectedArtist.value)
  } else if (currentFilter.value === 'albums' && selectedAlbum.value) {
    result = result.filter(song => {
      const albumName = getDisplayAlbum(song)
      const artistName = getDisplayArtist(song)
      return `${albumName} - ${artistName}` === selectedAlbum.value
    })
  } else if (currentFilter.value === 'playlist') {
    const playlist = playlists.value.find(p => p.id === selectedPlaylistId.value)
    if (playlist) {
      // 按歌单内保存的 id 顺序解析歌曲
      result = playlist.songs
        .map(id => songs.value.find(song => song.id === id))
        .filter((song): song is Song => !!song)
    } else {
      result = []
    }
  }
  
  // 应用搜索
  if (searchQuery.value) {
    const query = searchQuery.value.toLowerCase()
    result = result.filter(song => 
      song.title.toLowerCase().includes(query) ||
      song.artist.toLowerCase().includes(query) ||
      song.album.toLowerCase().includes(query)
    )
  }
  
  return result
})

const playbackModeImage = computed(() => {
  switch (playbackMode.value) {
    case 'order': return '/play-button_25b6-fe0f.png'
    case 'random': return '/shuffle-tracks-button_1f500.png'
    case 'repeat': return '/repeat-button_1f501.png'
    default: return '/play-button_25b6-fe0f.png'
  }
})

// 计算总时长
const totalDurationText = computed(() => {
  let totalSeconds = 0
  
  filteredSongs.value.forEach(song => {
    if (song.duration && song.duration !== '未知') {
      const parts = song.duration.split(':')
      if (parts.length === 2) {
        const minutes = parseInt(parts[0])
        const seconds = parseInt(parts[1])
        totalSeconds += minutes * 60 + seconds
      }
    }
  })
  
  // 转换为时分秒格式
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  
  if (hours > 0) {
    return `${hours}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`
  } else {
    return `${minutes}:${seconds.toString().padStart(2, '0')}`
  }
})

// 方法
const titleElement = ref<HTMLElement | null>(null)
const artistElement = ref<HTMLElement | null>(null)
const coverLyricsContainer = ref<HTMLElement | null>(null)
const coverLyricLineRefs = ref<(any | null)[]>([])
const songListContainer = ref<HTMLElement | null>(null)

// 滚动相关状态
const showScrollTopButton = ref(true) // 始终显示回到顶部按钮
const showJumpToCurrentButton = ref(true) // 始终显示跳到当前播放曲目的按钮

// 滚动事件处理函数
const handleScroll = () => {
  if (songListContainer.value) {
    // 尝试获取实际滚动容器的滚动位置
    let scrollTop = songListContainer.value.scrollTop
    const songList = songListContainer.value.querySelector('.song-list') as HTMLElement
    if (songList && songList.scrollTop > 0) {
      scrollTop = songList.scrollTop
    }
    
    // 控制回到顶部按钮的显示/隐藏
    // showScrollTopButton.value = scrollTop > 100
    logInfo('滚动事件触发: scrollTop=', scrollTop, 'showScrollTopButton=', showScrollTopButton.value)
    
    // 控制跳转到当前曲目按钮的显示/隐藏
    if (currentSong.value) {
      showJumpToCurrentButton.value = true
    } else {
      showJumpToCurrentButton.value = false
    }
  }
}

// 检测文本是否过长需要滚动
const isTextLong = (type: 'title' | 'artist'): boolean => {
  const element = type === 'title' ? titleElement.value : artistElement.value
  if (!element) return false
  
  // 检查文本是否超出容器宽度
  return element.scrollWidth > element.clientWidth
}

const toggleSidebar = () => {
  sidebarVisible.value = !sidebarVisible.value
}

const switchFilter = (filter: 'all' | 'favorites' | 'artists' | 'albums' | 'cue') => {
  currentFilter.value = filter
  // 重置选中的艺术家和专辑
  if (filter !== 'artists') {
    selectedArtist.value = ''
  }
  if (filter !== 'albums') {
    selectedAlbum.value = ''
  }
  if (filter !== 'cue') {
    selectedCueAlbum.value = null
  }
  selectedPlaylistId.value = ''

  // 更新悬浮按钮的显示状态
  nextTick(() => {
    handleScroll()
  })
}

// 进入指定用户歌单
const openPlaylist = (playlistId: string) => {
  selectedPlaylistId.value = playlistId
  currentFilter.value = 'playlist'
  selectedArtist.value = ''
  selectedAlbum.value = ''
  selectedCueAlbum.value = null

  nextTick(() => {
    handleScroll()
  })
}

// 播放CUE Track
const playCueTrackInApp = async (track: any) => {
  logDebug('playCueTrackInApp被调用，track参数:', track)
  logDebug('track类型:', typeof track)
  logDebug('track属性:', Object.keys(track))

  // 获取开始和结束时间（支持驼峰命名和蛇形命名）
  let startTime = track.startTime ?? track.start_time
  let endTime = track.endTime ?? track.end_time

  logDebug('获取到的时间参数:', { startTime, endTime })

  // 确保时间是数字类型
  if (typeof startTime === 'string') {
    startTime = parseInt(startTime, 10)
  }
  if (typeof endTime === 'string') {
    endTime = parseInt(endTime, 10)
  }

  // 如果仍然没有开始或结束时间，尝试从duration解析
  if ((!startTime && startTime !== 0) || (!endTime && endTime !== 0)) {
    logError('CUE track缺少时间参数:', track)
    logError('startTime:', startTime, 'endTime:', endTime)
    // 不设置模拟数据，而是报错
    alert('无法播放该音轨：缺少开始或结束时间参数')
    return
  }

  // 计算正确的时长（endTime - startTime）
  const durationSeconds = endTime - startTime
  const durationMins = Math.floor(durationSeconds / 60)
  const durationSecs = Math.floor(durationSeconds % 60)
  const durationStr = `${durationMins}:${durationSecs.toString().padStart(2, '0')}`

  // 将CUE Track转换为Song格式
  const song: Song = {
    id: track.id,
    title: track.title,
    artist: track.artist,
    album: track.album,
    path: track.path,
    duration: durationStr,
    cover: '',
    year: '',
    genre: '',
    lyric: '',
    isFavorite: false,
    isCueTrack: true,
    startTime: startTime,
    endTime: endTime,
    parentFile: track.parentFile || track.parent_file,
    trackNumber: String(track.trackNumber || track.track_number || ''),
    needs_transcode: false
  }

  logDebug('转换后的song:', song)
  logDebug('准备调用playSong，参数:', { song: song.title, position: startTime, cueStartTime: startTime, cueEndTime: endTime })
  await playSong(song, startTime, startTime, endTime)
}

const scanMusic = async () => {
  try {
    // 检测是否在Tauri环境中
    const tauri = isTauri()
    
    if (!tauri) {
      // 浏览器环境处理
      if (!musicDirectory.value) {
        alert('请先在设置中设置音乐目录')
        return
      }
      
      // 显示加载提示
      const loadingDiv = document.createElement('div')
      loadingDiv.id = 'loading-overlay'
      loadingDiv.innerHTML = '<div class="loading-spinner">正在扫描目录，请稍候...</div>'
      loadingDiv.style.cssText = `
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        background: rgba(0, 0, 0, 0.8);
        display: flex;
        justify-content: center;
        align-items: center;
        z-index: 9999;
      `
      document.body.appendChild(loadingDiv)
      
      try {
        // 在浏览器中，使用File API扫描音乐文件
        const audioFiles: Song[] = []

        // 创建文件选择器，允许选择多个文件
        const input = document.createElement('input')
        input.type = 'file'
        input.webkitdirectory = true
        input.multiple = true
        
        input.onchange = async (event) => {
          try {
            const target = event.target as HTMLInputElement
            if (target.files && target.files.length > 0) {
              const files = Array.from(target.files)
              
              // 过滤出音频文件（浏览器环境可播放的格式）
              const audioFileList = files.filter(file => isBrowserScannableAudio(file.name))
              
              logInfo(`找到 ${audioFileList.length} 个音频文件`)
              
              // 处理每个音频文件
              for (const file of audioFileList) {
                try {
                  // 创建blob URL并存储，播放时直接使用
                  const objectURL = URL.createObjectURL(file)
                  
                  // 从文件名解析艺术家和标题
                  // 支持格式: "艺术家-标题.mp3" 或 "艺术家 - 标题.mp3"
                  const fileNameWithoutExt = file.name.replace(/\.[^/.]+$/, '')
                  let title = fileNameWithoutExt
                  let artist = '未知艺术家'
                  let album = '未知专辑'
                  
                  // 尝试从文件名解析艺术家和标题
                  // 匹配格式: "艺术家-标题" 或 "艺术家 - 标题"
                  const match = fileNameWithoutExt.match(/^(.+?)[\s]*-[\s]*(.+)$/)
                  if (match) {
                    artist = match[1].trim()
                    title = match[2].trim()
                    logInfo(`从文件名解析: 艺术家="${artist}", 标题="${title}"`)
                  } else {
                    logInfo(`无法从文件名解析艺术家，使用文件名作为标题: "${title}"`)
                  }
                  
                  const song: Song = {
                    id: `browser_${Date.now()}_${Math.random().toString(36).substring(2, 11)}`,
                    title: title,
                    artist: artist,
                    album: album,
                    duration: '未知', // 时长在播放时获取
                    path: objectURL, // 使用blob URL作为路径
                    cover: '',
                    year: '',
                    genre: '',
                    lyric: '',
                    isCueTrack: false,
                    needs_transcode: false
                  }
                  
                  // 将File对象存储到Map中
                  browserFileMap.set(song.id, file)
                  
                  audioFiles.push(song)
                  logInfo(`已添加文件: ${file.name}, blob URL: ${objectURL}`)
                  logInfo(`文件大小: ${file.size}, 文件类型: ${file.type}`)
                } catch (fileError) {
                  logError(`处理文件 ${file.name} 时出错:`, fileError)
                }
              }
              
              // 添加扫描到的歌曲到播放列表
              if (audioFiles.length > 0) {
                songs.value = [...songs.value, ...audioFiles]
                logInfo(`已添加 ${audioFiles.length} 首歌曲到播放列表`)
                alert(`成功扫描到 ${audioFiles.length} 首歌曲`)
              } else {
                alert('未找到音频文件')
              }
            }
          } catch (error) {
            logError('处理文件时出错:', error)
            alert(`扫描失败：${error}`)
          } finally {
            // 移除加载提示
            if (document.getElementById('loading-overlay')) {
              document.body.removeChild(document.getElementById('loading-overlay')!)
            }
          }
        }
        
        input.click()
        
        return
      } catch (error) {
        logError('扫描音乐失败:', error)
        alert(`扫描失败：${error}`)
        // 移除加载提示
        if (document.getElementById('loading-overlay')) {
          document.body.removeChild(document.getElementById('loading-overlay')!)
        }
      }
      return
    }
    
    // 打开目录选择对话框
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择音乐目录'
    })
    
    if (selected) {
      const directory = typeof selected === 'string' ? selected : selected[0]
      
      // 显示加载提示
      const loadingDiv = document.createElement('div')
      loadingDiv.id = 'loading-overlay'
      loadingDiv.innerHTML = '<div class="loading-spinner">正在扫描目录，请稍候...</div>'
      loadingDiv.style.cssText = `
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        background: rgba(0, 0, 0, 0.8);
        display: flex;
        justify-content: center;
        align-items: center;
        z-index: 9999;
      `
      document.body.appendChild(loadingDiv)
      
      try {
        // 调用后端扫描命令
        const result = await invoke<{ tracks: Song[] }>('scan_directory', { directory })
        
        // 同时扫描CUE文件
        logInfo('开始扫描CUE文件...')
        await scanCueFiles(directory)
        
        if (result && result.tracks) {
          const trackCount = result.tracks.length
          const cueTrackCount = cueTracks.value.length
          
          if (trackCount > 0 || cueTrackCount > 0) {
            // 更新收藏状态
            result.tracks.forEach(track => {
              track.isFavorite = favorites.value.includes(track.id)
            })
            
            // 检查第一首歌的歌词和封面数据
            if (result.tracks.length > 0) {
              const firstTrack = result.tracks[0]
              logInfo('【扫描结果】第一首歌:', firstTrack.title)
              logInfo('【扫描结果】歌词字段存在:', 'lyric' in firstTrack)
              logInfo('【扫描结果】歌词长度:', firstTrack.lyric ? firstTrack.lyric.length : 0)
              logInfo('【扫描结果】歌词内容预览:', firstTrack.lyric ? firstTrack.lyric.substring(0, 100) : '空')
              logInfo('【扫描结果】封面字段存在:', 'cover' in firstTrack)
              logInfo('【扫描结果】封面长度:', firstTrack.cover ? firstTrack.cover.length : 0)
              logInfo('【扫描结果】完整track对象:', JSON.stringify(firstTrack, null, 2).substring(0, 500))
            }
            
            // 合并普通歌曲和CUE tracks
            // 创建CUE关联文件路径集合，用于过滤
            const cueParentFiles = new Set(cueTracks.value.map(track => track.parentFile))

            const cueSongs = cueTracks.value.map(track => ({
              id: track.id,
              title: track.title, // 保留完整的title(包含时间信息)
              artist: track.artist,
              album: track.album,
              path: track.path,
              duration: track.duration,
              cover: '',
              year: '',
              genre: '',
              lyric: '',
              isCueTrack: true,
              startTime: typeof track.startTime === 'string' ? parseFloat(track.startTime) : track.startTime,
              endTime: track.endTime ? (typeof track.endTime === 'string' ? parseFloat(track.endTime) : track.endTime) : undefined,
              parentFile: track.parentFile,
              trackNumber: String(track.trackNumber || ''),
              cueInfo: track.cueInfo,
              isFavorite: favorites.value.includes(track.id),
              needs_transcode: false
            } as Song))

            // 过滤掉CUE关联的音频文件
            const filteredTracks = result.tracks.filter((track: any) => !cueParentFiles.has(track.path)) as Song[]
            songs.value = [...filteredTracks, ...cueSongs]
            
            if (cueTrackCount > 0) {
              alert(`扫描完成，共找到 ${trackCount} 首歌曲和 ${cueTrackCount} 个CUE Track`)
            } else {
              alert(`扫描完成，共找到 ${trackCount} 首歌曲`)
            }
          } else {
            alert('未找到音频文件，请确认目录中包含支持的音频格式')
          }
        } else {
          alert('扫描失败：未返回有效数据')
        }
      } finally {
        // 移除加载提示
        document.getElementById('loading-overlay')?.remove()
      }
    }
  } catch (error) {
    logError('扫描目录失败:', error)
    alert(`扫描失败：${error}`)
  }
}

// 记录前端计算的播放位置
let frontendPosition = 0

// 播放状态管理
let playSongLock: Promise<void> | null = null
let currentPlayId = 0 // 用于跟踪当前播放请求的唯一ID

const playSong = async (song: Song, position: number = 0, cueStartTime?: number, cueEndTime?: number, autoPlay: boolean = true) => {
  // 生成本次播放请求的唯一ID
  const thisPlayId = ++currentPlayId
  logInfo(`[播放保护] 开始播放请求, ID: ${thisPlayId}, 歌曲: ${song.title}, autoPlay: ${autoPlay}`)
  
  let errorMessage = ''
  
  // 取消之前的播放操作，直接响应用户的最新操作
  if (playSongLock) {
    logInfo(`[播放保护] 检测到正在进行的播放操作，取消并直接处理最新请求`)
    // 重置锁定状态，允许新的播放请求立即执行
    playSongLock = null
  }
  
  // 再次检查，确保没有其他请求在此期间进入
  if (thisPlayId !== currentPlayId) {
    logInfo(`[播放保护] 播放请求 ${thisPlayId}: 获取锁之前发现有更新的请求 ${currentPlayId}，跳过`)
    return
  }
  
  // 创建新的锁定Promise
  let resolveLock: (() => void) | null = null
  playSongLock = new Promise<void>((resolve) => {
    resolveLock = resolve
  })

  // 以下变量需要在 try 对应的 catch 回退分支中访问，
  // 而 let/const 是块级作用域，在 try 内声明的变量在 catch 中不可见，
  // 因此统一在 try 外预先声明（修复 catch 分支 ReferenceError）
  let currentIndex = -1
  let positionForCue = position
  let playPath = ''
  let needsFFplay = false

  try {
    
    // 重置预转码标志
    hasPretranscodedNextSong = false
    logInfo('[预转码] 重置预转码标志，准备播放新歌曲')
    
    // 清除之前的播放完成检测定时器
    if (playbackTimerId !== null) {
      clearTimeout(playbackTimerId)
      logInfo('前端 清除之前的播放完成检测定时器')
      playbackTimerId = null
    }
    
    // 重置播放完成标志
    isPlaybackFinished = false
    
    logInfo('开始播放歌曲:', song.title, '路径:', song.path)
    logInfo('歌曲封面:', song.cover ? '有封面' : '无封面', '封面长度:', song.cover ? song.cover.length : 0)
    
    // 动态读取封面（如果歌曲对象中没有封面）
    if (!song.cover || song.cover.length === 0) {
      logInfo('【封面加载】歌曲对象中没有封面，尝试动态读取')
      logInfo('【封面加载】歌曲路径:', song.path)
      try {
        const { readFile } = await import('@tauri-apps/plugin-fs')
        const songPath = song.path
        const coverExtensions = ['jpg', 'jpeg', 'png', 'bmp', 'webp']
        
        // 获取歌曲所在目录和文件名（不含扩展名）
        const lastSlashIndex = Math.max(songPath.lastIndexOf('/'), songPath.lastIndexOf('\\'))
        const songDir = songPath.substring(0, lastSlashIndex + 1)
        const songFileName = songPath.substring(lastSlashIndex + 1)
        const songNameWithoutExt = songFileName.replace(/\.[^/.]+$/, '')
        
        logInfo('【封面加载】歌曲目录:', songDir)
        logInfo('【封面加载】歌曲文件名:', songFileName)
        logInfo('【封面加载】歌曲名(无扩展名):', songNameWithoutExt)
        
        for (const ext of coverExtensions) {
          const coverPath = songDir + songNameWithoutExt + '.' + ext
          logInfo('【封面加载】尝试读取:', coverPath)
          try {
            // 使用 decodeURIComponent 处理可能的中文编码问题
            const decodedPath = decodeURIComponent(coverPath)
            logInfo('【封面加载】解码后路径:', decodedPath)
            const imageData = await readFile(decodedPath)
            logInfo('【封面加载】文件存在，大小:', imageData.length)
            if (imageData && imageData.length > 0) {
              // 使用更安全的方式转换为base64，避免栈溢出
              const bytes = new Uint8Array(imageData)
              let binary = ''
              const len = bytes.byteLength
              for (let i = 0; i < len; i++) {
                binary += String.fromCharCode(bytes[i])
              }
              const base64Image = btoa(binary)
              const mimeType = ext === 'png' ? 'image/png' : 
                              ext === 'jpg' || ext === 'jpeg' ? 'image/jpeg' :
                              ext === 'bmp' ? 'image/bmp' :
                              ext === 'webp' ? 'image/webp' : 'image/jpeg'
              song.cover = `data:${mimeType};base64,${base64Image}`
              logInfo('【封面加载】成功读取封面:', coverPath, '大小:', imageData.length)
              break
            }
          } catch (e) {
            logInfo('【封面加载】文件不存在或读取失败:', coverPath, '错误:', e)
          }
        }
        
        // 如果没有找到同名封面，尝试常见封面文件名
        if (!song.cover) {
          logInfo('【封面加载】未找到同名封面，尝试常见封面文件名')
          const commonNames = ['cover', 'folder', 'album', 'front']
          for (const name of commonNames) {
            for (const ext of coverExtensions) {
              const coverPath = songDir + name + '.' + ext
              logInfo('【封面加载】尝试读取常见封面:', coverPath)
              try {
                const imageData = await readFile(coverPath)
                if (imageData && imageData.length > 0) {
                  // 使用更安全的方式转换为base64，避免栈溢出
                  const bytes = new Uint8Array(imageData)
                  let binary = ''
                  const len = bytes.byteLength
                  for (let i = 0; i < len; i++) {
                    binary += String.fromCharCode(bytes[i])
                  }
                  const base64Image = btoa(binary)
                  const mimeType = ext === 'png' ? 'image/png' : 
                                  ext === 'jpg' || ext === 'jpeg' ? 'image/jpeg' :
                                  ext === 'bmp' ? 'image/bmp' :
                                  ext === 'webp' ? 'image/webp' : 'image/jpeg'
                  song.cover = `data:${mimeType};base64,${base64Image}`
                  logInfo('【封面加载】成功读取常见封面:', coverPath)
                  break
                }
              } catch (e) {
                // 文件不存在
              }
            }
            if (song.cover) break
          }
        }
      } catch (error) {
        logError('【封面加载】动态读取封面失败:', error)
      }
    } else {
      logInfo('【封面加载】歌曲对象已有封面，长度:', song.cover.length)
    }
    
    // 获取当前歌曲索引（用于随机模式预先确定下一首）
    // 声明已提升到 try 外，供 catch 回退分支使用
    currentIndex = songs.value.findIndex(s => s.id === song.id)
    
    // 交叉淡入淡出处理
    let originalVolume = isMuted.value ? previousVolume.value : volume.value
    let volumeToPass = originalVolume // 传递给后端的音量值，始终使用原始音量

    logInfo('音量设置前: volume.value=', volume.value, 'isMuted.value=', isMuted.value, 'previousVolume.value=', previousVolume.value, 'originalVolume=', originalVolume, 'volumeToPass=', volumeToPass)

    if (crossfadeEnabled.value && crossfadeDuration.value > 0) {
      logInfo('启用交叉淡入淡出，时长:', crossfadeDuration.value, '秒')
      logInfo('交叉淡入淡出前的原始音量:', originalVolume, '当前音量:', volume.value, '静音状态:', isMuted.value)
      // 注意：不再修改 volume.value，而是在音频元素创建后直接操作其音量
      // 这样用户在 UI 上看到的音量值就不会受到交叉淡入淡出的影响
      logInfo('交叉淡入淡出: 准备在音频元素创建后直接操作其音量')
    } else {
      // 没有启用交叉淡入淡出，直接使用原始音量
      volumeToPass = originalVolume
      logInfo('未启用交叉淡入淡出: volumeToPass=', volumeToPass)
    }

    logInfo('音量设置后: volume.value=', volume.value, 'volumeToPass=', volumeToPass)

    // 交叉淡入淡出：对当前播放的歌曲执行淡出操作
    if (crossfadeEnabled.value && crossfadeDuration.value > 0 && audioElement.value && !audioElement.value.paused) {
      const fadeDuration = crossfadeDuration.value * 1000 // 转换为毫秒
      const steps = 20 // 淡出步骤数
      const stepDuration = fadeDuration / steps
      
      logInfo('开始交叉淡入淡出，对当前歌曲执行淡出操作')
      
      // 逐渐减少音频元素的音量
      for (let i = steps; i >= 0; i--) {
        await new Promise(resolve => setTimeout(resolve, stepDuration))
        const currentVolume = (originalVolume * i) / (steps * 100)
        if (audioElement.value) {
          audioElement.value.volume = currentVolume
        }
      }
      
      logInfo('淡出操作完成，准备播放新歌曲')
    }
    
    // 清理旧的音频元素，避免竞态条件
    if (audioElement.value && timeupdateHandler) {
      try {
        audioElement.value.pause()
        audioElement.value.removeEventListener('timeupdate', timeupdateHandler)
        // 清理其他事件监听器
        audioElement.value.oncanplay = null
        audioElement.value.onerror = null
        audioElement.value.onended = null
      } catch (error) {
        logError('清理音频元素事件监听器失败:', error)
      } finally {
        audioElement.value = null
        timeupdateHandler = null
      }
    }
    
    // 重置前端状态
    currentSong.value = song
    
    // 更新悬浮按钮的显示状态
    nextTick(() => {
      handleScroll()
    })
    // 对于CUE track，计算相对位置（positionForCue 已在 try 外初始化）
    if (song.isCueTrack && song.startTime) {
      // 首先验证position是否合理
      if (isNaN(position) || position < 0 || position > 1000000) {
        logError('CUE track位置转换: 无效的position值:', position, '使用0作为默认值')
        positionForCue = 0
      } else {
        // 检查position是否已经是相对位置（小于startTime）
        if (position < Number(song.startTime)) {
          // 已经是相对位置，直接使用
          logDebug('CUE track位置转换: position已为相对位置=' + position + 's')
        } else {
          // 是绝对位置，转换为相对位置
          positionForCue = position - Number(song.startTime)
          // 确保相对位置不小于0
          if (positionForCue < 0) {
            positionForCue = 0
            logDebug('CUE track位置修正: 相对位置小于0，设置为0')
          }
          // 确保相对位置不超过CUE track的长度
          if (song.endTime) {
            const cueTrackDuration = Number(song.endTime) - Number(song.startTime)
            if (positionForCue > cueTrackDuration) {
              positionForCue = cueTrackDuration
              logDebug('CUE track位置修正: 相对位置超过音轨长度，限制为', cueTrackDuration, '秒')
            }
          }
          logDebug('CUE track位置转换: 绝对位置=' + position + 's, startTime=' + song.startTime + 's, 相对位置=' + positionForCue + 's')
        }
      }
    } else {
      // 对于普通歌曲，验证position是否合理
      if (isNaN(position) || position < 0 || position > 1000000) {
        logError('普通歌曲位置转换: 无效的position值:', position, '使用0作为默认值')
        positionForCue = 0
      }
    }
    currentPosition.value = positionForCue
    // 计算进度百分比
    if (song.duration && song.duration !== '未知') {
      const parts = song.duration.split(':')
      if (parts.length === 2) {
        const minutes = parseInt(parts[0])
        const seconds = parseInt(parts[1])
        const totalSeconds = minutes * 60 + seconds
        if (totalSeconds > 0) {
          progress.value = Math.min((positionForCue / totalSeconds) * 100, 100)
        } else {
          progress.value = 0
        }
      } else {
        progress.value = 0
      }
    } else {
      progress.value = 0
    }
    // 暂时不设置isPlaying.value，等待音频元素真正开始播放后再设置
    
    // 重置播放完成标志
    isPlaybackFinished = false
    
    // 重置暂停相关状态
    pauseStartTime.value = null
    pausedDuration.value = 0
    logInfo('重置暂停状态: pauseStartTime=null, pausedDuration=0')

    // 准备CUE参数（如果是CUE track）
    let startTime = cueStartTime !== undefined ? cueStartTime : (song.isCueTrack ? song.startTime : undefined)
    let endTime = cueEndTime !== undefined ? cueEndTime : (song.isCueTrack ? song.endTime : undefined)
    playPath = song.isCueTrack && song.parentFile ? song.parentFile : song.path
    
    // 检查playPath是否有效
    logDebug('计算playPath:', {
      isCueTrack: song.isCueTrack,
      parentFile: song.parentFile,
      path: song.path,
      playPath: playPath,
      song: song
    })
    
    if (!playPath || playPath.length > 1000) {
      logError('无效的playPath:', playPath)
      throw new Error('无效的文件路径')
    }
    
    // 检查playPath是否是绝对路径
    if (!playPath.includes(':')) {
      logInfo('playPath可能是相对路径:', playPath)
    }

    // 检查文件路径格式
    if (playPath.includes('\\') && playPath.includes('/')) {
      logInfo('文件路径包含混合分隔符，可能导致问题:', playPath)
      // 统一使用Windows风格的分隔符
      const normalizedPath = playPath.replace(/\//g, '\\')
      logDebug('标准化后的路径:', normalizedPath)
    }

    // 修复：无论新歌是否需要FFplay，都先停止之前的FFplay播放
    // 这确保从FFplay格式切换到普通格式时，FFplay能正确停止
    if (isFFplayPlaying.value) {
      try {
        logInfo('【FFplay】停止之前的FFplay播放')
        await invoke('stop_ffplay')
        isFFplayPlaying.value = false
        logInfo('【FFplay】FFplay已停止')
      } catch (error) {
        logError('停止FFplay播放失败:', error)
      }
    }

    // 检查是否需要使用FFplay播放（原引擎不支持的无损音频）
    // 注意：.m4a和.aac实际上可以被HTML5 Audio支持，不需要FFplay
    needsFFplay = needsFFplayEngine(playPath)
    
    if (needsFFplay && !isBrowser.value) {
      logInfo('检测到需要FFplay播放的格式:', playPath)
      
      // 停止当前的rodio播放
      if (audioElement.value) {
        try {
          audioElement.value.pause()
          if (timeupdateHandler) {
            audioElement.value.removeEventListener('timeupdate', timeupdateHandler)
          }
          audioElement.value.oncanplay = null
          audioElement.value.onerror = null
          audioElement.value.onended = null
          audioElement.value.src = ''
          audioElement.value.load()
          audioElement.value = null
          timeupdateHandler = null
        } catch (error) {
          logError('清理音频元素失败:', error)
        }
      }
      
      // 清除之前的播放完成检测定时器
      if (playbackTimerId !== null) {
        clearTimeout(playbackTimerId)
        playbackTimerId = null
      }
      
      // 停止FFplay状态监控
      if (ffplayStatusInterval !== null) {
        clearInterval(ffplayStatusInterval)
        ffplayStatusInterval = null
        logInfo('已清除FFplay状态监控定时器')
      }
      
      try {
        logInfo('【FFmpeg】开始调用 play_audio_with_ffmpeg')
        // 使用FFmpeg播放
        const start_position = positionForCue
        logInfo('【FFmpeg】start_position 设置为:', start_position)
        let result
        logInfo('【FFmpeg】开始执行 invoke 调用')
        try {
          logInfo('【FFmpeg】准备调用 invoke，path:', playPath, 'start_time:', start_position)
          // 解析歌曲时长（如果有的话）
          let durationSeconds = 300; // 默认值
          if (currentSong.value && currentSong.value.duration) {
            const durationStr = currentSong.value.duration;
            const parts = durationStr.split(':');
            if (parts.length === 2) {
              const minutes = parseInt(parts[0]);
              const seconds = parseInt(parts[1]);
              durationSeconds = minutes * 60 + seconds;
            }
          }
          
          result = await invoke('play_audio_with_ffmpeg', {
            path: playPath,
            start_time: start_position,
            duration: durationSeconds
          }) as FFplayResult | string
          logInfo('【FFmpeg】invoke 调用成功，result:', JSON.stringify(result))
          logInfo('result类型:', typeof result)
          if (typeof result === 'object' && result !== null) {
            logInfo('result.duration:', (result as FFplayResult).duration)
          }
        } catch (invokeError) {
          logError('FFplay播放失败:', invokeError)
          const errorMsg = String(invokeError)
          if (errorMsg.includes('未找到') || errorMsg.includes('not found') || errorMsg.includes('FFplay')) {
            errorMessage = 'FFplay 可执行文件未找到，无法播放此格式。请下载 FFmpeg 并放置到 bin 目录或添加到系统 PATH。'
          } else {
            errorMessage = `播放失败：${errorMsg}`
          }
          return
        }

        // 检查是否返回错误（ffplay未找到）
        if (result && typeof result === 'string' && result.includes('未找到')) {
          logError('FFplay未找到，无法播放此格式:', result)
          errorMessage = 'FFplay 未找到，无法播放此格式'
          return
        }
        
        // 获取音频时长和详细信息
        try {
          const ffResult = result as FFplayResult
          if (ffResult && ffResult.duration !== undefined) {
            ffplayDuration.value = ffResult.duration
            ffplayPosition.value = start_position
            
            // 更新歌曲时长
            const totalSeconds = Math.round(ffResult.duration)
            const minutes = Math.floor(totalSeconds / 60)
            const seconds = totalSeconds % 60
            song.duration = `${minutes}:${seconds.toString().padStart(2, '0')}`
            
            // 更新音频文件详细信息
            if (ffResult.format) {
              song.format = ffResult.format
            }
            if (ffResult.sample_rate) {
              song.sample_rate = ffResult.sample_rate
            }
            if (ffResult.channels) {
              song.channels = ffResult.channels
            }
            if (ffResult.bit_rate) {
              song.bit_rate = ffResult.bit_rate
            }
            if (ffResult.bit_depth) {
              song.bit_depth = ffResult.bit_depth
            }
            
            // 计算进度百分比
            if (totalSeconds > 0) {
              progress.value = Math.min((start_position / totalSeconds) * 100, 100)
            }
            
            logInfo('音频文件信息:', {
              format: ffResult.format,
              sample_rate: ffResult.sample_rate,
              channels: ffResult.channels,
              bit_rate: ffResult.bit_rate,
              bit_depth: ffResult.bit_depth
            })
          }
        } catch (error) {
          logError('获取音频时长失败:', error)
        }
        
        // 更新播放状态
        currentSong.value = song
        isPlaying.value = true
        isFFplayPlaying.value = true
        currentPosition.value = start_position
        
        // 启动FFplay状态监控
        logInfo('启动FFplay状态监控定时器')
        // 清除可能存在的旧定时器
        if (ffplayStatusInterval) {
          clearInterval(ffplayStatusInterval)
          ffplayStatusInterval = null
          logInfo('已清除旧的FFplay状态监控定时器')
        }
        
        // 重置进度变量，避免累计上一首歌曲的进度
        logInfo('【FFplay】重置进度变量，start_position:', start_position)
        currentPosition.value = start_position
        progress.value = 0
        frontendPosition = start_position

        // 确保isFFplayPlaying.value为true
        isFFplayPlaying.value = true
        logInfo('设置isFFplayPlaying.value为true，当前值:', isFFplayPlaying.value);

        // 立即执行一次状态更新，确保前端能够立即获取到FFplay的状态
        (async () => {
          try {
            logInfo('立即执行FFplay状态更新')
            const status = await invoke('get_ffplay_status') as any
            logInfo('立即获取FFplay状态成功:', JSON.stringify(status))

            if (status) {
              logInfo('立即处理FFplay状态:', {
                duration: status.duration,
                position: status.position,
                volume: status.volume,
                is_playing: status.is_playing
              })

              ffplayDuration.value = status.duration || ffplayDuration.value
              ffplayPosition.value = status.position || ffplayPosition.value
              ffplayVolume.value = status.volume || ffplayVolume.value

              // 更新播放状态
              isPlaying.value = status.is_playing || false
              logInfo('isPlaying 立即更新为:', isPlaying.value, 'isFFplayPlaying:', isFFplayPlaying.value)

              // 只有当status.position有效时才更新currentPosition
              if (status.position !== undefined && status.position !== null) {
                logInfo('立即更新currentPosition前:', currentPosition.value, '更新后:', status.position)
                currentPosition.value = status.position
                // 更新前端计算的播放位置
                frontendPosition = status.position
                logInfo('立即更新播放进度:', currentPosition.value, '秒, 时长:', ffplayDuration.value)
              }

              // 计算进度百分比
              if (ffplayDuration.value > 0) {
                const newProgress = Math.min((currentPosition.value / ffplayDuration.value) * 100, 100)
                logInfo('立即更新进度百分比前:', progress.value, '更新后:', newProgress)
                progress.value = newProgress
                logInfo('立即更新进度百分比:', progress.value, '%')
              }
            }
          } catch (error) {
            logError('立即获取FFplay状态失败:', error)
          }
        })()
        
        ffplayStatusInterval = window.setInterval(() => {
          // 检查是否应该继续运行定时器
          if (!isFFplayPlaying.value) {
            logInfo('isFFplayPlaying为false，停止监控定时器')
            if (ffplayStatusInterval) {
              clearInterval(ffplayStatusInterval)
              ffplayStatusInterval = null
            }
            return
          }

          logInfo('FFplay状态监控定时器触发');
          // 使用IIFE包装async函数
          (async () => {
            try {
              logInfo('准备调用get_ffplay_status')
              const status = await invoke('get_ffplay_status') as any
              logInfo('获取FFplay状态成功:', JSON.stringify(status))

              if (status) {
                logInfo('处理FFplay状态:', {
                  duration: status.duration,
                  position: status.position,
                  volume: status.volume,
                  is_playing: status.is_playing
                })

                ffplayDuration.value = status.duration || ffplayDuration.value
                ffplayPosition.value = status.position || ffplayPosition.value
                ffplayVolume.value = status.volume || ffplayVolume.value

                // 更新播放状态
                isPlaying.value = status.is_playing || false
                logInfo('isPlaying 更新为:', isPlaying.value, 'isFFplayPlaying:', isFFplayPlaying.value)

                // 只有当status.position有效且Seek未在进行中、用户未在拖动时才更新currentPosition
                // 防止状态轮询覆盖拖动和Seek后的位置
                if (status.position !== undefined && status.position !== null && !seekInProgress && !isSeeking.value) {
                  logInfo('更新currentPosition前:', currentPosition.value, '更新后:', status.position)
                  currentPosition.value = status.position
                  // 更新前端计算的播放位置
                  frontendPosition = status.position
                  logInfo('更新播放进度:', currentPosition.value, '秒, 时长:', ffplayDuration.value)
                } else if (seekInProgress || isSeeking.value) {
                  logInfo('Seek进行中或正在拖动，跳过位置更新:', seekInProgress, isSeeking.value)
                }
                
                // 计算进度百分比（Seek进行中时也更新，因为currentPosition已被保护）
                if (ffplayDuration.value > 0) {
                  const newProgress = Math.min((currentPosition.value / ffplayDuration.value) * 100, 100)
                  logInfo('更新进度百分比前:', progress.value, '更新后:', newProgress)
                  progress.value = newProgress
                  logInfo('更新进度百分比:', progress.value, '%')
                }
              
                // 检查播放是否完成
                // 只有当满足以下所有条件时才认为播放完成：
                // 1. is_playing 为 false
                // 2. duration > 0 (确保已获取到有效时长)
                // 3. position >= duration - 1 (播放到接近结尾，允许1秒误差)
                // 4. ffplayDuration.value > 0 (确保前端也获取到了时长)
                // 5. isFFplayPlaying.value 为 true (确保当前确实在使用 ffplay)
                const isPlaybackComplete =
                  status.is_playing === false &&
                  status.duration > 0 &&
                  status.position >= status.duration - 1 &&
                  ffplayDuration.value > 0 &&
                  isFFplayPlaying.value

                logInfo('播放完成检测: is_playing=', status.is_playing, 'position=', status.position.toFixed(2), 'duration=', status.duration.toFixed(2), 'ffplayDuration=', ffplayDuration.value, 'isFFplayPlaying=', isFFplayPlaying.value, 'isPlaybackComplete=', isPlaybackComplete)

                if (isPlaybackComplete) {
                  logInfo('FFplay播放完成, position:', status.position, 'duration:', status.duration)
                  isPlaybackFinished = true

                  // 停止状态监控定时器
                  if (ffplayStatusInterval) {
                    clearInterval(ffplayStatusInterval)
                    ffplayStatusInterval = null
                  }

                  // 重置FFplay相关状态
                  isFFplayPlaying.value = false

                  if (autoPlayNext.value) {
                    // 延迟执行 playNext()，确保当前播放状态已经完全更新
                    logInfo('准备播放下一首歌曲');
                    setTimeout(() => {
                      logInfo('执行playNext()');
                      playNext();
                    }, 1000)
                  } else if (autoPlay) {
                    isPlaying.value = false
                    logInfo('播放完成，停止播放');
                  }
                }


              }
            } catch (error) {
              logError('获取FFplay状态失败:', error)
              // 即使在获取状态失败的情况下，也保持isFFplayPlaying.value为true
              // 确保即使在状态获取失败的情况下，前端也能正确检测到FFplay播放状态
              isFFplayPlaying.value = true
              logInfo('获取FFplay状态失败，确保isFFplayPlaying.value为true，当前值:', isFFplayPlaying.value)
            }
          })()
        }, 500) // 每500毫秒更新一次状态，提高响应速度
        
        // 添加前端进度更新定时器，作为备份
        console.log('【FFplay】启动前端进度更新定时器')
        logInfo('启动前端进度更新定时器')
        if (progressTimer) {
          clearInterval(progressTimer)
          console.log('【FFplay】已清除旧的前端进度更新定时器')
          logInfo('已清除旧的前端进度更新定时器')
        }
        
        progressTimer = window.setInterval(() => {
          // 检查是否正在使用FFplay播放
          if (isFFplayPlaying.value && isPlaying.value && ffplayDuration.value > 0) {
            // 无论是否收到后端状态更新，都使用前端计算的播放位置
            // 每次+300毫秒更新一次，直至曲目播放结束
            frontendPosition += 0.3 // 每300毫秒增加0.3秒
            if (frontendPosition < ffplayDuration.value) {
              currentPosition.value = frontendPosition
              progress.value = Math.min((frontendPosition / ffplayDuration.value) * 100, 100)
              console.log('【FFplay】前端进度更新(备份): currentPosition=', frontendPosition, '秒, progress=', progress.value, '%')
              logInfo('前端进度更新(备份): currentPosition=', frontendPosition, '秒, progress=', progress.value, '%')
              // 同步歌词显示
              syncLyrics()
            } else {
              // 曲目播放结束
              currentPosition.value = ffplayDuration.value
              progress.value = 100
              console.log('【FFplay】前端进度更新(备份): 曲目播放结束, currentPosition=', frontendPosition, '秒, progress=', progress.value, '%')
              logInfo('前端进度更新(备份): 曲目播放结束, currentPosition=', frontendPosition, '秒, progress=', progress.value, '%')
              // 同步歌词显示（显示最后一句）
              syncLyrics()
            }
          } else {
            // 播放暂停或停止，重置前端计算的播放位置
            frontendPosition = currentPosition.value
          }
        }, 300) // 每 300 毫秒更新一次，符合用户要求
        
        // 歌词加载优先级策略
        logInfo('[歌词加载] 开始加载歌词，song.id:', song.id)
        let lyricContent = ''
        let lyricSource = ''
        const songForLyric = songs.value.find(s => s.id === song.id) || song
        logInfo('[歌词加载] songForLyric 找到:', songForLyric ? songForLyric.title : '未找到')
        logInfo('[歌词加载] songForLyric.id:', songForLyric?.id)
        logInfo('[歌词加载] songForLyric.path:', songForLyric?.path)
        logInfo('[歌词加载] song.lyric:', song?.lyric ? '有值(' + song.lyric.length + ')' : '为空')
        
        // 优先级 1: 音频文件内嵌歌词（来自后端扫描）
        const embeddedLyric = songForLyric?.lyric || song.lyric || ''
        if (embeddedLyric && embeddedLyric.trim()) {
          lyricContent = embeddedLyric
          lyricSource = '内嵌歌词'
          logInfo('[歌词加载] 优先级 1: 使用内嵌歌词，长度:', lyricContent.length)
        } else {
          logInfo('[歌词加载] 优先级 1: 无内嵌歌词')
        }
        
        // 优先级 2: 同名 .lrc 文件（必须非空）
        if (!lyricContent && songForLyric && songForLyric.path) {
          try {
            const { readTextFile } = await import('@tauri-apps/plugin-fs')
            const lyricPath = songForLyric.path.replace(/\.[^/.]+$/, '.lrc')
            logInfo('[歌词加载] 优先级 2: 尝试读取.lrc文件:', lyricPath)
            const content = await readTextFile(lyricPath)
            if (content && content.trim()) {
              lyricContent = content
              lyricSource = '同名.lrc文件'
              logInfo('[歌词加载] 优先级 2: 成功读取.lrc文件，长度:', lyricContent.length)
            } else {
              logInfo('[歌词加载] 优先级 2: .lrc文件存在但内容为空，跳过')
            }
          } catch (e) {
            logInfo('[歌词加载] 优先级 2: 读取.lrc文件失败:', e)
          }
        } else if (!lyricContent) {
          logInfo('[歌词加载] 优先级 2: 跳过，songForLyric 或 path 不存在')
        }
        
        // 优先级 3: 在线匹配缓存歌词
        if (!lyricContent) {
          try {
            const cachedLyric = await localStorageService.getCachedLyric(song.id)
            if (cachedLyric && cachedLyric.trim()) {
              lyricContent = cachedLyric
              lyricSource = '在线匹配缓存'
              logInfo('[歌词加载] 优先级 3: 使用在线匹配缓存歌词，长度:', lyricContent.length)
            } else {
              logInfo('[歌词加载] 优先级 3: 无在线匹配缓存歌词')
            }
          } catch (e) {
            logInfo('[歌词加载] 优先级 3: 读取缓存歌词失败:', e)
          }
        }
        
        // 如果本地都没有歌词，使用多源在线歌词服务查询
        if (!lyricContent) {
          logInfo('[歌词加载] 本地无歌词，开始多源在线查询')
          try {
            const result = await multiSourceLyricService.getLyric({
              id: song.id,
              title: song.title,
              artist: song.artist,
              album: song.album,
              filePath: song.path
            }, 'auto', false)
            
            if (result.success && result.bestScore && result.bestScore.lyricLines) {
              // 转换为界面滚动使用的 { time(秒), text } 格式
              lyrics.value = toSimpleLyricLines(result.bestScore.lyricLines)
              if (result.bestScore.yrcData) {
                yrcData.value = result.bestScore.yrcData
              }
              lyricSource = result.bestSource ? String(result.bestSource) : '在线源'
              logInfo('[歌词加载] 使用多源在线歌词，来源:', lyricSource, '行数:', lyrics.value.length)
              coverLyricLineRefs.value = []
            } else {
              logInfo('[歌词加载] 多源在线查询无结果')
              lyrics.value = []
              coverLyricLineRefs.value = []
            }
          } catch (e) {
            logError('[歌词加载] 多源在线查询失败:', e)
            lyrics.value = []
            coverLyricLineRefs.value = []
          }
        } else if (lyricContent) {
          // 解析本地歌词
          logInfo('[歌词加载] 最终使用来源:', lyricSource, '长度:', lyricContent.length)
          logInfo('[歌词加载] 歌词预览:', lyricContent.substring(0, 100))
          lyrics.value = parseLyrics(lyricContent)
          logInfo('歌词解析完成，行数:', lyrics.value.length)
          coverLyricLineRefs.value = []
        } else {
          logInfo('[歌词加载] 最终结果: 无歌词')
          lyrics.value = []
          coverLyricLineRefs.value = []
        }

        // 自动滚动到当前播放歌曲
        scrollToCurrentSong()
        
        // 预先确定下一首歌曲（用于随机播放模式）
        if (playbackMode.value === 'random' && songs.value.length > 1) {
          let nextIndex
          do {
            nextIndex = Math.floor(Math.random() * songs.value.length)
          } while (nextIndex === currentIndex && songs.value.length > 1)
          randomNextIndex.value = nextIndex
          logInfo('随机模式：预先确定下一首索引:', nextIndex, '歌曲:', songs.value[nextIndex].title)
        } else {
          randomNextIndex.value = null
        }
        
        return
      } catch (ffplayError) {
        logError('FFplay播放失败:', ffplayError)
        errorMessage = 'FFplay播放失败: ' + String(ffplayError)
        isPlaying.value = false
        isFFplayPlaying.value = false
        isPlaybackFinished = true
        throw new Error(errorMessage)
      }
    }

    // 检查文件是否存在（仅桌面应用）
    let finalPlayPath = playPath
    let durationFromBackend: number | null = null
    let audioInfoFromBackend: any = null
    if (!isBrowser.value) {
      try {
        const fileExists = await exists(playPath)
        logDebug('文件存在性检查:', { path: playPath, exists: fileExists })
        if (!fileExists) {
          logError('❌ 音频文件不存在，无法播放:', playPath)
          errorMessage = '文件不存在: ' + playPath
          isPlaying.value = false
          isPlaybackFinished = true
          throw new Error(errorMessage)
        }
      } catch (error) {
        logError('❌ 文件存在性检查失败:', error)
        errorMessage = error && typeof error === 'object' && 'message' in error ? (error.message as string) : ('文件检查失败: ' + String(error))
        isPlaying.value = false
        isPlaybackFinished = true
        throw new Error(errorMessage)
      }

      // 从后端获取音频信息（包含时长、采样率、编码器等）
      try {
        const result = await invoke('get_audio_duration', { path: playPath })
        if (result && typeof result === 'object') {
          if ('duration' in result) {
            durationFromBackend = Number(result.duration)
            logDebug('从后端获取的音频时长:', durationFromBackend, '秒')
          }
          // 保存完整的音频信息
          audioInfoFromBackend = result
          logDebug('从后端获取的完整音频信息:', audioInfoFromBackend)
        }
      } catch (error) {
        logInfo('获取音频信息失败:', error)
      }

      // 检查是否需要转码（仅当启用转码功能时）
      if (enableTranscode.value) {
        logDebug('转码检查: 文件=' + playPath + ', 启用转码功能')
        try {
          // 传递音频信息给转码命令，避免重复调用ffprobe
          // 增加超时时间到600秒（10分钟），以支持大文件转码
          logInfo('调用get_transcoded_path，原文件路径:', playPath)
          const transcodedPath = await invoke('get_transcoded_path', {
            path: playPath, 
            timeout_secs: 600,
            audio_info: audioInfoFromBackend
          }) as string
          logInfo('get_transcoded_path返回:', transcodedPath)
          finalPlayPath = transcodedPath
          logInfo('更新finalPlayPath为转码后的路径:', finalPlayPath)
          
          // 验证转码后的文件是否存在
          // 如果是HTTP URL，跳过文件存在性检查，因为HTTP URL是通过本地HTTP服务器提供的
          if (!transcodedPath.startsWith('http://') && !transcodedPath.startsWith('https://')) {
            const transcodedExists = await exists(transcodedPath)
            if (!transcodedExists) {
              logError('❌ 转码后的文件不存在:', transcodedPath)
              errorMessage = '转码文件不存在: ' + transcodedPath
              isPlaying.value = false
              isPlaybackFinished = true
              throw new Error(errorMessage)
            }
          }
        } catch (transcodeError) {
          logError('❌ 获取转码文件失败:', transcodeError)
          errorMessage = '转码失败: ' + (transcodeError && typeof transcodeError === 'object' && 'message' in transcodeError ? (transcodeError.message as string) : String(transcodeError))
          logInfo('转码检查失败（无法播放原文件，因为原文件格式浏览器不支持）:', errorMessage)
          // 转码失败时，不尝试播放原文件，因为原文件格式浏览器不支持
          isPlaying.value = false
          isPlaybackFinished = true
          throw new Error(errorMessage)
        }
      }
    } else {
      logInfo('浏览器环境，跳过文件存在性检查和转码检查')
    }

    // 确保CUE track的时间参数正确传递
    if (song.isCueTrack) {
      logDebug('CUE track信息:')
    logDebug('- song.isCueTrack:', song.isCueTrack)
    logDebug('- song.startTime:', song.startTime)
    logDebug('- song.endTime:', song.endTime)
    logDebug('- startTime:', startTime)
    logDebug('- endTime:', endTime)

      // 确保startTime和endTime是数字类型
      if (typeof startTime === 'string') {
        startTime = parseInt(startTime, 10)
      }
      if (typeof endTime === 'string') {
        endTime = parseInt(endTime, 10)
      }

      if (typeof startTime === 'number' && typeof endTime === 'number') {
        logDebug('CUE时间参数类型正确，准备传递给后端')
      } else {
        logDebug('CUE时间参数类型错误:')
        logDebug('- startTime类型:', typeof startTime)
        logDebug('- endTime类型:', typeof endTime)
      }
    }

    logDebug('CUE播放参数:', { isCueTrack: song.isCueTrack, startTime, endTime, playPath, cueStartTime, cueEndTime })
    logDebug('song对象:', song)
    logDebug('song.startTime:', song.startTime)
    logDebug('song.endTime:', song.endTime)

    logInfo('前端播放，文件路径:', finalPlayPath)
    
    try {
      // 停止并清理之前的音频元素，确保彻底销毁
      if (audioElement.value) {
        logInfo('停止并清理之前的音频元素')
        try {
          // 先暂停播放
          audioElement.value.pause()
          // 移除所有事件监听器
          if (timeupdateHandler) {
            audioElement.value.removeEventListener('timeupdate', timeupdateHandler)
          }
          // 清理所有事件处理函数
          audioElement.value.oncanplay = null
          audioElement.value.onerror = null
          audioElement.value.onended = null
          // 清空src并强制加载，释放资源
          audioElement.value.src = ''
          audioElement.value.load()
          // 彻底销毁音频元素
          audioElement.value = null
          timeupdateHandler = null
          logInfo('音频元素清理完成')
        } catch (cleanupError) {
          logError('清理音频元素时发生错误:', cleanupError)
          // 即使出错也要将audioElement.value设为null
          audioElement.value = null
          timeupdateHandler = null
        }
      }
      
      // 额外的安全检查，确保音频元素已被清理
      if (audioElement.value) {
        console.warn('音频元素清理后仍然存在，强制设为null')
        audioElement.value = null
        timeupdateHandler = null
      }
      
      // 确保音频元素的src属性正确设置
      let audioUrl = finalPlayPath
      
      // 浏览器环境下的特殊处理
      if (isBrowser.value) {
        // 从Map中获取File对象
        const file = browserFileMap.get(song.id)
        console.log('浏览器环境，从Map获取File对象:', { songId: song.id, hasFile: !!file, mapSize: browserFileMap.size })
        if (file) {
          try {
            // 直接使用File对象创建blob URL
            console.log('浏览器环境，File对象信息:', { name: file.name, size: file.size, type: file.type })
            
            // 检查文件类型是否被浏览器支持
            const fileExtension = file.name.split('.').pop()?.toLowerCase() || ''
            console.log('文件扩展名:', fileExtension)
            
            // 检查浏览器是否支持该音频格式
            const audio = document.createElement('audio')
            const canPlayType = audio.canPlayType(file.type)
            console.log('浏览器支持该格式:', canPlayType)
            
            if (canPlayType === '') {
              console.error('浏览器不支持该音频格式:', file.type, '文件:', file.name)
              errorMessage = `浏览器不支持该音频格式: ${fileExtension.toUpperCase()}。请尝试使用MP3、WAV或OGG格式。`
              isPlaying.value = false
              isPlaybackFinished = true
              throw new Error(errorMessage)
            }
            
            const newObjectURL = URL.createObjectURL(file)
            audioUrl = newObjectURL
            console.log('浏览器环境，使用新创建的blob URL:', audioUrl)
          } catch (error) {
            console.error('创建blob URL失败:', error)
            // 如果创建blob URL失败，尝试使用其他方式
            errorMessage = '无法创建音频URL: ' + String(error)
            isPlaying.value = false
            isPlaybackFinished = true
            throw new Error(errorMessage)
          }
        } else {
          // 如果没有file对象，尝试直接使用path作为URL
          console.log('浏览器环境，没有原始文件对象，尝试使用path作为URL:', audioUrl)
          // 这里可以添加其他浏览器环境下的URL处理逻辑
        }
      } else if (!audioUrl.startsWith('http://') && !audioUrl.startsWith('https://') && !audioUrl.startsWith('blob:')) {
        // 桌面应用环境，使用 Tauri 的 convertFileSrc 将文件路径转换为 asset 协议 URL
        try {
          logInfo('前端播放: 桌面环境，使用 convertFileSrc 转换文件路径...')
          audioUrl = convertFileSrc(audioUrl)
          logInfo('前端播放: convertFileSrc 转换成功:', audioUrl)
          console.log('convertFileSrc 转换成功:', audioUrl)
        } catch (urlError) {
          console.error('❌ 前端播放: convertFileSrc 转换失败:', urlError)
          logError('❌ 前端播放: convertFileSrc 转换失败:', urlError)

          // 转换失败，尝试使用FFplay作为回退
          if (!isBrowser.value && !needsFFplay) {
            logInfo('convertFileSrc 转换失败，尝试使用FFplay回退播放')
            needsFFplay = true
          } else {
            errorMessage = '无法转换文件路径: ' + (urlError && typeof urlError === 'object' && 'message' in urlError ? (urlError.message as string) : String(urlError))
            isPlaying.value = false
            isPlaybackFinished = true
            throw new Error(errorMessage)
          }
        }
      }
      
      // 不要修改blob URL格式，保持URL.createObjectURL返回的原始格式
      console.log('最终使用的音频URL:', audioUrl)
      
      // 创建音频元素并设置src
      logInfo('前端播放: 使用URL:', audioUrl)
      
      // 直接创建Audio元素并设置src
      audioElement.value = new Audio()
      // 禁用autoplay，手动控制播放
      audioElement.value.autoplay = false
      
      // 浏览器环境下，直接使用File对象设置src
      if (isBrowser.value) {
        try {
          audioElement.value.src = audioUrl
          logInfo('浏览器环境，设置音频元素src为blob URL:', audioUrl)
        } catch (error) {
          logError('设置音频元素src失败:', error)
          errorMessage = '无法设置音频URL: ' + String(error)
          isPlaying.value = false
          isPlaybackFinished = true
          throw new Error(errorMessage)
        }
      } else {
        // 其他环境，使用普通URL
        audioElement.value.src = audioUrl
        logInfo('设置音频元素src为普通URL:', audioUrl)
      }
      
      // 浏览器环境下添加错误事件监听器
      if (isBrowser.value) {
        audioElement.value.addEventListener('error', (error) => {
          logError('浏览器环境音频元素错误:', error, 'URL:', audioUrl)
          if (error.target && 'error' in error.target) {
            const audioError = error.target as HTMLAudioElement
            logError('音频错误代码:', audioError.error?.code, 'URL:', audioUrl)
            
            // 错误代码解释：
            // 1 = MEDIA_ERR_ABORTED - 音频加载被中止
            // 2 = MEDIA_ERR_NETWORK - 网络错误
            // 3 = MEDIA_ERR_DECODE - 解码错误
            // 4 = MEDIA_ERR_SRC_NOT_SUPPORTED - 不支持的音频格式
            switch (audioError.error?.code) {
              case 1:
                logError('音频加载被中止，请检查网络连接')
                break
              case 2:
                logError('网络错误，请检查网络连接')
                break
              case 3:
                logError('音频解码错误，可能是格式不支持')
                break
              case 4:
                logError('不支持的音频格式')
                break
              default:
                logError('未知音频错误')
            }
          }
        })
      } else {
        // 桌面环境也添加错误监听器，用于捕获加载失败
        audioElement.value.addEventListener('error', (event) => {
          const audioError = (event as any).target?.error
          logError('桌面环境音频元素错误:', audioError, 'URL:', audioUrl)
          if (audioError) {
            logError('音频错误详情: code=', audioError.code, 'message=', audioError.message)
            
            // 桌面环境下，尝试使用FFplay回退
            if (!isFFplayPlaying.value) {
              logInfo('桌面环境音频加载失败，标记需要FFplay回退')
              needsFFplay = true
            }
          }
        })
      }
      
      // 添加timeupdate事件监听器，用于更新播放进度
      timeupdateHandler = () => {
        if (isPlaying.value && !isSeeking.value && audioElement.value) {
          updateProgress()
        }
      }
      audioElement.value.addEventListener('timeupdate', timeupdateHandler)
      logInfo('创建音频元素并设置src:', audioUrl, 'autoplay:', audioElement.value.autoplay)
      
      // 使用用户设置的音量
      // 注意：如果启用了交叉淡入淡出，volume.value 可能被临时设置为 0
      // 所以这里应该使用 originalVolume 来设置音频元素的音量
      let volumeValue = originalVolume / 100
      // 确保音量值在有效范围内
      volumeValue = Math.max(0, Math.min(1, volumeValue))
      audioElement.value.volume = volumeValue
      logInfo('设置音频元素音量:', volumeValue)
      
      // 确保音频元素不是静音状态
      if (isMuted.value) {
        audioElement.value.volume = 0
        logInfo('音频元素已静音')
      }
      
      // 设置音频时长（如果从后端获取到了时长）
      if (durationFromBackend && durationFromBackend > 0) {
        // 更新song的时长
        const totalSeconds = Math.round(durationFromBackend)
        const minutes = Math.floor(totalSeconds / 60)
        const seconds = totalSeconds % 60
        song.duration = `${minutes}:${seconds.toString().padStart(2, '0')}`
        logDebug('设置音频时长:', song.duration)
        
        // 计算进度百分比
        if (positionForCue >= 0) {
          const totalSec = minutes * 60 + seconds
          if (totalSec > 0) {
            progress.value = Math.min((positionForCue / totalSec) * 100, 100)
            logDebug('更新进度百分比:', progress.value)
          }
        }
      } else if (isBrowser.value && song.duration === '未知') {
        // 浏览器环境且时长未知，在播放时获取音频时长
        logInfo('浏览器环境，等待音频加载以获取时长')
        // 时长会在音频元素加载时自动更新
      } else {
        logInfo('未获取到音频时长，使用默认值')
      }
      
      // 现在设置currentSong
      // 如果是转码后的文件，创建一个新的song对象，更新path属性
      if (enableTranscode.value && song.needs_transcode) {
        // 创建转码后的song对象
        const transcodedSong = {
          ...song,
          path: finalPlayPath
        }
        currentSong.value = transcodedSong
        logInfo('转码后更新currentSong，新路径:', finalPlayPath)
      } else {
        currentSong.value = song
      }
      
      // 歌词加载优先级策略
      logInfo('[歌词加载] 开始加载歌词，song.id:', song.id)
      let lyricContent = ''
      let lyricSource = ''
      const songForLyric = songs.value.find(s => s.id === song.id) || song
      
      // 优先级 1: 音频文件内嵌歌词（来自后端扫描）
      const embeddedLyric = songForLyric.lyric || song.lyric || ''
      if (embeddedLyric && embeddedLyric.trim()) {
        lyricContent = embeddedLyric
        lyricSource = '内嵌歌词'
        logInfo('[歌词加载] 优先级 1: 使用内嵌歌词，长度:', lyricContent.length)
      } else {
        logInfo('[歌词加载] 优先级 1: 无内嵌歌词')
      }
      
      // 优先级 2: 同名 .lrc 文件（必须非空）
      if (!lyricContent && songForLyric) {
        try {
          const { readTextFile } = await import('@tauri-apps/plugin-fs')
          const lyricPath = songForLyric.path.replace(/\.[^/.]+$/, '.lrc')
          logInfo('[歌词加载] 优先级 2: 尝试读取.lrc文件:', lyricPath)
          const content = await readTextFile(lyricPath)
          if (content && content.trim()) {
            lyricContent = content
            lyricSource = '同名.lrc文件'
            logInfo('[歌词加载] 优先级 2: 成功读取.lrc文件，长度:', lyricContent.length)
          } else {
            logInfo('[歌词加载] 优先级 2: .lrc文件存在但内容为空，跳过')
          }
        } catch (e) {
          logInfo('[歌词加载] 优先级 2: 读取.lrc文件失败:', e)
        }
      }
      
      // 优先级 3: 在线匹配缓存歌词
      if (!lyricContent) {
        try {
          const cachedLyric = await localStorageService.getCachedLyric(song.id)
          if (cachedLyric && cachedLyric.trim()) {
            lyricContent = cachedLyric
            lyricSource = '在线匹配缓存'
            logInfo('[歌词加载] 优先级 3: 使用在线匹配缓存歌词，长度:', lyricContent.length)
          } else {
            logInfo('[歌词加载] 优先级 3: 无在线匹配缓存歌词')
          }
        } catch (e) {
          logInfo('[歌词加载] 优先级 3: 读取缓存歌词失败:', e)
        }
      }
      
      // 如果本地都没有歌词，使用多源在线歌词服务查询
      if (!lyricContent) {
        logInfo('[歌词加载] 本地无歌词，开始多源在线查询')
        try {
          const result = await multiSourceLyricService.getLyric({
            id: song.id,
            title: song.title,
            artist: song.artist,
            album: song.album,
            filePath: song.path
          }, 'auto', false)
          
          if (result.success && result.bestScore && result.bestScore.lyricLines) {
            // 转换为界面滚动使用的 { time(秒), text } 格式
            lyrics.value = toSimpleLyricLines(result.bestScore.lyricLines)
            if (result.bestScore.yrcData) {
              yrcData.value = result.bestScore.yrcData
            }
            lyricSource = result.bestSource ? String(result.bestSource) : '在线源'
            logInfo('[歌词加载] 使用多源在线歌词，来源:', lyricSource, '行数:', lyrics.value.length)
            coverLyricLineRefs.value = []
          } else {
            logInfo('[歌词加载] 多源在线查询无结果')
            lyrics.value = []
            coverLyricLineRefs.value = []
          }
        } catch (e) {
          logError('[歌词加载] 多源在线查询失败:', e)
          lyrics.value = []
          coverLyricLineRefs.value = []
        }
      } else if (lyricContent) {
        // 解析本地歌词
        logInfo('[歌词加载] 最终使用来源:', lyricSource, '长度:', lyricContent.length)
        logInfo('[歌词加载] 歌词预览:', lyricContent.substring(0, 100))
        lyrics.value = parseLyrics(lyricContent)
        logInfo('歌词解析完成，行数:', lyrics.value.length)
        coverLyricLineRefs.value = []
      } else {
        logInfo('[歌词加载] 最终结果: 无歌词')
        lyrics.value = []
        coverLyricLineRefs.value = []
      }
      
      // 设置播放位置
      let startTimeToUse = 0
      let endTimeToUse: number | null = null
      
      if (song.isCueTrack) {
        let startTimeNum = Number(song.startTime)
        let endTimeNum = Number(song.endTime)
        // 检查startTimeNum是否是时间戳（毫秒），如果是，转换为秒数
        if (!isNaN(startTimeNum) && startTimeNum > 9999999999) { // 如果大于10位数字，认为是毫秒级时间戳
          startTimeNum = startTimeNum / 1000
          logInfo('检测到CUE track startTime是时间戳，转换为秒数:', startTimeNum, '秒')
        }
        // 检查endTimeNum是否是时间戳（毫秒），如果是，转换为秒数
        if (!isNaN(endTimeNum) && endTimeNum > 9999999999) { // 如果大于10位数字，认为是毫秒级时间戳
          endTimeNum = endTimeNum / 1000
          logInfo('检测到CUE track endTime是时间戳，转换为秒数:', endTimeNum, '秒')
        }
        if (!isNaN(startTimeNum) && startTimeNum >= 0) {
          startTimeToUse = startTimeNum
        }
        if (!isNaN(endTimeNum) && endTimeNum > startTimeToUse) {
          endTimeToUse = endTimeNum
        }
        logInfo('CUE track前端播放设置: startTime=' + startTimeToUse + 's, endTime=' + (endTimeToUse || '无'))
      } else {
        if (positionForCue > 0) {
          startTimeToUse = positionForCue
        }
      }
      
      // 等待音频元素加载完成后再播放
      if (autoPlay) {
        await new Promise<void>((resolve, reject) => {
          // 保存当前音频元素的引用，避免被其他操作修改
          const currentAudioElement = audioElement.value
          
          // 检查音频元素是否为null
          if (!currentAudioElement) {
            logError('音频元素为null，无法继续播放')
            resolve()
            return
          }
          
          // 添加标志，确保oncanplay只执行一次
          let canplayExecuted = false
          
          // 等待音频元素加载完成后再播放
          currentAudioElement.oncanplay = async () => {
            // 确保只执行一次
            if (canplayExecuted) {
              logInfo('oncanplay事件已执行过，忽略重复触发')
              return
            }
            canplayExecuted = true
          
            // 检查音频元素是否为null或已被替换
            if (!audioElement.value || audioElement.value !== currentAudioElement) {
              logInfo('音频元素已被清理或替换，oncanplay事件处理被忽略')
              resolve()
              return
            }
          
            // 检查音频元素的src属性是否为空
            if (!audioElement.value.src) {
              logInfo('音频元素src属性为空，oncanplay事件处理被忽略')
              resolve()
              return
            }
            
            // 浏览器环境，更新音频时长
            if (isBrowser.value && currentSong.value && currentSong.value.duration === '未知') {
              const duration = audioElement.value.duration
              if (duration && !isNaN(duration)) {
                const totalSeconds = Math.round(duration)
                const minutes = Math.floor(totalSeconds / 60)
                const seconds = totalSeconds % 60
                currentSong.value.duration = `${minutes}:${seconds.toString().padStart(2, '0')}`
                logInfo('浏览器环境，更新音频时长:', currentSong.value.duration)
              }
            }
          
            try {
              // 设置播放位置
              audioElement.value.currentTime = startTimeToUse
              
              // 播放音频
              try {
                await audioElement.value.play()
                logInfo('✅ 前端播放开始，位置:', startTimeToUse, '秒')
                
                // 只有播放成功时才设置播放状态为true
                if (autoPlay) {
                  isPlaying.value = true
                }
              } catch (playError) {
                logError('播放请求失败:', playError)
                // 忽略中断错误和自动播放策略错误
                if (!String(playError).includes('interrupted') && !String(playError).includes('autoplay') && !String(playError).includes('NotAllowedError')) {
                  // 只记录错误，不抛出，避免清理音频元素
                  logError('严重播放错误:', playError)
                }
                // 播放失败时不设置播放状态为true
                if (autoPlay) {
                  isPlaying.value = false
                }
              } finally {
                // 无论播放成功还是失败，都设置播放开始时间，以便进度计算
                let adjustedStartTimeToUse = startTimeToUse
                if (adjustedStartTimeToUse > 9999999999) { // 如果大于10位数字，认为是毫秒级时间戳
                  adjustedStartTimeToUse = adjustedStartTimeToUse / 1000
                }
                playbackStartTime.value = Date.now() - (adjustedStartTimeToUse * 1000)
                logInfo('设置播放开始时间:', playbackStartTime.value, '开始位置:', adjustedStartTimeToUse, '秒')
              }
              
              resolve()
            } catch (playError) {
              logError('播放请求失败:', playError)
              // 检查音频元素是否为null或已被替换
              if (!audioElement.value || audioElement.value !== currentAudioElement) {
                logInfo('音频元素已被清理或替换，错误处理被忽略')
                resolve()
                return
              }
              // 检查音频元素的src属性是否为空
              if (!audioElement.value.src) {
                logInfo('音频元素src属性为空，错误处理被忽略')
                resolve()
                return
              }
              // 发生错误时设置播放状态为false
              if (autoPlay) {
                isPlaying.value = false
              }
              resolve()
            }
          }
        
        // 添加防重入标志，避免重复处理错误
        let errorHandled = false
        
        // 处理错误
        currentAudioElement.onerror = (event) => {
          // 检查是否已经处理过错误，避免重复处理导致级联错误
          if (errorHandled) {
            logInfo('错误已经处理过，忽略重复触发')
            return
          }
          errorHandled = true
          
          // 检查音频元素是否为null或已被替换
          if (!audioElement.value || audioElement.value !== currentAudioElement) {
            logInfo('音频元素已被清理或替换，onerror事件处理被忽略')
            resolve()
            return
          }
          
          // 获取更详细的错误信息
          const error = (event as any).target?.error
          logError('音频元素错误:', event)
          if (error) {
            logError('音频错误详情: code=', error.code, 'message=', error.message)
            
            // 针对不同类型的错误提供更具体的处理
            switch(error.code) {
              case error.MEDIA_ERR_ABORTED:
                logError('音频加载被中止: 可能是用户中断了加载过程')
                break
              case error.MEDIA_ERR_NETWORK:
                logError('网络错误导致音频加载失败: 检查网络连接或文件服务器')
                break
              case error.MEDIA_ERR_DECODE:
                logError('音频解码失败: 音频文件可能损坏或格式不被支持')
                break
              case error.MEDIA_ERR_SRC_NOT_SUPPORTED:
                logError('音频格式不支持: 当前浏览器不支持此音频格式')
                break
              default:
                logError('未知音频错误: 请检查音频文件和播放环境')
            }
            
            // 针对特定错误类型的处理策略
            if (error.code === error.MEDIA_ERR_SRC_NOT_SUPPORTED || error.code === error.MEDIA_ERR_DECODE) {
              logInfo('尝试启用转码来处理不支持的格式')
            }
            
            // 桌面环境下，如果HTML5 Audio播放失败，尝试使用FFplay回退
            if (!isBrowser.value && !isFFplayPlaying.value) {
              logInfo('HTML5 Audio播放失败，尝试使用FFplay回退')
              // 不在此处设置 needsFFplay，让 catch 块统一处理 FFplay 回退
              // 清理当前音频元素
              try {
                currentAudioElement.pause()
                currentAudioElement.src = ''
              } catch (cleanupError) {
                logError('清理音频元素失败:', cleanupError)
              }
              // 拒绝Promise，让调用方处理FFplay回退
              reject(new Error('HTML5 Audio播放失败，尝试FFplay回退'))
              return
            }
          }
          // 只记录错误，不设置播放状态为false，避免清理音频元素
          logInfo('音频元素错误，继续执行')
          resolve()
        }
        
        // 检查音频元素是否已经加载完成
        if (currentAudioElement.readyState >= 3) {
          // 音频已经加载完成
          logInfo('前端播放: 音频元素已加载完成')
          logInfo('音频元素状态: src=', currentAudioElement.src, 'readyState=', currentAudioElement.readyState, 'networkState=', currentAudioElement.networkState)
          
          // 设置播放位置
          currentAudioElement.currentTime = startTimeToUse
          logInfo('设置播放位置后: currentTime=', currentAudioElement.currentTime)
          
          if (autoPlay) {
            // 播放音频
            currentAudioElement.play()
              .then(() => {
                // 检查音频元素是否为null或已被替换
                if (!audioElement.value || audioElement.value !== currentAudioElement) {
                  logInfo('音频元素已被清理或替换，播放成功处理被忽略')
                  resolve()
                  return
                }
                
                logInfo('✅ 前端播放开始，位置:', startTimeToUse, '秒')
                logInfo('播放后音频元素状态: currentTime=', audioElement.value.currentTime, 'volume=', audioElement.value.volume, 'paused=', audioElement.value.paused)
                
                // 设置播放开始时间，用于后续的进度计算
                // 检查startTimeToUse是否是时间戳（毫秒），如果是，转换为秒数
                let adjustedStartTimeToUse = startTimeToUse
                if (adjustedStartTimeToUse > 9999999999) { // 如果大于10位数字，认为是毫秒级时间戳
                  adjustedStartTimeToUse = adjustedStartTimeToUse / 1000
                  logInfo('检测到时间戳，转换为秒数:', adjustedStartTimeToUse, '秒')
                }
                playbackStartTime.value = Date.now() - (adjustedStartTimeToUse * 1000)
                logInfo('设置播放开始时间:', playbackStartTime.value, '开始位置:', adjustedStartTimeToUse, '秒')
                
                isPlaying.value = true
                resolve()
              })
              .catch((playError) => {
                logError('播放请求失败:', playError)
                // 忽略中断错误和自动播放策略错误
                // 即使播放请求被中断，也要设置播放开始时间
                // 检查startTimeToUse是否是时间戳（毫秒），如果是，转换为秒数
                let adjustedStartTimeToUse = startTimeToUse
                if (adjustedStartTimeToUse > 9999999999) { // 如果大于10位数字，认为是毫秒级时间戳
                  adjustedStartTimeToUse = adjustedStartTimeToUse / 1000
                  logInfo('检测到时间戳，转换为秒数:', adjustedStartTimeToUse, '秒')
                }
                playbackStartTime.value = Date.now() - (adjustedStartTimeToUse * 1000)
                logInfo('设置播放开始时间:', playbackStartTime.value, '开始位置:', adjustedStartTimeToUse, '秒')
                // 尝试再次播放，可能用户已经交互过
                setTimeout(async () => {
                  // 检查音频元素是否为null或已被替换
                  if (!audioElement.value || audioElement.value !== currentAudioElement) {
                    logInfo('音频元素已被清理或替换，延迟播放被忽略')
                    return
                  }
                  
                  try {
                    await audioElement.value.play()
                    logInfo('✅ 延迟播放成功')
                    isPlaying.value = true
                  } catch (e) {
                    logError('延迟播放也失败:', e)
                  }
                }, 100)
                resolve()
              })
          } else {
            // 当autoPlay为false时，确保音频元素处于暂停状态
            try {
              currentAudioElement.pause()
              logInfo('当autoPlay为false时，确保音频元素处于暂停状态')
            } catch (pauseError) {
              logError('暂停音频元素失败:', pauseError)
            }
            resolve()
          }
        }
})
      } else {
        // 当autoPlay为false时，直接继续执行，不需要resolve
        // 确保音频元素处于暂停状态
        if (audioElement.value) {
          try {
            audioElement.value.pause()
            logInfo('当autoPlay为false时，确保音频元素处于暂停状态')
          } catch (pauseError) {
            logError('暂停音频元素失败:', pauseError)
          }
        }
        
        // 当autoPlay为false时，也要设置播放开始时间，以便后续手动播放时的进度计算
        let adjustedStartTimeToUse = startTimeToUse
        if (adjustedStartTimeToUse > 9999999999) { // 如果大于10位数字，认为是毫秒级时间戳
          adjustedStartTimeToUse = adjustedStartTimeToUse / 1000
        }
        playbackStartTime.value = Date.now() - (adjustedStartTimeToUse * 1000)
        logInfo('设置播放开始时间:', playbackStartTime.value, '开始位置:', adjustedStartTimeToUse, '秒')
      }
      
      // 检查音频元素是否为null
      if (audioElement.value) {
        audioElement.value.onended = () => {
          logInfo('前端播放结束')
          isPlaybackFinished = true
          // 触发播放完成事件
          if (autoPlayNext.value) {
            playNext()
          } else if (autoPlay) {
            isPlaying.value = false
          }
        }
        
        // 对于CUE track，添加结束时间检测
        if (song.isCueTrack && endTimeToUse) {
          const checkEndTime = () => {
            if (audioElement.value && audioElement.value.currentTime >= endTimeToUse - 0.5) { // 增加缓冲时间
              logInfo('CUE track达到结束时间，准备停止播放')
              logInfo('CUE track结束时间检测: currentTime=', audioElement.value.currentTime, 'endTimeToUse=', endTimeToUse)
              
              // 等待一小段时间，确保音频播放完成
              setTimeout(() => {
                if (audioElement.value && isPlaying.value) {
                  logInfo('CUE track确认结束，停止播放')
                  audioElement.value.pause()
                  isPlaybackFinished = true
                  if (autoPlay) {
                    isPlaying.value = false
                  }
                  if (autoPlayNext.value) {
                    playNext()
                  }
                }
              }, 500) // 500毫秒缓冲
            } else if (isPlaying.value) {
              setTimeout(checkEndTime, 100)
            }
          }
          checkEndTime()
        }
      }
      
      // 启动前端进度更新
      if (autoPlay) {
        updateProgress()
      }
      
      // 交叉淡入淡出：逐渐恢复音量
      if (autoPlay && crossfadeEnabled.value && crossfadeDuration.value > 0) {
        const fadeDuration = crossfadeDuration.value * 1000 // 转换为毫秒
        const steps = 20 // 淡入步骤数
        const stepDuration = fadeDuration / steps
        
        // 注意：这里直接操作音频元素的音量，而不是 volume.value
        // 这样用户在 UI 上看到的音量值就不会受到交叉淡入淡出的影响
        if (audioElement.value) {
          // 先将音频元素的音量设置为 0
          audioElement.value.volume = 0
          
          // 逐渐增加音频元素的音量
          for (let i = 1; i <= steps; i++) {
            await new Promise(resolve => setTimeout(resolve, stepDuration))
            const currentVolume = (originalVolume * i) / (steps * 100)
            if (audioElement.value) {
              audioElement.value.volume = currentVolume
            }
          }
          
          // 确保音频元素的音量恢复到原始值
          audioElement.value.volume = originalVolume / 100
          
          // 如果之前是静音状态，恢复静音
          if (isMuted.value) {
            audioElement.value.volume = 0
          }
        }
      } else {
        // 没有启用交叉淡入淡出，确保音频元素的音量设置正确
        if (audioElement.value) {
          const volumeValue = originalVolume / 100
          audioElement.value.volume = volumeValue
        }
      }
      
      // 重置播放完成标志
      isPlaybackFinished = false
      
      // 启动播放完成检测定时器
      if (currentSong.value) {
        const duration = currentSong.value!.duration
        // 只有当时长不是"未知"时才设置播放完成检测定时器
        if (duration !== '未知') {
          const parts = duration.split(':')
          if (parts.length === 2) {
            const minutes = parseInt(parts[0])
            const seconds = parseInt(parts[1])
            const totalSeconds = minutes * 60 + seconds
            if (totalSeconds > 0) {
              // 立即检测一次，确保播放完成检测逻辑正常
              const elapsedSeconds = (Date.now() - playbackStartTime.value) / 1000 - pausedDuration.value
              // 只有当elapsedSeconds大于0且接近总时长时才认为播放完成
              if (elapsedSeconds > 0 && elapsedSeconds >= totalSeconds - 0.5) {
                if (autoPlay) {
                  isPlaying.value = false
                }
                handlePlaybackFinished(autoPlay)
              }
              // 设置定时器，使用稍长的时间，确保歌曲真正完成
              const playbackTimer = setTimeout(() => {
                if (isPlaying.value) {
                  // 再次检查实际播放位置，确保确实接近结束
                  let actualPosition = 0
                  if (audioElement.value && !isNaN(audioElement.value.currentTime)) {
                    actualPosition = audioElement.value.currentTime
                  } else {
                    actualPosition = (Date.now() - playbackStartTime.value) / 1000 - pausedDuration.value
                  }
                  
                  // 对于CUE track，需要转换为相对位置
                  let positionInSeconds = actualPosition
                  if (currentSong.value && currentSong.value.isCueTrack && currentSong.value.startTime) {
                    const startTimeNum = Number(currentSong.value.startTime)
                    positionInSeconds = actualPosition - startTimeNum
                    if (positionInSeconds < 0) positionInSeconds = 0
                  }
                  
                  // 只有当实际位置接近总时长时才认为播放完成
                  if (positionInSeconds >= totalSeconds - 1) {
                    if (autoPlay) {
                      isPlaying.value = false
                    }
                    handlePlaybackFinished(autoPlay)
                  }
                }
              }, (totalSeconds + 2) * 1000) // 增加2秒缓冲，确保歌曲真正完成
              
              // 保存定时器ID，以便在需要时清除
              playbackTimerId = playbackTimer
            }
          }
        }
      }
      
      // 自动滚动到当前播放歌曲
      scrollToCurrentSong()
      
      // 预先确定下一首歌曲（用于随机播放模式）
      if (playbackMode.value === 'random' && songs.value.length > 1) {
        let nextIndex
        do {
          nextIndex = Math.floor(Math.random() * songs.value.length)
        } while (nextIndex === currentIndex && songs.value.length > 1)
        randomNextIndex.value = nextIndex
        logInfo('随机模式：预先确定下一首索引:', nextIndex, '歌曲:', songs.value[nextIndex].title)
      } else {
        randomNextIndex.value = null
      }
      
      return
    } catch (error) {
      logError('前端播放失败:', error)
      if (autoPlay) {
        isPlaying.value = false
      }
      isPlaybackFinished = true
      throw error
    }
  } catch (error) {
    logError('播放歌曲失败:', error)
    logError('播放歌曲失败详情:', typeof error, error)
    
    const errorMessage = error && typeof error === 'string' ? error : (error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error))
    
    // 前端播放失败时，尝试使用FFplay回退
    // 检查多种可能的错误情况：
    // 1. 包含"HTML5 Audio"的错误
    // 2. 包含"no supported source"的错误（HTML5 Audio常见错误）
    // 3. 包含"MEDIA_ERR_"的错误（音频元素错误代码）
    // 4. needsFFplay被标记为true
    const shouldFallbackToFFplay = !isBrowser.value && !isFFplayPlaying.value && 
      !needsFFplay && (
        errorMessage.includes('HTML5 Audio') || 
        errorMessage.includes('no supported source') || 
        errorMessage.includes('MEDIA_ERR_') ||
        errorMessage.includes('Failed to load')
      )
    
    if (shouldFallbackToFFplay) {
      logInfo('前端播放失败，尝试使用FFplay回退播放')
      needsFFplay = true
      
      // 清理当前状态
      if (audioElement.value) {
        try {
          audioElement.value.pause()
          audioElement.value.src = ''
        } catch (cleanupError) {
          logError('清理音频元素失败:', cleanupError)
        }
        audioElement.value = null
        timeupdateHandler = null
      }
      
      // 重新执行FFmpeg播放逻辑
      try {
        logInfo('【FFmpeg回退】开始调用 play_audio_with_ffmpeg')
        
        // 停止之前的FFmpeg播放（如果有）
        if (isFFplayPlaying.value) {
          try {
            await invoke('stop_ffplay')
            isFFplayPlaying.value = false
          } catch (stopError) {
            logError('停止FFmpeg播放失败:', stopError)
          }
        }
        
        // 使用FFmpeg播放
        const start_position = positionForCue
        let durationSeconds = 300
        if (currentSong.value && currentSong.value.duration) {
          const durationStr = currentSong.value.duration
          const parts = durationStr.split(':')
          if (parts.length === 2) {
            const minutes = parseInt(parts[0])
            const seconds = parseInt(parts[1])
            durationSeconds = minutes * 60 + seconds
          }
        }
        
        const result = await invoke('play_audio_with_ffmpeg', {
          path: playPath,
          start_time: start_position,
          duration: durationSeconds
        }) as FFplayResult | string
        
        if (typeof result === 'string' && result.includes('未找到')) {
          logError('FFmpeg未找到，无法播放:', result)
          if (autoPlay) {
            isPlaying.value = false
          }
          isPlaybackFinished = true
          throw new Error('FFplay未找到，无法播放此音频')
        }
        
        // 设置FFplay播放状态
        isFFplayPlaying.value = true
        isPlaying.value = true
        
        // 更新歌曲信息
        if (result && typeof result === 'object') {
          const ffResult = result as FFplayResult
          if (ffResult.duration !== undefined) {
            ffplayDuration.value = ffResult.duration
            ffplayPosition.value = start_position
            
            const totalSeconds = Math.round(ffResult.duration)
            const minutes = Math.floor(totalSeconds / 60)
            const seconds = totalSeconds % 60
            song.duration = `${minutes}:${seconds.toString().padStart(2, '0')}`
            
            if (ffResult.format) song.format = ffResult.format
            if (ffResult.sample_rate) song.sample_rate = ffResult.sample_rate
            if (ffResult.channels) song.channels = ffResult.channels
            if (ffResult.bit_rate) song.bit_rate = ffResult.bit_rate
          }
        }
        
        // 自动滚动到当前播放歌曲
        scrollToCurrentSong()
        
        if (playbackMode.value === 'random' && songs.value.length > 1) {
          let nextIndex
          do {
            nextIndex = Math.floor(Math.random() * songs.value.length)
          } while (nextIndex === currentIndex && songs.value.length > 1)
          randomNextIndex.value = nextIndex
        } else {
          randomNextIndex.value = null
        }
        
        return
      } catch (ffplayError) {
        logError('FFplay回退播放失败:', ffplayError)
        // 继续执行原有的错误处理逻辑
      }
    }
    
    if (autoPlay) {
      isPlaying.value = false
    }
    isPlaybackFinished = true
    if (audioElement.value && timeupdateHandler) {
      try {
        if (autoPlay) {
          audioElement.value.pause()
        }
        audioElement.value.removeEventListener('timeupdate', timeupdateHandler)
        // 清理其他事件监听器
        audioElement.value.oncanplay = null
        audioElement.value.onerror = null
        audioElement.value.onended = null
        audioElement.value.src = ''
      } catch (cleanupError) {
        logError('清理音频元素失败:', cleanupError)
      } finally {
        audioElement.value = null
        timeupdateHandler = null
      }
    }
    
    logError('❌ 播放失败:', errorMessage)
    
    if (errorMessage.includes('FFmpeg') || errorMessage.includes('转码')) {
      logInfo('转码相关错误，静默处理并尝试播放下一首')
      if (autoPlayNext.value && songs.value.length > 1) {
        logInfo('转码失败，自动跳到下一首')
        await playNext()
      }
    } else {
      if (errorMessage.includes('不存在') || errorMessage.includes('无法读取')) {
        logInfo('文件不存在或无法读取:', errorMessage)
        if (autoPlayNext.value && songs.value.length > 1) {
          logInfo('文件不存在，自动跳到下一首')
          await playNext()
        }
      } else {
        if (autoPlay) {
          alert(`播放失败：${errorMessage}\n请确认音频文件存在且格式受支持`)
        }
        if (autoPlayNext.value && songs.value.length > 1) {
          logInfo('播放失败，自动跳到下一首')
          await playNext()
        }
      }
    }
  } finally {
    // 释放锁定
    if (resolveLock) {
      resolveLock()
      logInfo(`[播放保护] 播放请求 ${thisPlayId}: 已释放锁`)
    }
    playSongLock = null
  }
}

// 播放状态锁，避免快速点击导致的操作竞态
let isToggling = false

const togglePlayback = async () => {
  // 防止快速连续点击导致的操作竞态
  if (isToggling) {
    logInfo('播放操作正在进行中，忽略重复点击')
    return
  }
  
  isToggling = true
  
  try {
    logInfo('togglePlayback 被调用,当前 isPlaying:', isPlaying.value)
    
    if (!currentSong.value) {
      logInfo('当前没有歌曲,播放第一首')
      if (songs.value.length > 0) {
        await playSong(songs.value[0])
      }
      return
    }

    // 如果使用FFplay播放
    logInfo('检查isFFplayPlaying.value:', isFFplayPlaying.value)
    if (isFFplayPlaying.value) {
      logInfo('使用FFplay播放，切换播放状态')

      if (isPlaying.value) {
        logInfo('暂停FFplay播放')
        try {
          await invoke('pause_ffplay')
          isPlaying.value = false
          // 不要将isFFplayPlaying.value设置为false，否则后续的恢复播放会失败
          // isFFplayPlaying.value = false
        } catch (error) {
          logError('暂停FFplay播放失败:', error)
        }
      } else {
        logInfo('恢复FFplay播放')
        try {
          const result = await invoke('resume_ffplay', { path: currentSong.value.path }) as any
          logInfo('FFplay播放已恢复:', result)
          isPlaying.value = true
          // isFFplayPlaying.value 应该保持 true
        } catch (error) {
          logError('恢复FFplay播放失败:', error)
          // 如果恢复失败，可能是 ffplay 进程已结束，尝试重新播放
          if (currentSong.value) {
            logInfo('尝试重新播放当前歌曲')
            await playSong(currentSong.value, currentPosition.value)
          }
        }
      }

      isToggling = false
      return
    }

    if (isPlaying.value) {
      logInfo('暂停播放')
      // 记录暂停开始时间
      pauseStartTime.value = Date.now()
      logInfo('暂停开始时间:', pauseStartTime.value)
      
      // 暂停前端音频元素
      if (audioElement.value) {
        audioElement.value.pause()
      }
      
      isPlaying.value = false
    } else {
      logInfo('恢复播放')
      // 检查音频元素是否存在
      if (!audioElement.value) {
        logError('没有音频元素，尝试使用FFplay播放')
        // 尝试使用FFplay播放当前歌曲
        if (currentSong.value) {
          logInfo('尝试使用FFplay播放当前歌曲')
          await playSong(currentSong.value, currentPosition.value)
        }
        return
      }
      
      // 计算暂停的持续时间
      if (pauseStartTime.value) {
        const pauseDuration = (Date.now() - pauseStartTime.value) / 1000
        pausedDuration.value += pauseDuration
        logInfo('暂停持续时间:', pauseDuration, '秒，累计暂停时间:', pausedDuration.value, '秒')
        pauseStartTime.value = null
      }
      // 更新开始播放的时间，减去已经播放的时间
      playbackStartTime.value = Date.now() - (currentPosition.value * 1000)
      logInfo('恢复播放，更新播放开始时间:', playbackStartTime.value)
      
      // 恢复前端音频元素
      try {
        await audioElement.value.play()
        logInfo('✅ 恢复播放成功')
        isPlaying.value = true
      } catch (playError) {
        logError('恢复播放失败:', playError)
        // 忽略中断错误和自动播放策略错误
        if (String(playError).includes('interrupted') || String(playError).includes('AbortError')) {
          logInfo('播放被中断，忽略错误')
          return
        }
        if (String(playError).includes('autoplay') || String(playError).includes('NotAllowedError')) {
          logInfo('自动播放策略限制，可能需要用户交互')
          return
        }
        throw playError
      }
    }
    
    logInfo('togglePlayback 完成,新 isPlaying:', isPlaying.value)
  } catch (error) {
    logError('切换播放状态失败:', error)
    const errorMessage = error && typeof error === 'object' && 'message' in error 
      ? (error as Error).message 
      : (error && typeof error === 'string' ? error : '未知错误')
    console.error('详细错误信息:', error)
    
    // 如果错误是FFplay回退相关的错误，不显示错误提示，让回退逻辑处理
    if (errorMessage.includes('HTML5 Audio') || 
        errorMessage.includes('no supported source') ||
        errorMessage.includes('MEDIA_ERR_') ||
        errorMessage.includes('Failed to load')) {
      logInfo('音频加载失败，已触发FFplay回退，不显示错误提示')
      // 不显示错误提示，让FFplay回退逻辑处理
      return
    }
    
    alert(`播放控制失败：${errorMessage}`)
  } finally {
    // 释放锁
    isToggling = false
    logInfo('播放操作锁已释放')
  }
}

const playPrevious = async () => {
  try {
    if (songs.value.length === 0) return
    
    let currentIndex = songs.value.findIndex(song => song.id === currentSong.value?.id)
    
    // 如果当前播放进度超过3秒,重新播放当前歌曲
    if (currentPosition.value > 3 && currentSong.value) {
      await playSong(currentSong.value, 0, undefined, undefined, false)
      return
    }
    
    if (currentIndex === -1) {
      // 如果当前没有播放歌曲，播放最后一首
      currentIndex = songs.value.length - 1
    } else {
      // 播放上一首
      currentIndex = (currentIndex - 1 + songs.value.length) % songs.value.length
    }
    
    await playSong(songs.value[currentIndex], 0, undefined, undefined, false)
    logInfo('已跳到上一首，保持暂停状态')
  } catch (error) {
    logError('播放上一首失败:', error)
  }
}

// 封面背景样式（计算属性）
const coverBackgroundStyle = computed(() => {
  if (currentSong.value?.cover) {
    return {
      backgroundImage: `url(${currentSong.value.cover})`,
    }
  }
  return {}
})

// 下一首歌曲（计算属性，只在依赖变化时更新）
const nextSong = computed<Song | null>(() => {
  if (songs.value.length === 0 || !currentSong.value) return null
  
  // 随机播放模式且已预先确定下一首
  if (playbackMode.value === 'random' && randomNextIndex.value !== null) {
    if (randomNextIndex.value >= 0 && randomNextIndex.value < songs.value.length) {
      return songs.value[randomNextIndex.value]
    }
  }
  
  let currentIndex = songs.value.findIndex(song => song.id === currentSong.value?.id)
  
  if (currentIndex === -1) {
    // 如果当前歌曲不在列表中，返回第一首
    return songs.value[0]
  }
  
  // 顺序播放或循环播放
  if (playbackMode.value === 'repeat' && currentIndex === songs.value.length - 1) {
    // 列表循环，回到开头
    return songs.value[0]
  }
  
  // 返回下一首
  return songs.value[(currentIndex + 1) % songs.value.length]
})

// 跳过下一首（删除下一首歌曲或移动到列表末尾）
const skipNextSong = () => {
  const songToSkip = nextSong.value
  if (!songToSkip) return
  
  // 找到下一首歌曲的索引
  const nextIndex = songs.value.findIndex(song => song.id === songToSkip.id)
  if (nextIndex === -1) return
  
  // 将下一首歌曲移动到列表末尾
  const [skippedSong] = songs.value.splice(nextIndex, 1)
  songs.value.push(skippedSong)
  
  logInfo('已跳过下一首:', skippedSong.title)
}

// 自动滚动到当前播放歌曲
const scrollToCurrentSong = () => {
  // 使用 nextTick 确保 DOM 更新后再滚动
  nextTick(() => {
    // 查找当前播放的歌曲元素
    const currentSongElement = document.querySelector('.song-row.active')
    if (currentSongElement) {
      // 滚动到视图中央
      currentSongElement.scrollIntoView({
        behavior: 'smooth',
        block: 'center'
      })
      logInfo('已滚动到当前播放歌曲')
    }
  })
}

const playNext = async () => {
  try {
    logInfo('playNext 被调用', {
      songsCount: songs.value.length,
      currentSong: currentSong.value?.title,
      playbackMode: playbackMode.value
    })

    if (songs.value.length === 0) {
      logInfo('歌曲列表为空,无法播放下一首')
      return
    }

    let targetIndex = 0

    if (playbackMode.value === 'random') {
      // 随机播放 - 使用预先确定的下一首
      if (randomNextIndex.value !== null && randomNextIndex.value >= 0 && randomNextIndex.value < songs.value.length) {
        targetIndex = randomNextIndex.value
        logInfo('随机播放模式,使用预先确定的索引:', targetIndex)
      } else {
        // 如果没有预先确定，随机选择一首
        targetIndex = Math.floor(Math.random() * songs.value.length)
        logInfo('随机播放模式,实时选择索引:', targetIndex)
      }
    } else {
      // 顺序播放或循环播放
      let currentIndex = songs.value.findIndex(song => song.id === currentSong.value?.id)

      if (currentIndex === -1) {
        // 如果当前没有播放歌曲，播放第一首
        targetIndex = 0
        logInfo('当前歌曲未找到,播放第一首')
      } else {
        // 播放下一首
        if (playbackMode.value === 'repeat' && currentIndex === songs.value.length - 1) {
          // 列表循环,回到开头
          targetIndex = 0
          logInfo('列表循环模式,回到开头')
        } else {
          targetIndex = (currentIndex + 1) % songs.value.length
          logInfo('顺序播放,下一首索引:', targetIndex)
        }
      }
    }

    logInfo('准备播放下一首:', songs.value[targetIndex].title)
    await playSong(songs.value[targetIndex], 0, undefined, undefined, true)
    logInfo('已跳到下一首并开始播放')
  } catch (error) {
    logError('播放下一首失败:', error)
  }
}

const changePlaybackMode = () => {
  const modes: Array<'order' | 'random' | 'repeat'> = ['order', 'random', 'repeat']
  const currentIndex = modes.indexOf(playbackMode.value)
  playbackMode.value = modes[(currentIndex + 1) % modes.length]
}

// 歌曲显示相关纯函数（getFileNameWithoutExtension / extractInfoFromFileName /
// getDisplayTitle / getDisplayArtist / getDisplayAlbum）已移至 utils/songDisplay.ts

// toggleRepeat 函数已移除，播放模式切换通过 changePlaybackMode 函数实现

const toggleMute = async () => {
  isMuted.value = !isMuted.value
  if (isMuted.value) {
    // 静音：保存当前音量并设置为0
    previousVolume.value = volume.value
    volume.value = 0
  } else {
    // 取消静音：恢复之前的音量
    volume.value = previousVolume.value || 80
  }
  await updateVolume()
}

const updateVolume = async () => {
  try {
    // 控制前端音频元素的音量
    if (audioElement.value) {
      audioElement.value.volume = volume.value / 100 // 转换为0-1范围
    }
  } catch (error) {
    logError('设置音量失败:', error)
  }
}

// 记录拖动前的播放状态
let wasPlayingBeforeSeek = false

const handleSeeking = () => {
  // 用户正在拖动进度条
  console.log('【SEEK】========== handleSeeking 触发 ==========')
  console.log('【SEEK】progress.value:', progress.value)
  console.log('【SEEK】isSeeking.value:', isSeeking.value)
  console.log('【SEEK】isPlaying.value:', isPlaying.value)
  console.log('【SEEK】isFFplayPlaying.value:', isFFplayPlaying.value)
  console.log('【SEEK】audioElement.value:', audioElement.value)
  if (audioElement.value) {
    console.log('【SEEK】audioElement.currentTime:', audioElement.value.currentTime)
    console.log('【SEEK】audioElement.duration:', audioElement.value.duration)
    console.log('【SEEK】audioElement.paused:', audioElement.value.paused)
  }
  logInfo('用户正在拖动进度条, progress.value:', progress.value)

  // 记录拖动前的播放状态
  if (!isSeeking.value) {
    wasPlayingBeforeSeek = isPlaying.value
    logInfo('记录拖动前播放状态:', wasPlayingBeforeSeek)
    console.log('【SEEK】设置 wasPlayingBeforeSeek =', wasPlayingBeforeSeek)
  }

  // 无论是否有音频元素，都设置isSeeking为true
  isSeeking.value = true
  console.log('【SEEK】设置 isSeeking.value = true')
  console.log('【SEEK】========== handleSeeking 结束 ==========')
}

// seek函数：处理进度条定位
const seek = async () => {
  console.log('【SEEK】========== seek 函数开始 ==========')
  console.log('【SEEK】progress.value:', progress.value, '%')
  console.log('【SEEK】isPlaying.value:', isPlaying.value)
  console.log('【SEEK】isFFplayPlaying.value:', isFFplayPlaying.value)
  console.log('【SEEK】isSeeking.value:', isSeeking.value)
  console.log('【SEEK】wasPlayingBeforeSeek:', wasPlayingBeforeSeek)
  console.log('【SEEK】currentSong.value:', currentSong.value)
  console.log('【SEEK】audioElement.value:', audioElement.value)
  logInfo('【SEEK】seek函数被调用, progress.value:', progress.value, '%, isPlaying:', isPlaying.value)

  // 通道判断必须以“当前实际播放通道”为准（isFFplayPlaying），
  // 不能再按文件扩展名重新猜测：m4a/aac 由 HTML5 Audio 播放，
  // 误判为 FFplay 会导致去 invoke 不存在的 ffplay 进程，且跳过
  // audioElement.currentTime 设置，表现为拖动进度条无效。
  const shouldUseFFplay = isFFplayPlaying.value
  console.log('【SEEK】shouldUseFFplay:', shouldUseFFplay)

  // 如果应该使用FFplay播放，使用FFplay的seek功能（带防抖）
  if (shouldUseFFplay && currentSong.value) {
    console.log('【SEEK】使用FFplay seek（带防抖）')
    
    try {
      // 解析时长格式 "mm:ss"
      const parts = currentSong.value.duration.split(':')
      if (parts.length === 2) {
        const minutes = parseInt(parts[0])
        const seconds = parseInt(parts[1])
        const totalSeconds = minutes * 60 + seconds
        
        if (totalSeconds > 0) {
          // 计算目标位置（秒）
          const clampedProgress = Math.min(Math.max(progress.value, 0), 100)
          const relativePosition = (clampedProgress / 100) * totalSeconds
          let actualPosition = relativePosition
          
          // 对于CUE track，将相对位置转换为绝对位置
          if (currentSong.value.isCueTrack && currentSong.value.startTime) {
            const startTimeNum = Number(currentSong.value.startTime)
            if (!isNaN(startTimeNum) && startTimeNum >= 0) {
              actualPosition = startTimeNum + relativePosition
              
              // 确保不超出CUE track的范围
              if (currentSong.value.endTime) {
                const endTimeNum = Number(currentSong.value.endTime)
                if (!isNaN(endTimeNum) && endTimeNum > 0 && actualPosition > endTimeNum) {
                  actualPosition = endTimeNum
                }
              }
              if (actualPosition < startTimeNum) {
                actualPosition = startTimeNum
              }
            }
          }
          
          // 使用防抖机制调用FFplay seek
          debouncedSeek(actualPosition)
          
          // 更新前端进度变量（不等待后端响应）
          playbackStartTime.value = Date.now() - (actualPosition * 1000)
          currentPosition.value = actualPosition
          frontendPosition = actualPosition // 更新前端计算的播放位置
          progress.value = clampedProgress
          ffplayPosition.value = actualPosition

          // 确保isFFplayPlaying.value为true
          isFFplayPlaying.value = true
          logInfo('【SEEK】FFplay seek完成: currentPosition=', actualPosition, 's, progress=', progress.value, '%');

          // 立即执行一次状态更新，确保前端能够立即获取到FFplay的状态
          (async () => {
            try {
              logInfo('立即执行FFplay状态更新')
              const status = await invoke('get_ffplay_status') as any
              logInfo('立即获取FFplay状态成功:', JSON.stringify(status))

              if (status) {
                logInfo('立即处理FFplay状态:', {
                  duration: status.duration,
                  position: status.position,
                  volume: status.volume,
                  is_playing: status.is_playing
                })

                ffplayDuration.value = status.duration || ffplayDuration.value
                ffplayPosition.value = status.position || ffplayPosition.value
                ffplayVolume.value = status.volume || ffplayVolume.value

                // 更新播放状态
                isPlaying.value = status.is_playing || false
                logInfo('isPlaying 立即更新为:', isPlaying.value, 'isFFplayPlaying:', isFFplayPlaying.value)

                // Seek进行中或正在拖动时，不允许状态轮询覆盖位置
                if (!seekInProgress && !isSeeking.value && status.position !== undefined && status.position !== null) {
                  logInfo('立即更新currentPosition前:', currentPosition.value, '更新后:', status.position)
                  currentPosition.value = status.position
                  frontendPosition = status.position
                  logInfo('立即更新播放进度:', currentPosition.value, '秒, 时长:', ffplayDuration.value)
                } else if (seekInProgress || isSeeking.value) {
                  logInfo('立即更新被跳过（Seek进行中或拖动中），保持位置:', currentPosition.value)
                }

                // 计算进度百分比
                if (ffplayDuration.value > 0) {
                  const newProgress = Math.min((currentPosition.value / ffplayDuration.value) * 100, 100)
                  logInfo('立即更新进度百分比前:', progress.value, '更新后:', newProgress)
                  progress.value = newProgress
                  logInfo('立即更新进度百分比:', progress.value, '%')
                }
              }
            } catch (error) {
              logError('立即获取FFplay状态失败:', error)
            }
          })()

          // 确保FFplay状态监控定时器正在运行
          if (!ffplayStatusInterval) {
            logInfo('FFplay状态监控定时器未运行，启动一个新的')
            // 启动FFplay状态监控定时器
            ffplayStatusInterval = window.setInterval(() => {
              logInfo('FFplay状态监控定时器触发');
              // 使用IIFE包装async函数
              (async () => {
                try {
                  logInfo('准备调用get_ffplay_status')
                  const status = await invoke('get_ffplay_status') as any
                  logInfo('获取FFplay状态成功:', JSON.stringify(status))

                  if (status) {
                    logInfo('处理FFplay状态:', {
                      duration: status.duration,
                      position: status.position,
                      volume: status.volume,
                      is_playing: status.is_playing
                    })

                    ffplayDuration.value = status.duration || ffplayDuration.value
                    ffplayPosition.value = status.position || ffplayPosition.value
                    ffplayVolume.value = status.volume || ffplayVolume.value

                    // 更新播放状态
                    isPlaying.value = status.is_playing || false
                    logInfo('isPlaying 更新为:', isPlaying.value, 'isFFplayPlaying:', isFFplayPlaying.value)

                    // 只有当status.position有效且Seek未在进行中、用户未在拖动时才更新currentPosition
                    // 防止状态轮询覆盖拖动和Seek后的位置
                    if (!seekInProgress && !isSeeking.value && status.position !== undefined && status.position !== null) {
                      logInfo('更新currentPosition前:', currentPosition.value, '更新后:', status.position)
                      currentPosition.value = status.position
                      // 更新前端计算的播放位置
                      frontendPosition = status.position
                      logInfo('更新播放进度:', currentPosition.value, '秒, 时长:', ffplayDuration.value)
                    } else if (seekInProgress || isSeeking.value) {
                      logInfo('Seek进行中或拖动中，跳过位置更新:', seekInProgress, isSeeking.value)
                    }

                    // 计算进度百分比
                    if (ffplayDuration.value > 0) {
                      const newProgress = Math.min((currentPosition.value / ffplayDuration.value) * 100, 100)
                      logInfo('更新进度百分比前:', progress.value, '更新后:', newProgress)
                      progress.value = newProgress
                      logInfo('更新进度百分比:', progress.value, '%')
                    }
                  }
                } catch (error) {
                  logError('获取FFplay状态失败:', error)
                }
              })()
            }, 500) // 每500毫秒更新一次状态，提高响应速度
          } else {
            logInfo('FFplay状态监控定时器已经在运行，不需要重新启动')
          }
        }
      }

      isSeeking.value = false
      wasPlayingBeforeSeek = false
      logInfo('========== seek 函数结束（FFplay） ==========')
      return
    } catch (error) {
      logError('【SEEK】FFplay seek失败:', error)
      isSeeking.value = false
      wasPlayingBeforeSeek = false
      return
    }
  }

  // 保存对 audioElement 的引用，防止在 seek 过程中被清理
  const audioElementRef = audioElement.value
  if (!audioElementRef) {
    logInfo('【SEEK】没有音频元素，无法定位')
    
    // 即使没有音频元素，也要更新播放状态和进度
    if (currentSong.value) {
      // 解析时长格式 "mm:ss"
      const parts = currentSong.value.duration.split(':')
      if (parts.length === 2) {
        const minutes = parseInt(parts[0])
        const seconds = parseInt(parts[1])
        const totalSeconds = minutes * 60 + seconds
        
        if (totalSeconds > 0) {
          // 计算目标位置（秒）
          const clampedProgress = Math.min(Math.max(progress.value, 0), 100)
          const relativePosition = (clampedProgress / 100) * totalSeconds
          let actualPosition = relativePosition
          
          // 对于CUE track，将相对位置转换为绝对位置
          if (currentSong.value.isCueTrack && currentSong.value.startTime) {
            const startTimeNum = Number(currentSong.value.startTime)
            if (!isNaN(startTimeNum) && startTimeNum >= 0) {
              actualPosition = startTimeNum + relativePosition
              
              // 确保不超出CUE track的范围
              if (currentSong.value.endTime) {
                const endTimeNum = Number(currentSong.value.endTime)
                if (!isNaN(endTimeNum) && endTimeNum > 0 && actualPosition > endTimeNum) {
                  actualPosition = endTimeNum
                }
              }
              if (actualPosition < startTimeNum) {
                actualPosition = startTimeNum
              }
            }
          }
          
          // 更新进度相关变量
          playbackStartTime.value = Date.now() - (actualPosition * 1000)
          currentPosition.value = actualPosition
          progress.value = clampedProgress
          logInfo('【SEEK】没有音频元素，更新进度变量: currentPosition=', actualPosition, 's, progress=', progress.value, '%')
        }
      }
    }
    
    // 重置标志，防止播放状态被锁定
    isSeeking.value = false
    wasPlayingBeforeSeek = false
    return
  }

  try {
    // 如果没有当前歌曲，不执行定位
    if (!currentSong.value) {
      console.log('【SEEK】❌ 没有当前歌曲，不执行定位')
      logInfo('【SEEK】没有当前歌曲，不执行定位')
      return
    }

    console.log('【SEEK】currentSong.duration:', currentSong.value.duration)
    console.log('【SEEK】currentSong.isCueTrack:', currentSong.value.isCueTrack)

    // 如果时长为"未知"，不允许拖动进度条
    if (currentSong.value.duration === '未知') {
      console.log('【SEEK】❌ 时长为未知，不允许拖动进度条')
      logInfo('【SEEK】时长为未知，不允许拖动进度条')
      return
    }

    // 解析时长格式 "mm:ss"
    const parts = currentSong.value.duration.split(':')
    console.log('【SEEK】解析时长 parts:', parts)
    if (parts.length !== 2) {
      console.log('【SEEK】❌ 时长格式错误:', currentSong.value.duration)
      logInfo('【SEEK】时长格式错误:', currentSong.value.duration)
      return
    }

    const minutes = parseInt(parts[0])
    const seconds = parseInt(parts[1])
    const totalSeconds = minutes * 60 + seconds
    console.log('【SEEK】totalSeconds:', totalSeconds)

    if (totalSeconds <= 0) {
      console.log('【SEEK】❌ 总时长为0或负数:', totalSeconds)
      logInfo('【SEEK】总时长为0或负数:', totalSeconds)
      return
    }

    // 如果还没有记录拖动前的播放状态（比如直接点击进度条），现在记录
    if (!isSeeking.value) {
      wasPlayingBeforeSeek = isPlaying.value
      console.log('【SEEK】直接点击进度条，设置 wasPlayingBeforeSeek =', wasPlayingBeforeSeek)
      logInfo('【SEEK】记录播放状态:', wasPlayingBeforeSeek)
      isSeeking.value = true
    }

    // 计算目标位置（秒）
    const clampedProgress = Math.min(Math.max(progress.value, 0), 100)
    const relativePosition = (clampedProgress / 100) * totalSeconds
    let actualPosition = relativePosition
    console.log('【SEEK】计算位置: clampedProgress =', clampedProgress, '%, relativePosition =', relativePosition, 's')

    // 对于CUE track，将相对位置转换为绝对位置
    if (currentSong.value.isCueTrack && currentSong.value.startTime) {
      console.log('【SEEK】处理 CUE track')
      const startTimeNum = Number(currentSong.value.startTime)
      console.log('【SEEK】startTimeNum:', startTimeNum)
      if (isNaN(startTimeNum) || startTimeNum < 0) {
        console.log('【SEEK】❌ CUE track: 无效的startTime值:', currentSong.value.startTime)
        logInfo('【SEEK】CUE track: 无效的startTime值:', currentSong.value.startTime)
        return
      }
      actualPosition = startTimeNum + relativePosition
      console.log('【SEEK】CUE track 计算后 actualPosition =', actualPosition, 's')

      // 确保不超出CUE track的范围
      if (currentSong.value.endTime) {
        const endTimeNum = Number(currentSong.value.endTime)
        console.log('【SEEK】endTimeNum:', endTimeNum)
        if (!isNaN(endTimeNum) && endTimeNum > 0 && actualPosition > endTimeNum) {
          actualPosition = endTimeNum
          console.log('【SEEK】CUE track 限制到结束时间:', actualPosition)
        }
      }
      if (actualPosition < startTimeNum) {
        actualPosition = startTimeNum
        console.log('【SEEK】CUE track 限制到开始时间:', actualPosition)
      }
      logInfo('【SEEK】CUE track: 相对位置=', relativePosition, 's, 开始时间=', startTimeNum, 's, 绝对位置=', actualPosition, 's')
    } else {
      console.log('【SEEK】普通歌曲')
      logInfo('【SEEK】普通歌曲: 绝对位置=', actualPosition, 's')
    }

    // 检查音频元素是否仍然存在
    if (!audioElementRef) {
      console.log('【SEEK】❌ audioElement 被清理了，无法定位')
      logInfo('【SEEK】audioElement 被清理了，无法定位')
      return
    }

    console.log('【SEEK】audioElementRef.currentTime (设置前):', audioElementRef.currentTime)
    console.log('【SEEK】audioElementRef.duration:', audioElementRef.duration)
    console.log('【SEEK】audioElementRef.paused:', audioElementRef.paused)
    console.log('【SEEK】audioElementRef.readyState:', audioElementRef.readyState)
    console.log('【SEEK】audioElementRef.networkState:', audioElementRef.networkState)
    console.log('【SEEK】audioElementRef.src:', audioElementRef.src)

    // 暂时移除timeupdate事件监听器，防止在seek过程中干扰
    if (timeupdateHandler && audioElementRef) {
      console.log('【SEEK】移除 timeupdate 事件监听器')
      audioElementRef.removeEventListener('timeupdate', timeupdateHandler)
    }

    // 更新进度相关变量
    playbackStartTime.value = Date.now() - (actualPosition * 1000)
    currentPosition.value = actualPosition
    progress.value = clampedProgress
    console.log('【SEEK】更新变量: playbackStartTime =', playbackStartTime.value, ', currentPosition =', currentPosition.value, ', progress =', progress.value, '%')
    logInfo('【SEEK】更新完成: playbackStartTime=', playbackStartTime.value, ', currentPosition=', currentPosition.value, ', progress=', progress.value, '%')

    // 设置音频元素的当前位置
    console.log('【SEEK】准备设置 audioElementRef.currentTime =', actualPosition, 's')
    logInfo('【SEEK】设置 audioElement.currentTime =', actualPosition, 's')

    // 直接设置currentTime，不暂停（因为暂停会导致HTTP Range请求问题）
    console.log('【SEEK】直接设置 currentTime（不暂停）')
    audioElementRef.currentTime = actualPosition

    // 等待一小段时间让浏览器处理
    await new Promise(resolve => setTimeout(resolve, 100))

    console.log('【SEEK】audioElementRef.currentTime (设置后 100ms):', audioElementRef.currentTime)

    // 如果currentTime设置成功，就继续
    if (audioElementRef.currentTime > 0) {
      console.log('【SEEK】✅ currentTime 设置成功！')
    } else {
      console.log('【SEEK】⚠️ currentTime 仍然是 0，可能需要检查服务器配置')
    }

    // 如果之前在播放，确保继续播放
    console.log('【SEEK】检查是否需要恢复播放: wasPlayingBeforeSeek =', wasPlayingBeforeSeek)
    if (wasPlayingBeforeSeek) {
      console.log('【SEEK】尝试恢复播放')
      try {
        console.log('【SEEK】调用 audioElementRef.play()')
        await audioElementRef.play()
        console.log('【SEEK】✅ 恢复播放成功')
        logInfo('【SEEK】恢复播放成功')
        isPlaying.value = true
        console.log('【SEEK】设置 isPlaying.value = true')
      } catch (playError) {
        console.log('【SEEK】❌ 恢复播放失败:', playError)
        logInfo('【SEEK】恢复播放失败:', playError)
      }
    } else {
      console.log('【SEEK】不需要恢复播放（之前不在播放）')
    }

    // 重新添加timeupdate事件监听器
    if (timeupdateHandler && audioElementRef) {
      console.log('【SEEK】重新添加 timeupdate 事件监听器')
      audioElementRef.addEventListener('timeupdate', timeupdateHandler)
    }

    // 重置标志
    isSeeking.value = false
    wasPlayingBeforeSeek = false
    console.log('【SEEK】重置标志: isSeeking = false, wasPlayingBeforeSeek = false')
    logInfo('【SEEK】定位完成, isSeeking已重置')
    console.log('【SEEK】========== seek 函数结束 ==========')

  } catch (error) {
    console.log('【SEEK】❌ 定位失败，错误:', error)
    logError('【SEEK】定位失败:', error)

    // 出错时也要重新添加timeupdate事件监听器
    if (timeupdateHandler && audioElementRef) {
      console.log('【SEEK】出错时重新添加 timeupdate 事件监听器')
      audioElementRef.addEventListener('timeupdate', timeupdateHandler)
    }

    isSeeking.value = false
    wasPlayingBeforeSeek = false
  }
}

const scrollToTop = () => {
  console.log('scrollToTop 函数被调用')
  console.log('songListContainer.value:', songListContainer.value)
  if (songListContainer.value) {
    // 尝试多种方式找到实际的滚动容器
    let scrollableContainer = songListContainer.value.querySelector('.song-list') as HTMLElement
    console.log('找到 .song-list:', scrollableContainer)
    
    // 如果没有找到 .song-list，或者 .song-list 没有滚动条，则使用 songListContainer 本身
    if (!scrollableContainer || scrollableContainer.scrollHeight <= scrollableContainer.clientHeight) {
      // 检查 songListContainer 本身是否可滚动
      if (songListContainer.value.scrollHeight > songListContainer.value.clientHeight) {
        scrollableContainer = songListContainer.value
        console.log('使用 songListContainer 作为滚动容器')
      } else {
        // 尝试查找其他可能的滚动容器
        const allScrollable = songListContainer.value.querySelectorAll('*')
        for (let i = 0; i < allScrollable.length; i++) {
          const el = allScrollable[i] as HTMLElement
          if (el.scrollHeight > el.clientHeight && el.clientHeight > 100) {
            scrollableContainer = el
            console.log('找到其他滚动容器:', el.className)
            break
          }
        }
      }
    }
    
    if (scrollableContainer) {
      console.log('执行滚动到顶部操作, scrollHeight:', scrollableContainer.scrollHeight)
      scrollableContainer.scrollTo({
        top: 0,
        behavior: 'smooth'
      })
    } else {
      console.log('未找到可滚动的容器')
    }
  } else {
    console.log('songListContainer.value 为 null，无法执行滚动操作')
  }
}

const toggleEqualizer = () => {
  equalizerVisible.value = !equalizerVisible.value
}

// 均衡器频段标签与预设数据已移至 utils/equalizer.ts

const applyPreset = async () => {
  try {
    const preset = getEqPreset(currentPreset.value)
    if (preset) {
      equalizerBands.value = preset
    } else {
      logError('未知的均衡器预设:', currentPreset.value)
    }
  } catch (error) {
    logError('应用均衡器预设失败:', error)
  }
}

const updateEqualizer = async () => {
  // 前端均衡器目前只存储配置，不实际应用到音频
  // Web Audio API的均衡器实现较为复杂，暂时仅保存配置
}

const handleSearch = () => {
  // 搜索逻辑已在computed属性中实现
  // 这里可以添加额外的搜索相关逻辑
}

// 歌曲右键菜单（由 useSongContextMenu composable 管理）
const {
  showSongMenu,
  menuPosition,
  selectedSong,
  openSongMenu,
  closeSongMenu
} = useSongContextMenu()

// 歌曲标签编辑（状态与动作由 useSongTagsEditor composable 管理）
const {
  showEditTagsModal,
  showOnlineMatchModal,
  editTagsForm,
  songToEdit,
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
} = useSongTagsEditor({
  songs,
  currentSong,
  closeSongMenu,
  logInfo,
  logError
})

// 滚动到当前歌词（封面模态框）
const scrollToCurrentLyric = () => {
  const index = currentLyricIndex.value
  logInfo('封面歌词滚动: 尝试滚动到歌词行', index, 'coverLyricsContainer:', !!coverLyricsContainer.value, 'coverLyricLineRefs:', coverLyricLineRefs.value.length)

  if (index < 0 || !coverLyricsContainer.value) {
    logInfo('封面歌词滚动: 条件不满足，index=', index, 'container=', !!coverLyricsContainer.value)
    return
  }

  // 尝试从 ref 获取元素
  let lineElement = coverLyricLineRefs.value[index]

  // 如果 ref 不存在，尝试使用 querySelector 作为备用方案
  if (!lineElement) {
    logInfo('封面歌词滚动: ref 不存在，尝试使用 querySelector')
    const allLines = coverLyricsContainer.value.querySelectorAll('.cover-lyric-line')
    if (allLines[index]) {
      lineElement = allLines[index] as any
      logInfo('封面歌词滚动: 通过 querySelector 找到元素')
    }
  }

  if (lineElement && coverLyricsContainer.value) {
    const container = coverLyricsContainer.value
    const lineTop = (lineElement as HTMLElement).offsetTop
    const lineHeight = (lineElement as HTMLElement).offsetHeight
    const containerHeight = container.clientHeight
    const scrollTop = lineTop - containerHeight / 2 + lineHeight / 2

    logInfo('封面歌词滚动: lineTop=', lineTop, 'lineHeight=', lineHeight, 'containerHeight=', containerHeight, 'scrollTop=', scrollTop)

    container.scrollTo({
      top: Math.max(0, scrollTop),
      behavior: 'smooth'
    })
    logInfo('封面歌词滚动: 成功滚动到歌词行', index)
  } else {
    logInfo('封面歌词滚动: 无法获取歌词行元素')
    if (coverLyricsContainer.value) {
      logInfo('封面歌词滚动: 所有行数:', coverLyricsContainer.value.querySelectorAll('.cover-lyric-line').length)
    }
  }
}

// 封面模态框窗口行为（开关/全屏/拖拽由 useCoverModal composable 管理）
const {
  showCoverModal,
  isCoverModalFullscreen,
  coverModalPosition,
  coverModalContent,
  openCoverModal,
  closeCoverModal,
  toggleCoverModalFullscreen,
  startDragCoverModal
} = useCoverModal({
  currentSong,
  onOpened: scrollToCurrentLyric,
  logInfo
})

// 歌曲库管理动作（收藏/加歌单/删除/建歌单由 useLibraryActions composable 管理）
const {
  toggleFavorite,
  showAddToPlaylistMenu,
  addMenuPosition,
  openAddToPlaylistMenu,
  addToPlaylist,
  createPlaylistAndAdd,
  removeSongFromLibrary,
  deleteSong,
  createPlaylist
} = useLibraryActions({
  songs,
  favorites,
  playlists,
  closeSongMenu,
  logError
})

// 侧边栏"创建歌单"：创建后自动进入新歌单
const handleCreatePlaylist = async () => {
  const newPlaylist = await createPlaylist()
  if (newPlaylist) {
    openPlaylist(newPlaylist.id)
  }
}

// 多选操作（勾选、Ctrl/点击行、批量播放/加歌单/删除由 useSongSelection 管理）
const {
  selectedSongIds,
  isSelectionMode,
  handleSongRowClick,
  toggleSongSelection,
  isSongSelected,
  clearSelection,
  addSelectedToPlaylist,
  playSelectedSongs,
  deleteSelectedSongs
} = useSongSelection({
  visibleSongs: filteredSongs,
  playSong,
  openAddToPlaylistMenu,
  // 批量删除注入无确认版本：批删本身已有一次总确认，不再逐首弹框
  deleteSong: removeSongFromLibrary,
  logInfo,
  logError
})

// formatTime 已移至 utils/format.ts；toSimpleLyricLines 已移至 utils/lyrics.ts

// 解析歌词
// parseLyrics 已移至 utils/lyrics.ts

// 同步歌词显示
const syncLyrics = () => {
  // 每10次同步输出一次日志，避免日志过多
  const callCount = (syncLyrics as any).callCount || 0;
  (syncLyrics as any).callCount = callCount + 1;
  
  if (lyrics.value.length === 0) {
    if (callCount % 50 === 0) {
      logInfo('歌词同步: 无歌词数据，showLyrics=', showLyrics.value)
    }
    return
  }
  
  const position = currentPosition.value
  let index = -1
  
  // 每10次同步输出一次详细日志
  const shouldLog = callCount % 10 === 0
  
  if (shouldLog) {
    logInfo('歌词同步: 当前播放位置', position.toFixed(1), '秒，歌词总数:', lyrics.value.length, '当前索引:', currentLyricIndex.value)
  }
  
  for (let i = 0; i < lyrics.value.length; i++) {
    if (lyrics.value[i].time <= position) {
      index = i
    } else {
      break
    }
  }
  
  if (index !== currentLyricIndex.value) {
    const prevIndex = currentLyricIndex.value
    currentLyricIndex.value = index

    if (index >= 0 && index < lyrics.value.length) {
      logInfo('歌词同步: 更新当前歌词索引从', prevIndex, '到', index, '文本:', lyrics.value[index].text)
    } else {
      logInfo('歌词同步: 更新当前歌词索引从', prevIndex, '到', index)
    }

    // 封面模态框歌词自动滚动到当前行
    logInfo('歌词同步: 封面模态框状态 showCoverModal=', showCoverModal.value, '当前索引=', index)
    if (showCoverModal.value && index >= 0) {
      logInfo('歌词同步: 将滚动封面歌词到行', index)
      nextTick(() => {
        scrollToCurrentLyric()
      })
    }
  }
}

// 标题栏窗口控制（由 useWindowControls composable 管理）
const { minimizeWindow, toggleMaximizeWindow, closeWindow } = useWindowControls({
  isBrowser,
  logError
})

// 预转码标志，防止重复预转码
let hasPretranscodedNextSong = false

const updateProgress = () => {
  try {
    // 如果未播放或正在拖动进度条,不更新进度
    if (!isPlaying.value || isSeeking.value) {
      if (isSeeking.value) {
        console.log('【UPDATE PROGRESS】跳过更新：isSeeking = true')
      } else {
        console.log('【UPDATE PROGRESS】跳过更新：isPlaying = false')
      }
      return
    }
    
    // 获取实际播放位置
    let actualPosition: number
    let positionInSeconds: number

    // 检查是否正在前端播放
    if (audioElement.value) {
      // 检查音频元素是否存在且currentTime有效
      if (!isNaN(audioElement.value.currentTime)) {
        // 从前端音频元素获取位置
        actualPosition = audioElement.value.currentTime
      } else {
        // 使用本地计算作为备用
        const now = Date.now()
        positionInSeconds = (now - playbackStartTime.value) / 1000 - pausedDuration.value
        actualPosition = positionInSeconds
      }

      // 对于CUE track，将绝对位置转换为相对位置
      if (currentSong.value && currentSong.value.isCueTrack && currentSong.value.startTime) {
        const startTimeNum = Number(currentSong.value.startTime)
        positionInSeconds = actualPosition - startTimeNum

        // 确保相对位置不小于0
        if (positionInSeconds < 0) {
          positionInSeconds = 0
        }
        // 确保相对位置不超过CUE track的长度
        if (currentSong.value.endTime) {
          const endTimeNum = Number(currentSong.value.endTime)
          const cueTrackDuration = endTimeNum - startTimeNum
          if (positionInSeconds > cueTrackDuration) {
            positionInSeconds = cueTrackDuration
          }
        }
      } else {
        // 对于普通歌曲，直接使用音频元素的位置
        positionInSeconds = actualPosition
      }
    } else {
      // 使用本地计算作为备用
      const now = Date.now()
      positionInSeconds = (now - playbackStartTime.value) / 1000 - pausedDuration.value
      actualPosition = positionInSeconds
    }
    
    if (currentSong.value) {
      // 如果时长为"未知"，只更新位置，不计算进度百分比
      if (currentSong.value.duration === '未知') {
        currentPosition.value = positionInSeconds
        progress.value = 0
        return
      }
      
      const parts = currentSong.value.duration.split(':')
      if (parts.length === 2) {
        const minutes = parseInt(parts[0])
        const seconds = parseInt(parts[1])
        const totalSeconds = minutes * 60 + seconds
        
        if (totalSeconds > 0) {
          // 确保位置值不超过总长度
          if (positionInSeconds > totalSeconds) {
            positionInSeconds = totalSeconds
          }
          
          // 计算进度百分比
          const calculatedProgress = (positionInSeconds / totalSeconds) * 100
          progress.value = Math.min(calculatedProgress, 100)
          
          // 预转码下一首歌曲（在剩余20秒时开始，且启用了转码功能）
          const remainingTime = totalSeconds - positionInSeconds
          if (enableTranscode.value && remainingTime <= 20 && !hasPretranscodedNextSong && nextSong.value) {
            hasPretranscodedNextSong = true
            
            // 在后台静默开始转码，不等待结果（仅桌面应用）
            if (!isBrowser.value) {
              invoke('pretranscode_audio', { path: nextSong.value.path, force_transcode: forceTranscode.value }).catch((error) => {
                logError('[预转码] 预转码失败:', error)
              })
            }
          }
        }
      }
    }
    
    // 直接赋值更新
    currentPosition.value = positionInSeconds

    // 同步歌词显示
    syncLyrics()
  } catch (error) {
    logError('更新进度失败:', error)
  }
}

// 播放完成标志，防止重复触发
let isPlaybackFinished = false

// 播放完成检测定时器ID
let playbackTimerId: ReturnType<typeof setTimeout> | null = null

const handlePlaybackFinished = async (autoPlay: boolean = true) => {
  // 防止重复触发
  if (isPlaybackFinished) {
    logInfo('前端 播放完成事件已处理,跳过')
    return
  }
  
  isPlaybackFinished = true
  
  // 立即清除播放完成检测定时器，防止重复触发
  if (playbackTimerId !== null) {
    clearTimeout(playbackTimerId)
    logInfo('前端 清除播放完成检测定时器')
    playbackTimerId = null
  }
  
  // 重置预转码标志，以便下一首歌曲播放时能够再次触发预转码
  hasPretranscodedNextSong = false
  logInfo('前端 重置预转码标志')
  
  // 立即设置播放状态为false,防止重复触发
  if (autoPlay) {
    isPlaying.value = false
    logInfo('前端 播放状态已设置为false')
  }

  logInfo('前端 播放完成,处理下一首', {
    playbackMode: playbackMode.value,
    autoPlayNext: autoPlayNext.value,
    crossfadeEnabled: crossfadeEnabled.value,
    crossfadeDuration: crossfadeDuration.value,
    currentSong: currentSong.value?.title,
    autoPlay: autoPlay
  })

  // 计算延迟时间：如果启用了交叉淡入淡出，延迟时间为淡出时间，否则为100ms
  const delay = (crossfadeEnabled.value && crossfadeDuration.value > 0) 
    ? crossfadeDuration.value * 1000 
    : 100

  logInfo('前端 播放完成延迟时间:', delay, 'ms')

  // 使用 setTimeout 确保状态更新后再处理下一首
  setTimeout(async () => {
    logInfo('前端 延迟后处理下一首')
    if (playbackMode.value === 'repeat') {
      // 单曲循环,重新播放当前歌曲
      logInfo('前端 单曲循环模式')
      if (currentSong.value) {
        isPlaybackFinished = false
        logInfo('前端 重新播放当前歌曲:', currentSong.value.title)
        await playSong(currentSong.value)
      }
    } else if (autoPlayNext.value) {
      // 播放下一首
      logInfo('前端 播放下一首')
      isPlaybackFinished = false
      await playNext()
    } else {
      logInfo('前端 自动播放下一首已禁用,停止播放')
      isPlaybackFinished = false
    }
  }, delay)
}


// 生命周期
onMounted(() => {
  try {
    // 初始化应用
    logInfo('TPlayer initialized')

    // 禁用右键菜单
    document.addEventListener('contextmenu', (e) => {
      e.preventDefault()
      e.stopPropagation()
      return false
    })

    // 禁用开发者工具快捷键 (可选,如果不需要开发工具可以启用)
    // document.addEventListener('keydown', (e) => {
    //   if (e.ctrlKey && e.shiftKey && e.key === 'I') {
    //     e.preventDefault()
    //     e.stopPropagation()
    //     return false
    //   }
    //   if (e.ctrlKey && e.shiftKey && e.key === 'J') {
    //     e.preventDefault()
    //     e.stopPropagation()
    //     return false
    //   }
    //   if (e.key === 'F12') {
    //     e.preventDefault()
    //     e.stopPropagation()
    //     return false
    //   }
    // })
    
    // 监听滚动事件
    nextTick(() => {
      try {
        if (songListContainer.value) {
          // 尝试找到实际的滚动容器并添加事件监听器
          const songList = songListContainer.value.querySelector('.song-list') as HTMLElement
          if (songList) {
            songList.addEventListener('scroll', handleScroll)
            console.log('滚动事件监听器已添加到 .song-list 元素')
          } else {
            songListContainer.value.addEventListener('scroll', handleScroll)
            console.log('滚动事件监听器已添加到 songListContainer 元素')
          }
          // 初始调用一次，设置初始状态
          handleScroll()
        }
      } catch (error) {
        logError('添加滚动事件监听器失败:', error)
      }
    })
  } catch (error) {
    logError('onMounted 初始化失败:', error)
  }
})

// 异步初始化
;(async () => {
  try {
    logInfo('【初始化】开始异步初始化')
    
    // 首先检测运行环境
    isBrowser.value = await checkIsBrowser()
    logInfo('【环境检测】最终结果 isBrowser:', isBrowser.value)
    
    // 加载保存的设置
    logInfo('【初始化】开始加载保存的设置')
    const savedSettings = await localStorageService.getSettings()
    logInfo('【初始化】加载到的设置:', savedSettings)
    volume.value = savedSettings.volume
    playbackMode.value = savedSettings.playbackMode
    currentPreset.value = savedSettings.equalizerPreset
    equalizerBands.value = savedSettings.equalizerBands
    theme.value = savedSettings.theme || 'dark'
    language.value = savedSettings.language || 'zh-CN'
    musicDirectory.value = savedSettings.musicDirectory || ''
    crossfadeEnabled.value = savedSettings.crossfadeEnabled ?? false
    crossfadeDuration.value = savedSettings.crossfadeDuration ?? 1
    autoPlayNext.value = savedSettings.autoPlayNext ?? true
    showLyrics.value = savedSettings.showLyrics ?? true
    lyricsPosition.value = (savedSettings as any).lyricsPosition || 'bottom'
    enableTranscode.value = savedSettings.enableTranscode ?? true
    forceTranscode.value = savedSettings.forceTranscode ?? false

    // 初始化语言服务
    logInfo('【初始化】开始初始化语言服务')
    await i18nService.initialize(language.value)
    logInfo('【初始化】语言服务初始化完成')

    // 加载保存的歌曲
    logInfo('【初始化】开始加载保存的歌曲')
    const savedSongs = await localStorageService.getSongs()
    logInfo('【初始化】加载到的歌曲数量:', savedSongs.length)
    if (savedSongs.length > 0) {
      songs.value = savedSongs as Song[]
      logInfo('【初始化】歌曲列表已更新')
    }
    
    // 加载歌单和收藏
    logInfo('【初始化】开始加载歌单和收藏')
    favorites.value = await localStorageService.getFavorites()
    playlists.value = await localStorageService.getPlaylists()
    logInfo('【初始化】收藏列表长度:', favorites.value.length)
    logInfo('【初始化】歌单列表长度:', playlists.value.length)
    
    // 更新歌曲的收藏状态
    songs.value.forEach(song => {
      song.isFavorite = favorites.value.includes(song.id)
    })
    logInfo('【初始化】歌曲收藏状态已更新')
    
    // 加载保存的播放进度
    logInfo('【初始化】开始加载保存的播放进度')
    const savedProgress = await localStorageService.getPlaybackProgress()
    logInfo('【初始化】加载到的播放进度:', savedProgress)
    if (savedProgress) {
      // 查找对应的歌曲
      const savedSong = songs.value.find(song => song.id === savedProgress.songId)
      logInfo('【初始化】找到的保存歌曲:', savedSong)
      if (savedSong) {
        currentSong.value = savedSong
        currentPosition.value = savedProgress.position
        isPlaying.value = savedProgress.isPlaying || false
        // 只有当时长不是"未知"时才计算进度百分比
        if (savedSong.duration && savedSong.duration !== '未知') {
          const parts = savedSong.duration.split(':')
          if (parts.length === 2) {
            const minutes = parseInt(parts[0])
            const seconds = parseInt(parts[1])
            const totalSeconds = minutes * 60 + seconds
            if (totalSeconds > 0) {
              progress.value = (savedProgress.position / totalSeconds) * 100
            } else {
              progress.value = 0
            }
          } else {
            progress.value = 0
          }
        } else {
          progress.value = 0
        }
        logInfo('【初始化】播放进度已恢复')
      }
    }

    // 监听播放完成事件
    if (window.__TAURI__?.event) {
      window.__TAURI__.event.listen('playback_finished', async () => {
        logInfo('播放完成,播放下一首')
        await handlePlaybackFinished()
      })

      // 监听系统托盘事件
      window.__TAURI__.event.listen('play-pause', () => {
        togglePlayback()
      })

      window.__TAURI__.event.listen('tray-next-song', () => {
        playNext()
      })

      window.__TAURI__.event.listen('tray-previous-song', () => {
        playPrevious()
      })
    }

    logInfo('前端 初始化完成')
    
    // 标记加载完成
    isLoading.value = false
    logInfo('【初始化】加载完成，isLoading设为false')
    
    // 初始化悬浮按钮状态
    nextTick(() => {
      handleScroll()
    })
    
    // 自动播放上次的歌曲或第一首歌曲
    nextTick(() => {
      setTimeout(async () => {
        try {
          if (currentSong.value && songs.value.length > 0) {
            // 播放上次的歌曲
            logInfo('【初始化】自动播放上次的歌曲:', currentSong.value.title)
            await playSong(currentSong.value, currentPosition.value)
            isPlaying.value = true
          } else if (songs.value.length > 0) {
            // 播放第一首歌曲
            logInfo('【初始化】自动播放第一首歌曲:', songs.value[0].title)
            await playSong(songs.value[0])
            isPlaying.value = true
          }
        } catch (error) {
          logError('【初始化】自动播放失败:', error)
        }
      }, 500)
    })
    
    // 检查是否是首次运行，如果是，则显示README.md文件
    nextTick(() => {
      try {
        const firstRun = localStorage.getItem('tplayer-first-run');
        if (firstRun === null && !isBrowser.value) {
          // 首次运行，显示README.md文件
          invoke('open_readme').catch(err => {
            logError('无法打开README.md文件:', err);
          });
          
          // 设置首次运行标志为false
          localStorage.setItem('tplayer-first-run', 'false');
        }
      } catch (e) {
        logError('检查首次运行状态失败:', e);
      }
    })
    
    // 绑定 F12 快捷键打开开发者工具
    document.addEventListener('keydown', (e) => {
      if (e.key === 'F12') {
        e.preventDefault();
        logInfo('F12 快捷键被按下，尝试打开开发者工具');
        // 使用 Tauri API 打开开发者工具
        invoke('open_devtools').catch(err => {
          logError('打开开发者工具失败:', err);
        });
      }
    })
  } catch (error) {
    logError('初始化失败:', error)
  }
})()

// 进度更新定时器
let progressTimer: number | null = null

// 监听播放状态变化，动态控制定时器
watch(isPlaying, (playing, oldPlaying) => {
  logInfo('【前端】isPlaying.value变化: 从', oldPlaying, '变为', playing)
  
  // 如果正在使用FFplay播放，不启动普通的进度更新定时器
  // FFplay有自己的进度更新定时器
  if (isFFplayPlaying.value) {
    logInfo('【前端】正在使用FFplay播放，跳过普通进度更新定时器')
    return
  }
  
  if (playing) {
    logInfo('【前端】播放状态变为true，启动进度更新定时器')
    if (!progressTimer) {
      progressTimer = window.setInterval(() => {
        updateProgress()
      }, 200)
      logInfo('【前端】进度更新定时器已启动，ID:', progressTimer)
    }
  } else {
    logInfo('【前端】播放状态变为false，停止进度更新定时器')
    if (progressTimer) {
      clearInterval(progressTimer)
      logInfo('【前端】进度更新定时器已停止，ID:', progressTimer)
      progressTimer = null
    }
  }
})

// 监听歌曲列表变化，自动保存
watch(songs, async (newSongs) => {
  try {
    // 确保数据是可克隆的
    const serializableSongs = JSON.parse(JSON.stringify(newSongs))
    await localStorageService.saveSongs(serializableSongs)
  } catch (error) {
    logError('保存歌曲列表失败:', error)
  }
}, { deep: true })

// 监听播放进度变化，自动保存
watch([currentSong, currentPosition, isPlaying], async ([newSong, newPosition, newPlaying]) => {
  try {
    if (newSong) {
      const progressData = {
        songId: newSong.id,
        position: newPosition,
        isPlaying: newPlaying,
        timestamp: new Date().toISOString() // 使用ISO字符串而不是Date对象
      }
      await localStorageService.savePlaybackProgress(progressData)
    }
  } catch (error) {
    logError('保存播放进度失败:', error)
  }
})

// 监听语言变化，更新语言服务
watch(language, async (newLanguage) => {
  await i18nService.changeLanguage(newLanguage)
  
  // 更新系统托盘菜单文本
  if (window.__TAURI__?.event) {
    try {
      const trayMenuTexts = {
        show: t('buttons.show'),
        next: t('buttons.next'),
        play_pause: t('buttons.playPause'),
        previous: t('buttons.previous'),
        quit: t('buttons.quit')
      }
      await (window.__TAURI__.event as any).emit('update-tray-menu', trayMenuTexts)
    } catch (error) {
      logError('更新系统托盘菜单失败:', error)
    }
  }
})

// 检查更新回调（由 Settings 触发，携带后端返回的更新信息）
const handleCheckUpdate = (info: any, version: string) => {
  showSettingsModal.value = false
  openUpdateModal(info, version)
}

// 浏览音乐目录（仅浏览器）
const browseMusicDirectory = () => {
  if (isBrowser.value) {
    // 在浏览器中，使用input[type="file"]来选择目录
    const input = document.createElement('input')
    input.type = 'file'
    input.webkitdirectory = true
    input.multiple = false
    
    input.onchange = (event) => {
      const target = event.target as HTMLInputElement
      if (target.files && target.files.length > 0) {
        const file = target.files[0]
        // 获取目录路径
        const directoryPath = file.webkitRelativePath ? file.webkitRelativePath.split('/')[0] : ''
        if (directoryPath) {
          musicDirectory.value = directoryPath
        }
      }
    }
    
    input.click()
  }
}

// 监听设置变化，自动保存
watch([volume, playbackMode, currentPreset, equalizerBands, theme, language, musicDirectory, crossfadeEnabled, crossfadeDuration, autoPlayNext, showLyrics, lyricsPosition, enableTranscode, forceTranscode], 
  async ([newVolume, newPlaybackMode, newPreset, newBands, newTheme, newLanguage, newMusicDirectory, newCrossfadeEnabled, newCrossfadeDuration, newAutoPlayNext, newShowLyrics, newLyricsPosition, newEnableTranscode, newForceTranscode]) => {
  try {
    // 确保所有数据都是可克隆的
    const serializableSettings = {
      theme: newTheme,
      language: newLanguage,
      musicDirectory: newMusicDirectory,
      volume: newVolume,
      playbackMode: newPlaybackMode,
      equalizerPreset: newPreset,
      equalizerBands: JSON.parse(JSON.stringify(newBands)), // 确保数组是可克隆的
      autoPlay: true, // 暂时固定为自动播放
      rememberProgress: true, // 暂时固定为记住进度
      crossfadeEnabled: newCrossfadeEnabled,
      crossfadeDuration: newCrossfadeDuration,
      autoPlayNext: newAutoPlayNext,
      showLyrics: newShowLyrics,
      lyricsPosition: newLyricsPosition,
      enableTranscode: newEnableTranscode,
      forceTranscode: newForceTranscode
    }
    await localStorageService.saveSettings(serializableSettings)
  } catch (error) {
    logError('保存设置失败:', error)
  }
})

// 当当前歌词行变化时，如果是封面模态框打开状态，自动滚动
watch(currentLyricIndex, (newIndex, oldIndex) => {
  if (showCoverModal.value && newIndex >= 0 && newIndex !== oldIndex) {
    logInfo('Watch currentLyricIndex: 索引变化', oldIndex, '->', newIndex, '封面模态框已打开，触发滚动')
    nextTick(() => {
      scrollToCurrentLyric()
    })
  }
})

// 当封面模态框打开/关闭时
watch(showCoverModal, (isOpen) => {
  if (isOpen && currentLyricIndex.value >= 0) {
    logInfo('Watch showCoverModal: 模态框打开，当前歌词索引=', currentLyricIndex.value, '触发滚动')
    // 等待 DOM 完全渲染后再滚动
    setTimeout(() => {
      nextTick(() => {
        scrollToCurrentLyric()
      })
    }, 100)
  }
})

// 当当前歌曲变化时，检测文本长度
watch(currentSong, () => {
  // 延迟检测，确保DOM已经更新
  setTimeout(() => {
    isTextLong('title')
    isTextLong('artist')
  }, 100)
})

onUnmounted(() => {
  // 清理资源
  logInfo('前端 开始清理资源')
  
  // 1. 清理进度更新定时器
  if (progressTimer) {
    clearInterval(progressTimer)
    progressTimer = null
    logInfo('前端 进度更新定时器已清理')
  }
  
  // 2. 清理 FFplay 状态轮询定时器
  if (ffplayStatusInterval) {
    clearInterval(ffplayStatusInterval)
    ffplayStatusInterval = null
    logInfo('前端 FFplay 状态轮询定时器已清理')
  }
  
  // 3. 清理 seek 防抖定时器
  if (seekDebounceTimer) {
    clearTimeout(seekDebounceTimer)
    seekDebounceTimer = null
    logInfo('前端 seek 防抖定时器已清理')
  }
  if (seekCompleteTimer) {
    clearTimeout(seekCompleteTimer)
    seekCompleteTimer = null
  }
  
  // 4. 清理播放定时器
  if (playbackTimerId) {
    clearTimeout(playbackTimerId)
    playbackTimerId = null
    logInfo('前端 播放定时器已清理')
  }
  
  // 5. 清理 audioElement 事件监听器
  if (audioElement.value) {
    const audio = audioElement.value
    // 移除所有事件监听器（通过 cloneNode 方式）
    // 或者使用特定的处理函数引用
    audio.pause()
    audio.src = ''
    logInfo('前端 audioElement 已清理')
  }
  
  // 6. 清理全局 document 事件监听器
  // 右键菜单与封面模态框的 document 监听由各自 composable 的 onUnmounted 自行清理
  
  // 7. 移除滚动事件监听器
  if (songListContainer.value) {
    const songList = songListContainer.value.querySelector('.song-list') as HTMLElement
    if (songList) {
      songList.removeEventListener('scroll', handleScroll)
    } else {
      songListContainer.value.removeEventListener('scroll', handleScroll)
    }
  }
  
  logInfo('前端 资源清理完成')
})
</script>

<style scoped src="./App.css"></style>
