<template>
  <div class="app">
    <!-- 顶部导航栏 -->
    <header class="app-header" data-tauri-drag-region>
      <div class="header-left">
        <div class="app-logo">
          <img src="/logo.png" alt="TPlayer Logo" class="logo-img" />
        </div>
        <h1 class="app-title">TPlayer</h1>
        <div class="header-actions">
          <button @click="scanDirectory" class="btn-primary">
            扫描目录
          </button>
        </div>
      </div>
      <div class="header-right">
        <button @click="toggleSettings" class="btn-icon" :class="{ active: showSettings }" title="设置">
          ⚙️
        </button>
        <div class="window-controls">
            <button @click="minimizeWindow" class="window-control-btn" title="最小化">
              —
            </button>
            <button @click="maximizeWindow" class="window-control-btn" title="最大化">
              ▢
            </button>
            <button @click="closeWindow" class="window-control-btn close-btn" title="关闭">
              ✕
            </button>
          </div>
      </div>
    </header>

    <div class="main-content">
      <!-- 左侧边栏 -->
      <aside :class="{ 'sidebar': true, 'sidebar-hidden': !sidebarVisible }">
        <div class="sidebar-header">
          <h3>音乐库</h3>
          <button @click="toggleSidebar" class="sidebar-toggle-btn" title="隐藏侧边栏">
            {{ sidebarVisible ? '◀' : '▶' }}
          </button>
        </div>
        <div class="sidebar-section">
          <h3>本地音乐</h3>
          <div class="filter-buttons">
            <button 
              v-for="filter in filters" 
              :key="filter"
              :class="{ active: currentFilter === filter }"
              @click="() => { currentFilter = filter; showAllTracks(); }"
            >
              {{ filter }}
            </button>
          </div>
        </div>

        <div class="sidebar-section">
          <h3>我的收藏</h3>
          <div class="playlist-list">
            <div 
              class="playlist-item"
              :class="{ active: showingFavorites }"
              @click="showFavorites"
            >
              <div class="playlist-item-content">
                <span class="playlist-name">♥ 我喜欢的</span>
                <span class="playlist-count">{{ localStore.favoriteSongs.length }} 首</span>
              </div>
            </div>
          </div>
        </div>

        <div class="sidebar-section">
          <h3>本地歌单</h3>
          <div class="playlist-list">
            <div 
              v-for="playlist in localStore.localPlaylists" 
              :key="playlist.id"
              class="playlist-item"
            >
              <div class="playlist-item-content" @click="loadPlaylist(playlist.id)">
                <span class="playlist-name">{{ playlist.name }}</span>
                <span class="playlist-count">{{ playlist.songs.length }} 首</span>
              </div>
              <div class="playlist-item-actions">
                <button @click.stop="editPlaylist(playlist)" class="playlist-action-btn" title="编辑歌单">✏️</button>
                <button @click.stop="confirmDeletePlaylist(playlist.id)" class="playlist-action-btn" title="删除歌单">🗑️</button>
              </div>
            </div>
            <div class="playlist-item add-playlist" @click="showCreatePlaylistModal = true">
              <span class="playlist-name">+ 创建歌单</span>
            </div>
          </div>
        </div>
      </aside>
      <!-- 侧边栏悬停触发器 -->
      <div v-if="!sidebarVisible" class="sidebar-trigger" @mouseenter="sidebarVisible = true"></div>

      <!-- 主内容区 -->
      <main class="content">
        <!-- 歌曲列表 -->
        <div class="song-list-container">
          <div class="list-header">
            <div class="header-left">
              <h3>{{ getListTitle() }} ({{ filteredTracks.length }} 首)</h3>
            </div>
            <div class="list-actions">
              <div class="search-box">
                <input 
                  v-model="searchQuery" 
                  type="text" 
                  placeholder="搜索歌曲、歌手、专辑..."
                  class="search-input"
                />
              </div>
              <button v-if="currentFilter !== '曲目'" @click="currentFilter = '曲目'" class="btn-secondary">
                显示全部
              </button>
              <button v-if="selectedTracks.length > 0" @click="deselectAll" class="btn-secondary">
                取消全选
              </button>
              <button v-if="selectedTracks.length > 0" @click="clearSelected" class="btn-secondary">
                清除选中
              </button>
              <button @click="clearAll" class="btn-secondary" :disabled="tracks.length === 0">
                清除全部
              </button>
            </div>
          </div>
          
          <div class="song-list">
            <!-- 标题行 -->
            <div class="track-header">
              <div class="track-checkbox">
                <input 
                  type="checkbox" 
                  :checked="filteredTracks.length > 0 && selectedTracks.length === filteredTracks.length"
                  @change="handleHeaderCheckbox"
                  :disabled="filteredTracks.length === 0"
                />
              </div>
              <div class="track-index">#</div>
              <div class="track-title-col">标题</div>
              <div class="track-artist-col">歌手</div>
              <div class="track-album-col">专辑</div>
              <div class="track-actions-col">
                <div class="track-duration-col">时长</div>
              </div>
            </div>
            
            <!-- 曲目列表 -->
            <div 
              v-for="(track, index) in filteredTracks" 
              :key="track.id"
              :class="{ active: currentTrackIndex === getOriginalIndex(track), selected: selectedTracks.includes(track.id) }"
              class="track-item"
            >
              <div class="track-checkbox">
                <input 
                  type="checkbox" 
                  :checked="selectedTracks.includes(track.id)"
                  @change="toggleSelectTrack(track.id)"
                  @click.stop
                />
              </div>
              <div class="track-index">{{ index + 1 }}</div>
              <div class="track-title-col" @click="handleTrackClick(track, index)">
                <span class="track-title">{{ track.title }}</span>
              </div>
              <div class="track-artist-col" @click="handleTrackClick(track, index)">
                <span class="track-artist">{{ track.artist }}</span>
              </div>
              <div class="track-album-col" @click="handleTrackClick(track, index)">
                <span class="track-album">{{ track.album }}</span>
              </div>
              <div class="track-actions-col">
                <div class="track-duration-col">{{ formatTime(track.duration) }}</div>
                <button 
                  @click.stop="openEditTrackModal(track)" 
                  class="edit-btn"
                  title="编辑标签"
                >
                  ✎
                </button>
                <button 
                  @click.stop="toggleFavorite(track)" 
                  class="favorite-btn" 
                  :class="{ 'is-favorite': localStore.isFavorite(track.id) }"
                  :title="localStore.isFavorite(track.id) ? '取消喜欢' : '添加到我喜欢'"
                >
                  {{ localStore.isFavorite(track.id) ? '♥' : '♡' }}
                </button>
              </div>
            </div>
            <div v-if="filteredTracks.length === 0" class="empty-state">
              <p>暂无歌曲，请点击扫描目录添加音乐</p>
            </div>
          </div>
        </div>
        
        <!-- 播放器控制 -->
        <div class="player-container">
          <div class="player-main">
            <div class="album-art">
              <img v-if="currentTrack?.album_art" :src="currentTrack.album_art" alt="专辑封面" />
              <div v-else class="no-album-art">
                <span>🎵</span>
              </div>
            </div>
            
            <div class="track-info">
              <h2>{{ currentTrack?.title || '未选择歌曲' }}</h2>
              <p>{{ currentTrack?.artist || '' }}</p>
              
              <div class="lyrics-container">
                <div v-if="currentLyric" class="current-lyric">{{ currentLyric }}</div>
                <div v-else class="no-lyric">暂无歌词</div>
              </div>
            </div>
          </div>
          
          <!-- 频谱分析器 -->
          <div v-if="visualizerEnabled" class="audio-visualizer">
            <div class="visualizer-bars">
              <div 
                v-for="(height, index) in visualizerData" 
                :key="index" 
                class="visualizer-bar"
                :style="{ height: `${height}%` }"
              ></div>
            </div>
          </div>
          
          <div class="progress-container">
            <span class="time">{{ formatTime(currentTime) }}</span>
            <input 
              type="range" 
              :value="currentTime"
              :min="0" 
              :max="duration || 100"
              step="0.1"
              @input="handleSeek"
              class="progress-slider"
            />
            <span class="time">{{ formatTime(duration) }}</span>
          </div>
          
          <div class="controls">
            <button @click="togglePlayMode" class="control-btn mode-btn" :title="playModeTitle">
              {{ playModeIcon }}
            </button>
            <button @click="playPrevious" class="control-btn">⏮️</button>
            <button @click="togglePlay" class="control-btn play-btn">
              {{ isPlaying ? '⏸️' : '▶️' }}
            </button>
            <button @click="playNext" class="control-btn">⏭️</button>
            <button @click="toggleMute" class="control-btn" :title="muted ? '取消静音' : '静音'">
              {{ muted ? '🔇' : '🔊' }}
            </button>
            <div class="volume-control">
              <input 
                type="range" 
                min="0" 
                max="1" 
                step="0.05" 
                v-model.number="volume" 
                @input="updateVolume"
                class="volume-slider"
              />
            </div>
          </div>
        </div>
      </main>
    </div>

    <!-- 创建歌单模态框 -->
    <div v-if="showCreatePlaylistModal" class="modal-overlay" @click="showCreatePlaylistModal = false">
      <div class="modal" @click.stop>
        <h3>创建歌单</h3>
        <input 
          v-model="newPlaylistName" 
          type="text" 
          placeholder="歌单名称" 
          class="input"
        />
        <textarea 
          v-model="newPlaylistDesc" 
          placeholder="歌单描述（可选）" 
          class="textarea"
        ></textarea>
        <div class="modal-actions">
          <button @click="showCreatePlaylistModal = false" class="btn-secondary">取消</button>
          <button @click="createPlaylist" class="btn-primary" :disabled="!newPlaylistName.trim()">
            创建
          </button>
        </div>
      </div>
    </div>

    <!-- 添加到歌单模态框 -->
    <div v-if="showAddToPlaylistModalVisible" class="modal-overlay" @click="showAddToPlaylistModalVisible = false">
      <div class="modal" @click.stop>
        <h3>添加到歌单</h3>
        <div class="playlist-selector">
          <div 
            v-for="playlist in localStore.localPlaylists" 
            :key="playlist.id"
            class="playlist-option"
            @click="addToPlaylist(playlist.id, currentTrackToAdd?.id)"
          >
            <span class="playlist-option-name">{{ playlist.name }}</span>
            <span class="playlist-option-count">{{ playlist.songs.length }} 首</span>
          </div>
        </div>
        <div class="modal-actions">
          <button @click="showAddToPlaylistModalVisible = false" class="btn-secondary">取消</button>
        </div>
      </div>
    </div>
    
    <!-- 设置面板 -->
    <div v-if="showSettings" class="settings-panel" @click.self="showSettings = false">
      <div class="settings-content" @click.stop>
        <div class="settings-header">
          <h3>设置</h3>
          <button @click="showSettings = false" class="close-btn">✕</button>
        </div>
        
        <div class="settings-body">
          <!-- 左侧导航 -->
          <div class="settings-nav">
            <button 
              v-for="tab in settingTabs" 
              :key="tab.id"
              @click="currentSettingTab = tab.id"
              :class="{ active: currentSettingTab === tab.id }"
              class="nav-btn"
            >
              {{ tab.name }}
            </button>
          </div>
          
          <!-- 右侧内容 -->
          <div class="settings-tab-content">
            <!-- 音频设置 -->
            <div v-if="currentSettingTab === 'audio'" class="tab-panel">
              <div class="settings-section">
                <h4>音频引擎</h4>
                <label class="setting-item">
                  <input type="checkbox" v-model="ffmpegEnabled" @change="toggleFFmpeg">
                  <div class="setting-info">
                    <span class="setting-label">启用 FFmpeg 引擎</span>
                    <span class="setting-desc">使用 FFmpeg 解码特殊音频格式</span>
                  </div>
                </label>
              </div>
              
              <div class="settings-section">
                <h4>转码设置</h4>
                <div class="setting-item">
                  <div class="setting-info">
                    <span class="setting-label">FFmpeg 状态</span>
                    <span class="setting-desc" :class="{ 'text-success': ffmpegAvailable, 'text-error': !ffmpegAvailable }">
                      {{ ffmpegAvailable ? '✓ 已检测到 FFmpeg' : '✗ 未检测到 FFmpeg，请安装并添加到环境变量' }}
                    </span>
                  </div>
                </div>
                <label class="setting-item">
                  <input type="checkbox" v-model="transcoderEnabled" @change="toggleTranscoder" :disabled="!ffmpegAvailable">
                  <div class="setting-info">
                    <span class="setting-label">启用转码功能</span>
                    <span class="setting-desc">自动将无法播放的音频格式（APE、DSD等）转码为FLAC（无损音质）</span>
                  </div>
                </label>
                <div class="setting-item slider-item" v-if="transcoderEnabled">
                  <div class="setting-info">
                    <span class="setting-label">预转码提前时间</span>
                    <span class="setting-desc">在播放当前曲目时提前转码下一首</span>
                  </div>
                  <div class="slider-control">
                    <input 
                      type="range" 
                      v-model.number="transcoderPreloadSeconds" 
                      min="5" 
                      max="30" 
                      step="1"
                      @change="updateTranscoderPreloadSeconds"
                      class="slider"
                    />
                    <span class="slider-value">{{ transcoderPreloadSeconds }}秒</span>
                  </div>
                </div>
              </div>
              
              <div class="settings-section">
                <h4>播放设置</h4>
                <div class="setting-item slider-item">
                  <div class="setting-info">
                    <span class="setting-label">交叉淡入淡出时长</span>
                    <span class="setting-desc">切换歌曲时的淡入淡出效果时长</span>
                  </div>
                  <div class="slider-control">
                    <input 
                      type="range" 
                      v-model.number="crossfadeDuration" 
                      min="0" 
                      max="5000" 
                      step="100"
                      @change="updateCrossfadeDuration"
                      class="slider"
                    />
                    <span class="slider-value">{{ crossfadeDuration }}ms</span>
                  </div>
                </div>
              </div>
              
              <div class="settings-section">
                <h4>均衡器</h4>
                <label class="setting-item">
                  <input type="checkbox" v-model="equalizerEnabled" @change="toggleEqualizerEnabled">
                  <div class="setting-info">
                    <span class="setting-label">启用均衡器</span>
                    <span class="setting-desc">调整音频频率响应</span>
                  </div>
                </label>
                <button @click="openEqualizer" class="btn-secondary equalizer-btn">
                  🎛️ 打开均衡器面板
                </button>
              </div>
            </div>
            
            <!-- 界面设置 -->
            <div v-if="currentSettingTab === 'ui'" class="tab-panel">
              <div class="settings-section">
                <h4>视觉效果</h4>
                <label class="setting-item">
                  <input type="checkbox" v-model="visualizerEnabled" @change="toggleVisualizer">
                  <div class="setting-info">
                    <span class="setting-label">启用音频可视化</span>
                    <span class="setting-desc">显示音频频谱分析</span>
                  </div>
                </label>
              </div>
              
              <div class="settings-section">
                <h4>外观主题</h4>
                <div class="theme-selector">
                  <div class="theme-option" v-for="theme in themes" :key="theme.id" @click="setTheme(theme.id)">
                    <div class="theme-preview" :class="`theme-${theme.id}`"></div>
                    <span class="theme-name">{{ theme.name }}</span>
                  </div>
                </div>
              </div>
            </div>
            
            <!-- 播放设置 -->
            <div v-if="currentSettingTab === 'playback'" class="tab-panel">
              <div class="settings-section">
                <h4>播放进度</h4>
                <label class="setting-item">
                  <input type="checkbox" v-model="savePlaybackProgress" @change="toggleSaveProgress">
                  <div class="setting-info">
                    <span class="setting-label">记住播放进度</span>
                    <span class="setting-desc">退出时保存播放位置，下次启动自动恢复</span>
                  </div>
                </label>
              </div>
              
              <div class="settings-section">
                <h4>播放模式</h4>
                <div class="setting-item">
                  <span class="setting-label">默认播放模式</span>
                  <select v-model="defaultPlayMode" @change="updateDefaultPlayMode" class="select-input">
                    <option value="0">顺序播放</option>
                    <option value="1">随机播放</option>
                    <option value="2">循环播放</option>
                  </select>
                </div>
              </div>
            </div>

            <!-- 关于 -->
            <div v-if="currentSettingTab === 'about'" class="tab-panel">
              <div class="about-section">
                <div class="about-logo">
                  <h1 class="about-title">TPlayer</h1>
                  <p class="about-version">版本 1.0.0</p>
                </div>
                
                <div class="about-description">
                  <h3>程序简介</h3>
                  <p>TPlayer 是一款基于 Tauri + Vue 3 开发的现代化音乐播放器，支持丰富的音频格式播放，提供优质的音频体验和简洁的用户界面。</p>
                  
                  <h3>主要功能</h3>
                  <ul class="feature-list">
                    <li>支持多种音频格式：MP3、FLAC、WAV、AAC、OGG、M4A、APE、DSD 等</li>
                    <li>智能音频转码：自动转码不支持的音频格式为 FLAC</li>
                    <li>均衡器控制：提供多种预设和自定义均衡器</li>
                    <li>播放列表管理：创建、编辑、删除歌单</li>
                    <li>播放模式：顺序播放、随机播放、循环播放</li>
                    <li>播放进度记忆：自动保存播放位置</li>
                    <li>可视化效果：音频频谱可视化</li>
                    <li>主题切换：支持亮色和暗色主题</li>
                  </ul>
                  
                  <h3>技术栈</h3>
                  <ul class="tech-list">
                    <li>Tauri 2.x - 跨平台桌面应用框架</li>
                    <li>Vue 3 - 现代化前端框架</li>
                    <li>Rust - 高性能后端</li>
                    <li>FFmpeg - 音频解码和转码</li>
                    <li>rodio - Rust 音频播放库</li>
                  </ul>
                  
                  <h3>开发者信息</h3>
                  <div class="developer-info">
                    <p><strong>开发者：</strong>TPlayer 开发团队</p>
                    <p><strong>感谢 格力森酒业 智睿舒畅 忧蓝对项目的贡献</strong></p>
                    <p><strong>许可证：</strong>ISC</p>
                    <p><strong>项目地址：</strong>https://github.com/ChrisHcn1/TPlayer</p>
                  </div>
                  
                  <h3>使用说明</h3>
                    <div class="usage-info">
                      <p>1. 点击"扫描目录"按钮选择音乐文件夹</p>
                      <p>2. 在播放列表中选择要播放的歌曲</p>
                      <p>3. 使用底部播放控制栏控制播放</p>
                      <p>4. 点击设置按钮调整音频和界面选项</p>
                      <p>5. 使用均衡器调整音效</p>
                    </div>
                    
                    <h3>FFmpeg 配置</h3>
                    <div class="usage-info">
                      <p><strong>下载地址：</strong><a href="https://ffmpeg.org/download.html" target="_blank">https://ffmpeg.org/download.html</a></p>
                      <p><strong>安装方法：</strong></p>
                      <ul>
                        <li>Windows：下载压缩包后解压到任意目录</li>
                        <li>macOS：使用 Homebrew 安装：<code>brew install ffmpeg</code></li>
                        <li>Linux：使用包管理器安装，如 <code>apt install ffmpeg</code></li>
                      </ul>
                      <p><strong>环境变量配置：</strong></p>
                      <ul>
                        <li>Windows：将FFmpeg的bin目录添加到系统环境变量PATH中</li>
                        <li>macOS/Linux：确保FFmpeg在系统PATH中，或在设置中指定FFmpeg可执行文件路径</li>
                      </ul>
                    </div>
                    
                    <div class="about-footer">
                      <p>感谢您使用 TPlayer！</p>
                      <p class="copyright">© 2026 TPlayer. All rights reserved.</p>
                    </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 均衡器面板 -->
    <div v-if="showEqualizer" class="equalizer-panel" @click.self="showEqualizer = false">
      <div class="equalizer-content" @click.stop>
        <div class="equalizer-header">
          <h3>均衡器</h3>
          <button @click="showEqualizer = false" class="close-btn">✕</button>
        </div>
        
        <div class="equalizer-presets">
          <select v-model="selectedPreset" @change="applyPreset" class="preset-select">
            <option value="custom">自定义</option>
            <option value="flat">平坦</option>
            <option value="rock">摇滚</option>
            <option value="pop">流行</option>
            <option value="jazz">爵士</option>
            <option value="classical">古典</option>
            <option value="electronic">电子</option>
          </select>
        </div>
        
        <div class="equalizer-controls">
          <div v-for="(band, index) in equalizerBands" :key="index" class="equalizer-band">
            <span class="band-value">{{ equalizerBands[index] }} dB</span>
            <input 
              type="range" 
              min="-12" 
              max="12" 
              step="0.5" 
              v-model.number="equalizerBands[index]"
              @input="updateBand($event, index)"
              class="vertical-slider"
              orient="vertical"
            />
            <label class="band-label">{{ getFrequencyLabel(index) }}</label>
          </div>
        </div>
      </div>
    </div>
    
    <!-- 编辑歌单模态框 -->
    <div v-if="showEditPlaylistModal" class="modal-overlay" @click="showEditPlaylistModal = false">
      <div class="modal" @click.stop>
        <h3>编辑歌单</h3>
        <input 
          v-model="editPlaylistName" 
          type="text" 
          placeholder="歌单名称" 
          class="input"
        />
        <textarea 
          v-model="editPlaylistDesc" 
          placeholder="歌单描述（可选）" 
          class="textarea"
        ></textarea>
        <div class="modal-actions">
          <button @click="showEditPlaylistModal = false" class="btn-secondary">取消</button>
          <button @click="updatePlaylist" class="btn-primary" :disabled="!editPlaylistName.trim()">
            保存
          </button>
        </div>
      </div>
    </div>
    
    <!-- 删除歌单确认模态框 -->
    <div v-if="showDeletePlaylistModal" class="modal-overlay" @click="showDeletePlaylistModal = false">
      <div class="modal" @click.stop>
        <h3>删除歌单</h3>
        <p>确定要删除此歌单吗？此操作无法撤销。</p>
        <div class="modal-actions">
          <button @click="showDeletePlaylistModal = false" class="btn-secondary">取消</button>
          <button @click="deletePlaylist" class="btn-primary" style="background-color: #e74c3c;">
            删除
          </button>
        </div>
      </div>
    </div>
    
    <!-- 编辑曲目标签模态框 -->
    <div v-if="showEditTrackModal" class="modal-overlay" @click="showEditTrackModal = false">
      <div class="modal edit-track-modal" @click.stop>
        <h3>编辑曲目信息</h3>
        <div class="form-group">
          <label>标题</label>
          <input v-model="editTrackTitle" type="text" placeholder="歌曲标题" />
        </div>
        <div class="form-group">
          <label>艺术家</label>
          <input v-model="editTrackArtist" type="text" placeholder="艺术家" />
        </div>
        <div class="form-group">
          <label>专辑</label>
          <input v-model="editTrackAlbum" type="text" placeholder="专辑名称" />
        </div>
        <div class="form-group">
          <label>歌词</label>
          <textarea v-model="editTrackLyrics" placeholder="歌词内容（支持LRC格式）" rows="6"></textarea>
        </div>
        <div class="modal-actions">
          <button @click="showEditTrackModal = false" class="btn-secondary">取消</button>
          <button @click="saveTrackMetadata" class="btn-primary">保存</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open, ask } from '@tauri-apps/plugin-dialog'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { TrayIcon } from '@tauri-apps/api/tray'
