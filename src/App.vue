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
          </ul>
        </nav>
        <div class="sidebar-footer">
          <button class="btn primary" @click="createPlaylist" :title="t('playlist.createPlaylist')">
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
                    :class="{ 'active': item.id === currentSong?.id }"
                    @click="playSong(item)"
                    @contextmenu.prevent="openSongMenu(item, $event)"
                  >
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
        <li @click="addSongToPlaylist(selectedSong!)">{{ t('menu.addToPlaylist') }}</li>
        <li @click="toggleFavorite(selectedSong!)">
          {{ selectedSong?.isFavorite ? t('menu.removeFromFavorites') : t('menu.addToFavorites') }}
        </li>
        <li @click="editSongTags(selectedSong!)">{{ t('menu.editTags') }}</li>
        <li @click="deleteSong(selectedSong!)" class="danger">{{ t('menu.delete') }}</li>
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
          <!-- 在线匹配 -->
          <div class="match-section">
            <span>{{ t('modal.dontWantToFill') }}</span>
            <button class="match-btn" @click="openOnlineMatch">
              {{ t('modal.onlineMatchTags') }}
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
                {{ t('modal.basicInfo') }}
              </button>
              <button 
                class="tab-button" 
                :class="{ active: activeTab === 'lyric' }"
                @click="activeTab = 'lyric'"
              >
                {{ t('modal.lyrics') }}
              </button>
              <button 
                class="tab-button" 
                :class="{ active: activeTab === 'cover' }"
                @click="activeTab = 'cover'"
              >
                {{ t('modal.cover') }}
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
              <div class="form-row">
                <div class="form-group">
                  <label>{{ t('modal.title') }}</label>
                  <input type="text" v-model="editTagsForm.title" :placeholder="t('modal.enterTitle')">
                </div>
              </div>
              <div class="form-row">
                <div class="form-group">
                  <label>{{ t('modal.artist') }}</label>
                  <input type="text" v-model="editTagsForm.artist" :placeholder="t('modal.enterArtist')">
                </div>
              </div>
              <div class="form-row">
                <div class="form-group">
                  <label>{{ t('modal.album') }}</label>
                  <input type="text" v-model="editTagsForm.album" :placeholder="t('modal.enterAlbum')">
                </div>
              </div>
              <div class="form-row">
                <div class="form-group">
                  <label>{{ t('modal.albumArtist') }}</label>
                  <input type="text" v-model="editTagsForm.albumArtist" :placeholder="t('modal.enterAlbumArtist')">
                </div>
              </div>
              <div class="form-row">
                <div class="form-group">
                  <label>{{ t('modal.genre') }}</label>
                  <input type="text" v-model="editTagsForm.genre" :placeholder="t('modal.enterGenre')">
                </div>
              </div>
              <div class="form-row three-col">
                <div class="form-group">
                  <label>{{ t('modal.year') }}</label>
                  <input type="text" v-model="editTagsForm.year" :placeholder="t('modal.enterYear')">
                </div>
                <div class="form-group">
                  <label>{{ t('modal.trackNumber') }}</label>
                  <input type="text" v-model="editTagsForm.trackNumber" :placeholder="t('modal.enterTrackNumber')">
                </div>
                <div class="form-group">
                  <label>{{ t('modal.discNumber') }}</label>
                  <input type="text" v-model="editTagsForm.discNumber" :placeholder="t('modal.enterDiscNumber')">
                </div>
              </div>
              <div class="form-row">
                <div class="form-group">
                  <label>{{ t('modal.alia') }}</label>
                  <input type="text" v-model="editTagsForm.alia" :placeholder="t('modal.enterAlia')">
                </div>
              </div>
              
              <!-- CUE信息区域 -->
              <div v-if="songToEdit && songToEdit.isCueTrack" class="cue-info-section">
                <h4>{{ t('modal.cueInfo') }}</h4>
                <div class="form-row">
                  <div class="form-group">
                    <label>{{ t('modal.trackNumber') }}</label>
                    <input type="text" v-model="editTagsForm.trackNumber" :placeholder="t('modal.enterTrackNumber')">
                  </div>
                </div>
                <div class="form-row">
                  <div class="form-group">
                    <label>{{ t('modal.startTime') }}</label>
                    <input type="number" v-model="songToEdit.startTime" :placeholder="t('modal.enterStartTime')">
                  </div>
                </div>
                <div class="form-row">
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
              <div class="lyric-actions">
                <button class="action-btn" @click="readLocalMetadata">{{ t('modal.readLocalMetadata') }}</button>
                <button class="action-btn" @click="autoMatchTags">{{ t('modal.matchFromFilename') }}</button>
                <button class="action-btn" @click="openOnlineMatch">{{ t('modal.onlineMatchTags') }}</button>
                <button class="action-btn" @click="fetchCover">{{ t('modal.getCover') }}</button>
              </div>
            </div>
            
            <!-- 歌词标签页 -->
            <div v-show="activeTab === 'lyric'" class="tab-content">
              <div class="form-group">
                <label>{{ t('modal.lyrics') }}</label>
                <textarea 
                  v-model="editTagsForm.lyric" 
                  placeholder="[00:00.00] Lyrics content" 
                  rows="10"
                ></textarea>
              </div>
              <div class="lyric-actions">
                <button class="action-btn" @click="fetchLyric">{{ t('modal.getLyrics') }}</button>
              </div>
            </div>
            
            <!-- 封面标签页 -->
            <div v-show="activeTab === 'cover'" class="tab-content">
              <div class="cover-section">
                <div class="cover-preview" @click="changeCover">
                  <img v-if="editTagsForm.cover" :src="editTagsForm.cover" alt="封面">
                  <div v-else class="cover-placeholder">{{ t('modal.clickToChangeCover') }}</div>
                </div>
                <div class="cover-actions">
                  <button class="action-btn" @click="changeCover">{{ t('modal.selectCover') }}</button>
                  <button class="action-btn" @click="fetchCover">{{ t('modal.getCover') }}</button>
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
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { isTauri } from '@tauri-apps/api/core'
import { localStorageService, type Playlist } from './stores/local'
import { matchSong, songLyric, searchSong, fetchLyricById } from './api/music'
import { musicDataService } from './services/musicDataService'
import { parseSmartLrc } from './services/lyricParser'
import * as mm from 'music-metadata'
import Settings from './components/Settings.vue'
import OnlineMatchModal from './components/OnlineMatchModal.vue'
import { i18nService, t } from './services/i18n'
import {
  cueAlbums,
  cueTracks,
  selectedCueAlbum,
  selectCueAlbum,
  getCueAlbumTracks,
  scanCueFiles
} from './composables/useCue'
import { exists } from '@tauri-apps/plugin-fs'
// RecycleScroller组件通过VueVirtualScroller插件注册

// 日志开关：设置为 false 可禁用所有日志输出
const ENABLE_LOGS = true

// 调试日志级别：0=无日志，1=仅错误，2=基本信息，3=详细信息
const LOG_LEVEL = 2

// 日志函数
function logInfo(...args: any[]) {
  // 输出所有日志
  if (ENABLE_LOGS) {
    console.log(...args)
  }
}

function logError(...args: any[]) {
  // 输出错误日志
  if (ENABLE_LOGS) {
    console.error(...args)
  }
}

// 详细日志函数（仅在LOG_LEVEL=3时输出）
function logDebug(...args: any[]) {
  // 禁用详细日志
  if (ENABLE_LOGS && LOG_LEVEL >= 3) {
    console.log(...args)
  }
}

// 类型定义 - 独立的Song接口，与local.ts中的Song兼容
interface Song {
  id: string
  title: string
  artist: string
  album: string
  path: string
  duration: string
  cover: string
  year: string
  genre: string
  lyric?: string
  isFavorite?: boolean
  isCueTrack?: boolean
  startTime?: string | number
  endTime?: string | number
  parentFile?: string
  trackNumber?: string
  cueInfo?: string
  dynamicCoverUrl?: string
  // 转码相关
  needs_transcode: boolean
  // 浏览器环境下的原始文件对象（仅浏览器环境使用）
  file?: File
}

// 歌词行类型
interface LyricLine {
  time: number // 时间戳（秒）
  text: string // 歌词内容
}

// 状态管理
const sidebarVisible = ref(true)
const equalizerVisible = ref(false)
const showFullControls = ref(false)
const showSongMenu = ref(false)
const menuPosition = ref({ left: '0px', top: '0px' })
const selectedSong = ref<Song | null>(null)
const showEditTagsModal = ref(false)
const showSettingsModal = ref(false)
const showCoverModal = ref(false)
const showOnlineMatchModal = ref(false)
const activeTab = ref('info')
const isLoading = ref(true) // 加载状态

// 歌词相关状态
const lyrics = ref<LyricLine[]>([])
const currentLyricIndex = ref(-1)
const showLyrics = ref(true)
const lyricsPosition = ref<'top' | 'bottom'>('bottom')

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