import { Menu, MenuItem } from '@tauri-apps/api/menu'
import { defaultWindowIcon } from '@tauri-apps/api/app'
import { useLocalStore, SongType } from './stores/local'
import * as mm from 'music-metadata'

const localStore = useLocalStore()

interface LyricLine {
  time: number
  text: string
}

const tracks = ref<SongType[]>([])
const currentTrackIndex = ref(-1)
const isPlaying = ref(false)
const currentTime = ref(0)
const duration = ref(0)
// 播放模式：0=顺序播放, 1=随机播放, 2=循环播放
const playMode = ref(1) // 默认随机播放
const volume = ref(0.7) // 默认音量 70%
const currentFilter = ref('曲目')
const filters = ['曲目', '歌手', '专辑']
const showingFavorites = ref(false) // 是否显示我喜欢的
const currentLyric = ref('')
const parsedLyrics = ref<LyricLine[]>([])
const visualizerData = ref<number[]>(Array(20).fill(0)) // 20 个频谱柱
const showEqualizer = ref(false) // 均衡器显示状态
const equalizerEnabled = ref(true) // 均衡器启用状态
const selectedPreset = ref('custom') // 均衡器预设
const repeatOne = ref(false) // 单曲循环
const playbackSpeed = ref(1.0) // 播放速度
const muted = ref(false) // 静音状态
const ffmpegEnabled = ref(false) // ffmpeg 引擎默认关闭
const showSettings = ref(false) // 设置面板显示状态
const visualizerEnabled = ref(true) // 可视化启用状态
const sidebarVisible = ref(true) // 侧边栏显示状态
const currentTheme = ref('default') // 当前主题
const currentSettingTab = ref('audio') // 当前设置标签页
const crossfadeDuration = ref(3000) // 交叉淡入淡出时长（毫秒）
const savePlaybackProgress = ref(true) // 是否保存播放进度
const defaultPlayMode = ref(0) // 默认播放模式
const searchQuery = ref('') // 搜索关键词
const selectedTracks = ref<string[]>([]) // 选中的曲目ID列表
const transcoderEnabled = ref(true) // 转码功能默认启用
const transcoderPreloadSeconds = ref(15) // 预转码提前时间（秒）
const ffmpegAvailable = ref(false) // FFmpeg 是否可用
const currentPlaylistId = ref<number | null>(null) // 当前选中的歌单ID

// 设置标签页
const settingTabs = [
  { id: 'audio', name: '音频' },
  { id: 'ui', name: '界面' },
  { id: 'playback', name: '播放' },
  { id: 'about', name: '关于' }
]

let progressInterval: ReturnType<typeof setInterval> | null = null
let visualizerInterval: ReturnType<typeof setInterval> | null = null
let isAutoSwitching = false

// 模态框状态
const showCreatePlaylistModal = ref(false)
const showAddToPlaylistModalVisible = ref(false)
const showEditPlaylistModal = ref(false)
const showDeletePlaylistModal = ref(false)
const newPlaylistName = ref('')
const newPlaylistDesc = ref('')
const editPlaylistName = ref('')
const editPlaylistDesc = ref('')
const currentEditPlaylistId = ref<number | null>(null)
const currentTrackToAdd = ref<SongType | null>(null)

// 标签编辑模态框状态
const showEditTrackModal = ref(false)
const editingTrack = ref<SongType | null>(null)
const editTrackTitle = ref('')
const editTrackArtist = ref('')
const editTrackAlbum = ref('')
const editTrackLyrics = ref('')

const currentTrack = computed(() => {
  return currentTrackIndex.value >= 0 ? tracks.value[currentTrackIndex.value] : null
})

// 播放模式图标
const playModeIcon = computed(() => {
  switch (playMode.value) {
    case 0: return '➡️' // 顺序播放
    case 1: return '🔀' // 随机播放
    case 2: return '🔁' // 循环播放
    default: return '➡️'
  }
})

// 播放模式标题
const playModeTitle = computed(() => {
  switch (playMode.value) {
    case 0: return '顺序播放'
    case 1: return '随机播放'
    case 2: return '循环播放'
    default: return '顺序播放'
  }
})

// 切换播放模式
const togglePlayMode = () => {
  playMode.value = (playMode.value + 1) % 3
}

// 均衡器
const equalizerBands = ref<number[]>(Array(10).fill(0))

// 获取频段频率标签
const getFrequencyLabel = (index: number) => {
  const frequencies = ['31 Hz', '63 Hz', '125 Hz', '250 Hz', '500 Hz', '1 kHz', '2 kHz', '4 kHz', '8 kHz', '16 kHz']
  return frequencies[index] || `${index}`
}

// 更新均衡器
const updateEqualizer = async () => {
  try {
    await invoke('set_equalizer_bands', {
      bands: equalizerBands.value
    })
  } catch (error) {
    console.error('更新均衡器失败:', error)
  }
}

// 加载均衡器设置
const loadEqualizerSettings = async () => {
  try {
    const result = await invoke('get_equalizer_bands')
    if (result && Array.isArray(result)) {
      equalizerBands.value = result
    }
  } catch (error) {
    console.error('加载均衡器设置失败:', error)
  }
}

// 切换均衡器显示/隐藏
const toggleEqualizer = () => {
  showEqualizer.value = !showEqualizer.value
}

// 切换设置面板显示/隐藏
const toggleSettings = () => {
  showSettings.value = !showSettings.value
}

// 切换可视化
const toggleVisualizer = async () => {
  if (!visualizerEnabled.value) {
    stopVisualizer()
  }
  await saveAppSettings()
}

// 切换侧边栏
const toggleSidebar = async () => {
  sidebarVisible.value = !sidebarVisible.value
  await saveAppSettings()
}

// 窗口控制
const minimizeWindow = async () => {
  try {
    await invoke('minimize_window')
  } catch (error) {
    console.error('最小化窗口失败:', error)
  }
}

const maximizeWindow = async () => {
  try {
    await invoke('maximize_window')
  } catch (error) {
    console.error('最大化/还原窗口失败:', error)
  }
}

const closeWindow = async () => {
  try {
    const confirmed = await ask(
      '您想要退出程序还是关闭到托盘？\n\n点击"是"退出程序\n点击"否"关闭到托盘继续播放',
      '确认关闭'
    )
    
    if (confirmed) {
      await invoke('exit_app')
    } else {
      await invoke('close_window')
    }
  } catch (error) {
    console.error('关闭窗口失败:', error)
  }
}

// 打开均衡器面板
const openEqualizer = () => {
  showEqualizer.value = true
  showSettings.value = false
}

// 更新交叉淡入淡出时长
const updateCrossfadeDuration = async () => {
  try {
    await invoke('set_crossfade_duration', { duration: crossfadeDuration.value })
    await saveAppSettings()
  } catch (error) {
    console.error('更新交叉淡入淡出时长失败:', error)
  }
}

// 切换保存播放进度
const toggleSaveProgress = async () => {
  try {
    await invoke('set_save_playback_progress', { save: savePlaybackProgress.value })
    await saveAppSettings()
  } catch (error) {
    console.error('切换保存播放进度失败:', error)
  }
}

// 更新默认播放模式
const updateDefaultPlayMode = async () => {
  try {
    await invoke('set_default_play_mode', { mode: defaultPlayMode.value })
    await saveAppSettings()
  } catch (error) {
    console.error('更新默认播放模式失败:', error)
  }
}

// 保存应用设置
const saveAppSettings = async () => {
  try {
    await invoke('save_app_settings', {
      settings: {
        theme: currentTheme.value,
        visualizer_enabled: visualizerEnabled.value,
        sidebar_visible: sidebarVisible.value,
        crossfade_duration: crossfadeDuration.value,
        save_playback_progress: savePlaybackProgress.value,
        default_play_mode: defaultPlayMode.value,
        ffmpeg_enabled: ffmpegEnabled.value,
        equalizer_enabled: equalizerEnabled.value,
        transcoder_enabled: transcoderEnabled.value,
        transcoder_preload_seconds: transcoderPreloadSeconds.value,
        playback_speed: playbackSpeed.value,
        muted: muted.value,
        volume: volume.value
      }
    })
  } catch (error) {
    console.error('保存应用设置失败:', error)
  }
}

// 加载应用设置
const loadAppSettings = async () => {
  try {
    const settings = await invoke<{
      theme: string
      visualizer_enabled: boolean
      sidebar_visible: boolean
      crossfade_duration: number
      save_playback_progress: boolean
      default_play_mode: number
      ffmpeg_enabled: boolean
      equalizer_enabled: boolean
      transcoder_enabled: boolean
      transcoder_preload_seconds: number
      playback_speed: number
      muted: boolean
      volume: number
    }>('load_app_settings')
    
    if (settings) {
      currentTheme.value = settings.theme || 'default'
      visualizerEnabled.value = settings.visualizer_enabled !== false
      sidebarVisible.value = settings.sidebar_visible !== false
      crossfadeDuration.value = settings.crossfade_duration || 3000
      savePlaybackProgress.value = settings.save_playback_progress !== false
      defaultPlayMode.value = settings.default_play_mode || 0
      ffmpegEnabled.value = settings.ffmpeg_enabled !== false
      equalizerEnabled.value = settings.equalizer_enabled !== false
      transcoderEnabled.value = settings.transcoder_enabled === true
      transcoderPreloadSeconds.value = settings.transcoder_preload_seconds || 15
      playbackSpeed.value = settings.playback_speed || 1.0
      muted.value = settings.muted === true
      volume.value = settings.volume !== undefined ? settings.volume : 0.7
      
      // 应用主题
      document.documentElement.setAttribute('data-theme', currentTheme.value)
      
      // 如果转码已启用，同步到后端
      if (transcoderEnabled.value) {
        await invoke('set_transcoder_enabled', { enabled: true })
      }
      
      // 同步播放速度到后端
      await invoke('set_playback_speed', { speed: playbackSpeed.value })
      
      // 同步静音状态到后端
      await invoke('set_mute', { mute: muted.value })
      
      // 同步音量到后端
      await invoke('set_volume', { volume: volume.value })
    }
  } catch (error) {
    console.error('加载应用设置失败:', error)
  }
}

// 主题数据
const themes = [
  { id: 'system', name: '随系统' },
  { id: 'default', name: '默认' },
  { id: 'dark', name: '深色' },
  { id: 'blue', name: '蓝色' },
  { id: 'purple', name: '紫色' },
  { id: 'green', name: '绿色' }
]

// 检测系统主题
const getSystemTheme = (): string => {
  if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
    return 'dark'
  }
  return 'default'
}

// 设置主题
const setTheme = async (themeId: string) => {
  currentTheme.value = themeId
  const actualTheme = themeId === 'system' ? getSystemTheme() : themeId
  document.documentElement.setAttribute('data-theme', actualTheme)
  await saveAppSettings()
}

// 监听系统主题变化
const setupSystemThemeListener = () => {
  const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
  mediaQuery.addEventListener('change', (e) => {
    if (currentTheme.value === 'system') {
      const newTheme = e.matches ? 'dark' : 'default'
      document.documentElement.setAttribute('data-theme', newTheme)
    }
  })
}

// 更新均衡器频段
const updateBand = (event: Event, index: number) => {
  const target = event.target as HTMLInputElement
  equalizerBands.value[index] = parseFloat(target.value)
  updateEqualizer()
  selectedPreset.value = 'custom'
}

// 均衡器预设
const presets = {
  flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  rock: [5, 4, 3, 2, 0, -2, -3, -2, 0, 3],
  pop: [-2, -1, 0, 2, 4, 4, 2, 0, -1, -2],
  jazz: [3, 2, 1, 0, -1, -1, 0, 1, 2, 3],
  classical: [7, 5, 3, 1, 0, 0, 1, 3, 5, 7],
  electronic: [4, 3, 2, 0, -2, -2, 0, 2, 3, 4]
}

// 应用均衡器预设
const applyPreset = () => {
  if (selectedPreset.value !== 'custom' && presets[selectedPreset.value as keyof typeof presets]) {
    equalizerBands.value = [...presets[selectedPreset.value as keyof typeof presets]]
    updateEqualizer()
  }
}

const filteredTracks = computed(() => {
  let result: SongType[]
  
  if (currentFilter.value === '曲目') {
    // 按曲目名称排序
    result = [...tracks.value].sort((a, b) => a.title.localeCompare(b.title))
  } else if (currentFilter.value === '歌手') {
    // 按歌手分组，每个歌手只显示一次
    const artistMap = new Map<string, SongType>()
    for (const track of tracks.value) {
      if (!artistMap.has(track.artist)) {
        artistMap.set(track.artist, track)
      }
    }
    result = Array.from(artistMap.values()).sort((a, b) => a.artist.localeCompare(b.artist))
  } else if (currentFilter.value === '专辑') {
    // 按专辑分组，每个专辑只显示一次
    const albumMap = new Map<string, SongType>()
    for (const track of tracks.value) {
      const key = `${track.artist}-${track.album}`
      if (!albumMap.has(key)) {
        albumMap.set(key, track)
      }
    }
    result = Array.from(albumMap.values()).sort((a, b) => {
      const albumCompare = a.album.localeCompare(b.album)
      return albumCompare !== 0 ? albumCompare : a.artist.localeCompare(b.artist)
    })
  } else {
    result = tracks.value
  }
  
  // 应用搜索过滤
  if (searchQuery.value.trim()) {
    const query = searchQuery.value.toLowerCase().trim()
    result = result.filter(track => {
      // 搜索曲目名称、歌手、专辑和文件名
      const titleMatch = track.title.toLowerCase().includes(query)
      const artistMatch = track.artist.toLowerCase().includes(query)
      const albumMatch = track.album.toLowerCase().includes(query)
      
      // 从路径中提取文件名
      const fileName = track.path.split('\\').pop() || ''
      const fileNameMatch = fileName.toLowerCase().includes(query)
      
      // 提取文件后缀名
      const extension = fileName.split('.').pop() || ''
      const extensionMatch = extension.toLowerCase().includes(query)
      
      return titleMatch || artistMatch || albumMatch || fileNameMatch || extensionMatch
    })
  }
  
  return result
})