const editTagsForm = ref({
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

// 浏览器环境下的文件对象存储
const browserFileMap = new Map<string, File>()

// 启用日志输出以便调试
function enableLogs(): void {
  (window as any).logInfo = logInfo
  ;(window as any).logError = logError
  ;(window as any).browserFileMap = browserFileMap
}

// 调用以启用全局日志
enableLogs()

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
const currentFilter = ref<'all' | 'favorites' | 'artists' | 'albums' | 'cue'>('all')
const searchQuery = ref('')
const selectedArtist = ref<string>('')
const selectedAlbum = ref<string>('')

// 均衡器
const currentPreset = ref('flat')
const equalizerBands = ref<number[]>([0, 0, 0, 0, 0, 0, 0, 0, 0, 0])

// 计算属性
const currentFilterText = computed(() => {
  // 依赖语言和翻译状态，确保语言切换时重新计算
  const currentLang = i18nService.getCurrentLanguage()
  const filters = {
    all: t('playlist.allSongs'),
    favorites: t('playlist.favorites'),
    artists: t('playlist.artists'),
    albums: t('playlist.albums'),
    cue: 'CUE专辑'
  }
  return filters[currentFilter.value]
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
const mainLyricsContainer = ref<HTMLElement | null>(null)
const mainLyricLineRefs = ref<(any | null)[]>([])
const coverModalContent = ref<HTMLElement | null>(null)
const songListContainer = ref<HTMLElement | null>(null)

// 滚动相关状态
const showScrollTopButton = ref(true) // 始终显示回到顶部按钮
const showJumpToCurrentButton = ref(true) // 始终显示跳到当前播放曲目的按钮

// 封面模态框拖动和全屏状态
const isCoverModalFullscreen = ref(false)
const coverModalPosition = ref<{ left: string; top: string; transform?: string }>({ left: '50%', top: '50%', transform: 'translate(-50%, -50%)' })
let isDraggingCoverModal = false
let dragStartX = 0
let dragStartY = 0
let modalStartX = 0
let modalStartY = 0

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
  
  // 更新悬浮按钮的显示状态
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
        
        // 支持的音频格式
        const audioExtensions = ['.mp3', '.flac', '.wav', '.ogg', '.aac', '.m4a']
        
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
              
              // 过滤出音频文件
              const audioFileList = files.filter(file => {
                const extension = '.' + file.name.split('.').pop()?.toLowerCase() || ''
                return audioExtensions.includes(extension)
              })
              
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

// 记录上次后端状态更新的时间
let lastBackendUpdateTime = Date.now()
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
    const currentIndex = songs.value.findIndex(s => s.id === song.id)
    
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
    // 对于CUE track，计算相对位置
    let positionForCue = position
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
    const playPath = song.isCueTrack && song.parentFile ? song.parentFile : song.path
    
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

    // 检查是否需要使用FFplay播放（原引擎不支持的无损音频）
    const unsupportedFormats = ['.dsf', '.dff', '.dsd', '.mqa', '.wv', '.tta', '.ape', '.wma', '.m4a', '.aac']
    const needsFFplay = unsupportedFormats.some(ext => playPath.toLowerCase().endsWith(ext))
    
    if (needsFFplay && !isBrowser.value) {
      logInfo('检测到需要FFplay播放的格式:', playPath)
      
      // 停止之前的FFplay播放
      if (isFFplayPlaying.value) {
        try {
          await invoke('stop_ffplay')
          isFFplayPlaying.value = false
        } catch (error) {
          logError('停止FFplay播放失败:', error)
        }
      }
      
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
        // 使用FFplay播放
        const start_position = positionForCue
        let result
        try {
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
          
          result = await invoke('play_with_ffplay', { 
            path: playPath, 
            start_time: start_position,
            duration: durationSeconds
          })
          logInfo('FFplay播放已启动:', result)
          logInfo('result类型:', typeof result)
          logInfo('result.duration:', result.duration)
        } catch (invokeError) {
          logError('FFplay播放失败:', invokeError)
          // 如果是ffplay未找到，给出更明确的提示
          const errorMsg = String(invokeError)
          if (errorMsg.includes('未找到') || errorMsg.includes('not found') || errorMsg.includes('FFplay')) {
            logError('错误原因：FFplay可执行文件未找到，请下载FFmpeg并放置到项目bin目录或添加到系统PATH')
          }
          return
        }
        
        // 检查是否返回错误（ffplay未找到）
        if (result && typeof result === 'string' && result.includes('未找到')) {
          logError('FFplay未找到，无法播放此格式:', result)
          return
        }
        
        // 获取音频时长和详细信息
        try {
          if (result && result.duration) {
            ffplayDuration.value = result.duration
            ffplayPosition.value = start_position
            
            // 更新歌曲时长
            const totalSeconds = Math.round(result.duration)
            const minutes = Math.floor(totalSeconds / 60)
            const seconds = totalSeconds % 60
            song.duration = `${minutes}:${seconds.toString().padStart(2, '0')}`
            
            // 更新音频文件详细信息
            if (result.format) {
              song.format = result.format
            }
            if (result.sample_rate) {
              song.sample_rate = result.sample_rate
            }
            if (result.channels) {
              song.channels = result.channels
            }
            if (result.bit_rate) {
              song.bit_rate = result.bit_rate
            }
            if (result.bit_depth) {
              song.bit_depth = result.bit_depth
            }
            
            // 计算进度百分比
            if (totalSeconds > 0) {
              progress.value = Math.min((start_position / totalSeconds) * 100, 100)
            }
            
            console.log('【FFplay】音频文件信息:', {
              format: result.format,
              sample_rate: result.sample_rate,
              channels: result.channels,
              bit_rate: result.bit_rate,
              bit_depth: result.bit_depth
            })
            logInfo('音频文件信息:', {
              format: result.format,
              sample_rate: result.sample_rate,
              channels: result.channels,
              bit_rate: result.bit_rate,
              bit_depth: result.bit_depth
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
        console.log('【FFplay】启动FFplay状态监控定时器')
        logInfo('启动FFplay状态监控定时器')
        // 清除可能存在的旧定时器
        if (ffplayStatusInterval) {
          clearInterval(ffplayStatusInterval)
          ffplayStatusInterval = null
          console.log('【FFplay】已清除旧的FFplay状态监控定时器')
          logInfo('已清除旧的FFplay状态监控定时器')
        }
        // 初始化全局变量
        lastBackendUpdateTime = Date.now()
        frontendPosition = currentPosition.value
        
        // 确保isFFplayPlaying.value为true
        isFFplayPlaying.value = true
        console.log('【FFplay】设置isFFplayPlaying.value为true，当前值:', isFFplayPlaying.value)
        logInfo('设置isFFplayPlaying.value为true，当前值:', isFFplayPlaying.value)
        
        // 立即执行一次状态更新，确保前端能够立即获取到FFplay的状态
        (async () => {
          try {
            console.log('【FFplay】立即执行FFplay状态更新')
            logInfo('立即执行FFplay状态更新')
            const status = await invoke('get_ffplay_status') as any
            console.log('【FFplay】立即获取FFplay状态成功:', JSON.stringify(status))
            logInfo('立即获取FFplay状态成功:', JSON.stringify(status))

            if (status) {
              // 更新上次后端状态更新的时间
              lastBackendUpdateTime = Date.now()
              console.log('【FFplay】立即处理FFplay状态:', {
                duration: status.duration,
                position: status.position,
                volume: status.volume,
                is_playing: status.is_playing
              })
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
              console.log('【FFplay】isPlaying 立即更新为:', isPlaying.value, 'isFFplayPlaying:', isFFplayPlaying.value)
              logInfo('isPlaying 立即更新为:', isPlaying.value, 'isFFplayPlaying:', isFFplayPlaying.value)

              // 只有当status.position有效时才更新currentPosition
              if (status.position !== undefined && status.position !== null) {
                console.log('【FFplay】立即更新currentPosition前:', currentPosition.value, '更新后:', status.position)
                logInfo('立即更新currentPosition前:', currentPosition.value, '更新后:', status.position)
                currentPosition.value = status.position
                // 更新前端计算的播放位置
                frontendPosition = status.position
                console.log('【FFplay】立即更新播放进度:', currentPosition.value, '秒, 时长:', ffplayDuration.value)
                logInfo('立即更新播放进度:', currentPosition.value, '秒, 时长:', ffplayDuration.value)
              }
              
              // 计算进度百分比
              if (ffplayDuration.value > 0) {
                const newProgress = Math.min((currentPosition.value / ffplayDuration.value) * 100, 100)
                console.log('【FFplay】立即更新进度百分比前:', progress.value, '更新后:', newProgress)
                logInfo('立即更新进度百分比前:', progress.value, '更新后:', newProgress)
                progress.value = newProgress
                console.log('【FFplay】立即更新进度百分比:', progress.value, '%')
                logInfo('立即更新进度百分比:', progress.value, '%')
              }
            }
          } catch (error) {
            console.error('【FFplay】立即获取FFplay状态失败:', error)
            logError('立即获取FFplay状态失败:', error)
          }
        })()
        
        ffplayStatusInterval = window.setInterval(() => {
          // 检查是否应该继续运行定时器
          if (!isFFplayPlaying.value) {
            console.log('【FFplay】isFFplayPlaying为false，停止监控定时器')
            logInfo('isFFplayPlaying为false，停止监控定时器')
            if (ffplayStatusInterval) {
              clearInterval(ffplayStatusInterval)
              ffplayStatusInterval = null
            }
            return
          }
          
          console.log('【FFplay】FFplay状态监控定时器触发')
          logInfo('FFplay状态监控定时器触发')
          // 使用IIFE包装async函数
          (async () => {
            try {
              console.log('【FFplay】准备调用get_ffplay_status')
              logInfo('准备调用get_ffplay_status')
              const status = await invoke('get_ffplay_status') as any
              console.log('【FFplay】获取FFplay状态成功:', JSON.stringify(status))
              logInfo('获取FFplay状态成功:', JSON.stringify(status))

              if (status) {
                // 更新上次后端状态更新的时间
                lastBackendUpdateTime = Date.now()
                console.log('【FFplay】处理FFplay状态:', {
                  duration: status.duration,
                  position: status.position,
                  volume: status.volume,
                  is_playing: status.is_playing
                })
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
                console.log('【FFplay】isPlaying 更新为:', isPlaying.value, 'isFFplayPlaying:', isFFplayPlaying.value)
                logInfo('isPlaying 更新为:', isPlaying.value, 'isFFplayPlaying:', isFFplayPlaying.value)

                // 只有当status.position有效时才更新currentPosition
                if (status.position !== undefined && status.position !== null) {
                  console.log('【FFplay】更新currentPosition前:', currentPosition.value, '更新后:', status.position)
                  logInfo('更新currentPosition前:', currentPosition.value, '更新后:', status.position)
                  currentPosition.value = status.position
                  // 更新前端计算的播放位置
                  frontendPosition = status.position
                  console.log('【FFplay】更新播放进度:', currentPosition.value, '秒, 时长:', ffplayDuration.value)
                  logInfo('更新播放进度:', currentPosition.value, '秒, 时长:', ffplayDuration.value)
                }
                
                // 计算进度百分比
                if (ffplayDuration.value > 0) {
                  const newProgress = Math.min((currentPosition.value / ffplayDuration.value) * 100, 100)
                  console.log('【FFplay】更新进度百分比前:', progress.value, '更新后:', newProgress)
                  logInfo('更新进度百分比前:', progress.value, '更新后:', newProgress)
                  progress.value = newProgress
                  console.log('【FFplay】更新进度百分比:', progress.value, '%')
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

                console.log('【FFplay】播放完成检测: is_playing=', status.is_playing, 'position=', status.position.toFixed(2), 'duration=', status.duration.toFixed(2), 'ffplayDuration=', ffplayDuration.value, 'isFFplayPlaying=', isFFplayPlaying.value, 'isPlaybackComplete=', isPlaybackComplete)
                logInfo('播放完成检测: is_playing=', status.is_playing, 'position=', status.position.toFixed(2), 'duration=', status.duration.toFixed(2), 'ffplayDuration=', ffplayDuration.value, 'isFFplayPlaying=', isFFplayPlaying.value, 'isPlaybackComplete=', isPlaybackComplete)

                if (isPlaybackComplete) {
                  console.log('【FFplay】FFplay播放完成, position:', status.position, 'duration:', status.duration)
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
                    console.log('【FFplay】准备播放下一首歌曲');
                    logInfo('准备播放下一首歌曲');
                    setTimeout(() => {
                      console.log('【FFplay】执行playNext()');
                      logInfo('执行playNext()');
                      playNext();
                    }, 1000)
                  } else if (autoPlay) {
                    isPlaying.value = false
                    console.log('【FFplay】播放完成，停止播放');
                    logInfo('播放完成，停止播放');
                  }
                }
                

              }
            } catch (error) {
              console.error('【FFplay】获取FFplay状态失败:', error)
              logError('获取FFplay状态失败:', error)
              // 即使在获取状态失败的情况下，也保持isFFplayPlaying.value为true
              // 确保即使在状态获取失败的情况下，前端也能正确检测到FFplay播放状态
              isFFplayPlaying.value = true
              console.log('【FFplay】获取FFplay状态失败，确保isFFplayPlaying.value为true，当前值:', isFFplayPlaying.value)
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
            } else {
              // 曲目播放结束
              currentPosition.value = ffplayDuration.value
              progress.value = 100
              console.log('【FFplay】前端进度更新(备份): 曲目播放结束, currentPosition=', frontendPosition, '秒, progress=', progress.value, '%')
              logInfo('前端进度更新(备份): 曲目播放结束, currentPosition=', frontendPosition, '秒, progress=', progress.value, '%')
            }
          } else {
            // 播放暂停或停止，重置前端计算的播放位置
            frontendPosition = currentPosition.value
          }
        }, 300) // 每 300 毫秒更新一次，符合用户要求
        
        // 解析歌词（使用最新的歌曲数据）
        const songForLyric = songs.value.find(s => s.id === song.id) || song
        if (songForLyric.lyric) {
          logInfo('解析歌词，长度:', songForLyric.lyric.length)
          lyrics.value = parseLyrics(songForLyric.lyric)
          logInfo('歌词解析完成，行数:', lyrics.value.length)
          coverLyricLineRefs.value = []
        } else {
          logInfo('歌曲无歌词')
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
      } else if (!audioUrl.startsWith('http://') && !audioUrl.startsWith('https://') && !audioUrl.startsWith('blob:') && !audioUrl.startsWith('blob:http://')) {
        // 桌面应用环境，获取HTTP URL
        try {
          audioUrl = await invoke('get_file_http_url', { filePath: audioUrl }) as string
          console.log('获取HTTP URL成功:', audioUrl)
        } catch (urlError) {
          console.error('❌ 前端播放: 获取HTTP URL失败:', urlError)
          errorMessage = '无法获取文件URL: ' + (urlError && typeof urlError === 'object' && 'message' in urlError ? (urlError.message as string) : String(urlError))
          isPlaying.value = false
          isPlaybackFinished = true
          throw new Error(errorMessage)
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
      
      // 解析歌词（使用最新的歌曲数据）
      const songForLyric = songs.value.find(s => s.id === song.id) || song
      if (songForLyric.lyric) {
        logInfo('解析歌词，长度:', songForLyric.lyric.length)
        lyrics.value = parseLyrics(songForLyric.lyric)
        logInfo('歌词解析完成，行数:', lyrics.value.length)
        // 清空歌词行 refs，等待 DOM 渲染后重新填充
        coverLyricLineRefs.value = []
      } else {
        logInfo('歌曲无歌词')
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
        await new Promise<void>((resolve) => {
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
        
        // 处理错误
        currentAudioElement.onerror = (event) => {
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
                // 可以尝试重新加载
                break
              case error.MEDIA_ERR_NETWORK:
                logError('网络错误导致音频加载失败: 检查网络连接或文件服务器')
                // 可以尝试重新加载或切换到备用源
                break
              case error.MEDIA_ERR_DECODE:
                logError('音频解码失败: 音频文件可能损坏或格式不被支持')
                // 可以尝试转码或使用其他播放器
                break
              case error.MEDIA_ERR_SRC_NOT_SUPPORTED:
                logError('音频格式不支持: 当前浏览器不支持此音频格式')
                // 可以尝试转码为支持的格式
                break
              default:
                logError('未知音频错误: 请检查音频文件和播放环境')
            }
            
            // 针对特定错误类型的处理策略
            if (error.code === error.MEDIA_ERR_SRC_NOT_SUPPORTED) {
              logInfo('尝试启用转码来处理不支持的格式')
              // 可以在这里触发转码逻辑
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
    
    const errorMessage = error && typeof error === 'string' ? error : (error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error))
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
          const result = await invoke('resume_ffplay') as any
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

// 从文件路径中提取文件名并移除后缀名
const getFileNameWithoutExtension = (path: string): string => {
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
const extractInfoFromFileName = (fileName: string): { artist: string; album: string } => {
  // 尝试从文件名中提取艺术家和专辑信息
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
const getDisplayTitle = (song: Song): string => {
  // 定义常见的音频文件扩展名
  const audioExtensions = ['mp3', 'flac', 'wav', 'aac', 'ogg', 'm4a', 'ape', 'dsd', 'dts', 'wma', 'opus']

  // 检查标题是否只是一个文件扩展名
  if (song.title && audioExtensions.includes(song.title.toLowerCase())) {
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
const getDisplayArtist = (song: Song): string => {
  if (song.artist && song.artist !== '未知艺术家') {
    return song.artist
  }
  
  // 尝试从文件名中提取艺术家信息
  const fileName = getFileNameWithoutExtension(song.path)
  const info = extractInfoFromFileName(fileName)
  
  return info.artist || '未知艺术家'
}

// 获取显示的专辑名称
const getDisplayAlbum = (song: Song): string => {
  if (song.album && song.album !== '未知专辑') {
    return song.album
  }
  
  // 尝试从文件名中提取专辑信息
  const fileName = getFileNameWithoutExtension(song.path)
  const info = extractInfoFromFileName(fileName)
  
  return info.album || '未知专辑'
}

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

  // 检查是否需要使用FFplay播放（基于文件格式）
  const unsupportedFormats = ['.dsf', '.dff', '.dsd', '.mqa', '.wv', '.tta', '.ape', '.wma', '.m4a', '.aac']
  const shouldUseFFplay = currentSong.value && unsupportedFormats.some(ext => currentSong.value.path.toLowerCase().endsWith(ext))
  console.log('【SEEK】shouldUseFFplay:', shouldUseFFplay)

  // 如果应该使用FFplay播放，使用FFplay的seek功能
  if (shouldUseFFplay && currentSong.value) {
    console.log('【SEEK】使用FFplay seek')
    
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
          
          // 调用FFplay seek
          const result = await invoke('seek_ffplay', {
            path: currentSong.value.path,
            position: actualPosition
          }) as any
          
          console.log('【SEEK】FFplay seek成功:', result)
          
          // 更新进度相关变量
          playbackStartTime.value = Date.now() - (actualPosition * 1000)
          currentPosition.value = actualPosition
          frontendPosition = actualPosition // 更新前端计算的播放位置
          progress.value = clampedProgress
          ffplayPosition.value = actualPosition
          
          // 确保isFFplayPlaying.value为true
          isFFplayPlaying.value = true
          logInfo('【SEEK】FFplay seek完成: currentPosition=', actualPosition, 's, progress=', progress.value, '%')
          
          // 立即执行一次状态更新，确保前端能够立即获取到FFplay的状态
          (async () => {
            try {
              console.log('【SEEK】立即执行FFplay状态更新')
              logInfo('立即执行FFplay状态更新')
              const status = await invoke('get_ffplay_status') as any
              console.log('【SEEK】立即获取FFplay状态成功:', JSON.stringify(status))
              logInfo('立即获取FFplay状态成功:', JSON.stringify(status))

              if (status) {
                console.log('【SEEK】立即处理FFplay状态:', {
                  duration: status.duration,
                  position: status.position,
                  volume: status.volume,
                  is_playing: status.is_playing
                })
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
                console.log('【SEEK】isPlaying 立即更新为:', isPlaying.value, 'isFFplayPlaying:', isFFplayPlaying.value)
                logInfo('isPlaying 立即更新为:', isPlaying.value, 'isFFplayPlaying:', isFFplayPlaying.value)

                // 只有当status.position有效时才更新currentPosition
                if (status.position !== undefined && status.position !== null) {
                  console.log('【SEEK】立即更新currentPosition前:', currentPosition.value, '更新后:', status.position)
                  logInfo('立即更新currentPosition前:', currentPosition.value, '更新后:', status.position)
                  currentPosition.value = status.position
                  frontendPosition = status.position // 更新前端计算的播放位置
                  console.log('【SEEK】立即更新播放进度:', currentPosition.value, '秒, 时长:', ffplayDuration.value)
                  logInfo('立即更新播放进度:', currentPosition.value, '秒, 时长:', ffplayDuration.value)
                }
                
                // 计算进度百分比
                if (ffplayDuration.value > 0) {
                  const newProgress = Math.min((currentPosition.value / ffplayDuration.value) * 100, 100)
                  console.log('【SEEK】立即更新进度百分比前:', progress.value, '更新后:', newProgress)
                  logInfo('立即更新进度百分比前:', progress.value, '更新后:', newProgress)
                  progress.value = newProgress
                  console.log('【SEEK】立即更新进度百分比:', progress.value, '%')
                  logInfo('立即更新进度百分比:', progress.value, '%')
                }
              }
            } catch (error) {
              console.error('【SEEK】立即获取FFplay状态失败:', error)
              logError('立即获取FFplay状态失败:', error)
            }
          })()
          
          // 确保FFplay状态监控定时器正在运行
          if (!ffplayStatusInterval) {
            console.log('【SEEK】FFplay状态监控定时器未运行，启动一个新的')
            logInfo('FFplay状态监控定时器未运行，启动一个新的')
            // 启动FFplay状态监控定时器
            ffplayStatusInterval = window.setInterval(() => {
              console.log('【FFplay】状态监控定时器触发')
              logInfo('FFplay状态监控定时器触发')
              // 使用IIFE包装async函数
              (async () => {
                try {
                  console.log('【FFplay】准备调用get_ffplay_status')
                  logInfo('准备调用get_ffplay_status')
                  const status = await invoke('get_ffplay_status') as any
                  console.log('【FFplay】获取FFplay状态成功:', JSON.stringify(status))
                  logInfo('获取FFplay状态成功:', JSON.stringify(status))

                  if (status) {
                    console.log('【FFplay】处理FFplay状态:', {
                      duration: status.duration,
                      position: status.position,
                      volume: status.volume,
                      is_playing: status.is_playing
                    })
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
                    console.log('【FFplay】isPlaying 更新为:', isPlaying.value, 'isFFplayPlaying:', isFFplayPlaying.value)
                    logInfo('isPlaying 更新为:', isPlaying.value, 'isFFplayPlaying:', isFFplayPlaying.value)

                    // 只有当status.position有效时才更新currentPosition
                    if (status.position !== undefined && status.position !== null) {
                      console.log('【FFplay】更新currentPosition前:', currentPosition.value, '更新后:', status.position)
                      logInfo('更新currentPosition前:', currentPosition.value, '更新后:', status.position)
                      currentPosition.value = status.position
                      // 更新前端计算的播放位置
                      frontendPosition = status.position
                      console.log('【FFplay】更新播放进度:', currentPosition.value, '秒, 时长:', ffplayDuration.value)
                      logInfo('更新播放进度:', currentPosition.value, '秒, 时长:', ffplayDuration.value)
                    }
                    
                    // 计算进度百分比
                    if (ffplayDuration.value > 0) {
                      const newProgress = Math.min((currentPosition.value / ffplayDuration.value) * 100, 100)
                      console.log('【FFplay】更新进度百分比前:', progress.value, '更新后:', newProgress)
                      logInfo('更新进度百分比前:', progress.value, '更新后:', newProgress)
                      progress.value = newProgress
                      console.log('【FFplay】更新进度百分比:', progress.value, '%')
                      logInfo('更新进度百分比:', progress.value, '%')
                    }
                  }
                } catch (error) {
                  console.error('【FFplay】获取FFplay状态失败:', error)
                  logError('获取FFplay状态失败:', error)
                }
              })()
            }, 500) // 每500毫秒更新一次状态，提高响应速度
          } else {
            console.log('【SEEK】FFplay状态监控定时器已经在运行，不需要重新启动')
            logInfo('FFplay状态监控定时器已经在运行，不需要重新启动')
          }
        }
      }
      
      isSeeking.value = false
      wasPlayingBeforeSeek = false
      console.log('【SEEK】========== seek 函数结束（FFplay） ==========')
      return
    } catch (error) {
      console.log('【SEEK】❌ FFplay seek失败:', error)
      logError('【SEEK】FFplay seek失败:', error)
      isSeeking.value = false
      wasPlayingBeforeSeek = false
      return
    }
  }

  // 保存对 audioElement 的引用，防止在 seek 过程中被清理
  const audioElementRef = audioElement.value
  if (!audioElementRef) {
    console.log('【SEEK】❌ 没有音频元素，无法定位')
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

    // 保存当前播放状态
    const wasPaused = audioElementRef.paused

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

const getBandLabel = (index: number) => {
  const bands = ['31Hz', '62Hz', '125Hz', '250Hz', '500Hz', '1kHz', '2kHz', '4kHz', '8kHz', '16kHz']
  return bands[index]
}

const applyPreset = async () => {
  try {
    const presets = {
      flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
      rock: [6, 5, 4, 3, 2, -1, -2, -3, -2, 0],
      pop: [-2, -1, 0, 2, 4, 4, 3, 2, 1, 0],
      jazz: [4, 3, 2, 1, -1, -2, -1, 0, 2, 4],
      classical: [7, 5, 3, 1, -1, -2, -1, 1, 3, 5],
      electronic: [4, 3, 2, -1, -3, -2, 0, 2, 3, 4]
    }
    equalizerBands.value = [...presets[currentPreset.value as keyof typeof presets]]
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

const openSongMenu = (song: Song, event: MouseEvent) => {
  selectedSong.value = song
  menuPosition.value = {
    left: `${event.clientX}px`,
    top: `${event.clientY}px`
  }
  showSongMenu.value = true
  
  // 点击其他地方关闭菜单
  setTimeout(() => {
    document.addEventListener('click', closeSongMenu)
  }, 10)
}

const closeSongMenu = () => {
  showSongMenu.value = false
  document.removeEventListener('click', closeSongMenu)
}

// 编辑歌曲标签
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

// 打开封面模态框
const openCoverModal = () => {
  if (currentSong.value) {
    showCoverModal.value = true
    logInfo('打开封面模态框')
    
    // 打开后等待模态框完全渲染，然后滚动到当前歌词
    setTimeout(() => {
      nextTick(() => {
        scrollToCurrentLyric()
      })
    }, 100)
  }
}

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

// 滚动到当前歌词（主界面）
const scrollToMainCurrentLyric = () => {
  const index = currentLyricIndex.value
  if (index >= 0 && mainLyricsContainer.value) {
    const lineElement = mainLyricLineRefs.value[index]
    if (lineElement && mainLyricsContainer.value) {
      const container = mainLyricsContainer.value
      const lineTop = lineElement.offsetTop
      const lineHeight = lineElement.offsetHeight
      const containerHeight = container.clientHeight
      const scrollTop = lineTop - containerHeight / 2 + lineHeight / 2
      
      container.scrollTo({
        top: Math.max(0, scrollTop),
        behavior: 'smooth'
      })
      logInfo('主界面歌词滚动: 滚动到歌词行', index)
    }
  }
}

// 关闭封面模态框
const closeCoverModal = () => {
  showCoverModal.value = false
  isCoverModalFullscreen.value = false
  coverModalPosition.value = { left: '50%', top: '50%', transform: 'translate(-50%, -50%)' }
  logInfo('关闭封面模态框')
}

// 切换封面模态框全屏状态
const toggleCoverModalFullscreen = () => {
  isCoverModalFullscreen.value = !isCoverModalFullscreen.value
  if (isCoverModalFullscreen.value) {
    // 全屏时重置位置
    coverModalPosition.value = { left: '50%', top: '50%', transform: 'translate(-50%, -50%)' }
  } else {
    // 退出全屏时恢复居中
    coverModalPosition.value = { left: '50%', top: '50%', transform: 'translate(-50%, -50%)' }
  }
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

// 复制路径
const copyPath = () => {
  if (songToEdit.value?.path) {
    navigator.clipboard.writeText(songToEdit.value.path)
      .then(() => {
        alert('路径已复制到剪贴板')
      })
      .catch(err => {
        logError('复制失败:', err)
        alert('复制失败，请手动复制')
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
    
    alert('从本地文件读取元数据成功')
  } catch (error) {
    logError('【本地元数据】读取失败:', error)
    alert('读取本地元数据失败，请检查文件格式是否支持')
  }
}

// 在线查找歌词
const fetchLyric = async () => {
  try {
    if (!songToEdit.value) {
      logInfo('【在线歌词】没有歌曲可编辑')
      alert('没有歌曲可编辑')
      return
    }
    
    logInfo('【在线歌词】开始在线查找歌词')
    
    const title = editTagsForm.value.title || songToEdit.value.title
    const artist = editTagsForm.value.artist || songToEdit.value.artist
    const keyword = `${title} ${artist}`.trim()
    
    if (!keyword) {
      logInfo('【在线歌词】关键词为空')
      alert('请先填写歌曲标题和艺术家信息')
      return
    }
    
    logInfo('【在线歌词】搜索关键词:', keyword)
    
    // 使用新的音乐数据服务获取歌词
    let result
    try {
      result = await musicDataService.getSongInfoWithLyric(keyword)
    } catch (apiError) {
      logError('【在线歌词】API调用失败:', apiError)
      alert('网络请求失败，请检查网络连接')
      return
    }
    
    logInfo('【在线歌词】API返回结果:', result)
    
    if (!result || !result.song) {
      logInfo('【在线歌词】未找到匹配的歌曲')
      alert('未找到匹配的歌曲，请修改歌曲信息后重试')
      return
    }
    
    const lyricData = result.lyricData
    if (!lyricData || (!lyricData.lrcData.length && !lyricData.yrcData.length)) {
      logInfo('【在线歌词】未找到歌词数据')
      alert('找到歌曲但未找到歌词，请尝试手动搜索')
      return
    }
    
    const lyricLines = lyricData.lrcData.length > 0 ? lyricData.lrcData : lyricData.yrcData
    
    // 转换为LRC格式
    const lrcContent = lyricLines.map(line => {
      const text = line.words.map(w => w.word).join('')
      return `[${formatTime(line.startTime)}]${text}`
    }).join('\n')
    
    editTagsForm.value.lyric = lrcContent
    
    logInfo('【在线歌词】找到歌曲:', result.song.title, '歌词行数:', lyricLines.length, 
             '是否有TTML:', result.hasTTML, '是否有QRC:', result.hasQRC)
    
    alert('获取歌词成功')
  } catch (error) {
    logError('【在线歌词】获取歌词失败:', error)
    alert('获取歌词失败，请检查网络连接后重试')
  }
}

// 获取封面
const fetchCover = async () => {
  try {
    if (!songToEdit.value) {
      logInfo('【在线封面】没有歌曲可编辑')
      alert('没有歌曲可编辑')
      return
    }
    
    logInfo('【在线封面】开始在线查找封面')
    
    const title = editTagsForm.value.title || songToEdit.value.title
    const artist = editTagsForm.value.artist || songToEdit.value.artist
    const keyword = `${title} ${artist}`.trim()
    
    if (!keyword) {
      logInfo('【在线封面】关键词为空')
      alert('请先填写歌曲标题和艺术家信息')
      return
    }
    
    logInfo('【在线封面】搜索关键词:', keyword)
    
    // 使用新的音乐数据服务获取封面
    let result
    try {
      result = await musicDataService.getSongInfoWithLyric(keyword)
    } catch (apiError) {
      logError('【在线封面】API调用失败:', apiError)
      alert('网络请求失败，请检查网络连接')
      return
    }
    
    logInfo('【在线封面】API返回结果:', result)
    
    if (!result || !result.song) {
      logInfo('【在线封面】未找到匹配的歌曲')
      alert('未找到匹配的歌曲，请修改歌曲信息后重试')
      return
    }
    
    if (!result.song.coverUrl) {
      logInfo('【在线封面】找到歌曲但没有封面')
      alert('找到歌曲但未找到封面')
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
    
    alert('获取封面成功')
  } catch (error) {
    logError('【在线封面】获取封面失败:', error)
    alert('获取封面失败，请检查网络连接后重试')
  }
}

// 格式化时间为 LRC 格式
const formatTimeForLrc = (ms: number): string => {
  const totalSeconds = Math.floor(ms / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  const centiseconds = Math.floor((ms % 1000) / 10)
  return `${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}.${centiseconds.toString().padStart(2, '0')}`
}

// 打开在线匹配对话框
const openOnlineMatch = () => {
  if (!songToEdit.value) return
  showOnlineMatchModal.value = true
}

// 处理在线匹配结果
const handleOnlineMatchApply = (data: { 
  title?: string
  artist?: string
  album?: string
  lyric?: string
  coverUrl?: string
}) => {
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
      alert('没有歌曲可编辑')
      return
    }
    
    logInfo('【自动匹配】开始自动匹配标签')
    
    const title = editTagsForm.value.title || songToEdit.value.title
    const artist = editTagsForm.value.artist || songToEdit.value.artist
    const keyword = `${title} ${artist}`.trim()
    
    if (!keyword) {
      logInfo('【自动匹配】关键词为空')
      alert('请先填写歌曲标题和艺术家信息')
      return
    }
    
    logInfo('【自动匹配】搜索关键词:', keyword)
    
    // 使用新的音乐数据服务获取歌曲信息
    let result
    try {
      result = await musicDataService.getSongInfoWithLyric(keyword)
    } catch (apiError) {
      logError('【自动匹配】API调用失败:', apiError)
      alert('网络请求失败，请检查网络连接')
      return
    }
    
    logInfo('【自动匹配】API返回结果:', result)
    
    if (!result || !result.song) {
      logInfo('【自动匹配】未找到匹配的歌曲')
      alert('未找到匹配的歌曲，请修改歌曲信息后重试')
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
      alert(`自动匹配完成，共匹配 ${matchedCount} 项`)
    } else {
      alert('已找到歌曲，但没有新的信息可以匹配（可能已有完整信息）')
    }
  } catch (error) {
    logError('【自动匹配】自动匹配失败:', error)
    alert('自动匹配失败，请检查网络连接后重试')
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
    alert('选择封面失败，请重试')
  }
}

// 在线匹配标签
const onlineMatch = async () => {
  try {
    if (!songToEdit.value) return
    
    // 显示加载提示
    alert('正在在线匹配标签，请稍候...')
    
    // 准备匹配参数
    const title = editTagsForm.value.title || songToEdit.value.title
    const artist = editTagsForm.value.artist || songToEdit.value.artist
    
    // 调用搜索API
    const response = await matchSong(title, artist, '', 0, '')
    const song = response.data.result?.songs?.[0]
    
    if (!song) {
      alert('无法匹配到歌曲信息，请修改后重试')
      return
    }
    
    // 更新标签信息
    editTagsForm.value.title = song.name
    editTagsForm.value.artist = song.artists?.map((ar: any) => ar.name).join(' / ') || ''
    editTagsForm.value.album = song.album?.name || ''
    
    // 获取歌词
    try {
      const lyricResponse = await songLyric(song.id)
      if (lyricResponse.data.lrc?.lyric) {
        editTagsForm.value.lyric = lyricResponse.data.lrc.lyric
      }
    } catch (lyricError) {
      logError('获取歌词失败:', lyricError)
    }
    
    alert('在线匹配标签成功')
  } catch (error) {
    logError('在线匹配标签失败:', error)
    alert('在线匹配标签失败，请检查网络连接后重试')
  }
}

// 保存歌曲标签
const saveSongTags = async () => {
  if (!songToEdit.value) return
  
  try {
    // 验证歌词内容
    if (editTagsForm.value.lyric && editTagsForm.value.lyric.length > 100000) {
      alert('歌词内容过长，请精简后重试')
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
    })) as import('./stores/local').Song[]
    await localStorageService.saveSongs(songsToSave)
    
    alert('标签编辑成功')
    closeEditTagsModal()
  } catch (error) {
    logError('保存标签失败:', error)
    // 提供更详细的错误信息
    if (error instanceof Error) {
      alert(`保存标签失败: ${error.message}\n请检查歌词内容是否过大或包含特殊字符`)
    } else {
      alert('保存标签失败，请重试')
    }
  }
}

const toggleFavorite = async (song: Song) => {
  try {
    const newStatus = !song.isFavorite
    song.isFavorite = newStatus

    if (newStatus) {
      await localStorageService.addToFavorites(song.id)
    } else {
      await localStorageService.removeFromFavorites(song.id)
    }

    // 更新收藏列表
    favorites.value = await localStorageService.getFavorites()
  } catch (error) {
    logError('更新收藏状态失败:', error)
    // 回滚状态
    song.isFavorite = !song.isFavorite
    alert('收藏操作失败,请重试')
  }
}

const addSongToPlaylist = async (song: Song) => {
  const playlistList = await localStorageService.getPlaylists()
  if (playlistList.length === 0) {
    alert('请先创建歌单')
    closeSongMenu()
    return
  }
  
  let options = playlistList.map((p, index) => `${index + 1}. ${p.name}`).join('\n')
  const choice = prompt(`选择歌单:\n${options}\n\n请输入序号:`)
  
  if (choice) {
    const index = parseInt(choice) - 1
    if (index >= 0 && index < playlistList.length) {
      const playlist = playlistList[index]
      if (!playlist.songs.includes(song.id)) {
        playlist.songs.push(song.id)
        await localStorageService.updatePlaylist(playlist.id, { songs: playlist.songs })
        alert(`已添加到歌单 "${playlist.name}"`)
      } else {
        alert(`歌曲已在歌单 "${playlist.name}" 中`)
      }
    }
  }
  
  closeSongMenu()
}

const deleteSong = (song: Song) => {
  if (confirm('确定要删除这首歌吗？')) {
    const index = songs.value.findIndex(s => s.id === song.id)
    if (index !== -1) {
      songs.value.splice(index, 1)
    }
  }
  closeSongMenu()
}

const createPlaylist = async () => {
  const name = prompt('请输入歌单名称:')
  if (name && name.trim()) {
    try {
      await localStorageService.createPlaylist(name.trim())
      playlists.value = await localStorageService.getPlaylists()
      alert(`歌单 "${name}" 创建成功`)
    } catch (error) {
      logError('创建歌单失败:', error)
      alert('创建歌单失败')
    }
  }
}

const formatTime = (seconds: number): string => {
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

// 解析歌词
const parseLyrics = (lyricContent: string): LyricLine[] => {
  if (!lyricContent) return []
  
  try {
    const parsed = parseSmartLrc(lyricContent)
    logInfo('歌词解析完成，格式:', parsed.format, '行数:', parsed.lines.length)
    
    // 转换为现有的LyricLine格式
    return parsed.lines.map(line => ({
      time: line.startTime / 1000, // 转换为秒
      text: line.words.map(w => w.word).join('')
    }))
  } catch (error) {
    logError('歌词解析失败，使用简单解析:', error)
    
    // 回退到简单解析
    const lines: LyricLine[] = []
    const lyricLines = lyricContent.split('\n')
    const timeRegex = /\[(\d+):(\d+\.\d+)\]/g
    
    for (const line of lyricLines) {
      const matches = [...line.matchAll(timeRegex)]
      if (matches.length > 0) {
        const text = line.replace(timeRegex, '').trim()
        if (text) {
          for (const match of matches) {
            const minutes = parseInt(match[1])
            const seconds = parseFloat(match[2])
            const time = minutes * 60 + seconds
            lines.push({ time, text })
          }
        }
      }
    }
    
    lines.sort((a, b) => a.time - b.time)
    return lines
  }
}

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

const minimizeWindow = async () => {
  try {
    if (!isBrowser.value) {
      await invoke('minimize_window')
    }
  } catch (error) {
    logError('最小化窗口失败:', error)
  }
}

const toggleMaximizeWindow = async () => {
  try {
    if (!isBrowser.value) {
      await invoke('toggle_maximize_window')
    }
  } catch (error) {
    logError('切换最大化状态失败:', error)
  }
}

const closeWindow = async () => {
  try {
    // 隐藏窗口到托盘，而不是退出应用
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    const currentWindow = getCurrentWindow()
    await currentWindow.hide()
  } catch (error) {
    logError('隐藏窗口失败:', error)
  }
}

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
let playbackTimerId: number | null = null

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

// 格式化时长
const formatDuration = (seconds: number): string => {
  if (isNaN(seconds)) return '0:00'
  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = Math.floor(seconds % 60)
  return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}`
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
  logInfo('前端 清理进度更新定时器')
  if (progressTimer) {
    clearInterval(progressTimer)
    logInfo('前端 进度更新定时器已清理')
  }
  
  // 移除滚动事件监听器
  if (songListContainer.value) {
    const songList = songListContainer.value.querySelector('.song-list') as HTMLElement
    if (songList) {
      songList.removeEventListener('scroll', handleScroll)
    } else {
      songListContainer.value.removeEventListener('scroll', handleScroll)
    }
  }
})
</script>

<style scoped>
/* 加载页面样式 */
.loading-page {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background-color: #1a1a1a;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  z-index: 9999;
}

.loading-spinner {
  width: 60px;
  height: 60px;
  border: 5px solid rgba(255, 255, 255, 0.3);
  border-radius: 50%;
  border-top-color: #5cb85c;
  animation: spin 1s ease-in-out infinite;
  margin-bottom: 20px;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.loading-page h2 {
  color: #ffffff;
  font-size: 18px;
  font-weight: 500;
}

/* 全局样式 */
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body, html {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  overflow: hidden !important;
  background-color: #1a1a1a;
}

.tplayer-container {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100vh;
  max-width: 100%;
  max-height: 100vh;
  overflow: hidden;
  background-color: #1a1a1a;
  color: #ffffff;
  font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
  border: none;
  outline: none;
  --text-primary: #ffffff;
  --text-secondary: #cccccc;
  --bg-secondary: #2a2a2a;
  --bg-hover: rgba(255, 255, 255, 0.15);
  --border-color: rgba(255, 255, 255, 0.2);
  --btn-secondary-bg: rgba(255, 255, 255, 0.1);
  --btn-secondary-hover: rgba(255, 255, 255, 0.15);

  /* 按钮主题颜色 */
  --btn-primary: #5cb85c;
  --btn-primary-hover: #4aa34a;
  --btn-secondary: #3a3a3a;
  --btn-secondary-hover-light: #4a4a4a;
  --btn-danger: #d9534f;
  --btn-danger-hover: #c9302c;
  --btn-success: #5cb85c;
  --btn-success-hover: #4aa34a;
  --btn-info: #5bc0de;
  --btn-info-hover: #46b8da;
}

.tplayer-container.light {
  --text-primary: #333333;
  --text-secondary: #666666;
  --bg-secondary: #ffffff;
  --bg-hover: #e0e0e0;
  --border-color: rgba(0, 0, 0, 0.2);
  --btn-secondary-bg: #e0e0e0;
  --btn-secondary-hover: #d0d0d0;

  /* 按钮主题颜色 - 浅色 */
  --btn-primary: #5cb85c;
  --btn-primary-hover: #4aa34a;
  --btn-secondary: #e0e0e0;
  --btn-secondary-hover-light: #d0d0d0;
  --btn-danger: #d9534f;
  --btn-danger-hover: #c9302c;
  --btn-success: #5cb85c;
  --btn-success-hover: #4aa34a;
  --btn-info: #5cb85c;
  --btn-info-hover: #4aa34a;
}

/* 顶部信息栏 */
.top-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 20px;
  background-color: #2a2a2a;
  border-bottom: 1px solid #3a3a3a;
  /* Tauri 窗口拖动区域属性 */
  -webkit-app-region: drag;
  app-region: drag;
}

.app-logo {
  /* Tauri 窗口拖动排除区域属性 */
  -webkit-app-region: no-drag;
  app-region: no-drag;
  display: flex;
  align-items: center;
  gap: 10px;
}

.window-controls {
  /* Tauri 窗口拖动排除区域属性 */
  -webkit-app-region: no-drag;
  app-region: no-drag;
  display: flex;
  gap: 10px;
}

.app-logo h1 {
  margin: 0;
  font-size: 18px;
  font-weight: bold;
  color: var(--btn-success);
}

.logo-image {
  width: 24px;
  height: 24px;
  border-radius: 4px;
  object-fit: cover;
}

.window-controls {
  display: flex;
  gap: 10px;
}

.control-btn {
  width: 30px;
  height: 30px;
  border: none;
  border-radius: 6px;
  background-color: transparent;
  color: var(--text-primary, #ffffff);
  font-size: 14px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
}

.control-btn:hover {
  background-color: var(--bg-hover);
  transform: translateY(-1px);
}

.control-btn.close:hover {
  background-color: var(--btn-danger);
  transform: translateY(-1px);
}

.tplayer-container.light .control-btn.close:hover {
  background-color: #ff4757;
}

/* 主内容区 */
.main-content {
  display: flex;
  flex: 1;
  overflow: hidden;
  min-height: 0; /* 确保flex子元素可以正确收缩 */
}

/* 左侧边栏 */
.sidebar {
  width: 250px;
  min-width: 60px;
  max-width: 350px;
  background-color: #2a2a2a;
  border-right: 1px solid #3a3a3a;
  transition: width 0.3s ease;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  flex-shrink: 0; /* 防止边栏被压缩 */
}

.sidebar.collapsed {
  width: 60px;
}

.sidebar-header {
  display: flex;
  align-items: center;
  padding: 20px;
  border-bottom: 1px solid #3a3a3a;
}

.toggle-btn {
  background: none;
  border: none;
  color: var(--text-primary, #ffffff);
  font-size: 16px;
  cursor: pointer;
  margin-right: 10px;
  padding: 6px;
  border-radius: 6px;
  transition: all 0.2s ease;
}

.toggle-btn:hover {
  background-color: var(--bg-hover);
  transform: translateY(-1px);
}

.sidebar-header h2 {
  margin: 0;
  font-size: 16px;
  font-weight: 500;
  transition: opacity 0.3s ease;
}

.sidebar.collapsed .sidebar-header h2 {
  opacity: 0;
  width: 0;
  overflow: hidden;
}

.sidebar-nav {
  flex: 1;
  padding: 20px 0;
  overflow-y: auto;
  /* 隐藏滚动条但保留滚动功能 */
  scrollbar-width: none; /* Firefox */
  -ms-overflow-style: none; /* IE and Edge */
}

/* 隐藏滚动条但保留滚动功能 for Chrome, Safari and Opera */
.sidebar-nav::-webkit-scrollbar {
  display: none;
}

.sidebar-nav ul {
  list-style: none;
  padding: 0;
  margin: 0;
}

.nav-item {
  display: flex;
  align-items: center;
  padding: 10px 20px;
  cursor: pointer;
  transition: background-color 0.2s;
}

.nav-item:hover {
  background-color: rgba(255, 255, 255, 0.1);
}

.nav-item.active {
  background-color: rgba(92, 184, 92, 0.2);
  border-left: 3px solid var(--btn-success);
}

.nav-icon {
  font-size: 18px;
  margin-right: 10px;
}

.nav-text {
  transition: opacity 0.3s ease;
}

.sidebar.collapsed .nav-text {
  opacity: 0;
  width: 0;
  overflow: hidden;
}

.sidebar-footer {
  padding: 20px;
  border-top: 1px solid #3a3a3a;
}

/* ========== 统一按钮样式系统 ========== */

.btn {
  padding: 10px 16px;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
  font-weight: 500;
  transition: all 0.2s ease;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  user-select: none;
}

.btn:hover {
  transform: translateY(-1px);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.15);
}

.btn:active {
  transform: translateY(0);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  transform: none !important;
  box-shadow: none !important;
}

/* 主要按钮 (绿色主题) */
.btn.primary {
  background-color: var(--btn-primary);
  color: #ffffff;
}

.btn.primary:hover {
  background-color: var(--btn-primary-hover);
}

/* 次要按钮 */
.btn.secondary {
  background-color: var(--btn-secondary-bg);
  color: var(--text-primary);
  border: 1px solid var(--border-color);
}

.btn.secondary:hover {
  background-color: var(--btn-secondary-hover);
}

/* 危险按钮 (红色) */
.btn.danger {
  background-color: var(--btn-danger);
  color: #ffffff;
}

.btn.danger:hover {
  background-color: var(--btn-danger-hover);
}

/* 成功按钮 */
.btn.success {
  background-color: var(--btn-success);
  color: #ffffff;
}

.btn.success:hover {
  background-color: var(--btn-success-hover);
}

/* 信息按钮 */
.btn.info {
  background-color: var(--btn-info);
  color: #ffffff;
}

.btn.info:hover {
  background-color: var(--btn-info-hover);
}

/* 右侧内容区 */
.content-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 20px;
  min-width: 0; /* 确保flex子元素可以正确收缩 */
  overflow: hidden;
  transition: margin-left 0.3s ease;
  max-width: 100%;
}

/* 隐藏滚动条但保留滚动功能 for Chrome, Safari and Opera */
.content-area::-webkit-scrollbar {
  display: none;
}

.sidebar-collapsed .content-area {
  margin-left: -190px;
}

/* 过滤控制区 */
.filter-controls {
  margin-bottom: 20px;
}

.filter-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 15px;
  padding: 16px;
  background-color: rgba(255, 255, 255, 0.05);
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.filter-header-left {
  flex: 0 0 auto;
  min-width: 200px;
}

.filter-header-center {
  flex: 1;
  display: flex;
  justify-content: center;
  align-items: center;
  padding: 0 20px;
  min-width: 300px;
}

.filter-header-right {
  flex: 0 0 auto;
  min-width: 120px;
  display: flex;
  justify-content: flex-end;
}

.filter-header h2 {
  margin: 0;
  font-size: 20px;
  font-weight: 500;
}

.playlist-info {
  font-size: 12px;
  color: #888;
  margin: 0;
  display: flex;
  align-items: center;
  gap: 16px;
}

.current-lyric-display {
  padding: 4px 12px;
  background-color: rgba(92, 184, 92, 0.1);
  border-radius: 16px;
  border: 1px solid rgba(92, 184, 92, 0.3);
  max-width: 400px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: center;
}

.current-lyric {
  color: #5cb85c;
  font-size: 14px;
  font-weight: 500;
}

.no-lyric {
  color: #888;
  font-size: 14px;
}

.filter-actions {
  display: flex;
  gap: 10px;
}

.search-box {
  position: relative;
  display: flex;
  align-items: center;
}

.search-box input {
  width: 100%;
  padding: 10px 40px 10px 15px;
  border: 1px solid #3a3a3a;
  border-radius: 20px;
  background-color: #2a2a2a;
  color: #ffffff;
  font-size: 14px;
}

.search-box input::placeholder {
  color: #888;
}

.search-btn {
  position: absolute;
  right: 10px;
  background: none;
  border: none;
  color: #888;
  cursor: pointer;
  font-size: 14px;
  padding: 6px;
  border-radius: 6px;
  transition: all 0.2s ease;
}

.search-btn:hover {
  background-color: var(--bg-hover);
  color: var(--text-primary, #ffffff);
  transform: translateY(-1px);
}

/* 歌曲列表 */
.song-list-container {
  flex: 1;
  overflow: auto;
  display: flex;
  flex-direction: column;
  min-height: 0; /* 确保flex子元素可以正确收缩 */
  max-width: 100%;
  position: relative; /* 为悬浮按钮提供定位上下文 */
}

/* 悬浮控制按钮 */
.playlist-float-buttons {
  position: absolute;
  bottom: 20px;
  right: 20px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  z-index: 1000;
}

.float-button {
  width: 48px;
  height: 48px;
  border-radius: 50%;
  background: var(--btn-success);
  color: #ffffff;
  font-size: 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 4px 12px rgba(92, 184, 92, 0.3);
  transition: all 0.3s ease;
  border: none;
}

.float-button:hover {
  background: var(--btn-success-hover);
  transform: translateY(-2px);
  box-shadow: 0 8px 20px rgba(92, 184, 92, 0.4);
}

.float-button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  background: var(--btn-secondary-bg);
  color: var(--text-secondary);
  box-shadow: none;
}

/* 操作按钮 */
.action-btn {
  background: none;
  border: none;
  color: var(--text-secondary, #888);
  cursor: pointer;
  font-size: 16px;
  padding: 6px;
  border-radius: 4px;
  transition: all 0.2s ease;
  display: flex;
  align-items: center;
  justify-content: center;
}

.action-btn:hover {
  background-color: var(--bg-hover);
  color: var(--text-primary, #ffffff);
  transform: translateY(-1px);
}

.action-btn.favorite.active {
  color: var(--btn-danger);
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 300px;
  color: #888;
}

.empty-icon {
  font-size: 64px;
  margin-bottom: 20px;
}

.empty-state p {
  margin: 5px 0;
}

.empty-hint {
  font-size: 14px;
  color: #666;
}

.song-list {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow-y: auto; /* 允许垂直滚动 */
  overflow-x: hidden; /* 隐藏水平滚动条 */
}

.table-header {
  flex-shrink: 0;
}

.songs-table {
  width: 100%;
  border-collapse: collapse;
}

.songs-table thead tr {
  display: flex;
  align-items: center;
  padding: 12px 15px;
  background-color: var(--bg-secondary, #2a2a2a);
  border-bottom: 1px solid var(--border-color, #3a3a3a);
}

.songs-table th {
  text-align: left;
  font-weight: 500;
  font-size: 14px;
  color: var(--text-secondary, #888);
  padding: 0;
}

.songs-table th.col-index {
  width: 50px;
  text-align: center;
  flex-shrink: 0;
}

.songs-table th.col-title {
  flex: 3;
  min-width: 300px;
  margin-right: 15px;
  text-align: left;
}

.songs-table th.col-artist {
  width: 150px;
  flex-shrink: 0;
  margin-right: 15px;
}

.songs-table th.col-album {
  width: 150px;
  flex-shrink: 0;
  margin-right: 15px;
}

.songs-table th.col-duration {
  width: 80px;
  text-align: right;
  flex-shrink: 0;
  margin-right: 15px;
}

.songs-table th.col-actions {
  width: 50px;
  text-align: center;
  flex-shrink: 0;
}

.virtual-scroller, .song-list {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  /* 隐藏滚动条但保留滚动功能 */
  scrollbar-width: none; /* Firefox */
  -ms-overflow-style: none; /* IE and Edge */
}

/* 艺术家视图 - 双栏布局 */
.artists-view {
  display: flex;
  flex: 1;
  overflow: hidden;
}

.artists-sidebar {
  width: 200px;
  background-color: #252525;
  border-right: 1px solid #3a3a3a;
  overflow-y: auto;
  padding: 10px;
  flex-shrink: 0;
}

.artist-item {
  padding: 12px 15px;
  margin-bottom: 8px;
  border-radius: 8px;
  cursor: pointer;
  transition: background-color 0.2s;
  background-color: #333;
}

.artist-item:hover {
  background-color: #3a3a3a;
}

.artist-item.active {
  background-color: #4a4a4a;
  border: 1px solid #666;
}

.artist-name {
  font-size: 14px;
  font-weight: 500;
  color: #fff;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.artist-count {
  font-size: 12px;
  color: #888;
  margin-top: 4px;
}

.artists-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* 专辑视图 - 双栏布局 */
.albums-view {
  display: flex;
  flex: 1;
  overflow: hidden;
}

.albums-sidebar {
  width: 200px;
  background-color: #252525;
  border-right: 1px solid #3a3a3a;
  overflow-y: auto;
  padding: 10px;
  flex-shrink: 0;
}

.album-item {
  padding: 12px 15px;
  margin-bottom: 8px;
  border-radius: 8px;
  cursor: pointer;
  transition: background-color 0.2s;
  background-color: #333;
}

.album-item:hover {
  background-color: #3a3a3a;
}

.album-item.active {
  background-color: #4a4a4a;
  border: 1px solid #666;
}

.album-name {
  font-size: 14px;
  font-weight: 500;
  color: #fff;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.album-artist {
  font-size: 12px;
  color: #aaa;
  margin-top: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.album-count {
  font-size: 12px;
  color: #888;
  margin-top: 4px;
}

/* CUE专辑视图样式已合并到普通专辑视图样式 */
.cue-badge {
  display: inline-block;
  background-color: #5cb85c;
  color: #fff;
  font-size: 10px;
  padding: 2px 6px;
  border-radius: 4px;
  margin-top: 4px;
}


.nav-badge {
  background-color: #5cb85c;
  color: #fff;
  font-size: 10px;
  padding: 2px 6px;
  border-radius: 10px;
  margin-left: auto;
}

.albums-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.empty-selection {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #888;
  font-size: 16px;
}

/* 隐藏滚动条但保留滚动功能 for Chrome, Safari and Opera */
.virtual-scroller::-webkit-scrollbar, .song-list::-webkit-scrollbar {
  width: 0px;
  height: 0px;
  display: none;
}

.song-row {
  display: flex;
  align-items: center;
  padding: 12px 15px;
  border-bottom: 1px solid #3a3a3a;
  cursor: pointer;
  transition: background-color 0.2s;
}

.song-row:hover {
  background-color: rgba(255, 255, 255, 0.05);
}

.song-row.active {
  background-color: rgba(92, 184, 92, 0.1);
}

.col-index {
  width: 50px;
  text-align: center;
  color: #888;
  flex-shrink: 0;
}

.col-title {
  flex: 3;
  min-width: 300px;
  margin-right: 15px;
  text-align: left;
  display: flex;
  flex-direction: column;
  justify-content: center;
  overflow: hidden;
}

.song-title {
  font-weight: 500;
  margin-bottom: 4px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.song-info {
  font-size: 12px;
  color: #888;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.col-artist {
  width: 150px;
  flex-shrink: 0;
  margin-right: 15px;
}

.col-album {
  width: 150px;
  flex-shrink: 0;
  margin-right: 15px;
}

.col-duration {
  width: 80px;
  text-align: right;
  color: #888;
  flex-shrink: 0;
  margin-right: 15px;
}

.col-actions {
  width: 50px;
  text-align: center;
  flex-shrink: 0;
}

.action-btn {
  background: none;
  border: none;
  color: var(--text-secondary, #888);
  cursor: pointer;
  font-size: 16px;
  margin-left: 10px;
  transition: color 0.2s;
}

.action-btn:hover {
  color: var(--text-primary, #ffffff);
}

.action-btn.favorite {
  background-color: var(--btn-secondary-bg, rgba(255, 255, 255, 0.1));
  border-radius: 4px;
  padding: 4px 8px;
  margin-left: 0;
}

.action-btn.favorite:hover {
  background-color: var(--btn-secondary-hover, rgba(255, 255, 255, 0.15));
}

.action-btn.favorite.active {
  color: var(--btn-danger);
}

/* 均衡器面板 */
.equalizer-panel {
  position: fixed;
  top: 0;
  right: -400px;
  width: 400px;
  height: 100vh;
  background-color: #2a2a2a;
  border-left: 1px solid #3a3a3a;
  transition: right 0.3s ease;
  z-index: 100;
  display: flex;
  flex-direction: column;
}

.equalizer-panel.visible {
  right: 0;
}

.equalizer-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 20px;
  border-bottom: 1px solid #3a3a3a;
}

.equalizer-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 500;
}

.equalizer-header .close-btn {
  background: none;
  border: none;
  color: var(--text-primary);
  font-size: 20px;
  cursor: pointer;
}

.equalizer-content {
  flex: 1;
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.presets select {
  width: 100%;
  padding: 10px;
  border: 1px solid #3a3a3a;
  border-radius: 4px;
  background-color: #1a1a1a;
  color: #ffffff;
  font-size: 14px;
}

.bands {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 15px;
  justify-content: center;
}

.band {
  display: flex;
  align-items: center;
  gap: 10px;
}

.band label {
  width: 60px;
  font-size: 12px;
  color: #888;
}

.band input[type="range"] {
  flex: 1;
  height: 4px;
  background: #3a3a3a;
  border-radius: 2px;
  outline: none;
  appearance: none;
      -webkit-appearance: none;
}

.band input[type="range"]::-webkit-slider-thumb {
  appearance: none;
      -webkit-appearance: none;
  appearance: none;
  width: 16px;
  height: 16px;
  background: #5cb85c;
  border-radius: 50%;
  cursor: pointer;
}

.band input[type="range"]::-moz-range-thumb {
  width: 16px;
  height: 16px;
  background: #5cb85c;
  border-radius: 50%;
  cursor: pointer;
  border: none;
}

.band span {
  width: 50px;
  font-size: 12px;
  text-align: right;
  color: #888;
}

/* 底部播放控制栏 */
.player-controls {
  background-color: #2a2a2a;
  border-top: 1px solid #3a3a3a;
  padding: 10px 20px;
  display: flex;
  align-items: center;
  gap: 20px;
  transition: all 0.3s ease;
  min-height: 140px;
  height: auto;
}

.player-controls.expanded {
  min-height: 180px;
}

.player-left {
  width: 30%;
  flex: 0 0 30%;
  display: flex;
  align-items: center;
  gap: 15px;
  overflow: hidden;
}

.current-song {
  display: flex;
  align-items: center;
  gap: 15px;
}

.song-cover {
  width: 120px;
  height: 120px;
  border-radius: 8px;
  overflow: hidden;
  background-color: #3a3a3a;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  cursor: pointer;
  transition: transform 0.3s ease;
}

.song-cover:hover {
  transform: scale(1.05);
}

.song-cover img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

/* 动态封面样式 */
.song-cover .dynamic-cover {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.cover-placeholder {
  font-size: 24px;
}

.song-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 8px;
}

.song-info h3 {
  margin: 0;
  font-size: 18px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: #fff;
  position: relative;
}

.song-info p {
  margin: 0;
  font-size: 14px;
  color: #aaa;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  position: relative;
}

/* 自动滚屏动画 */
.ellipsis-text {
  display: inline-block;
  animation: scroll 10s linear infinite;
  white-space: nowrap;
}

@keyframes scroll {
  0% {
    transform: translateX(0);
  }
  100% {
    transform: translateX(-100%);
  }
}

/* 为长文本添加滚动容器 */
.song-info h3.long-text {
  position: relative;
  overflow: hidden;
}

.song-info p.long-text {
  position: relative;
  overflow: hidden;
}

/* 为滚动文本添加一些空间，确保滚动时不会完全消失 */
.song-info h3.long-text .ellipsis-text {
  padding-right: 100%;
}

.song-info p.long-text .ellipsis-text {
  padding-right: 100%;
}

/* 当文本过长时显示滚动动画 */
.song-info h3.long-text .ellipsis-text {
  animation-play-state: running;
}

.song-info p.long-text .ellipsis-text {
  animation-play-state: running;
}

/* 当文本不太长时不显示滚动动画 */
.song-info h3:not(.long-text) .ellipsis-text {
  animation: none;
  transform: translateX(0);
}

.song-info p:not(.long-text) .ellipsis-text {
  animation: none;
  transform: translateX(0);
}

.no-song {
  color: #666;
}

.player-center {
  width: 40%;
  flex: 0 0 40%;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  min-width: 0;
  overflow: hidden;
  justify-content: center;
  min-height: 120px;
}

.playback-controls {
  display: flex;
  align-items: center;
  gap: 15px;
}

/* 播放控制按钮 - 统一风格 */
.playback-controls .control-btn {
  width: 36px;
  height: 36px;
  border-radius: 6px;
  background-color: var(--btn-secondary-bg, #f0f0f0);
  color: var(--text-primary, #333333);
  transition: all 0.2s ease;
  border: 1px solid var(--border-color, #e0e0e0);
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  user-select: none;
  padding: 6px;
}

/* 控制按钮图标样式 */
.control-icon {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.tplayer-container.light .playback-controls .control-btn {
  background-color: #f0f0f0;
  color: #333333;
  border: 1px solid #e0e0e0;
}

.tplayer-container .playback-controls .control-btn {
  background-color: rgba(255, 255, 255, 0.1);
  color: #ffffff;
  border: 1px solid rgba(255, 255, 255, 0.2);
}

.playback-controls .control-btn:hover {
  transform: translateY(-1px);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.15);
}

.tplayer-container.light .playback-controls .control-btn:hover {
  background-color: #e0e0e0;
  border-color: #d0d0d0;
}

.tplayer-container .playback-controls .control-btn:hover {
  background-color: rgba(255, 255, 255, 0.2);
  border-color: rgba(255, 255, 255, 0.3);
}

.playback-controls .control-btn:active {
  transform: translateY(0);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

/* 播放按钮特殊样式 - 绿色主题 */
.playback-controls .control-btn.play {
  width: 40px;
  height: 40px;
  font-size: 20px;
  background-color: var(--btn-success);
  color: #ffffff;
  border: none;
  box-shadow: 0 2px 6px rgba(92, 184, 92, 0.3);
}

.playback-controls .control-btn.play:hover {
  background-color: var(--btn-success-hover);
  box-shadow: 0 4px 12px rgba(92, 184, 92, 0.4);
}

.playback-controls .control-btn.play:active {
  box-shadow: 0 2px 6px rgba(92, 184, 92, 0.3);
}

.progress-bar {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.progress-info {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: #888;
}

.progress-bar input[type="range"] {
  width: 100%;
  height: 4px;
  background: #3a3a3a;
  border-radius: 2px;
  outline: none;
  appearance: none;
      -webkit-appearance: none;
}

.progress-bar input[type="range"]::-webkit-slider-thumb {
  appearance: none;
      -webkit-appearance: none;
  appearance: none;
  width: 16px;
  height: 16px;
  background: #5cb85c;
  border-radius: 50%;
  cursor: pointer;
}

.progress-bar input[type="range"]::-moz-range-thumb {
  width: 16px;
  height: 16px;
  background: #5cb85c;
  border-radius: 50%;
  cursor: pointer;
  border: none;
}

.player-right {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  justify-content: center;
  gap: 8px;
  overflow: hidden;
  min-width: 200px;
}

.player-right-top {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 15px;
  width: 100%;
}

/* 下一首歌曲信息 */
.next-song-info {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  background-color: rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  padding: 8px 12px;
  width: 100%;
  box-sizing: border-box;
}

.next-song-label {
  font-size: 10px;
  color: #999;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: 2px;
}

.next-song-title {
  font-size: 12px;
  font-weight: 600;
  color: #ffffff;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
  line-height: 1.3;
}

.next-song-artist {
  font-size: 10px;
  color: #aaa;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
  line-height: 1.2;
}

.skip-next-btn {
  background-color: var(--btn-success);
  color: #ffffff;
  padding: 3px 8px;
  font-size: 10px;
  font-weight: 500;
  margin-top: 4px;
  border: none;
  outline: none;
}

.skip-next-btn:hover {
  background-color: var(--btn-success-hover);
}

.skip-next-btn:focus {
  outline: none;
  box-shadow: none;
}

.volume-control {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 150px;
}

.crossfade-control {
  display: flex;
  align-items: center;
  gap: 10px;
  color: #ffffff;
}

.crossfade-label {
  display: flex;
  align-items: center;
  gap: 5px;
  cursor: pointer;
}

.crossfade-duration {
  display: flex;
  align-items: center;
  gap: 10px;
}

.crossfade-duration input[type="range"] {
  width: 80px;
  height: 4px;
  background: #3a3a3a;
  border-radius: 2px;
  outline: none;
  -webkit-appearance: none;
  appearance: none;
}

.crossfade-duration input[type="range"]::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 12px;
  height: 12px;
  background: #5cb85c;
  border-radius: 50%;
  cursor: pointer;
}

.crossfade-duration input[type="range"]::-moz-range-thumb {
  width: 12px;
  height: 12px;
  background: #5cb85c;
  border-radius: 50%;
  cursor: pointer;
  border: none;
}

.volume-control input[type="range"] {
  flex: 1;
  height: 4px;
  background: #3a3a3a;
  border-radius: 2px;
  outline: none;
  appearance: none;
      -webkit-appearance: none;
}

.volume-control input[type="range"]::-webkit-slider-thumb {
  appearance: none;
      -webkit-appearance: none;
  appearance: none;
  width: 12px;
  height: 12px;
  background: #5cb85c;
  border-radius: 50%;
  cursor: pointer;
}

.volume-control input[type="range"]::-moz-range-thumb {
  width: 12px;
  height: 12px;
  background: #5cb85c;
  border-radius: 50%;
  cursor: pointer;
  border: none;
}

/* 歌曲菜单 */
.song-menu {
  position: fixed;
  background-color: var(--bg-secondary, #2a2a2a);
  border: 1px solid var(--border-color, #3a3a3a);
  border-radius: 4px;
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
  z-index: 1000;
  min-width: 150px;
}

.song-menu ul {
  list-style: none;
  padding: 0;
  margin: 0;
}

.song-menu li {
  padding: 10px 15px;
  cursor: pointer;
  transition: background-color 0.2s;
  color: var(--text-primary, #ffffff);
}

.song-menu li:hover {
  background-color: var(--bg-hover, rgba(255, 255, 255, 0.1));
}

.song-menu li.danger {
  color: #ff4757;
}

.song-menu li.danger:hover {
  background-color: rgba(255, 71, 87, 0.2);
}

/* 编辑歌曲标签模态框 */
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2000;
}

.modal-content {
  background-color: #2a2a2a;
  border: 1px solid #3a3a3a;
  border-radius: 8px;
  width: 400px;
  max-width: 90%;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.7);
}

.settings-modal {
  width: 900px;
  max-width: 95%;
  max-height: 70vh;
  overflow-y: auto;
}

.edit-tags-modal {
  width: 800px;
  max-width: 90%;
  max-height: 90vh;
  display: flex;
  flex-direction: column;
}

/* CUE信息区域 */
.cue-info-section {
  margin-top: 20px;
  padding: 15px;
  background-color: rgba(255, 255, 255, 0.05);
  border-radius: 8px;
  border-left: 4px solid #5cb85c;
}

.cue-info-section h4 {
  margin-top: 0;
  color: #5cb85c;
  font-size: 16px;
  margin-bottom: 15px;
}

.cue-info-text {
  margin-top: 15px;
  padding: 10px;
  background-color: rgba(0, 0, 0, 0.2);
  border-radius: 4px;
  font-family: monospace;
  font-size: 14px;
  line-height: 1.4;
  white-space: pre-wrap;
}

.cue-info-text pre {
  margin: 0;
  color: #f0f0f0;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid #3a3a3a;
}

.modal-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 500;
}

.modal-header .close-btn {
  background: none;
  border: none;
  color: #ffffff;
  font-size: 20px;
  cursor: pointer;
  padding: 0;
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  transition: background-color 0.2s;
}

.modal-header .close-btn:hover {
  background-color: #3a3a3a;
}

.modal-body {
  padding: 12px;
  flex: 1;
  overflow-y: auto;
}

/* 设置窗口的特殊样式 */
.settings-modal .modal-body {
  max-height: calc(70vh - 80px);
  overflow-y: auto;
}

/* 匹配区域 */
.match-section {
  display: flex;
  justify-content: space-between;
  align-items: center;
  background-color: rgba(74, 144, 226, 0.1);
  border: 1px solid rgba(74, 144, 226, 0.3);
  border-radius: 8px;
  padding: 12px 16px;
  margin-bottom: 20px;
}

.match-btn {
  background-color: var(--btn-success);
  color: #ffffff;
  border: none;
  outline: none;
}

.match-btn:hover {
  background-color: var(--btn-success-hover);
}

.match-btn:focus {
  outline: none;
  box-shadow: none;
}

/* 标签页 */
.tabs {
  display: flex;
  flex-direction: column;
}

.tab-buttons {
  display: flex;
  border-bottom: 1px solid #3a3a3a;
  margin-bottom: 20px;
}

.tab-button {
  padding: 10px 20px;
  background: none;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.2s;
  border-bottom: 2px solid transparent;
}

.tab-button:hover {
  color: var(--text-primary);
}

.tab-button.active {
  color: var(--btn-success);
  border-bottom-color: var(--btn-success);
}

.tab-content {
  flex: 1;
}

/* 表单样式 */
.form-row {
  display: flex;
  gap: 16px;
  margin-bottom: 16px;
}

.form-row.three-col {
  gap: 12px;
}

.form-row.three-col .form-group {
  flex: 1;
}

.form-group {
  flex: 1;
  margin-bottom: 0;
}

.form-group label {
  display: block;
  margin-bottom: 8px;
  font-size: 14px;
  color: #cccccc;
}

.form-group input,
.form-group textarea {
  width: 100%;
  padding: 8px 12px;
  background-color: #1a1a1a;
  border: 1px solid #3a3a3a;
  border-radius: 4px;
  color: #ffffff;
  font-size: 14px;
  transition: border-color 0.2s;
}

.form-group input:focus,
.form-group textarea:focus {
  outline: none;
  border-color: #4a90e2;
}

.form-group textarea {
  resize: vertical;
  min-height: 200px;
}

.input-with-button {
  display: flex;
  gap: 8px;
}

.input-with-button input {
  flex: 1;
}

.copy-btn {
  padding: 6px 12px;
  background-color: var(--btn-secondary-bg);
  color: var(--text-primary);
  border: 1px solid var(--border-color);
  white-space: nowrap;
}

.copy-btn:hover {
  background-color: var(--btn-secondary-hover);
}

/* 歌词操作 */
.lyric-actions {
  margin-top: 12px;
  display: flex;
  gap: 10px;
}

/* 歌词显示区域 */
.lyrics-display {
  width: 100%;
  min-height: 60px;
  max-height: 80px;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: rgba(0, 0, 0, 0.3);
  border: none;
  border-radius: 8px;
  margin-top: 10px;
  position: relative;
  z-index: 100;
  padding: 8px;
}

.lyrics-display.has-lyrics {
  background-color: rgba(0, 0, 0, 0.2);
  border: none;
}

.lyrics-placeholder {
  color: #888;
  font-size: 14px;
  text-align: center;
  padding: 10px;
}

.lyrics-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-start;
  gap: 8px;
  width: 100%;
  padding: 10px 0;
  overflow-y: auto;
  max-height: 100%;
}

.lyric-line {
  font-size: 14px;
  color: #ccc;
  text-align: center;
  transition: all 0.3s ease;
  opacity: 0.7;
  padding: 3px 10px;
  word-wrap: break-word;
  max-width: 100%;
  line-height: 1.4;
}

.lyric-line.active {
  font-size: 16px;
  color: #5cb85c;
  font-weight: bold;
  opacity: 1;
  transform: scale(1.08);
  text-shadow: 0 0 8px rgba(92, 184, 92, 0.5);
}

/* 淡色主题 */
.tplayer-container.light {
  background-color: #f8f9fa;
  color: #333333;
}

.tplayer-container.light body,
.tplayer-container.light html {
  background-color: #f8f9fa;
}

.tplayer-container.light .top-bar {
  background-color: #ffffff;
  border-bottom: 1px solid #e0e0e0;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
}

.tplayer-container.light .app-logo h1 {
  color: #5cb85c;
}

.tplayer-container.light .control-btn {
  color: #333333;
}

.tplayer-container.light .control-btn:hover {
  background-color: rgba(0, 0, 0, 0.08);
}

.tplayer-container.light .sidebar {
  background-color: #ffffff;
  border-right: 1px solid #e0e0e0;
  box-shadow: 1px 0 3px rgba(0, 0, 0, 0.05);
}

.tplayer-container.light .sidebar-header {
  border-bottom: 1px solid #e0e0e0;
}

.tplayer-container.light .nav-item:hover {
  background-color: rgba(0, 0, 0, 0.08);
}

.tplayer-container.light .nav-item.active {
  background-color: rgba(92, 184, 92, 0.15);
  border-left: 3px solid #5cb85c;
  font-weight: 500;
}

.tplayer-container.light .sidebar-footer {
  border-top: 1px solid #e0e0e0;
}

/* 浅色主题按钮会自动使用 CSS 变量，无需额外定义 */

.tplayer-container.light .content-area {
  background-color: #f8f9fa;
}

.tplayer-container.light .search-box input {
  border: 1px solid #e0e0e0;
  background-color: #ffffff;
  color: #333333;
}

.tplayer-container.light .search-box input:focus {
  border-color: #5cb85c;
  outline: none;
}

.tplayer-container.light .search-box input::placeholder {
  color: #999999;
}

.tplayer-container.light .search-btn {
  color: #999999;
  transition: color 0.2s ease;
}

.tplayer-container.light .search-btn:hover {
  color: #5cb85c;
}

.tplayer-container.light .songs-table {
  background-color: var(--bg-secondary);
}

.tplayer-container.light .songs-table th {
  background-color: #f8f9fa;
  border-bottom: 2px solid #e0e0e0;
  color: #666666;
}

.tplayer-container.light .song-row {
  border-bottom: 1px solid #f0f0f0;
  padding: 12px 15px;
}

.tplayer-container.light .song-row:hover {
  background-color: rgba(0, 0, 0, 0.03);
}

.tplayer-container.light .song-row.active {
  background-color: rgba(92, 184, 92, 0.12);
  border-left: 3px solid #5cb85c;
}

.tplayer-container.light .col-index,
.tplayer-container.light .col-duration {
  color: #999999;
}

.tplayer-container.light .song-info {
  color: #666666;
}

.tplayer-container.light .action-btn {
  color: #999999;
}

.tplayer-container.light .action-btn:hover {
  color: #333333;
  background-color: rgba(0, 0, 0, 0.05);
}

.tplayer-container.light .player-controls {
  background-color: #ffffff;
  border-top: 1px solid #e0e0e0;
  padding: 10px 20px;
}

.tplayer-container.light .lyrics-display {
  background-color: rgba(255, 255, 255, 0.8);
  border: none;
}

.tplayer-container.light .lyrics-display.has-lyrics {
  background-color: rgba(255, 255, 255, 0.9);
  border: none;
}

.tplayer-container.light .lyrics-placeholder {
  color: #999;
  font-style: italic;
}

.tplayer-container.light .lyric-line {
  color: #555;
  opacity: 0.8;
}

.tplayer-container.light .lyric-line.active {
  color: #2e7d32;
  font-size: 16px;
  font-weight: 500;
  opacity: 1;
}

.tplayer-container.light .progress-info span {
  color: #666666;
  font-size: 12px;
}

.tplayer-container.light .progress-bar input[type="range"] {
  background: #e0e0e0;
  height: 6px;
  border-radius: 3px;
}

.tplayer-container.light .progress-bar input[type="range"]::-webkit-slider-thumb {
  background: #5cb85c;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  transition: all 0.2s ease;
}

.tplayer-container.light .progress-bar input[type="range"]::-webkit-slider-thumb:hover {
  transform: scale(1.1);
  box-shadow: 0 4px 8px rgba(92, 184, 92, 0.4);
}

.tplayer-container.light .progress-bar input[type="range"]::-moz-range-thumb {
  background: #5cb85c;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  border: none;
  transition: all 0.2s ease;
}

.tplayer-container.light .progress-bar input[type="range"]::-moz-range-thumb:hover {
  transform: scale(1.1);
  box-shadow: 0 4px 8px rgba(92, 184, 92, 0.4);
}

.tplayer-container.light .volume-control input[type="range"] {
  background: #e0e0e0;
  height: 4px;
  border-radius: 2px;
}

.tplayer-container.light .volume-control input[type="range"]::-webkit-slider-thumb {
  background: #5cb85c;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  transition: all 0.2s ease;
}

.tplayer-container.light .volume-control input[type="range"]::-webkit-slider-thumb:hover {
  transform: scale(1.1);
  box-shadow: 0 2px 6px rgba(92, 184, 92, 0.4);
}

.tplayer-container.light .volume-control input[type="range"]::-moz-range-thumb {
  background: #5cb85c;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  border: none;
  transition: all 0.2s ease;
}

.tplayer-container.light .volume-control input[type="range"]::-moz-range-thumb:hover {
  transform: scale(1.1);
  box-shadow: 0 2px 6px rgba(92, 184, 92, 0.4);
}

.tplayer-container.light .equalizer-panel {
  background-color: #ffffff;
  border-left: 1px solid #e0e0e0;
}

.tplayer-container.light .equalizer-header {
  border-bottom: 1px solid #e0e0e0;
  background-color: #f8f9fa;
  padding: 20px;
}

.tplayer-container.light .equalizer-header .close-btn {
  color: #333333;
}

.tplayer-container.light .equalizer-header .close-btn:hover {
  background-color: rgba(0, 0, 0, 0.05);
}

.tplayer-container.light .presets select {
  border: 1px solid #e0e0e0;
  background-color: #ffffff;
  color: #333333;
}

.tplayer-container.light .presets select:focus {
  border-color: #5cb85c;
  outline: none;
}

.tplayer-container.light .band input[type="range"] {
  background: #e0e0e0;
  height: 4px;
  border-radius: 2px;
}

.tplayer-container.light .band input[type="range"]::-webkit-slider-thumb {
  background: #5cb85c;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  transition: all 0.2s ease;
}

.tplayer-container.light .band input[type="range"]::-webkit-slider-thumb:hover {
  transform: scale(1.1);
  box-shadow: 0 2px 6px rgba(92, 184, 92, 0.4);
}

.tplayer-container.light .band input[type="range"]::-moz-range-thumb {
  background: #5cb85c;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  border: none;
  transition: all 0.2s ease;
}

.tplayer-container.light .band input[type="range"]::-moz-range-thumb:hover {
  transform: scale(1.1);
  box-shadow: 0 2px 6px rgba(92, 184, 92, 0.4);
}

.tplayer-container.light .band span {
  color: #666666;
  font-size: 12px;
  font-weight: 500;
}

.tplayer-container.light .lyric-line {
  color: #666666;
}

.tplayer-container.light .lyric-line.active {
  color: #333333;
  font-weight: 500;
}

.tplayer-container.light .modal-content {
  background-color: #ffffff;
  color: #333333;
  border: none;
}

.tplayer-container.light .modal-header {
  border-bottom: 1px solid #e0e0e0;
  background-color: #f8f9fa;
  padding: 12px 16px;
}

.tplayer-container.light .form-group label {
  color: #666666;
  font-weight: 500;
  margin-bottom: 8px;
  display: block;
}

.tplayer-container.light .form-group input,
.tplayer-container.light .form-group select,
.tplayer-container.light .form-group textarea {
  border: 1px solid #e0e0e0;
  background-color: #ffffff;
  color: #333333;
}

.tplayer-container.light .form-group input:focus,
.tplayer-container.light .form-group select:focus,
.tplayer-container.light .form-group textarea:focus {
  border-color: #5cb85c;
  outline: none;
}

.tplayer-container.light .form-group input::placeholder,
.tplayer-container.light .form-group textarea::placeholder {
  color: #999999;
}

.tplayer-container.light .form-actions button {
  background-color: #e0e0e0;
  color: #333333;
  border: none;
  cursor: pointer;
}

.tplayer-container.light .form-actions button:hover {
  background-color: #d0d0d0;
}

.tplayer-container.light .form-actions button.primary {
  background-color: #5cb85c;
  color: #ffffff;
  box-shadow: 0 2px 4px rgba(92, 184, 92, 0.3);
}

.tplayer-container.light .form-actions button.primary:hover {
  background-color: #45a049;
  transform: translateY(-1px);
  box-shadow: 0 4px 8px rgba(92, 184, 92, 0.4);
}

.tplayer-container.light .lyric-actions button {
  border: 1px solid #e0e0e0;
  background: #f8f9fa;
  color: #333333;
  border-radius: 8px;
  padding: 8px 16px;
  transition: all 0.2s ease;
  margin-right: 8px;
  font-weight: 500;
}

.tplayer-container.light .lyric-actions button:hover {
  background: #e0e0e0;
}

/* 艺术家视图 - 浅色主题 */
.tplayer-container.light .artists-sidebar {
  background-color: #f8f9fa;
  border-right: 1px solid #e0e0e0;
}

.tplayer-container.light .artist-item {
  background-color: #ffffff;
  border: 1px solid #e0e0e0;
}

.tplayer-container.light .artist-item:hover {
  background-color: #f8f9fa;
}

.tplayer-container.light .artist-item.active {
  background-color: #e8f5e9;
  border: 2px solid #5cb85c;
}

.tplayer-container.light .artist-name {
  color: #333333;
  font-weight: 600;
  margin-bottom: 4px;
}

.tplayer-container.light .artist-count {
  color: #666666;
  font-size: 14px;
}

/* 专辑视图 - 浅色主题 */
.tplayer-container.light .albums-sidebar {
  background-color: #f8f9fa;
  border-right: 1px solid #e0e0e0;
}

.tplayer-container.light .album-item {
  background-color: #ffffff;
  border: 1px solid #e0e0e0;
}

.tplayer-container.light .album-item:hover {
  background-color: #f8f9fa;
}

.tplayer-container.light .album-item.active {
  background-color: #e8f5e9;
  border: 2px solid #5cb85c;
}

.tplayer-container.light .album-name {
  color: #333333;
  font-weight: 600;
  margin-bottom: 4px;
}

.tplayer-container.light .album-artist {
  color: #666666;
  font-size: 14px;
  margin-bottom: 2px;
}

.tplayer-container.light .album-count {
  color: #666666;
  font-size: 14px;
  font-style: italic;
}

.tplayer-container.light .empty-selection {
  color: #666666;
}

/* 其他浅色主题样式 */
.tplayer-container.light .toggle-btn {
  color: #333333;
}

.tplayer-container.light .toggle-btn:hover {
  background-color: rgba(0, 0, 0, 0.05);
}

.tplayer-container.light .nav-text {
  color: #333333;
  font-weight: 400;
}

.tplayer-container.light .empty-icon {
  color: #999999;
  font-size: 48px;
  margin-bottom: 16px;
}

.tplayer-container.light .empty-hint {
  color: #666666;
  font-size: 16px;
  text-align: center;
  padding: 32px;
}

.tplayer-container.light .filter-header {
  background-color: rgba(0, 0, 0, 0.05);
  border: 1px solid rgba(0, 0, 0, 0.1);
}

.tplayer-container.light .filter-header h2 {
  color: #333333;
  font-size: 18px;
  font-weight: 600;
  margin: 0;
}

.tplayer-container.light .song-title {
  color: #333333;
  font-weight: 500;
  margin-bottom: 4px;
}

.tplayer-container.light .col-title {
  color: #333333;
  font-weight: 500;
}

.tplayer-container.light .col-artist {
  color: #666666;
}

.tplayer-container.light .col-album {
  color: #666666;
}

.tplayer-container.light .col-duration {
  color: #666666;
}

.tplayer-container.light .action-btn {
  color: #666666;
}

.tplayer-container.light .action-btn:hover {
  color: #333333;
}

.tplayer-container.light .action-btn.active {
  color: #e91e63;
  font-weight: 500;
}

.tplayer-container.light .song-info {
  color: #666666;
}

.tplayer-container.light .song-cover {
  background-color: #f8f9fa;
}

.tplayer-container.light .cover-placeholder {
  color: #999999;
  font-size: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  background-color: #f0f0f0;
}

.tplayer-container.light .song-info h3 {
  color: #333333;
  font-size: 16px;
  font-weight: 600;
  margin: 0 0 8px 0;
}

.tplayer-container.light .song-info p {
  color: #666666;
  font-size: 14px;
  margin: 0 0 4px 0;
}

/* 浅色主题播放控制按钮样式已在前面统一定义 */

/* 浅色主题 - 下一首歌曲信息 */
.tplayer-container.light .player-right {
  align-items: stretch;
}

.tplayer-container.light .next-song-info {
  background-color: rgba(0, 0, 0, 0.05);
  border: 1px solid #e0e0e0;
}

.tplayer-container.light .next-song-label {
  color: #666666;
}

.tplayer-container.light .next-song-title {
  color: #333333;
}

.tplayer-container.light .next-song-artist {
  color: #666666;
}

/* 浅色主题按钮会自动使用 CSS 变量，无需额外定义 */

.tplayer-container.light .modal-header .close-btn {
  color: var(--text-secondary);
}

.tplayer-container.light .modal-header .close-btn:hover {
  color: var(--text-primary);
  background-color: var(--bg-hover);
}

/* 浅色主题 - 标签页 */
.tplayer-container.light .tab-buttons {
  border-bottom: 1px solid #e0e0e0;
}

/* 浅色主题标签页会自动使用 CSS 变量，无需额外定义 */

/* 浅色主题 - 表单 */
.tplayer-container.light .form-group label {
  color: #333333;
}

.tplayer-container.light .form-group input,
.tplayer-container.light .form-group textarea {
  background-color: #ffffff;
  border: 1px solid #e0e0e0;
  color: #333333;
}

.tplayer-container.light .form-group input:focus,
.tplayer-container.light .form-group textarea:focus {
  border-color: #5cb85c;
  outline: none;
}

/* 浅色主题 - 匹配区域 */
.tplayer-container.light .match-section {
  background-color: rgba(92, 184, 92, 0.1);
  border: 1px solid rgba(92, 184, 92, 0.3);
}

/* 浅色主题按钮会自动使用 CSS 变量，无需额外定义 */

/* 浅色主题 - 按钮 */
/* 浅色主题按钮会自动使用 CSS 变量，无需额外定义 */

/* 浅色主题 - 封面预览 */
.tplayer-container.light .cover-preview {
  background-color: #f8f9fa;
  border: 1px solid #e0e0e0;
}

.tplayer-container.light .cover-placeholder {
  color: #999999;
}

/* 浅色主题 - 模态框底部 */
.tplayer-container.light .modal-footer {
  border-top: 1px solid #e0e0e0;
}

/* 浅色主题 - 播放器左侧 */
.tplayer-container.light .player-left {
  background-color: transparent;
}

/* 浅色主题 - 当前歌曲信息 */
.tplayer-container.light .current-song .song-title {
  color: #333333;
}

.tplayer-container.light .current-song .song-artist {
  color: #666666;
}

/* 浅色主题 - 歌曲信息 */
.tplayer-container.light .song-info .song-title {
  color: #333333;
}

.tplayer-container.light .song-info .song-artist {
  color: #666666;
}

/* 浅色主题 - 歌曲封面 */
.tplayer-container.light .song-cover {
  background-color: #f8f9fa;
  border: 1px solid #e0e0e0;
}

/* 浅色主题 - 操作按钮 */
.tplayer-container.light .action-btn {
  color: var(--text-secondary);
}

.tplayer-container.light .action-btn:hover {
  color: var(--text-primary);
}

.tplayer-container.light .action-btn.favorite {
  color: var(--text-secondary);
}

.tplayer-container.light .action-btn.favorite:hover {
  color: var(--text-primary);
}

.tplayer-container.light .action-btn.favorite.active {
  color: var(--btn-danger);
}

/* 浅色主题 - 表头 */
.tplayer-container.light .songs-table thead tr {
  background-color: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
}

.tplayer-container.light .songs-table th {
  color: var(--text-secondary);
}

/* 浅色主题 - 均衡器 */
.tplayer-container.light .equalizer-header h3 {
  color: #333333;
}

/* 浅色主题 - 播放列表信息 */
.tplayer-container.light .playlist-info {
  color: #666666;
}

/* 浅色主题 - 歌词状态 */
.tplayer-container.light .lyric-status {
  background-color: #e0e0e0;
  color: #666666;
}

.tplayer-container.light .lyric-status.has-lyrics {
  background-color: #5cb85c;
  color: #ffffff;
}

/* 浅色主题 - 当前歌词显示 */
.tplayer-container.light .current-lyric-display {
  background-color: rgba(92, 184, 92, 0.1);
  border: 1px solid rgba(92, 184, 92, 0.3);
}

.tplayer-container.light .current-lyric {
  color: #5cb85c;
}

.tplayer-container.light .no-lyric {
  color: #666666;
}

/* 浅色主题 - 侧边栏标题 */
.tplayer-container.light .sidebar-header h2 {
  color: #333333;
}

/* 浅色主题 - 设置模态框 */
.tplayer-container.light .settings-modal .settings-section {
  background-color: rgba(0, 0, 0, 0.05);
  border: 1px solid rgba(0, 0, 0, 0.1);
}

.tplayer-container.light .settings-modal h3 {
  color: var(--text-primary);
  border-bottom: 2px solid rgba(92, 184, 92, 0.3);
}

.tplayer-container.light .settings-modal .setting-label {
  color: var(--text-primary);
}

.tplayer-container.light .settings-modal .setting-value,
.tplayer-container.light .settings-modal .setting-control span {
  color: var(--text-secondary);
}

.tplayer-container.light .settings-modal .setting-item {
  border-bottom: 1px solid rgba(0, 0, 0, 0.1);
}

.tplayer-container.light .settings-modal .setting-control input[type="range"] {
  background: #e0e0e0;
}

.tplayer-container.light .settings-modal .setting-control select {
  background-color: var(--bg-secondary);
  color: var(--text-primary);
  border: 1px solid var(--border-color);
}

.tplayer-container.light .settings-modal .btn-secondary {
  background-color: var(--btn-secondary-bg);
  color: var(--text-primary);
  border: 1px solid var(--border-color);
}

.tplayer-container.light .settings-modal .btn-secondary:hover {
  background-color: var(--btn-secondary-hover);
}

.tplayer-container.light .settings-modal .settings-actions {
  border-top: 1px solid rgba(0, 0, 0, 0.1);
}

/* 浅色主题 - 封面模态框 */
.tplayer-container.light .cover-modal-content {
  background-color: rgba(255, 255, 255, 0.95);
}

.tplayer-container.light .cover-modal-placeholder {
  background: linear-gradient(135deg, #f0f0f0, #e0e0e0);
  color: #999;
}

.tplayer-container.light .cover-modal-title {
  color: #333;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
}

.tplayer-container.light .cover-modal-artist {
  color: #666;
}

.tplayer-container.light .cover-modal-album {
  color: #999;
}

.tplayer-container.light .cover-lyric-line {
  color: rgba(0, 0, 0, 0.4);
}

.tplayer-container.light .cover-lyric-line.active {
  color: #000;
  font-weight: 700;
  text-shadow: 0 0 20px rgba(0, 0, 0, 0.2), 0 0 40px rgba(0, 0, 0, 0.1), 0 2px 8px rgba(0, 0, 0, 0.1);
  transform: scale(1.05);
}

.tplayer-container.light .cover-modal-no-lyrics {
  color: rgba(0, 0, 0, 0.5);
}

.tplayer-container.light .cover-modal-close {
  background-color: rgba(0, 0, 0, 0.1);
  color: #333;
}

.tplayer-container.light .cover-modal-close:hover {
  background-color: rgba(0, 0, 0, 0.2);
}

.tplayer-container.light .cover-modal-header {
  background: linear-gradient(to bottom, rgba(0, 0, 0, 0.1), transparent);
}

.tplayer-container.light .cover-modal-drag-hint {
  color: rgba(0, 0, 0, 0.5);
}

/* 浅色主题按钮会自动使用 CSS 变量，无需额外定义 */

/* 封面部分 */
.cover-section {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 20px;
}

.cover-preview {
  width: 300px;
  height: 300px;
  border-radius: 8px;
  overflow: hidden;
  background-color: #1a1a1a;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.2s;
}

.cover-preview:hover {
  transform: scale(1.02);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
}

.cover-preview img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.cover-placeholder {
  color: #888;
  font-size: 14px;
  text-align: center;
  padding: 20px;
}

/* 封面模态框 */
.cover-modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.9);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2000;
  padding: 40px;
}

.cover-modal-content {
  position: relative;
  background-color: rgba(30, 30, 30, 0.95);
  border-radius: 16px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

/* 窗口模式 */
.cover-modal-content.windowed {
  width: 100%;
  max-width: 1200px;
  height: 80vh;
  position: fixed;
}

/* 全屏模式 */
.cover-modal-content.fullscreen {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  width: 100vw;
  height: 100vh;
  border-radius: 0;
  max-width: none;
}

/* 拖动标题栏 */
.cover-modal-header {
  position: relative;
  z-index: 10;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 16px;
  background: linear-gradient(to bottom, rgba(0, 0, 0, 0.3), transparent);
  cursor: move;
  user-select: none;
}

.cover-modal-drag-hint {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
}

.cover-modal-controls {
  display: flex;
  gap: 8px;
}

.cover-modal-btn {
  width: 32px;
  height: 32px;
  border: none;
  background-color: var(--btn-secondary-bg);
  color: var(--text-primary);
  font-size: 14px;
  border-radius: 6px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
}

.cover-modal-btn:hover {
  background-color: var(--btn-secondary-hover);
}

/* 全屏模式下的调整 */
.cover-modal-content.fullscreen .cover-modal-body {
  padding: 60px 80px;
}

.cover-modal-content.fullscreen .cover-modal-image {
  width: 500px;
  height: 500px;
}

.cover-modal-content.fullscreen .cover-modal-lyrics {
  font-size: 22px;
}

.cover-modal-content.fullscreen .cover-lyric-line {
  font-size: 24px;
  color: rgba(255, 255, 255, 0.35);
}

.cover-modal-content.fullscreen .cover-lyric-line.active {
  font-size: 36px;
  font-weight: 700;
  color: #fff;
  text-shadow: 0 0 30px rgba(255, 255, 255, 0.6), 0 0 60px rgba(255, 255, 255, 0.4), 0 2px 10px rgba(0, 0, 0, 0.5);
  transform: scale(1.08);
}

.tplayer-container.light .cover-modal-content.fullscreen .cover-lyric-line {
  color: rgba(0, 0, 0, 0.35);
}

.tplayer-container.light .cover-modal-content.fullscreen .cover-lyric-line.active {
  color: #000;
  text-shadow: 0 0 30px rgba(0, 0, 0, 0.3), 0 0 60px rgba(0, 0, 0, 0.2), 0 2px 10px rgba(0, 0, 0, 0.1);
}

.cover-modal-background {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-size: cover;
  background-position: center;
  filter: blur(60px) brightness(0.4);
  z-index: 0;
}

.cover-modal-body {
  position: relative;
  z-index: 1;
  display: flex;
  flex: 1;
  padding: 40px;
  gap: 60px;
  overflow: hidden;
}

.cover-modal-left {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 30px;
  flex-shrink: 0;
}

.cover-modal-image {
  width: 400px;
  height: 400px;
  border-radius: 12px;
  overflow: hidden;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
}

.cover-modal-image img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.cover-modal-placeholder {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #2a2a2a, #1a1a1a);
  color: #666;
  font-size: 120px;
}

.cover-modal-info {
  text-align: center;
  color: #fff;
}

.cover-modal-title {
  font-size: 28px;
  font-weight: 600;
  margin: 0 0 12px 0;
  color: #fff;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
}

.cover-modal-artist {
  font-size: 18px;
  margin: 0 0 8px 0;
  color: rgba(255, 255, 255, 0.8);
}

.cover-modal-album {
  font-size: 14px;
  margin: 0;
  color: rgba(255, 255, 255, 0.6);
}

.cover-modal-right {
  flex: 1;
  display: flex;
  flex-direction: column;
  justify-content: center;
  overflow: hidden;
}

.cover-modal-lyrics {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
  padding-top: 40%;
  padding-bottom: 40%;
  text-align: center;
  mask-image: linear-gradient(to bottom, transparent 0%, black 10%, black 90%, transparent 100%);
  -webkit-mask-image: linear-gradient(to bottom, transparent 0%, black 10%, black 90%, transparent 100%);
  scroll-behavior: smooth;
}

.cover-modal-lyrics::-webkit-scrollbar {
  width: 4px;
}

.cover-modal-lyrics::-webkit-scrollbar-track {
  background: transparent;
}

.cover-modal-lyrics::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.2);
  border-radius: 2px;
}

.cover-lyric-line {
  font-size: 20px;
  line-height: 2;
  color: rgba(255, 255, 255, 0.4);
  transition: all 0.3s ease;
  padding: 8px 0;
}

.cover-lyric-line.active {
  font-size: 28px;
  font-weight: 700;
  color: #fff;
  text-shadow: 0 0 20px rgba(255, 255, 255, 0.5), 0 0 40px rgba(255, 255, 255, 0.3), 0 2px 8px rgba(0, 0, 0, 0.5);
  transform: scale(1.05);
}

.cover-modal-no-lyrics {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 18px;
  color: rgba(255, 255, 255, 0.5);
}

.cover-modal-close {
  position: absolute;
  top: 20px;
  right: 20px;
  width: 40px;
  height: 40px;
  border: none;
  background-color: rgba(255, 255, 255, 0.1);
  color: #fff;
  font-size: 20px;
  border-radius: 50%;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
  z-index: 10;
}

.cover-modal-close:hover {
  background-color: rgba(255, 255, 255, 0.2);
  transform: scale(1.1);
}

.cover-actions {
  display: flex;
  gap: 10px;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  padding: 16px;
  border-top: 1px solid #3a3a3a;
  gap: 10px;
}

/* 取消按钮 */
.btn-cancel {
  background-color: var(--btn-secondary-bg);
  color: var(--text-primary);
  border: 1px solid var(--border-color);
  outline: none;
}

.btn-cancel:hover {
  background-color: var(--btn-secondary-hover);
}

.btn-cancel:focus {
  outline: none;
  box-shadow: none;
}

/* 保存按钮 */
.btn-save {
  background-color: var(--btn-success);
  color: #ffffff;
  border: none;
  outline: none;
}

.btn-save:hover {
  background-color: var(--btn-success-hover);
}

.btn-save:focus {
  outline: none;
  box-shadow: none;
}

/* 启动画面 */
.splash-screen {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background: linear-gradient(135deg, #2c3e50 0%, #34495e 100%);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 9999;
  animation: fadeIn 0.5s ease-in-out;
}

.splash-content {
  text-align: center;
  color: white;
  padding: 40px;
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(10px);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
  border: 1px solid rgba(255, 255, 255, 0.2);
  max-width: 400px;
  width: 90%;
  animation: slideUp 0.8s ease-out;
}

.splash-logo {
  width: 100px;
  height: 100px;
  margin-bottom: 20px;
  animation: pulse 2s infinite ease-in-out;
}

.splash-title {
  font-size: 2.5rem;
  font-weight: 700;
  margin-bottom: 10px;
  color: #ffffff;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
}

.splash-slogan {
  font-size: 1.2rem;
  margin-bottom: 30px;
  color: rgba(255, 255, 255, 0.8);
  font-style: italic;
}

.splash-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 15px;
}

.loading-spinner {
  width: 40px;
  height: 40px;
  border: 4px solid rgba(255, 255, 255, 0.3);
  border-top: 4px solid #ffffff;
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

.splash-loading span {
  color: rgba(255, 255, 255, 0.9);
  font-size: 1rem;
}

/* 动画效果 */
@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@keyframes slideUp {
  from {
    opacity: 0;
    transform: translateY(30px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes pulse {
  0% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.05);
  }
  100% {
    transform: scale(1);
  }
}

@keyframes spin {
  0% {
    transform: rotate(0deg);
  }
  100% {
    transform: rotate(360deg);
  }
}

/* 响应式设计 */
@media (max-width: 768px) {
  .splash-content {
    padding: 30px;
  }
  
  .splash-title {
    font-size: 2rem;
  }
  
  .splash-slogan {
    font-size: 1rem;
  }
  
  .splash-logo {
    width: 80px;
    height: 80px;
  }
}

/* 滚动条样式 - 隐藏滚动条但保留滚动功能 */
.tplayer-container ::-webkit-scrollbar,
.tplayer-container ::-webkit-scrollbar-horizontal,
.tplayer-container ::-webkit-scrollbar-vertical {
  width: 0;
  height: 0;
  display: none;
}

.tplayer-container {
  scrollbar-width: none;
  -ms-overflow-style: none;
}

/* 确保body和html不显示滚动条 */
body, html {
  overflow: hidden !important;
}
</style>