// 切换曲目选择状态
const toggleSelectTrack = (trackId: string) => {
  const index = selectedTracks.value.indexOf(trackId)
  if (index > -1) {
    selectedTracks.value.splice(index, 1)
  } else {
    selectedTracks.value.push(trackId)
  }
}

// 全选当前过滤后的曲目
const selectAll = () => {
  selectedTracks.value = filteredTracks.value.map(track => track.id)
}

// 取消全选
const deselectAll = () => {
  selectedTracks.value = []
}

// 清除选中的曲目
const clearSelected = async () => {
  if (selectedTracks.value.length === 0) return
  
  // 从曲目列表中移除选中的曲目
  tracks.value = tracks.value.filter(track => !selectedTracks.value.includes(track.id))
  
  // 保存到本地存储
  await localStore.updateLocalSong(tracks.value)
  
  // 清空选中列表
  selectedTracks.value = []
  
  // 如果当前播放的曲目被删除，重置播放状态
  if (currentTrackIndex.value >= 0 && currentTrackIndex.value >= tracks.value.length) {
    currentTrackIndex.value = -1
    isPlaying.value = false
    currentTime.value = 0
    duration.value = 0
  }
}

// 清除所有曲目
const clearAll = async () => {
  if (tracks.value.length === 0) return
  
  // 清空曲目列表
  tracks.value = []
  
  // 保存到本地存储
  await localStore.updateLocalSong(tracks.value)
  
  // 清空选中列表
  selectedTracks.value = []
  
  // 重置播放状态
  currentTrackIndex.value = -1
  isPlaying.value = false
  currentTime.value = 0
  duration.value = 0
  currentLyric.value = ''
  parsedLyrics.value = []
}

// 处理标题行复选框
const handleHeaderCheckbox = () => {
  if (filteredTracks.value.length === 0) return
  
  if (selectedTracks.value.length === filteredTracks.value.length) {
    // 取消全选
    deselectAll()
  } else {
    // 全选
    selectAll()
  }
}

const formatTime = (input: number | string): string => {
  let totalSeconds: number = 0
  
  if (typeof input === 'string') {
    // 处理格式如 "3:45" 的字符串
    const parts = input.split(':')
    if (parts.length === 2) {
      const [mins, secs] = parts.map(Number)
      if (!isNaN(mins) && !isNaN(secs)) {
        totalSeconds = mins * 60 + secs
      }
    }
  } else if (typeof input === 'number' && !isNaN(input)) {
    // 处理秒数
    totalSeconds = input
  }
  
  if (totalSeconds === 0) return '00:00'
  
  const mins = Math.floor(totalSeconds / 60)
  const secs = Math.floor(totalSeconds % 60)
  return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`
}

// 获取曲目在原始列表中的索引
const getOriginalIndex = (track: SongType): number => {
  return tracks.value.findIndex(t => t.id === track.id)
}

// 获取显示标题
const getDisplayTitle = (track: SongType): string => {
  if (currentFilter.value === '歌手') {
    return track.artist
  } else if (currentFilter.value === '专辑') {
    return track.album
  }
  return track.title
}

// 获取显示副标题
const getDisplaySubtitle = (track: SongType): string => {
  if (currentFilter.value === '歌手') {
    return ''
  } else if (currentFilter.value === '专辑') {
    return track.artist
  }
  return track.artist
}

// 获取歌手的曲目数量
const getArtistTrackCount = (artist: string): number => {
  return tracks.value.filter(t => t.artist === artist).length
}

// 获取专辑的曲目数量
const getAlbumTrackCount = (artist: string, album: string): number => {
  return tracks.value.filter(t => t.artist === artist && t.album === album).length
}

// 获取列表标题
const getListTitle = (): string => {
  switch (currentFilter.value) {
    case '曲目': return '歌曲列表'
    case '歌手': return '歌手列表'
    case '专辑': return '专辑列表'
    default: return '播放列表'
  }
}

// 处理曲目点击
const handleTrackClick = (track: SongType, filteredIndex: number) => {
  if (currentFilter.value === '曲目') {
    playTrack(getOriginalIndex(track), true)
  } else if (currentFilter.value === '歌手') {
    // 播放该歌手的第一首曲目
    const artistTracks = tracks.value.filter(t => t.artist === track.artist)
    if (artistTracks.length > 0) {
      const firstTrackIndex = tracks.value.findIndex(t => t.id === artistTracks[0].id)
      playTrack(firstTrackIndex, true)
    }
  } else if (currentFilter.value === '专辑') {
    // 播放该专辑的第一首曲目
    const albumTracks = tracks.value.filter(t => t.artist === track.artist && t.album === track.album)
    if (albumTracks.length > 0) {
      const firstTrackIndex = tracks.value.findIndex(t => t.id === albumTracks[0].id)
      playTrack(firstTrackIndex, true)
    }
  }
}

const parseLyrics = (lrcContent: string): LyricLine[] => {
  const lines: LyricLine[] = []
  const regex = /\[(\d{2}):(\d{2})\.(\d{2,3})\](.*)/g
  let match
  
  while ((match = regex.exec(lrcContent)) !== null) {
    const minutes = parseInt(match[1])
    const seconds = parseInt(match[2])
    const milliseconds = parseInt(match[3].padEnd(3, '0'))
    const time = minutes * 60 + seconds + milliseconds / 1000
    const text = match[4].trim()
    
    if (text) {
      lines.push({ time, text })
    }
  }
  
  return lines.sort((a, b) => a.time - b.time)
}

const updateCurrentLyric = () => {
  if (parsedLyrics.value.length === 0) {
    currentLyric.value = ''
    return
  }
  
  let currentLine = ''
  for (const line of parsedLyrics.value) {
    if (currentTime.value >= line.time) {
      currentLine = line.text
    } else {
      break
    }
  }
  currentLyric.value = currentLine
}

// 处理通过右键菜单打开的文件
const handleOpenFile = async (filePath: string) => {
  try {
    // 获取文件信息
    const metadata = await getTrackMetadata(filePath)
    if (metadata) {
      // 创建曲目对象
      const newTrack: SongType = {
        id: Date.now().toString(),
        title: metadata.title || '未知标题',
        artist: metadata.artist || '未知艺术家',
        album: metadata.album || '未知专辑',
        duration: metadata.duration || '0:00',
        path: filePath,
        lyrics: metadata.lyrics || undefined,
        album_art: metadata.album_art || undefined
      }
      
      // 添加到曲目列表
      tracks.value = [newTrack]
      
      // 保存到本地存储
      await localStore.updateLocalSong(tracks.value)
      
      // 播放该曲目
      await playTrack(0, false)
    }
  } catch (error) {
    console.error('打开文件失败:', error)
  }
}

// 监听文件打开事件
const setupFileOpenListener = async () => {
  try {
    // 获取启动参数
    const args = await invoke<string[]>('get_startup_args')
    console.log('启动参数:', args)
    
    // 检查是否有文件路径参数（通常第一个参数是程序路径，后续的是传入的文件）
    if (args.length > 1) {
      // 过滤出音频文件
      const audioExtensions = ['.mp3', '.flac', '.wav', '.aac', '.ogg', '.m4a', '.wma']
      for (let i = 1; i < args.length; i++) {
        const arg = args[i]
        const ext = arg.toLowerCase()
        if (audioExtensions.some(ae => ext.endsWith(ae))) {
          console.log('检测到音频文件:', arg)
          await handleOpenFile(arg)
          break // 只处理第一个音频文件
        }
      }
    }
    
    const appWindow = getCurrentWindow()
    
    // 监听窗口焦点事件，检查是否有新文件要打开
    appWindow.listen('tauri://focus', async () => {
      // 在实际应用中，这里可以通过后端获取命令行参数
      // 暂时通过 localStorage 模拟
      const pendingFile = localStorage.getItem('pendingOpenFile')
      if (pendingFile) {
        localStorage.removeItem('pendingOpenFile')
        await handleOpenFile(pendingFile)
      }
    })
  } catch (error) {
    console.error('设置文件打开监听失败:', error)
  }
}

const scanDirectory = async () => {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择音频目录'
    })
    
    if (selected) {
      const directory = typeof selected === 'string' ? selected : selected[0]
      const scanResult = await invoke<{ tracks: SongType[] }>('scan_dir', { directory })
      if (scanResult && scanResult.tracks) {
        tracks.value = scanResult.tracks
        // 保存到本地存储
        await localStore.updateLocalSong(scanResult.tracks)
        // 回到全部曲目状态
        currentPlaylistId.value = null
        showingFavorites.value = false
      }
    }
  } catch (error) {
    console.error('扫描目录失败:', error)
  }
}

const startProgressPolling = () => {
  if (progressInterval) {
    clearInterval(progressInterval)
  }
  
  progressInterval = setInterval(async () => {
    try {
      const info = await invoke<{
        is_playing: boolean
        is_empty: boolean
        current_time: number
        duration: number
      }>('get_playback_info')
      
      currentTime.value = info.current_time
      duration.value = info.duration
      isPlaying.value = info.is_playing
      
      updateCurrentLyric()
      
      // 预转码下一首曲目（当剩余时间小于预转码提前时间时）
      if (transcoderEnabled.value && info.duration > 0 && info.current_time > 0) {
        const remainingTime = info.duration - info.current_time
        if (remainingTime <= transcoderPreloadSeconds.value) {
          const nextIndex = getNextTrackIndex()
          if (nextIndex >= 0 && nextIndex < tracks.value.length) {
            const nextTrack = tracks.value[nextIndex]
            // 调用预转码
            invoke('preload_next_track', { nextPath: nextTrack.path }).catch((error) => {
              console.error('预转码失败:', error)
            })
          }
        }
      }
      
      // 当播放完成时，自动播放下一首
      // 只有当正在播放且播放为空时才自动切换，避免播放失败时误触发
      if (info.is_empty && !isAutoSwitching && isPlaying.value && info.duration > 0) {
        isAutoSwitching = true
        nextTrack()
        // 延迟重置标志，避免重复触发
        setTimeout(() => {
          isAutoSwitching = false
        }, 1000)
      }
    } catch (error) {
      console.error('获取播放信息失败:', error)
    }
  }, 500)
}

const stopProgressPolling = () => {
  if (progressInterval) {
    clearInterval(progressInterval)
    progressInterval = null
  }
}

const playTrack = async (index: number, useFade: boolean = false) => {
  if (index < 0 || index >= tracks.value.length) return
  
  const track = tracks.value[index]
  const isSwitching = currentTrackIndex.value !== -1 && currentTrackIndex.value !== index && isPlaying.value
  currentTrackIndex.value = index
  
  // 重置播放状态
  currentTime.value = 0
  currentLyric.value = ''
  
  // 切换曲目时先停止轮询，避免获取到不正确的状态
  if (isSwitching) {
    stopProgressPolling()
  }
  
  try {
    // 检查是否需要转码
    if (transcoderEnabled.value) {
      const needsTranscode = await invoke<boolean>('needs_transcode', { path: track.path })
      if (needsTranscode) {
        // 检查是否已转码
        const isTranscoded = await invoke<boolean>('is_transcoded', { path: track.path })
        if (!isTranscoded) {
          // 开始转码
          const transcodeResult = await invoke<{ status: string }>('start_transcode', { path: track.path })
          if (transcodeResult.status === 'started') {
            // 显示转码提示
            const notification = document.createElement('div')
            notification.className = 'transcode-notification'
            notification.innerHTML = `
              <div class="transcode-content">
                <div class="transcode-icon">⏳</div>
                <div class="transcode-text">
                  <div class="transcode-title">正在转码</div>
                  <div class="transcode-message">${track.title} 正在转码中，请稍候...</div>
                </div>
              </div>
            `
            document.body.appendChild(notification)
            
            // 定期检查转码状态
            const checkTranscodeStatus = async () => {
              const status = await invoke<{ status: string, path?: string }>('get_transcode_status', { path: track.path })
              if (status.status === 'completed' && status.path) {
                // 转码完成，自动播放
                notification.innerHTML = `
                  <div class="transcode-content">
                    <div class="transcode-icon">✅</div>
                    <div class="transcode-text">
                      <div class="transcode-title">转码完成</div>
                      <div class="transcode-message">开始播放 ${track.title}</div>
                    </div>
                  </div>
                `
                
                // 延迟一下再播放，让用户看到转码完成的提示
                setTimeout(async () => {
                  // 重新播放（此时应该使用转码后的文件）
                  await playTrack(index, useFade)
                  // 移除通知
                  if (notification.parentNode) {
                    notification.parentNode.removeChild(notification)
                  }
                }, 1000)
              } else if (status.status === 'failed') {
                // 转码失败
                notification.innerHTML = `
                  <div class="transcode-content">
                    <div class="transcode-icon">❌</div>
                    <div class="transcode-text">
                      <div class="transcode-title">转码失败</div>
                      <div class="transcode-message">无法转码 ${track.title}</div>
                    </div>
                  </div>
                `
                setTimeout(() => {
                  if (notification.parentNode) {
                    notification.parentNode.removeChild(notification)
                  }
                }, 3000)
              } else {
                // 继续检查
                setTimeout(checkTranscodeStatus, 1000)
              }
            }
            
            checkTranscodeStatus()
            return
          }
        }
      }
    }
    
    // 如果是切换曲目且正在播放，使用带淡出效果的命令
    const command = (useFade && isSwitching) ? 'play_next_track' : 'play_track'
    const result = await invoke<{ duration: number }>(command, { path: track.path })
    duration.value = result.duration
    isPlaying.value = true
    
    if (track.lyrics) {
      parsedLyrics.value = parseLyrics(track.lyrics)
    } else {
      parsedLyrics.value = []
      currentLyric.value = ''
    }
    
    startProgressPolling()
    startVisualizer()
  } catch (error) {
    console.error('播放失败:', error)
    // 播放失败时也要恢复轮询，以便用户可以操作
    startProgressPolling()
    
    // 获取错误信息
    let errorMsg = '未知错误'
    if (error instanceof Error) {
      errorMsg = error.message
    } else if (typeof error === 'string') {
      errorMsg = error
    } else if (error && typeof error === 'object') {
      errorMsg = (error as any).message || (error as any).error || JSON.stringify(error)
    }
    
    // 检查是否是跳过错误（以 SKIP: 开头）
    if (errorMsg.startsWith('SKIP:')) {
      console.log('当前曲目无法播放，自动跳到下一首')
      // 延迟一下再跳到下一首，避免太快切换
      setTimeout(() => {
        nextTrack()
      }, 1000)
    } else {
      // 显示错误提示
      alert('播放失败: ' + errorMsg)
    }
  }
}

const togglePlay = async () => {
  try {
    if (isPlaying.value) {
      await invoke('pause_track')
      isPlaying.value = false
      stopVisualizer()
    } else {
      if (currentTrackIndex.value >= 0) {
        await invoke('resume_track')
        isPlaying.value = true
        startProgressPolling()
        startVisualizer()
      } else if (tracks.value.length > 0) {
        await playTrack(0)
      }
    }
  } catch (error) {
    console.error('切换播放状态失败:', error)
  }
}

// 播放上一曲
const playPrevious = async () => {
  if (tracks.value.length === 0) return
  
  let nextIndex: number
  
  if (playMode.value === 1) { // 随机播放
    nextIndex = Math.floor(Math.random() * tracks.value.length)
  } else if (playMode.value === 2) { // 循环播放
    nextIndex = (currentTrackIndex.value - 1 + tracks.value.length) % tracks.value.length
  } else { // 顺序播放
    nextIndex = currentTrackIndex.value - 1
    if (nextIndex < 0) {
      nextIndex = 0
    }
  }
  
  await playTrack(nextIndex, true)
}

// 播放下一曲
const playNext = async () => {
  if (tracks.value.length === 0) return
  
  let nextIndex: number
  
  if (playMode.value === 1) { // 随机播放
    nextIndex = Math.floor(Math.random() * tracks.value.length)
  } else if (playMode.value === 2) { // 循环播放
    nextIndex = (currentTrackIndex.value + 1) % tracks.value.length
  } else { // 顺序播放
    nextIndex = currentTrackIndex.value + 1
    if (nextIndex >= tracks.value.length) {
      nextIndex = tracks.value.length - 1
    }
  }
  
  await playTrack(nextIndex, true)
}

const handleSeek = async (event: Event) => {
  const target = event.target as HTMLInputElement
  const newTime = parseFloat(target.value)
  currentTime.value = newTime
  
  try {
    await invoke('seek_track', { position: newTime })
  } catch (error) {
    console.error('跳转失败:', error)
  }
}

const prevTrack = () => {
  if (tracks.value.length === 0) return
  
  let newIndex: number
  if (playMode.value === 1) {
    // 随机播放
    newIndex = Math.floor(Math.random() * tracks.value.length)
  } else {
    // 顺序播放或循环播放
    newIndex = (currentTrackIndex.value - 1 + tracks.value.length) % tracks.value.length
  }
  playTrack(newIndex, true) // 使用淡出效果
}

// 获取下一首曲目的索引（不播放，仅返回索引）
const getNextTrackIndex = (): number => {
  if (tracks.value.length === 0) return -1
  
  if (playMode.value === 1) {
    // 随机播放
    return Math.floor(Math.random() * tracks.value.length)
  } else {
    // 顺序播放或循环播放
    return (currentTrackIndex.value + 1) % tracks.value.length
  }
}

const nextTrack = () => {
  const newIndex = getNextTrackIndex()
  if (newIndex >= 0) {
    playTrack(newIndex, true) // 使用淡出效果
  }
}

// 创建歌单
const createPlaylist = async () => {
  if (!newPlaylistName.value.trim()) return
  
  try {
    await localStore.createLocalPlaylist(newPlaylistName.value, newPlaylistDesc.value)
    showCreatePlaylistModal.value = false
    newPlaylistName.value = ''
    newPlaylistDesc.value = ''
  } catch (error) {
    console.error('创建歌单失败:', error)
  }
}

// 显示添加到歌单模态框
const showAddToPlaylistModal = (track: SongType) => {
  currentTrackToAdd.value = track
  showAddToPlaylistModalVisible.value = true
}

// 添加到歌单
const addToPlaylist = async (playlistId: number, songId: string | undefined) => {
  if (!songId) return
  
  try {
    await localStore.addSongsToLocalPlaylist(playlistId, [songId])
    showAddToPlaylistModalVisible.value = false
    currentTrackToAdd.value = null
  } catch (error) {
    console.error('添加到歌单失败:', error)
  }
}

// 加载歌单
const loadPlaylist = (playlistId: number) => {
  const playlistDetail = localStore.getLocalPlaylistDetail(playlistId)
  if (playlistDetail) {
    tracks.value = playlistDetail.songs
    currentFilter.value = '曲目'
    showingFavorites.value = false
    currentPlaylistId.value = playlistId
  }
}

// 显示全部曲目
const showAllTracks = () => {
  tracks.value = localStore.localSongs
  currentFilter.value = '曲目'
  showingFavorites.value = false
  currentPlaylistId.value = null
}

// 显示我喜欢的
const showFavorites = () => {
  tracks.value = localStore.getFavoriteSongs()
  currentFilter.value = '曲目'
  showingFavorites.value = true
  currentPlaylistId.value = null
}

// 切换喜欢状态
const toggleFavorite = async (track: SongType) => {
  try {
    if (localStore.isFavorite(track.id)) {
      await localStore.removeFromFavorites(track.id)
    } else {
      await localStore.addToFavorites(track.id)
    }
    // 如果当前在显示我喜欢的列表，刷新列表
    if (showingFavorites.value) {
      tracks.value = localStore.getFavoriteSongs()
    }
  } catch (error) {
    console.error('切换喜欢状态失败:', error)
  }
}

// 打开编辑曲目标签模态框
const openEditTrackModal = async (track: SongType) => {
  try {
    editingTrack.value = track
    const metadata = await invoke<{
      title: string
      artist: string
      album: string
      lyrics?: string
    }>('get_track_metadata', { path: track.path })
    
    editTrackTitle.value = metadata.title
    editTrackArtist.value = metadata.artist
    editTrackAlbum.value = metadata.album
    editTrackLyrics.value = metadata.lyrics || ''
    showEditTrackModal.value = true
  } catch (error) {
    console.error('获取曲目标签信息失败:', error)
    // 如果获取失败，使用当前显示的信息
    editingTrack.value = track
    editTrackTitle.value = track.title
    editTrackArtist.value = track.artist
    editTrackAlbum.value = track.album
    editTrackLyrics.value = track.lyrics || ''
    showEditTrackModal.value = true
  }
}

// 保存曲目标签
const saveTrackMetadata = async () => {
  if (!editingTrack.value) return
  
  try {
    await invoke('save_track_metadata', {
      path: editingTrack.value.path,
      metadata: {
        title: editTrackTitle.value,
        artist: editTrackArtist.value,
        album: editTrackAlbum.value,
        lyrics: editTrackLyrics.value || null
      }
    })
    
    // 更新本地曲目信息
    const trackIndex = tracks.value.findIndex(t => t.id === editingTrack.value?.id)
    if (trackIndex !== -1) {
      tracks.value[trackIndex] = {
        ...tracks.value[trackIndex],
        title: editTrackTitle.value,
        artist: editTrackArtist.value,
        album: editTrackAlbum.value,
        lyrics: editTrackLyrics.value || undefined
      }
    }
    
    showEditTrackModal.value = false
    editingTrack.value = null
  } catch (error) {
    console.error('保存曲目标签失败:', error)
    alert('保存失败: ' + error)
  }
}

// 编辑歌单
const editPlaylist = (playlist: any) => {
  currentEditPlaylistId.value = playlist.id
  editPlaylistName.value = playlist.name
  editPlaylistDesc.value = playlist.description || ''
  showEditPlaylistModal.value = true
}

// 更新歌单
const updatePlaylist = async () => {
  if (!currentEditPlaylistId.value || !editPlaylistName.value.trim()) return
  
  try {
    await localStore.updateLocalPlaylist(currentEditPlaylistId.value, {
      name: editPlaylistName.value.trim(),
      description: editPlaylistDesc.value.trim() || undefined
    })
    showEditPlaylistModal.value = false
  } catch (error) {
    console.error('更新歌单失败:', error)
  }
}

// 确认删除歌单
const confirmDeletePlaylist = (playlistId: number) => {
  currentEditPlaylistId.value = playlistId
  showDeletePlaylistModal.value = true
}

// 删除歌单
const deletePlaylist = async () => {
  if (!currentEditPlaylistId.value) return
  
  try {
    await localStore.deleteLocalPlaylist(currentEditPlaylistId.value)
    showDeletePlaylistModal.value = false
    // 如果当前正在播放该歌单的歌曲，清空播放列表
    if (tracks.value.length > 0) {
      const playlistDetail = localStore.getLocalPlaylistDetail(currentEditPlaylistId.value)
      if (playlistDetail) {
        tracks.value = []
        currentTrackIndex.value = -1
      }
    }
  } catch (error) {
    console.error('删除歌单失败:', error)
  }
}

watch(currentTime, () => {
  updateCurrentLyric()
})

// 系统托盘初始化
let trayIcon: TrayIcon | null = null

const initSystemTray = async () => {
  try {
    // 创建托盘菜单
    const menu = await Menu.new({
      items: [
        await MenuItem.new({
          id: 'show',
          text: '显示窗口',
          action: async () => {
            const window = getCurrentWindow()
            await window.show()
            await window.setFocus()
          }
        }),
        await MenuItem.new({
          id: 'play-pause',
          text: '播放/暂停',
          action: async () => {
            togglePlay()
          }
        }),
        await MenuItem.new({
          id: 'next',
          text: '下一首',
          action: async () => {
            playNext()
          }
        }),
        await MenuItem.new({
          id: 'previous',
          text: '上一首',
          action: async () => {
            playPrevious()
          }
        }),
        await MenuItem.new({
          id: 'separator1',
          text: '-'
        }),
        await MenuItem.new({
          id: 'quit',
          text: '退出',
          action: async () => {
            const confirmed = await open({
              title: '确认退出',
              message: '确定要退出 TPlayer 吗？',
              kind: 'warning',
              okLabel: '退出',
              cancelLabel: '取消'
            })
            if (confirmed) {
              await invoke('exit_app')
            }
          }
        })
      ]
    })

    // 创建托盘图标
    const icon = await defaultWindowIcon()
    if (!icon) {
      console.error('无法获取默认窗口图标')
      return
    }

    trayIcon = await TrayIcon.new({
      icon,
      menu,
      menuOnLeftClick: false,
      tooltip: 'TPlayer 音乐播放器'
    })

    console.log('系统托盘初始化成功')
  } catch (error) {
    console.error('初始化系统托盘失败:', error)
  }
}

onMounted(async () => {
  try {
    // 设置系统主题监听
    setupSystemThemeListener()
    
    // 设置文件打开监听
    await setupFileOpenListener()
    
    // 加载应用设置
    await loadAppSettings()
    
    // 从本地存储加载歌曲
    const localSongs = await localStore.readLocalSong()
    if (localSongs.length > 0) {
      tracks.value = localSongs
    }
    
    // 加载歌单和收藏数据
    await localStore.readLocalPlaylists()

    // 加载上次的播放状态
    const result = await invoke<{
      tracks: SongType[]
      current_track_index: number
      current_time: number
      is_playing: boolean
      play_mode: number
    }>('load_state')

    // 加载均衡器设置
    await loadEqualizerSettings()
    // 加载音量设置
    await loadVolumeSetting()
    // 加载播放设置
    await loadPlaybackSettings()
    
    if (result) {
      if (result.tracks && result.tracks.length > 0 && tracks.value.length === 0) {
        tracks.value = result.tracks
      }
      // 恢复播放模式
      if (result.play_mode !== undefined) {
        playMode.value = result.play_mode
      }
      if (result.current_track_index !== undefined && result.current_track_index >= 0) {
        currentTrackIndex.value = result.current_track_index
        // 恢复上次的播放位置
        if (result.current_time !== undefined) {
          currentTime.value = result.current_time
        }
        // 如果上次是播放状态，恢复播放
        if (result.is_playing && currentTrackIndex.value >= 0) {
          const track = tracks.value[currentTrackIndex.value]
          if (track) {
            // 加载曲目并自动播放
            const playResult = await invoke<{ duration: number }>('play_track', { path: track.path })
            duration.value = playResult.duration
            // 跳转到上次播放位置（忽略错误，因为跳转功能暂不支持）
            try {
              await invoke('seek_track', { position: currentTime.value })
            } catch (error) {
              console.warn('跳转功能暂不支持，将从开始播放')
            }
            isPlaying.value = true
            
            if (track.lyrics) {
              parsedLyrics.value = parseLyrics(track.lyrics)
            }
            startProgressPolling()
          }
        }
      }
    }
  } catch (error) {
    console.error('加载上次状态失败:', error)
  }

  // 初始化系统托盘
  await initSystemTray()

  // 监听托盘菜单事件
  const unlistenPrevious = await listen('play-previous', () => {
    playPrevious()
  })
  const unlistenNext = await listen('play-next', () => {
    playNext()
  })
})

// 获取曲目信息
const getTrackMetadata = async (path: string): Promise<Partial<SongType> | null> => {
  try {
    const metadata = await mm.parseFile(path)
    return {
      title: metadata.common.title || '未知标题',
      artist: metadata.common.artists?.join(', ') || '未知艺术家',
      album: metadata.common.album || '未知专辑',
      duration: metadata.format.duration ? Math.floor(metadata.format.duration).toString() : '0'
    }
  } catch (error) {
    console.error('获取曲目信息失败:', error)
    return null
  }
}

// 音量控制
const updateVolume = async () => {
  try {
    await invoke('set_volume', {
      volume: volume.value
    })
    await saveAppSettings()
  } catch (error) {
    console.error('更新音量失败:', error)
  }
}

const loadVolumeSetting = async () => {
  try {
    const result = await invoke<number>('get_volume')
    volume.value = result
  } catch (error) {
    console.error('加载音量设置失败:', error)
  }
}

// 切换单曲循环
const toggleRepeatOne = async () => {
  try {
    const result = await invoke<boolean>('toggle_repeat_one')
    repeatOne.value = result
  } catch (error) {
    console.error('切换单曲循环失败:', error)
  }
}

// 更新播放速度
const updatePlaybackSpeed = async (speed: number) => {
  try {
    await invoke('set_playback_speed', { speed })
    playbackSpeed.value = speed
    await saveAppSettings()
  } catch (error) {
    console.error('更新播放速度失败:', error)
  }
}

// 切换静音
const toggleMute = async () => {
  try {
    const result = await invoke<boolean>('toggle_mute')
    muted.value = result
    await saveAppSettings()
  } catch (error) {
    console.error('切换静音失败:', error)
  }
}

// 切换 ffmpeg 引擎
const toggleFFmpeg = async () => {
  try {
    const result = await invoke<boolean>('toggle_ffmpeg')
    ffmpegEnabled.value = result
    await saveAppSettings()
  } catch (error) {
    console.error('切换 ffmpeg 引擎失败:', error)
  }
}

// 切换均衡器启用/禁用
const toggleEqualizerEnabled = async () => {
  try {
    const result = await invoke<boolean>('toggle_equalizer')
    equalizerEnabled.value = result
    await saveAppSettings()
  } catch (error) {
    console.error('切换均衡器状态失败:', error)
  }
}

// 加载播放设置
const loadPlaybackSettings = async () => {
  try {
    const repeatResult = await invoke<boolean>('get_repeat_one')
    repeatOne.value = repeatResult
    
    const speedResult = await invoke<number>('get_playback_speed')
    playbackSpeed.value = speedResult
    
    const muteResult = await invoke<boolean>('get_mute')
    muted.value = muteResult
    
    const ffmpegResult = await invoke<boolean>('get_ffmpeg')
    ffmpegEnabled.value = ffmpegResult
    
    const equalizerResult = await invoke<boolean>('get_equalizer')
    equalizerEnabled.value = equalizerResult
    
    // 检查 FFmpeg 是否可用
    const ffmpegAvailableResult = await invoke<boolean>('check_ffmpeg_available')
    ffmpegAvailable.value = ffmpegAvailableResult
    
    // 加载转码设置
    const transcoderResult = await invoke<boolean>('is_transcoder_enabled')
    transcoderEnabled.value = transcoderResult
  } catch (error) {
    console.error('加载播放设置失败:', error)
  }
}

// 切换转码功能
const toggleTranscoder = async () => {
  try {
    await invoke('set_transcoder_enabled', { enabled: transcoderEnabled.value })
    await saveAppSettings()
  } catch (error) {
    console.error('切换转码功能失败:', error)
  }
}

// 更新预转码提前时间
const updateTranscoderPreloadSeconds = async () => {
  try {
    await invoke('set_transcoder_preload_seconds', { seconds: transcoderPreloadSeconds.value })
    await saveAppSettings()
  } catch (error) {
    console.error('更新预转码时间失败:', error)
  }
}

// 启动频谱分析器
const startVisualizer = () => {
  if (visualizerInterval) {
    clearInterval(visualizerInterval)
  }
  
  visualizerInterval = setInterval(async () => {
    try {
      // 这里可以从后端获取真实的音频频谱数据
      // 暂时使用模拟数据
      const newData = Array(20).fill(0).map(() => Math.random() * 100)
      visualizerData.value = newData
    } catch (error) {
      console.error('获取频谱数据失败:', error)
    }
  }, 50)
}

// 停止频谱分析器
const stopVisualizer = () => {
  if (visualizerInterval) {
    clearInterval(visualizerInterval)
    visualizerInterval = null
    visualizerData.value = Array(20).fill(0)
  }
}

onUnmounted(async () => {
  stopProgressPolling()
  stopVisualizer()
  
  try {
    await invoke('save_state', {
      state: {
        tracks: tracks.value,
        current_track_index: currentTrackIndex.value,
        current_time: currentTime.value,
        is_playing: isPlaying.value,
        play_mode: playMode.value
      }
    })
  } catch (error) {
    console.error('保存状态失败:', error)
  }
})
</script>

<style>
/* 全局样式 */
html, body {
  margin: 0;
  padding: 0;
  height: 100%;
  width: 100%;
  overflow: hidden;
}

#app {
  height: 100%;
  width: 100%;
}

/* 主题变量 */
:root {
  /* 默认主题 */
  --primary-color: #1db954;
  --primary-hover: #1ed760;
  --background-color: #121212;
  --surface-color: #1e1e1e;
  --border-color: #333;
  --text-color: #ffffff;
  --text-secondary: #999;
  --text-muted: #666;
  --accent-color: rgba(255, 255, 255, 0.05);
}

/* 深色主题 */
[data-theme="dark"] {
  --primary-color: #1ed760;
  --primary-hover: #26f57a;
  --background-color: #0a0a0a;
  --surface-color: #131313;
  --border-color: #2a2a2a;
  --text-color: #ffffff;
  --text-secondary: #888;
  --text-muted: #555;
  --accent-color: rgba(255, 255, 255, 0.03);
}

/* 蓝色主题 */
[data-theme="blue"] {
  --primary-color: #3498db;
  --primary-hover: #2980b9;
  --background-color: #121212;
  --surface-color: #1a1a2e;
  --border-color: #2a3b5a;
  --text-color: #ffffff;
  --text-secondary: #999;
  --text-muted: #666;
  --accent-color: rgba(52, 152, 219, 0.1);
}

/* 紫色主题 */
[data-theme="purple"] {
  --primary-color: #9b59b6;
  --primary-hover: #8e44ad;
  --background-color: #121212;
  --surface-color: #2c3e50;
  --border-color: #34495e;
  --text-color: #ffffff;
  --text-secondary: #999;
  --text-muted: #666;
  --accent-color: rgba(155, 89, 182, 0.1);
}

/* 绿色主题 */
[data-theme="green"] {
  --primary-color: #27ae60;
  --primary-hover: #229954;
  --background-color: #121212;
  --surface-color: #1e3a2f;
  --border-color: #2c5541;
  --text-color: #ffffff;
  --text-secondary: #999;
  --text-muted: #666;
  --accent-color: rgba(39, 174, 96, 0.1);
}
</style>

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background-color: var(--background-color);
  color: var(--text-color);
  font-family: Arial, sans-serif;
  transition: background-color 0.3s ease;
}

.app-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 20px;
  background-color: var(--surface-color);
  border-bottom: 1px solid var(--border-color);
  transition: all 0.3s ease;
  user-select: none;
  -webkit-user-select: none;
  -webkit-app-region: drag;
}

.app-header button,
.app-header .btn-primary {
  -webkit-app-region: no-drag;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 20px;
}

.header-right {
  display: flex;
  gap: 8px;
}

.app-logo {
  display: flex;
  align-items: center;
  margin-right: 10px;
}

.logo-img {
  width: 32px;
  height: 32px;
  border-radius: 6px;
  object-fit: contain;
}

.app-title {
  margin: 0;
  font-size: 20px;
  font-weight: bold;
  color: #1db954;
}

.header-actions {
  display: flex;
  gap: 10px;
}

.btn-icon {
  width: 36px;
  height: 36px;
  border: none;
  border-radius: 8px;
  background-color: transparent;
  font-size: 18px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
  opacity: 0.7;
}

.btn-icon:hover {
  background-color: rgba(255, 255, 255, 0.1);
  opacity: 1;
}

.btn-icon.active {
  background-color: rgba(29, 185, 84, 0.2);
  color: var(--primary-color);
  opacity: 1;
}

.window-controls {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-left: 20px;
}

.window-control-btn {
  width: 32px;
  height: 32px;
  border: none;
  border-radius: 6px;
  background-color: transparent;
  color: #999;
  font-size: 12px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
}

.window-control-btn:hover {
  background-color: rgba(255, 255, 255, 0.1);
  color: #fff;
}

.window-control-btn.close-btn:hover {
  background-color: #e74c3c;
  color: #fff;
}

.btn-primary {
  padding: 8px 16px;
  border: none;
  border-radius: 4px;
  background-color: var(--primary-color);
  color: #fff;
  cursor: pointer;
  font-size: 14px;
  transition: background-color 0.2s ease;
}

.btn-primary:hover {
  background-color: var(--primary-hover);
}

.btn-secondary {
  padding: 8px 16px;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  background-color: var(--surface-color);
  color: var(--text-color);
  cursor: pointer;
  font-size: 14px;
  transition: all 0.2s ease;
}

.btn-secondary:hover {
  border-color: var(--primary-color);
  background-color: var(--accent-color);
}

.main-content {
  display: flex;
  flex: 1;
  overflow: hidden;
}

.sidebar {
  width: 200px;
  min-width: 180px;
  max-width: 220px;
  background-color: var(--surface-color);
  border-right: 1px solid var(--border-color);
  padding: 15px;
  overflow-y: auto;
  transition: all 0.3s ease;
  flex-shrink: 0;
  position: relative;
  z-index: 100;
}

.sidebar-hidden {
  width: 0;
  padding: 0;
  overflow: hidden;
}

.sidebar-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.sidebar-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: bold;
  color: #1db954;
}

.sidebar-toggle-btn {
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 6px;
  background-color: transparent;
  color: #999;
  font-size: 14px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
}

.sidebar-toggle-btn:hover {
  background-color: rgba(255, 255, 255, 0.1);
  color: #fff;
}

.sidebar-trigger {
  width: 8px;
  height: 100%;
  background-color: rgba(29, 185, 84, 0.1);
  cursor: pointer;
  transition: all 0.2s ease;
  position: relative;
  z-index: 99;
}

.sidebar-trigger:hover {
  background-color: rgba(29, 185, 84, 0.2);
}

.main-content {
  display: flex;
  flex: 1;
  overflow: hidden;
  transition: all 0.3s ease;
}

.sidebar-section {
  margin-bottom: 30px;
}

.sidebar-section h3 {
  margin: 0 0 15px 0;
  font-size: 14px;
  color: #999;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.clickable-header {
  cursor: pointer;
  transition: all 0.2s ease;
  padding: 8px 12px;
  border-radius: 4px;
}

.clickable-header:hover {
  background-color: #2a2a2a;
  color: #fff;
}

.clickable-header.active {
  background-color: #1db954;
  color: #fff;
}

.filter-buttons {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.filter-buttons button {
  padding: 8px 12px;
  border: none;
  border-radius: 4px;
  background-color: transparent;
  color: #fff;
  cursor: pointer;
  font-size: 14px;
  text-align: left;
  transition: all 0.2s ease;
}

.filter-buttons button:hover {
  background-color: #2a2a2a;
}

.filter-buttons button.active {
  background-color: #1db954;
  color: #fff;
}

.playlist-list {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.playlist-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-radius: 4px;
  cursor: pointer;
  transition: background-color 0.2s ease;
}

.playlist-item:hover {
  background-color: #2a2a2a;
}

.playlist-item.add-playlist {
  color: #1db954;
}

.playlist-name {
  font-size: 14px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.playlist-count {
  font-size: 12px;
  color: #666;
  background-color: #2a2a2a;
  padding: 2px 6px;
  border-radius: 10px;
}

.playlist-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-radius: 4px;
  cursor: pointer;
  transition: background-color 0.2s ease;
}

.playlist-item-content {
  flex: 1;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.playlist-item-actions {
  display: flex;
  gap: 5px;
  opacity: 0;
  transition: opacity 0.2s ease;
}

.playlist-item:hover .playlist-item-actions {
  opacity: 1;
}

.playlist-action-btn {
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 4px;
  background-color: transparent;
  font-size: 12px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
}

.playlist-action-btn:hover {
  background-color: rgba(255, 255, 255, 0.1);
}

.content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  transition: flex 0.3s ease;
}

.player-container {
  display: flex;
  flex-direction: column;
  padding: 16px 20px;
  background-color: var(--surface-color);
  border-top: 1px solid var(--border-color);
  gap: 12px;
  transition: all 0.3s ease;
}

.player-main {
  display: flex;
  align-items: center;
  gap: 16px;
}

.album-art {
  width: 80px;
  height: 80px;
  border-radius: 8px;
  overflow: hidden;
  background-color: #333;
  flex-shrink: 0;
}

.album-art img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.no-album-art {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 32px;
  color: #666;
}

.track-info {
  flex: 1;
  min-width: 0;
}

.track-info h2 {
  margin: 0 0 4px 0;
  font-size: 16px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.track-info p {
  margin: 0 0 8px 0;
  font-size: 13px;
  color: #999;
}

.lyrics-container {
  min-height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  color: #fff;
  background-color: rgba(255, 255, 255, 0.05);
  border-radius: 6px;
  padding: 8px 12px;
}

.audio-visualizer {
  height: 60px;
  background-color: rgba(255, 255, 255, 0.05);
  border-radius: 6px;
  padding: 8px;
  display: flex;
  align-items: flex-end;
  justify-content: center;
}

.visualizer-bars {
  display: flex;
  align-items: flex-end;
  gap: 4px;
  height: 100%;
  width: 100%;
}

.visualizer-bar {
  flex: 1;
  min-width: 4px;
  background: linear-gradient(to top, var(--primary-color), var(--primary-hover));
  border-radius: 2px 2px 0 0;
  transition: height 0.1s ease;
}

.current-lyric {
  text-align: center;
  animation: fadeIn 0.3s ease;
}

.no-lyric {
  color: #666;
  font-size: 14px;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

.progress-container {
  display: flex;
  align-items: center;
  margin: 10px 0;
  gap: 10px;
}

.time {
  font-size: 12px;
  color: #999;
  min-width: 45px;
  text-align: center;
}

.progress-slider {
  flex: 1;
  height: 4px;
  -webkit-appearance: none;
  appearance: none;
  background: var(--border-color);
  border-radius: 2px;
  cursor: pointer;
}

.progress-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--primary-color);
  cursor: pointer;
}

.progress-slider::-moz-range-thumb {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--primary-color);
  cursor: pointer;
  border: none;
}

.controls {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.volume-control {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-left: 20px;
}

.volume-icon {
  font-size: 16px;
  color: #999;
}

.volume-slider {
  width: 100px;
  height: 4px;
  -webkit-appearance: none;
  appearance: none;
  background: #333;
  outline: none;
  border-radius: 2px;
  cursor: pointer;
}

.volume-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #1db954;
  cursor: pointer;
  transition: all 0.2s ease;
}

.volume-slider::-webkit-slider-thumb:hover {
  background: #1ed760;
  transform: scale(1.1);
}

.volume-slider::-moz-range-thumb {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #1db954;
  cursor: pointer;
  border: none;
  transition: all 0.2s ease;
}

.volume-slider::-moz-range-thumb:hover {
  background: #1ed760;
  transform: scale(1.1);
}

.playback-speed-control {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-left: 20px;
}

.speed-label {
  font-size: 12px;
  color: #999;
  min-width: 40px;
  text-align: center;
}

.speed-slider {
  width: 80px;
  height: 4px;
  -webkit-appearance: none;
  appearance: none;
  background: #333;
  outline: none;
  border-radius: 2px;
  cursor: pointer;
}

.speed-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #1db954;
  cursor: pointer;
  transition: all 0.2s ease;
}

.speed-slider::-webkit-slider-thumb:hover {
  background: #1ed760;
  transform: scale(1.1);
}

.speed-slider::-moz-range-thumb {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #1db954;
  cursor: pointer;
  border: none;
  transition: all 0.2s ease;
}

.speed-slider::-moz-range-thumb:hover {
  background: #1ed760;
  transform: scale(1.1);
}

.control-btn {
  width: 40px;
  height: 40px;
  border: none;
  border-radius: 50%;
  background-color: #333;
  color: #fff;
  font-size: 16px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background-color 0.2s ease;
}

.control-btn:hover {
  background-color: #444;
}

.play-btn {
  width: 50px;
  height: 50px;
  font-size: 20px;
  background-color: #1db954;
  transition: background-color 0.2s ease;
}

.play-btn:hover {
  background-color: #1ed760;
}

.mode-btn {
  width: 36px;
  height: 36px;
  border: none;
  border-radius: 4px;
  background-color: transparent;
  font-size: 18px;
  cursor: pointer;
  opacity: 0.7;
  transition: all 0.2s ease;
}

.mode-btn:hover {
  opacity: 1;
  background-color: rgba(255, 255, 255, 0.1);
}

.song-list-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 20px;
  overflow: hidden;
}

.list-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.list-header h3 {
  margin: 0;
  font-size: 18px;
}

.list-actions {
  display: flex;
  gap: 10px;
  align-items: center;
}

.search-box {
  margin-right: 10px;
}

.search-input {
  padding: 6px 10px;
  border: 1px solid #333;
  border-radius: 20px;
  background-color: #2a2a2a;
  color: #fff;
  font-size: 14px;
  width: 200px;
  transition: all 0.2s ease;
}

.search-input:focus {
  outline: none;
  border-color: #1db954;
  box-shadow: 0 0 0 2px rgba(29, 185, 84, 0.2);
  width: 250px;
}

.song-list {
  flex: 1;
  overflow-y: auto;
}

.track-header {
  display: flex;
  align-items: center;
  padding: 10px;
  background-color: #2a2a2a;
  border-bottom: 1px solid #333;
  font-size: 12px;
  font-weight: 600;
  color: #999;
  margin-bottom: 5px;
}

.track-item {
  display: flex;
  align-items: center;
  padding: 10px;
  border-radius: 4px;
  cursor: pointer;
  margin-bottom: 5px;
  transition: background-color 0.2s ease;
}

.track-item:hover {
  background-color: #2a2a2a;
}

.track-item.active {
  background-color: #333;
}

.track-item.selected {
  background-color: #2a2a2a;
  border-left: 3px solid #1db954;
}

.track-checkbox {
  width: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.track-checkbox input[type="checkbox"] {
  width: 16px;
  height: 16px;
  accent-color: #1db954;
  cursor: pointer;
}

.track-index {
  width: 40px;
  font-size: 12px;
  color: #999;
  text-align: center;
}

.track-title-col {
  flex: 2;
  min-width: 0;
  padding: 0 10px;
}

.track-artist-col {
  flex: 1;
  min-width: 0;
  padding: 0 10px;
}

.track-album-col {
  flex: 1;
  min-width: 0;
  padding: 0 10px;
}

.track-actions-col {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 120px;
  justify-content: flex-end;
}

.track-duration-col {
  font-size: 12px;
  color: #999;
  width: 60px;
  text-align: right;
}

.track-title {
  display: block;
  font-size: 14px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.track-artist {
  display: block;
  font-size: 12px;
  color: #999;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.track-album {
  display: block;
  font-size: 12px;
  color: #999;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.track-count {
  font-size: 12px;
  color: #666;
  background-color: #2a2a2a;
  padding: 2px 8px;
  border-radius: 10px;
}

.favorite-btn {
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 4px;
  background-color: transparent;
  font-size: 16px;
  cursor: pointer;
  opacity: 0.7;
  transition: all 0.2s ease;
  color: var(--text-secondary);
  display: flex;
  align-items: center;
  justify-content: center;
}

.favorite-btn:hover {
  opacity: 1;
  background-color: rgba(255, 255, 255, 0.1);
}

.favorite-btn.is-favorite {
  color: #ff4757;
  opacity: 1;
}

.edit-btn {
  background: none;
  border: none;
  color: #999;
  cursor: pointer;
  font-size: 16px;
  padding: 4px 8px;
  border-radius: 4px;
  transition: all 0.2s;
  opacity: 0.6;
}

.edit-btn:hover {
  opacity: 1;
  background-color: rgba(255, 255, 255, 0.1);
  color: #4facfe;
}

.edit-track-modal {
  width: 480px;
  max-width: 90vw;
}

.edit-track-modal .form-group {
  margin-bottom: 16px;
}

.edit-track-modal .form-group label {
  display: block;
  margin-bottom: 6px;
  color: #ccc;
  font-size: 14px;
}

.edit-track-modal .form-group input,
.edit-track-modal .form-group textarea {
  width: 100%;
  padding: 10px 12px;
  border: 1px solid #444;
  border-radius: 6px;
  background: #2a2a2a;
  color: #fff;
  font-size: 14px;
  box-sizing: border-box;
}

.edit-track-modal .form-group input:focus,
.edit-track-modal .form-group textarea:focus {
  outline: none;
  border-color: #4facfe;
}

.edit-track-modal .form-group textarea {
  resize: vertical;
  min-height: 100px;
  font-family: inherit;
}

.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 200px;
  color: #666;
  font-size: 14px;
}

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
  z-index: 1000;
}

.modal {
  background-color: #1e1e1e;
  border-radius: 8px;
  padding: 20px;
  width: 400px;
  max-width: 90vw;
}

.modal h3 {
  margin: 0 0 20px 0;
  font-size: 18px;
}

.input {
  width: 100%;
  padding: 10px;
  border: 1px solid #333;
  border-radius: 4px;
  background-color: #121212;
  color: #fff;
  font-size: 14px;
}

/* 设置面板样式 */
.settings-panel {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  animation: fadeIn 0.2s ease;
}

.settings-content {
  background-color: var(--surface-color);
  border-radius: 12px;
  padding: 24px;
  width: 600px;
  height: 500px;
  max-width: 90vw;
  max-height: 90vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.settings-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.settings-header h3 {
  margin: 0;
  font-size: 18px;
}

.close-btn {
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 6px;
  background-color: transparent;
  color: #999;
  font-size: 16px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
}

.close-btn:hover {
  background-color: rgba(255, 255, 255, 0.1);
  color: #fff;
}

.settings-section {
  margin-bottom: 24px;
}

.settings-section h4 {
  margin: 0 0 12px 0;
  font-size: 14px;
  color: #999;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.setting-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px;
  border-radius: 8px;
  background-color: var(--accent-color);
  cursor: pointer;
  transition: background-color 0.2s ease;
}

.setting-item:hover {
  background-color: rgba(255, 255, 255, 0.08);
}

.theme-selector {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 12px;
  margin-top: 12px;
}

.theme-option {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  padding: 8px;
  border-radius: 8px;
  transition: all 0.2s ease;
}

.theme-option:hover {
  background-color: var(--accent-color);
}

.theme-preview {
  width: 60px;
  height: 60px;
  border-radius: 8px;
  border: 2px solid var(--border-color);
  transition: all 0.2s ease;
}

.theme-option:hover .theme-preview {
  border-color: var(--primary-color);
}

.theme-name {
  font-size: 12px;
  color: var(--text-secondary);
}

/* 主题预览颜色 */
.theme-default {
  background: linear-gradient(135deg, #1db954, #121212);
}

.theme-system {
  background: linear-gradient(135deg, #808080, #404040);
  position: relative;
}

.theme-system::before {
  content: '☀️';
  position: absolute;
  top: 50%;
  left: 25%;
  transform: translate(-50%, -50%);
  font-size: 20px;
}

.theme-system::after {
  content: '🌙';
  position: absolute;
  top: 50%;
  left: 75%;
  transform: translate(-50%, -50%);
  font-size: 20px;
}

.theme-dark {
  background: linear-gradient(135deg, #1ed760, #0a0a0a);
}

.theme-blue {
  background: linear-gradient(135deg, #3498db, #1a1a2e);
}

.theme-purple {
  background: linear-gradient(135deg, #9b59b6, #2c3e50);
}

.theme-green {
  background: linear-gradient(135deg, #27ae60, #1e3a2f);
}

.setting-item input[type="checkbox"] {
  width: 18px;
  height: 18px;
  accent-color: var(--primary-color);
  cursor: pointer;
}

.setting-label {
  font-size: 14px;
  color: var(--text-color);
  font-weight: 500;
}

.setting-desc {
  font-size: 12px;
  color: var(--text-secondary);
  margin-top: 4px;
}

.setting-desc.text-success {
  color: #4caf50;
}

.setting-desc.text-error {
  color: #f44336;
}

/* 设置面板新布局 */
.settings-body {
  display: flex;
  flex: 1;
  overflow: hidden;
  gap: 20px;
}

.settings-nav {
  width: 120px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  border-right: 1px solid var(--border-color);
  padding-right: 16px;
}

.nav-btn {
  padding: 10px 12px;
  border: none;
  border-radius: 8px;
  background-color: transparent;
  color: var(--text-secondary);
  font-size: 14px;
  cursor: pointer;
  text-align: left;
  transition: all 0.2s ease;
}

.nav-btn:hover {
  background-color: var(--accent-color);
  color: var(--text-color);
}

.nav-btn.active {
  background-color: var(--primary-color);
  color: #fff;
}

.settings-tab-content {
  flex: 1;
  overflow-y: auto;
  padding-left: 4px;
}

.tab-panel {
  animation: fadeIn 0.3s ease;
}

/* 关于页面样式 */
.about-section {
  padding: 20px;
  max-width: 800px;
  margin: 0 auto;
}

.about-logo {
  text-align: center;
  margin-bottom: 30px;
  padding-bottom: 20px;
  border-bottom: 1px solid var(--border-color);
}

.about-title {
  font-size: 48px;
  font-weight: bold;
  color: var(--primary-color);
  margin: 0 0 10px 0;
  letter-spacing: 2px;
}

.about-version {
  font-size: 16px;
  color: var(--text-secondary);
  margin: 0;
}

.about-description h3 {
  font-size: 18px;
  font-weight: 600;
  color: var(--text-color);
  margin: 25px 0 12px 0;
  border-left: 3px solid var(--primary-color);
  padding-left: 10px;
}

.about-description p {
  color: var(--text-secondary);
  line-height: 1.6;
  margin: 0 0 10px 0;
}

.about-description ul {
  list-style: none;
  padding: 0 0 0 20px;
  margin: 5px 0 15px 0;
}

.about-description li {
  color: var(--text-secondary);
  line-height: 1.6;
  margin: 5px 0;
  position: relative;
}

.about-description li::before {
  content: '•';
  color: var(--primary-color);
  font-weight: bold;
  position: absolute;
  left: -15px;
}

.about-description a {
  color: var(--primary-color);
  text-decoration: none;
  transition: color 0.2s ease;
}

.about-description a:hover {
  color: #1ed760;
  text-decoration: underline;
}

.about-description code {
  font-family: 'Courier New', Courier, monospace;
  font-size: 14px;
  color: var(--text-secondary);
  background-color: var(--background-secondary);
  padding: 2px 6px;
  border-radius: 4px;
  border: 1px solid var(--border-color);
}

.feature-list,
.tech-list {
  list-style: none;
  padding: 0;
  margin: 0 0 20px 0;
}

.feature-list li,
.tech-list li {
  padding: 8px 0;
  padding-left: 20px;
  position: relative;
  color: var(--text-secondary);
  line-height: 1.5;
}

.feature-list li:before,
.tech-list li:before {
  content: "•";
  color: var(--primary-color);
  font-weight: bold;
  position: absolute;
  left: 0;
}

.developer-info p,
.usage-info p {
  color: var(--text-secondary);
  line-height: 1.6;
  margin: 0 0 8px 0;
}

.developer-info strong {
  color: var(--text-color);
}

.usage-info {
  background: var(--surface-color);
  padding: 15px;
  border-radius: 8px;
  margin-bottom: 20px;
}

.about-footer {
  text-align: center;
  margin-top: 40px;
  padding-top: 20px;
  border-top: 1px solid var(--border-color);
}

.about-footer p {
  color: var(--text-secondary);
  margin: 0 0 8px 0;
}

.copyright {
  font-size: 14px;
  color: var(--text-secondary) !important;
}

.setting-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
}

.slider-item {
  flex-direction: row;
  align-items: center;
  gap: 16px;
}

.slider-control {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 200px;
}

.slider {
  flex: 1;
  height: 6px;
  -webkit-appearance: none;
  appearance: none;
  background: var(--border-color);
  border-radius: 3px;
  cursor: pointer;
}

.slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--primary-color);
  cursor: pointer;
}

.slider::-moz-range-thumb {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--primary-color);
  cursor: pointer;
  border: none;
}

.slider-value {
  font-size: 13px;
  color: var(--text-secondary);
  min-width: 60px;
  text-align: right;
}

.select-input {
  padding: 8px 12px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background-color: var(--background-color);
  color: var(--text-color);
  font-size: 14px;
  cursor: pointer;
  min-width: 120px;
}

.select-input:focus {
  outline: none;
  border-color: var(--primary-color);
}

.equalizer-btn {
  margin-top: 12px;
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

/* 均衡器面板样式 */
.equalizer-panel {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  animation: fadeIn 0.2s ease;
}

.equalizer-content {
  background-color: #1e1e1e;
  border-radius: 12px;
  padding: 24px;
  width: 500px;
  max-width: 90vw;
  max-height: 80vh;
  overflow-y: auto;
}

.equalizer-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.equalizer-header h3 {
  margin: 0;
  font-size: 18px;
}

.equalizer-presets {
  margin-bottom: 20px;
}

.preset-select {
  width: 100%;
  padding: 10px 12px;
  border: 1px solid #333;
  border-radius: 8px;
  background-color: #2a2a2a;
  color: #fff;
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.preset-select:focus {
  outline: none;
  border-color: #1db954;
  box-shadow: 0 0 0 2px rgba(29, 185, 84, 0.2);
}

.preset-select:hover {
  border-color: #1db954;
}

.equalizer-controls {
  display: flex;
  flex-direction: row;
  justify-content: space-between;
  gap: 8px;
  padding: 20px 10px;
  height: 300px;
}

.equalizer-band {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  flex: 1;
}

.band-label {
  font-size: 11px;
  color: #999;
  text-align: center;
  white-space: nowrap;
}

.band-value {
  font-size: 11px;
  color: #1db954;
  text-align: center;
  min-height: 16px;
}

.vertical-slider {
  -webkit-appearance: none;
  appearance: none;
  width: 4px;
  height: 200px;
  background: #333;
  outline: none;
  border-radius: 2px;
  writing-mode: bt-lr; /* IE */
  -webkit-appearance: slider-vertical; /* WebKit */
}

/* 自定义垂直滑块样式 */
.equalizer-band input[type="range"].vertical-slider {
  -webkit-appearance: none;
  appearance: none;
  width: 200px;
  height: 4px;
  background: #333;
  border-radius: 2px;
  transform: rotate(-90deg);
  transform-origin: center;
  margin: 100px 0;
}

.equalizer-band input[type="range"].vertical-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 16px;
  height: 16px;
  background: #1db954;
  cursor: pointer;
  border-radius: 50%;
  transition: all 0.2s ease;
}

.equalizer-band input[type="range"].vertical-slider::-webkit-slider-thumb:hover {
  background: #1ed760;
  transform: scale(1.2);
}

.equalizer-band input[type="range"].vertical-slider::-moz-range-thumb {
  width: 16px;
  height: 16px;
  background: #1db954;
  cursor: pointer;
  border-radius: 50%;
  border: none;
  transition: all 0.2s ease;
}

.equalizer-band input[type="range"].vertical-slider::-moz-range-thumb:hover {
  background: #1ed760;
  transform: scale(1.2);
}

/* 响应式设计 */
@media (max-width: 768px) {
  .equalizer-controls {
    flex-wrap: wrap;
    height: auto;
  }

  .equalizer-band {
    min-width: 40px;
  }

  .equalizer-band span {
    width: 100%;
    text-align: left;
  }
}

.input {
  margin-bottom: 15px;
  box-sizing: border-box;
}

.textarea {
  width: 100%;
  padding: 10px;
  border: 1px solid #333;
  border-radius: 4px;
  background-color: #121212;
  color: #fff;
  font-size: 14px;
  margin-bottom: 20px;
  box-sizing: border-box;
  min-height: 100px;
  resize: vertical;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.playlist-selector {
  max-height: 300px;
  overflow-y: auto;
  margin-bottom: 20px;
}

.playlist-option {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px;
  border-radius: 4px;
  cursor: pointer;
  margin-bottom: 5px;
  transition: background-color 0.2s ease;
}

.playlist-option:hover {
  background-color: #2a2a2a;
}

.playlist-option-name {
  font-size: 14px;
}

.playlist-option-count {
  font-size: 12px;
  color: #666;
  background-color: #2a2a2a;
  padding: 2px 6px;
  border-radius: 10px;
}

/* 滚动条样式 */
.sidebar::-webkit-scrollbar,
.song-list::-webkit-scrollbar,
.playlist-selector::-webkit-scrollbar {
  width: 8px;
}

.sidebar::-webkit-scrollbar-track,
.song-list::-webkit-scrollbar-track,
.playlist-selector::-webkit-scrollbar-track {
  background: #1e1e1e;
}

.sidebar::-webkit-scrollbar-thumb,
.song-list::-webkit-scrollbar-thumb,
.playlist-selector::-webkit-scrollbar-thumb {
  background: #333;
  border-radius: 4px;
}

.sidebar::-webkit-scrollbar-thumb:hover,
.song-list::-webkit-scrollbar-thumb:hover,
.playlist-selector::-webkit-scrollbar-thumb:hover {
  background: #444;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .main-content {
    flex-direction: column;
  }
  
  .sidebar {
    width: 100%;
    border-right: none;
    border-bottom: 1px solid #333;
    padding: 10px;
  }
  
  .player-container {
    flex-direction: column;
    text-align: center;
  }
  
  .album-art {
    margin-right: 0;
    margin-bottom: 15px;
  }
}

/* 转码通知样式 */
.transcode-notification {
  position: fixed;
  top: 20px;
  right: 20px;
  background: rgba(0, 0, 0, 0.85);
  color: white;
  padding: 16px;
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  z-index: 10000;
  min-width: 300px;
  max-width: 400px;
  animation: slideIn 0.3s ease-out;
}

.transcode-content {
  display: flex;
  align-items: center;
  gap: 12px;
}

.transcode-icon {
  font-size: 24px;
  min-width: 30px;
  text-align: center;
}

.transcode-text {
  flex: 1;
}

.transcode-title {
  font-weight: bold;
  margin-bottom: 4px;
  font-size: 14px;
}

.transcode-message {
  font-size: 13px;
  opacity: 0.9;
}

@keyframes slideIn {
  from {
    transform: translateX(100%);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}
</style>