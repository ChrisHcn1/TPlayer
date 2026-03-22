<template>
  <div id="app" class="tplayer-container" :class="{ 'light': theme === 'light' }">
    <!-- 顶部信息栏 -->
    <header class="top-bar" data-tauri-drag-region>
      <div class="app-logo" data-tauri-drag-region="false">
        <h1>TPlayer</h1>
      </div>
      <div class="window-controls" data-tauri-drag-region="false">
        <button class="control-btn minimize" @click="minimizeWindow" title="最小化">−</button>
        <button class="control-btn maximize" @click="toggleMaximizeWindow" title="最大化/还原">□</button>
        <button class="control-btn close" @click="closeWindow" title="关闭">×</button>
      </div>
    </header>
    
    <!-- 主内容区 -->
    <main class="main-content">
      <!-- 左侧边栏 -->
      <aside class="sidebar" :class="{ 'collapsed': !sidebarVisible }">
        <div class="sidebar-header">
          <button class="toggle-btn" @click="toggleSidebar" title="切换侧边栏">
            {{ sidebarVisible ? '◀' : '▶' }}
          </button>
          <h2>播放列表</h2>
        </div>
        <nav class="sidebar-nav">
          <ul>
            <li class="nav-item active" @click="switchFilter('all')">
              <span class="nav-icon">🎵</span>
              <span class="nav-text">全部歌曲</span>
            </li>
            <li class="nav-item" @click="switchFilter('favorites')">
              <span class="nav-icon">❤️</span>
              <span class="nav-text">我喜欢</span>
            </li>
            <li class="nav-item" @click="switchFilter('artists')">
              <span class="nav-icon">👤</span>
              <span class="nav-text">艺术家</span>
            </li>
            <li class="nav-item" @click="switchFilter('albums')">
              <span class="nav-icon">💽</span>
              <span class="nav-text">专辑</span>
            </li>
          </ul>
        </nav>
        <div class="sidebar-footer">
          <button class="btn primary" @click="createPlaylist" title="创建歌单">
            + 歌单
          </button>
        </div>
      </aside>
      
      <!-- 右侧内容区 -->
      <section class="content-area" :class="{ 'sidebar-collapsed': !sidebarVisible }">
        <!-- 过滤控制区 -->
        <div class="filter-controls">
          <div class="filter-header">
            <div class="filter-title">
              <h2>{{ currentFilterText }}</h2>
              <div class="playlist-info">
                {{ filteredSongs.length }} 首歌曲 • 时长: {{ totalDurationText }}
              </div>
            </div>
            <div class="filter-actions">
              <button class="btn primary" @click="scanMusic" title="扫描音乐">
                📁 扫描音乐
              </button>
            </div>
          </div>
          <!-- 搜索框 -->
          <div class="search-box">
            <input 
              type="text" 
              v-model="searchQuery" 
              placeholder="搜索歌曲、艺术家或专辑..."
              @input="handleSearch"
            />
            <button class="search-btn" @click="handleSearch">🔍</button>
          </div>
        </div>
        
        <!-- 歌曲列表 -->
        <div class="song-list-container">
          <div v-if="songs.length === 0" class="empty-state">
            <div class="empty-icon">🎵</div>
            <p>暂无歌曲</p>
            <p class="empty-hint">点击上方的"扫描音乐"按钮添加音乐</p>
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
                <div class="artist-count">{{ artist.count }} 首</div>
              </div>
            </div>
            <div class="artists-content">
              <div v-if="!selectedArtist" class="empty-selection">
                <p>请选择一个艺术家</p>
              </div>
              <div v-else class="song-list">
                <!-- 表头 -->
                <table class="songs-table table-header">
                  <thead>
                    <tr>
                      <th class="col-index">#</th>
                      <th class="col-title">标题</th>
                      <th class="col-album">专辑</th>
                      <th class="col-duration">时长</th>
                      <th class="col-actions">操作</th>
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
                        title="收藏"
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
                <div class="album-count">{{ album.count }} 首</div>
              </div>
            </div>
            <div class="albums-content">
              <div v-if="!selectedAlbum" class="empty-selection">
                <p>请选择一个专辑</p>
              </div>
              <div v-else class="song-list">
                <!-- 表头 -->
                <table class="songs-table table-header">
                  <thead>
                    <tr>
                      <th class="col-index">#</th>
                      <th class="col-title">标题</th>
                      <th class="col-artist">艺术家</th>
                      <th class="col-duration">时长</th>
                      <th class="col-actions">操作</th>
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
                        title="收藏"
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
                  <th class="col-title">标题</th>
                  <th class="col-artist">艺术家</th>
                  <th class="col-album">专辑</th>
                  <th class="col-duration">时长</th>
                  <th class="col-actions">操作</th>
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
        </div>
        
        <!-- 均衡器面板 -->
        <div class="equalizer-panel" :class="{ 'visible': equalizerVisible }">
          <div class="equalizer-header">
            <h3>均衡器</h3>
            <button class="close-btn" @click="toggleEqualizer" title="关闭">×</button>
          </div>
          <div class="equalizer-content">
            <div class="presets">
              <select v-model="currentPreset" @change="applyPreset">
                <option value="flat">平坦</option>
                <option value="rock">摇滚</option>
                <option value="pop">流行</option>
                <option value="jazz">爵士</option>
                <option value="classical">古典</option>
                <option value="electronic">电子</option>
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
            <img v-if="currentSong.cover" :src="currentSong.cover" alt="封面" />
            <div v-else class="cover-placeholder">🎵</div>
          </div>
          <div class="song-info">
            <h3 v-if="currentSong" ref="titleElement" :class="{ 'long-text': isTextLong('title') }">
              <span class="ellipsis-text">{{ getDisplayTitle(currentSong) }}</span>
            </h3>
            <p v-if="currentSong" ref="artistElement" :class="{ 'long-text': isTextLong('artist') }">
              <span class="ellipsis-text">{{ getDisplayArtist(currentSong) }} - {{ getDisplayAlbum(currentSong) }}</span>
            </p>
            <p v-else class="no-song">未选择歌曲</p>
          </div>
        </div>
      </div>
      <div class="player-center">
        <div class="playback-controls">
          <button class="control-btn" @click="changePlaybackMode" title="播放模式">
            {{ playbackModeIcon }}
          </button>
          <button class="control-btn" @click="playPrevious" title="上一首">⏮</button>
          <button class="control-btn play" @click="togglePlayback" title="播放/暂停">
            {{ isPlaying ? '⏸' : '▶' }}
          </button>
          <button class="control-btn" @click="playNext" title="下一首">⏭</button>
          <button class="control-btn" @click="toggleRepeat" title="重复">
            {{ isRepeating ? '🔁' : '➡️' }}
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
        <div v-if="showLyrics" class="lyrics-display" :class="{ 'has-lyrics': lyrics.length > 0 }">
          <div v-if="lyrics.length === 0" class="lyrics-placeholder">
            暂无歌词
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
            <button class="control-btn" @click="toggleMute" title="静音">
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
          
          <button class="control-btn" @click="showLyrics = !showLyrics" title="显示/隐藏歌词">
            🎵
          </button>
          <button class="control-btn" @click="showSettingsModal = true" title="设置">
            ⚙️
          </button>
        </div>
        
        <!-- 下一首歌曲信息 - 仅当正在播放时显示 -->
        <div class="next-song-info" v-if="currentSong && nextSong">
          <div class="next-song-label">下一首</div>
          <div class="next-song-title" :title="getDisplayTitle(nextSong)">
            {{ getDisplayTitle(nextSong) }}
          </div>
          <div class="next-song-artist" :title="getDisplayArtist(nextSong)">
            {{ getDisplayArtist(nextSong) }}
          </div>
          <button class="skip-next-btn" @click="skipNextSong" title="跳过下一首">
            跳过 ⏭
          </button>
        </div>
      </div>
    </footer>
    
    <!-- 歌曲菜单 -->
    <div v-if="showSongMenu" class="song-menu" :style="menuPosition">
      <ul>
        <li @click="playSong(selectedSong!)">播放</li>
        <li @click="addSongToPlaylist(selectedSong!)">添加到歌单</li>
        <li @click="toggleFavorite(selectedSong!)">
          {{ selectedSong?.isFavorite ? '取消收藏' : '添加到收藏' }}
        </li>
        <li @click="editSongTags(selectedSong!)">编辑歌曲标签</li>
        <li @click="deleteSong(selectedSong!)" class="danger">删除</li>
      </ul>
    </div>
    
    <!-- 编辑歌曲标签模态框 -->
    <div v-if="showEditTagsModal" class="modal-overlay" @click="closeEditTagsModal">
      <div class="modal-content edit-tags-modal" @click.stop>
        <div class="modal-header">
          <h3>编辑歌曲标签</h3>
          <button class="close-btn" @click="closeEditTagsModal">×</button>
        </div>
        <div class="modal-body">
          <!-- 在线匹配 -->
          <div class="match-section">
            <span>不想手动填写标签？</span>
            <button class="match-btn" @click="onlineMatch">
              自动匹配标签
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
                基本信息
              </button>
              <button 
                class="tab-button" 
                :class="{ active: activeTab === 'lyric' }"
                @click="activeTab = 'lyric'"
              >
                歌词
              </button>
              <button 
                class="tab-button" 
                :class="{ active: activeTab === 'cover' }"
                @click="activeTab = 'cover'"
              >
                封面
              </button>
            </div>
            
            <!-- 基本信息标签页 -->
            <div v-show="activeTab === 'info'" class="tab-content">
              <div class="form-row">
                <div class="form-group">
                  <label>文件名</label>
                  <input type="text" v-model="editTagsForm.fileName" disabled>
                </div>
              </div>
              <div class="form-row">
                <div class="form-group">
                  <label>标题</label>
                  <input type="text" v-model="editTagsForm.title" placeholder="请输入标题">
                </div>
              </div>
              <div class="form-row">
                <div class="form-group">
                  <label>艺术家</label>
                  <input type="text" v-model="editTagsForm.artist" placeholder="请输入艺术家">
                </div>
              </div>
              <div class="form-row">
                <div class="form-group">
                  <label>专辑</label>
                  <input type="text" v-model="editTagsForm.album" placeholder="请输入专辑">
                </div>
              </div>
              <div class="form-row">
                <div class="form-group">
                  <label>专辑艺术家</label>
                  <input type="text" v-model="editTagsForm.albumArtist" placeholder="请输入专辑艺术家">
                </div>
              </div>
              <div class="form-row">
                <div class="form-group">
                  <label>流派</label>
                  <input type="text" v-model="editTagsForm.genre" placeholder="请输入流派">
                </div>
              </div>
              <div class="form-row three-col">
                <div class="form-group">
                  <label>年份</label>
                  <input type="text" v-model="editTagsForm.year" placeholder="请输入年份">
                </div>
                <div class="form-group">
                  <label>音轨号</label>
                  <input type="text" v-model="editTagsForm.trackNumber" placeholder="请输入音轨号">
                </div>
                <div class="form-group">
                  <label>光盘号</label>
                  <input type="text" v-model="editTagsForm.discNumber" placeholder="请输入光盘号">
                </div>
              </div>
              <div class="form-row">
                <div class="form-group">
                  <label>别名</label>
                  <input type="text" v-model="editTagsForm.alia" placeholder="请输入别名">
                </div>
              </div>
              <div class="form-row">
                <div class="form-group">
                  <label>路径</label>
                  <div class="input-with-button">
                    <input type="text" :value="songToEdit?.path" disabled>
                    <button class="copy-btn" @click="copyPath">复制</button>
                  </div>
                </div>
              </div>
              <div class="lyric-actions">
                <button class="action-btn" @click="readLocalMetadata">从文件读取元数据</button>
                <button class="action-btn" @click="autoMatchTags">从文件名匹配</button>
                <button class="action-btn" @click="onlineMatch">在线匹配标签</button>
                <button class="action-btn" @click="fetchCover">获取封面</button>
              </div>
            </div>
            
            <!-- 歌词标签页 -->
            <div v-show="activeTab === 'lyric'" class="tab-content">
              <div class="form-group">
                <label>歌词</label>
                <textarea 
                  v-model="editTagsForm.lyric" 
                  placeholder="请输入歌词" 
                  rows="10"
                ></textarea>
              </div>
              <div class="lyric-actions">
                <button class="action-btn" @click="fetchLyric">获取歌词</button>
              </div>
            </div>
            
            <!-- 封面标签页 -->
            <div v-show="activeTab === 'cover'" class="tab-content">
              <div class="cover-section">
                <div class="cover-preview" @click="changeCover">
                  <img v-if="editTagsForm.cover" :src="editTagsForm.cover" alt="封面">
                  <div v-else class="cover-placeholder">点击更换封面</div>
                </div>
                <div class="cover-actions">
                  <button class="action-btn" @click="changeCover">选择封面</button>
                  <button class="action-btn" @click="fetchCover">获取封面</button>
                </div>
              </div>
            </div>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn-cancel" @click="closeEditTagsModal">取消</button>
          <button class="btn-save" @click="saveSongTags">保存</button>
        </div>
      </div>
    </div>
    
    <!-- 设置模态框 -->
    <div v-if="showSettingsModal" class="modal-overlay" @click="showSettingsModal = false">
      <div class="modal-content settings-modal" @click.stop>
        <div class="modal-header">
          <h2>设置</h2>
          <button class="close-btn" @click="showSettingsModal = false">×</button>
        </div>
        <div class="modal-body">
          <Settings
            v-model:crossfadeEnabled="crossfadeEnabled"
            v-model:crossfadeDuration="crossfadeDuration"
            v-model:autoPlayNext="autoPlayNext"
            v-model:theme="theme"
            v-model:showLyrics="showLyrics"
            v-model:equalizerEnabled="equalizerVisible"
            v-model:currentPreset="currentPreset"
            v-model:enableTranscode="enableTranscode"
            v-model:forceTranscode="forceTranscode"
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
            <button class="cover-modal-btn" @click="toggleCoverModalFullscreen" title="全屏/还原">
              {{ isCoverModalFullscreen ? '⛶' : '□' }}
            </button>
            <button class="cover-modal-btn" @click="closeCoverModal" title="关闭">✕</button>
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
                class="cover-lyric-line"
                :class="{ 'active': index === currentLyricIndex }"
                :ref="el => { if (el && el instanceof HTMLElement) coverLyricLineRefs[index] = el }"
              >
                {{ line.text }}
              </div>
            </div>
            <div v-else class="cover-modal-no-lyrics">
              暂无歌词
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { isTauri } from '@tauri-apps/api/core'
import { localStorageService, type Playlist } from './stores/local'
import { matchSong, songLyric, searchSong, fetchLyricById } from './api/music'
import * as mm from 'music-metadata'
import Settings from './components/Settings.vue'
// RecycleScroller组件通过VueVirtualScroller插件注册

// 日志开关：设置为 false 可禁用所有日志输出
const ENABLE_LOGS = false

// 日志函数
function logInfo(...args: any[]) {
  if (ENABLE_LOGS) {
    console.log(...args)
  }
}

function logError(...args: any[]) {
  if (ENABLE_LOGS) {
    console.error(...args)
  }
}

// 类型定义
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
const activeTab = ref('info')

// 歌词相关状态
const lyrics = ref<LyricLine[]>([])
const currentLyricIndex = ref(-1)
const showLyrics = ref(true)

// 主题相关状态
const theme = ref<'dark' | 'light'>('dark')

// 播放设置
const crossfadeEnabled = ref(false)
const crossfadeDuration = ref(1) // 默认为1秒，范围0-3秒
const autoPlayNext = ref(true) // 自动播放下一首
const enableTranscode = ref(true) // 启用转码
const forceTranscode = ref(false) // 强制转码
const playbackStartTime = ref(Date.now()) // 开始播放的时间
const pauseStartTime = ref<number | null>(null) // 开始暂停的时间
const pausedDuration = ref(0) // 累计暂停的时间

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
const isRepeating = ref(false)
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
const currentFilter = ref<'all' | 'favorites' | 'artists' | 'albums'>('all')
const searchQuery = ref('')
const selectedArtist = ref<string>('')
const selectedAlbum = ref<string>('')

// 均衡器
const currentPreset = ref('flat')
const equalizerBands = ref<number[]>([0, 0, 0, 0, 0, 0, 0, 0, 0, 0])

// 计算属性
const currentFilterText = computed(() => {
  const filters = {
    all: '全部歌曲',
    favorites: '我喜欢的歌曲',
    artists: '艺术家',
    albums: '专辑'
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

const playbackModeIcon = computed(() => {
  switch (playbackMode.value) {
    case 'order': return '➡️'
    case 'random': return '🔀'
    case 'repeat': return '🔁'
    default: return '➡️'
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
const coverLyricLineRefs = ref<(HTMLElement | null)[]>([])
const coverModalContent = ref<HTMLElement | null>(null)

// 封面模态框拖动和全屏状态
const isCoverModalFullscreen = ref(false)
const coverModalPosition = ref<{ left: string; top: string; transform?: string }>({ left: '50%', top: '50%', transform: 'translate(-50%, -50%)' })
let isDraggingCoverModal = false
let dragStartX = 0
let dragStartY = 0
let modalStartX = 0
let modalStartY = 0

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

const switchFilter = (filter: 'all' | 'favorites' | 'artists' | 'albums') => {
  currentFilter.value = filter
  // 重置选中的艺术家和专辑
  if (filter !== 'artists') {
    selectedArtist.value = ''
  }
  if (filter !== 'albums') {
    selectedAlbum.value = ''
  }
}

const scanMusic = async () => {
  try {
    // 检测是否在Tauri环境中
    const tauri = await isTauri()
    
    if (!tauri) {
      alert('请在桌面应用中运行此功能')
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
        
        if (result && result.tracks) {
          const trackCount = result.tracks.length
          
          if (trackCount > 0) {
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
            
            songs.value = result.tracks
            alert(`扫描完成，共找到 ${trackCount} 首歌曲`)
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

// 播放状态管理
let playSongLock: Promise<void> | null = null
let currentPlayId = 0 // 用于跟踪当前播放请求的唯一ID

const playSong = async (song: Song, position: number = 0) => {
  // 生成本次播放请求的唯一ID
  const thisPlayId = ++currentPlayId
  logInfo(`[播放保护] 开始播放请求, ID: ${thisPlayId}, 歌曲: ${song.title}`)
  
  // 如果有正在执行的播放操作，等待它完成
  if (playSongLock) {
    logInfo(`[播放保护] 播放请求 ${thisPlayId}: 检测到正在进行的播放操作，等待完成`)
    try {
      await playSongLock
    } catch {
      // 忽略之前操作的错误
    }
    
    // 等待完成后，检查是否已经有更新的播放请求（不是当前这个）
    if (thisPlayId !== currentPlayId) {
      logInfo(`[播放保护] 播放请求 ${thisPlayId}: 等待完成后发现有更新的请求 ${currentPlayId}，跳过本次调用`)
      return
    }
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
    if (crossfadeEnabled.value && crossfadeDuration.value > 0) {
      logInfo('启用交叉淡入淡出，时长:', crossfadeDuration.value, '秒')
      logInfo('交叉淡入淡出前的原始音量:', originalVolume, '当前音量:', volume.value, '静音状态:', isMuted.value)
      // 临时将音量设置为0
      volume.value = 0
      await updateVolume()
    }

    // 重置前端状态
    currentSong.value = song
    currentPosition.value = position
    // 计算进度百分比
    if (song.duration && song.duration !== '未知') {
      const parts = song.duration.split(':')
      if (parts.length === 2) {
        const minutes = parseInt(parts[0])
        const seconds = parseInt(parts[1])
        const totalSeconds = minutes * 60 + seconds
        if (totalSeconds > 0) {
          progress.value = Math.min((position / totalSeconds) * 100, 100)
        } else {
          progress.value = 0
        }
      } else {
        progress.value = 0
      }
    } else {
      progress.value = 0
    }
    isPlaying.value = true
    
    // 重置播放完成标志
    isPlaybackFinished = false
    
    // 重置暂停相关状态
    pauseStartTime.value = null
    pausedDuration.value = 0
    logInfo('重置暂停状态: pauseStartTime=null, pausedDuration=0')
    
    // 更新开始播放的时间
    playbackStartTime.value = Date.now() - (position * 1000)
    logInfo('更新播放开始时间:', playbackStartTime.value, '位置:', position)

    // 调用后端播放命令，传入当前音量、转码设置和位置
    // 只有在启用转码时才传递转码参数
    const shouldTranscode = enableTranscode.value ? forceTranscode.value : false
    logInfo('调用后端 play_song 命令，音量:', volume.value, '路径:', song.path, '启用转码:', enableTranscode.value, '强制转码:', forceTranscode.value, '实际转码:', shouldTranscode, '位置:', position)
    const result: any = await invoke('play_song', { path: song.path, volume: volume.value, force_transcode: shouldTranscode, position: position })

    logInfo('后端返回结果:', result)

    // 解析歌词（优先使用本地歌词）
    logInfo('【歌词加载】开始加载歌词，歌曲:', song.title)
    logInfo('【歌词加载】歌曲lyric字段长度:', song.lyric ? song.lyric.length : 0)
    logInfo('【歌词加载】歌曲lyric字段内容:', song.lyric ? song.lyric.substring(0, 100) + '...' : '空')
    
    // 首先尝试从文件系统读取歌词文件（动态加载）
    logInfo('【歌词加载】尝试从文件系统动态加载歌词')
    try {
      // 构建歌词文件路径
      const songPath = song.path
      const lyricPath = songPath.replace(/\.[^/.]+$/, '.lrc')
      logInfo('【歌词加载】尝试读取歌词文件:', lyricPath)
      
      // 使用Tauri的文件系统API读取歌词文件
      const { readTextFile } = await import('@tauri-apps/plugin-fs')
      const lyricContent = await readTextFile(lyricPath)
      logInfo('【歌词加载】成功读取歌词文件，长度:', lyricContent.length)
      logInfo('【歌词加载】歌词文件内容预览:', lyricContent.substring(0, 200))
      
      // 解析歌词
      lyrics.value = parseLyrics(lyricContent)
      logInfo('【歌词加载】文件系统歌词已加载:', lyrics.value.length, '行')
      if (lyrics.value.length > 0) {
        logInfo('【歌词加载】前3行歌词:')
        lyrics.value.slice(0, 3).forEach((line, i) => {
          logInfo(`  [${i}] ${line.time.toFixed(1)}s: ${line.text}`)
        })
      }
    } catch (error) {
      logInfo('【歌词加载】从文件系统读取歌词失败，尝试使用歌曲对象中的歌词:', error)
      
      // 如果文件系统读取失败，尝试使用歌曲对象中的歌词
      if (song.lyric && song.lyric.trim()) {
        logInfo('【歌词加载】从歌曲对象加载歌词')
        lyrics.value = parseLyrics(song.lyric)
        logInfo('【歌词加载】本地歌词已加载:', lyrics.value.length, '行')
        if (lyrics.value.length > 0) {
          logInfo('【歌词加载】前3行歌词:')
          lyrics.value.slice(0, 3).forEach((line, i) => {
            logInfo(`  [${i}] ${line.time.toFixed(1)}s: ${line.text}`)
          })
        }
      } else {
        logInfo('【歌词加载】歌曲对象中也没有歌词')
        lyrics.value = []
      }
    }
    
    logInfo('【歌词加载】最终歌词状态:', {
      歌词行数: lyrics.value.length,
      showLyrics: showLyrics.value,
      currentLyricIndex: currentLyricIndex.value
    })
    currentLyricIndex.value = -1

    if (result) {
      isPlaying.value = true
      logInfo('playSong 完成, isPlaying 设置为 true, 结果时长:', result.duration)
    } else {
      console.warn('后端返回了空结果')
      isPlaying.value = true // 即使没有返回结果也设置为播放状态
    }

    // 交叉淡入淡出：逐渐恢复音量
    if (crossfadeEnabled.value && crossfadeDuration.value > 0) {
      logInfo('开始交叉淡入淡出效果')
      const fadeDuration = crossfadeDuration.value * 1000 // 转换为毫秒
      const steps = 20 // 淡入步骤数
      const stepDuration = fadeDuration / steps
      const volumeStep = originalVolume / steps
      
      // 逐渐增加音量
      for (let i = 1; i <= steps; i++) {
        await new Promise(resolve => setTimeout(resolve, stepDuration))
        volume.value = volumeStep * i
        await updateVolume()
      }
      
      // 确保音量恢复到原始值
      volume.value = originalVolume
      await updateVolume()
      
      // 如果之前是静音状态，恢复静音
      if (isMuted.value) {
        volume.value = 0
        await updateVolume()
        logInfo('交叉淡入淡出后恢复静音状态')
      }
      
      logInfo('交叉淡入淡出效果完成，最终音量:', volume.value, '静音状态:', isMuted.value)
    }
    
    // 启动前端进度更新
    logInfo('前端 播放开始，立即调用 updateProgress')
    updateProgress()
    
    // 重置播放完成标志
    isPlaybackFinished = false
    
    // 启动播放完成检测定时器
    if (currentSong.value) {
      const duration = currentSong.value.duration
      // 只有当时长不是"未知"时才设置播放完成定时器
      if (duration !== '未知') {
        const parts = duration.split(':')
        if (parts.length === 2) {
          const minutes = parseInt(parts[0])
          const seconds = parseInt(parts[1])
          const totalSeconds = minutes * 60 + seconds
          if (totalSeconds > 0) {
            logInfo('前端 启动播放完成检测定时器，总时长:', totalSeconds, '秒')
            // 立即检测一次，确保播放完成检测逻辑正常
            logInfo('前端 立即检测播放完成状态')
            const elapsedSeconds = (Date.now() - playbackStartTime.value) / 1000 - pausedDuration.value
            if (elapsedSeconds >= totalSeconds - 0.5) {
              logInfo('前端 立即检测到播放完成，调用 handlePlaybackFinished')
              isPlaying.value = false
              handlePlaybackFinished()
            } else {
              logInfo('前端 立即检测未完成，elapsed:', elapsedSeconds, 'total:', totalSeconds)
            }
            // 设置定时器
            const playbackTimer = setTimeout(() => {
              logInfo('前端 播放完成检测定时器触发')
              if (isPlaying.value) {
                logInfo('前端 检测到播放完成，调用 handlePlaybackFinished')
                isPlaying.value = false
                handlePlaybackFinished()
              }
            }, (totalSeconds - 0.5) * 1000)
            
            // 保存定时器ID，以便在需要时清除
            playbackTimerId = playbackTimer
          }
        }
      } else {
        logInfo('前端 时长为未知，不设置播放完成检测定时器')
      }
    }
    
    // 输出一些调试信息
    logInfo('前端 播放开始调试信息:', {
      currentSong: currentSong.value?.title,
      isPlaying: isPlaying.value,
      playbackStartTime: playbackStartTime.value,
      pausedDuration: pausedDuration.value
    })
    
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
  } catch (error) {
    logError('播放歌曲失败:', error)
    // 检查是否是转码相关的错误
    const errorMessage = error && typeof error === 'string' ? error : ''
    if (errorMessage.includes('FFmpeg') || errorMessage.includes('转码')) {
      logInfo('转码相关错误，静默处理并尝试播放下一首')
      // 转码错误，不显示错误信息，自动尝试播放下一首
      isPlaying.value = false
      // 延迟500ms后尝试播放下一首
      setTimeout(async () => {
        if (autoPlayNext.value && songs.value.length > 1) {
          logInfo('转码失败，自动跳到下一首')
          await playNext()
        }
      }, 500)
    } else {
      // 其他错误，显示提示
      alert(`播放失败：${error} \n请确认音频文件存在且格式受支持`)
      isPlaying.value = false
      // 延迟500ms后尝试播放下一首
      setTimeout(async () => {
        if (autoPlayNext.value && songs.value.length > 1) {
          logInfo('播放失败，自动跳到下一首')
          await playNext()
        }
      }, 500)
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

const togglePlayback = async () => {
  try {
    logInfo('togglePlayback 被调用,当前 isPlaying:', isPlaying.value)
    
    if (!currentSong.value) {
      logInfo('当前没有歌曲,播放第一首')
      if (songs.value.length > 0) {
        await playSong(songs.value[0])
      }
      return
    }

    if (isPlaying.value) {
      logInfo('暂停播放')
      // 记录暂停开始时间
      pauseStartTime.value = Date.now()
      logInfo('暂停开始时间:', pauseStartTime.value)
      await invoke('pause_song')
      isPlaying.value = false
    } else {
      logInfo('恢复播放')
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
      await invoke('resume_song')
      isPlaying.value = true
    }
    
    logInfo('togglePlayback 完成,新 isPlaying:', isPlaying.value)
  } catch (error) {
    logError('切换播放状态失败:', error)
    const errorMessage = error && typeof error === 'string' ? error : '未知错误'
    alert(`播放控制失败：${errorMessage}`)
  }
}

const playPrevious = async () => {
  try {
    if (songs.value.length === 0) return
    
    let currentIndex = songs.value.findIndex(song => song.id === currentSong.value?.id)
    
    // 如果当前播放进度超过3秒,重新播放当前歌曲
    if (currentPosition.value > 3 && currentSong.value) {
      await playSong(currentSong.value)
      return
    }
    
    if (currentIndex === -1) {
      // 如果当前没有播放歌曲，播放最后一首
      currentIndex = songs.value.length - 1
    } else {
      // 播放上一首
      currentIndex = (currentIndex - 1 + songs.value.length) % songs.value.length
    }
    
    await playSong(songs.value[currentIndex])
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
    await playSong(songs.value[targetIndex])
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
  
  // 否则使用标题或文件名
  return song.title || getFileNameWithoutExtension(song.path)
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

const toggleRepeat = () => {
  isRepeating.value = !isRepeating.value
}

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
    await invoke('set_volume', { volume: volume.value })
  } catch (error) {
    logError('设置音量失败:', error)
  }
}

// 记录拖动前的播放状态
let wasPlayingBeforeSeek = false

const handleSeeking = () => {
  // 用户正在拖动进度条
  logInfo('用户正在拖动进度条, progress.value:', progress.value)
  
  // 记录拖动前的播放状态
  if (!isSeeking.value) {
    wasPlayingBeforeSeek = isPlaying.value
    logInfo('记录拖动前播放状态:', wasPlayingBeforeSeek)
  }
  
  isSeeking.value = true
}

const seek = async () => {
  isSeeking.value = true
  logInfo('seek 被调用, progress.value:', progress.value, '拖动前播放状态:', wasPlayingBeforeSeek)

  try {
    if (currentSong.value) {
      // 如果时长为"未知"，不允许拖动进度条
      if (currentSong.value.duration === '未知') {
        logInfo('时长为未知，不允许拖动进度条')
        isSeeking.value = false
        return
      }
      
      // 计算实际的播放位置（秒）
      const parts = currentSong.value.duration.split(':')
      if (parts.length === 2) {
        const minutes = parseInt(parts[0])
        const seconds = parseInt(parts[1])
        const totalSeconds = minutes * 60 + seconds
        if (totalSeconds > 0) {
          const actualPosition = (progress.value / 100) * totalSeconds

          logInfo('seek 计算位置: progress=' + progress.value + '%, totalSeconds=' + totalSeconds + ', actualPosition=' + actualPosition + 's')

          // 重新播放歌曲，从指定位置开始
          logInfo('重新播放歌曲，从指定位置开始:', actualPosition, '秒')
          
          // 保存当前播放状态
          const wasPlaying = isPlaying.value
          
          // 重新调用playSong，传入位置参数
          await playSong(currentSong.value, actualPosition)
          
          // 如果之前在播放，确保继续播放
          if (wasPlaying && !isPlaying.value) {
            logInfo('恢复播放状态')
            isPlaying.value = true
          }
        }
      }
    }
  } catch (error) {
    logError('跳转失败:', error)
  } finally {
    // 延迟重置标志,确保seek完成
    setTimeout(() => {
      logInfo('seek 完成,重置 isSeeking 标志')
      isSeeking.value = false
      wasPlayingBeforeSeek = false
    }, 300)
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
    await invoke('apply_equalizer_preset', { presetName: currentPreset.value })
    
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
  try {
    await invoke('set_equalizer', { bands: equalizerBands.value })
  } catch (error) {
    logError('设置均衡器失败:', error)
  }
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
    trackNumber: '',
    discNumber: '',
    alia: '',
    lyric: lyric,
    cover: song.cover || ''
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
    
    // 打开后立即滚动到当前歌词
    nextTick(() => {
      scrollToCurrentLyric()
    })
  }
}

// 滚动到当前歌词
const scrollToCurrentLyric = () => {
  const index = currentLyricIndex.value
  if (index >= 0 && coverLyricsContainer.value) {
    const lineElement = coverLyricLineRefs.value[index]
    if (lineElement && coverLyricsContainer.value) {
      const container = coverLyricsContainer.value
      const lineTop = lineElement.offsetTop
      const lineHeight = lineElement.offsetHeight
      const containerHeight = container.clientHeight
      const scrollTop = lineTop - containerHeight / 2 + lineHeight / 2
      
      container.scrollTo({
        top: Math.max(0, scrollTop),
        behavior: 'smooth'
      })
      logInfo('歌词滚动: 滚动到歌词行', index)
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
    coverModalPosition.value = {}
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
    if (!songToEdit.value) return
    
    logInfo('【在线歌词】开始在线查找歌词')
    
    const title = editTagsForm.value.title || songToEdit.value.title
    const artist = editTagsForm.value.artist || songToEdit.value.artist
    const keyword = `${title} ${artist}`
    
    logInfo('【在线歌词】搜索关键词:', keyword)
    
    const response = await searchSong(keyword, 5)
    const song = response.data.result?.songs?.[0]
    
    if (!song || !song.id) {
      logInfo('【在线歌词】未找到匹配的歌曲')
      alert('未找到匹配的歌曲，请修改歌曲信息后重试')
      return
    }
    
    logInfo('【在线歌词】找到歌曲:', song.name, 'ID:', song.id)
    
    const lyricResponse = await fetchLyricById(song.id)
    const lyricContent = lyricResponse.data.lrc?.lyric
    
    if (!lyricContent) {
      logInfo('【在线歌词】该歌曲暂无歌词')
      alert('该歌曲暂无歌词')
      return
    }
    
    logInfo('【在线歌词】获取歌词成功，长度:', lyricContent.length)
    logInfo('【在线歌词】歌词内容预览:', lyricContent.substring(0, 200))
    
    editTagsForm.value.lyric = lyricContent
    alert('获取歌词成功')
  } catch (error) {
    logError('【在线歌词】获取歌词失败:', error)
    alert('获取歌词失败，请检查网络连接后重试')
  }
}

// 获取封面
const fetchCover = async () => {
  try {
    if (!songToEdit.value) return
    
    logInfo('【在线封面】开始在线查找封面')
    
    const title = editTagsForm.value.title || songToEdit.value.title
    const artist = editTagsForm.value.artist || songToEdit.value.artist
    const keyword = `${title} ${artist}`
    
    logInfo('【在线封面】搜索关键词:', keyword)
    
    const response = await searchSong(keyword, 5)
    const song = response.data.result?.songs?.[0]
    
    if (!song || !song.album?.picUrl) {
      logInfo('【在线封面】未找到封面')
      alert('无法获取封面，请修改歌曲信息后重试')
      return
    }
    
    logInfo('【在线封面】获取封面成功:', song.album.picUrl)
    
    editTagsForm.value.cover = song.album.picUrl
    alert('获取封面成功')
  } catch (error) {
    logError('【在线封面】获取封面失败:', error)
    alert('获取封面失败，请检查网络连接后重试')
  }
}

// 自动匹配标签
const autoMatchTags = async () => {
  try {
    if (!songToEdit.value) return
    
    logInfo('【自动匹配】开始自动匹配标签')
    
    const title = editTagsForm.value.title || songToEdit.value.title
    const artist = editTagsForm.value.artist || songToEdit.value.artist
    const keyword = `${title} ${artist}`
    
    logInfo('【自动匹配】搜索关键词:', keyword)
    
    const response = await searchSong(keyword, 5)
    const song = response.data.result?.songs?.[0]
    
    if (!song || !song.id) {
      logInfo('【自动匹配】未找到匹配的歌曲')
      alert('未找到匹配的歌曲，请修改歌曲信息后重试')
      return
    }
    
    logInfo('【自动匹配】找到歌曲:', song.name, 'ID:', song.id)
    
    let matchedCount = 0
    
    // 更新元数据
    if (song.name && !editTagsForm.value.title) {
      editTagsForm.value.title = song.name
      matchedCount++
      logInfo('【自动匹配】更新标题:', song.name)
    }
    
    if (song.artists && song.artists.length > 0 && !editTagsForm.value.artist) {
      editTagsForm.value.artist = song.artists.map((a: any) => a.name).join(', ')
      matchedCount++
      logInfo('【自动匹配】更新艺术家:', editTagsForm.value.artist)
    }
    
    if (song.album && song.album.name && !editTagsForm.value.album) {
      editTagsForm.value.album = song.album.name
      matchedCount++
      logInfo('【自动匹配】更新专辑:', song.album.name)
    }
    
    // 获取封面
    if (song.album?.picUrl && !editTagsForm.value.cover) {
      editTagsForm.value.cover = song.album.picUrl
      matchedCount++
      logInfo('【自动匹配】更新封面:', song.album.picUrl)
    }
    
    // 获取歌词
    try {
      const lyricResponse = await fetchLyricById(song.id)
      const lyricContent = lyricResponse.data.lrc?.lyric
      
      if (lyricContent && !editTagsForm.value.lyric) {
        editTagsForm.value.lyric = lyricContent
        matchedCount++
        logInfo('【自动匹配】更新歌词，长度:', lyricContent.length)
      }
    } catch (error) {
      logInfo('【自动匹配】获取歌词失败:', error)
    }
    
    logInfo('【自动匹配】自动匹配完成，共匹配', matchedCount, '项')
    alert(`自动匹配完成，共匹配 ${matchedCount} 项`)
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
    const updatedSong = {
      ...songToEdit.value,
      ...editTagsForm.value
    }
    
    // 找到并更新歌曲列表中的歌曲
    const index = songs.value.findIndex(s => s.id === songToEdit.value?.id)
    if (index !== -1) {
      songs.value[index] = updatedSong
    }
    
    // 如果是当前播放的歌曲，也更新当前歌曲信息
    if (currentSong.value?.id === songToEdit.value.id) {
      currentSong.value = updatedSong
    }
    
    // 保存到本地存储
    await localStorageService.saveSongs(songs.value)
    
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
const parseLyrics = (lyricText: string): LyricLine[] => {
  const lines: LyricLine[] = []
  // 修改正则表达式，支持[mm:ss]和[mm:ss.sss]格式
  const regex = /\[(\d{2}):(\d{2}(?:\.\d{2,3})?)\](.*)/g
  let match
  
  logInfo('【歌词解析】开始解析歌词，文本长度:', lyricText.length)
  logInfo('【歌词解析】歌词文本前200字符:', lyricText.substring(0, 200).replace(/\n/g, '\\n'))
  
  let matchCount = 0
  while ((match = regex.exec(lyricText)) !== null) {
    matchCount++
    const minutes = parseInt(match[1])
    const seconds = parseFloat(match[2])
    const time = minutes * 60 + seconds
    const text = match[3].trim()
    
    // 只输出前5个匹配，避免日志过多
    if (matchCount <= 5) {
      logInfo(`【歌词解析】匹配 #${matchCount}: 时间=${time.toFixed(2)}s, 文本="${text.substring(0, 30)}"`)
    }
    
    if (text) {
      lines.push({ time, text })
    }
  }
  
  if (matchCount > 5) {
    logInfo(`【歌词解析】... 还有 ${matchCount - 5} 个匹配`)
  }
  
  // 按时间排序
  lines.sort((a, b) => a.time - b.time)
  logInfo('【歌词解析】解析完成，有效歌词行数:', lines.length)
  return lines
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
    if (showCoverModal.value && index >= 0) {
      nextTick(() => {
        scrollToCurrentLyric()
      })
    }
  }
}

const minimizeWindow = async () => {
  try {
    await invoke('minimize_window')
  } catch (error) {
    logError('最小化窗口失败:', error)
  }
}

const toggleMaximizeWindow = async () => {
  try {
    await invoke('toggle_maximize_window')
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

const updateProgress = async () => {
  updateProgressCallCount++
  logInfo('【前端】updateProgress 被调用, 次数:', updateProgressCallCount)

  try {
    // 每5秒输出一次,确认定时器在运行
    if (updateProgressCallCount % 25 === 0) {
      logInfo('前端 updateProgress 定时器运行中,已调用次数:', updateProgressCallCount, 'isPlaying=', isPlaying.value)
    }

    // 如果未播放或正在拖动进度条,不更新进度
    if (!isPlaying.value) {
      if (updateProgressCallCount % 25 === 0) {
        logInfo('前端 updateProgress: 未播放,跳过更新')
      }
      return
    }
    
    if (isSeeking.value) {
      if (updateProgressCallCount % 25 === 0) {
        logInfo('前端 updateProgress: 正在拖动进度条,跳过更新')
      }
      return
    }
    
    logInfo('前端 updateProgress: 开始获取位置')
      
    // 从后端获取实际播放位置，确保前后端状态一致
    try {
      const actualPosition = await invoke('get_position') as number
      logInfo('前端 从后端获取实际位置:', actualPosition, '秒')
      
      // 确保位置值不超过歌曲总长度
      let positionInSeconds = actualPosition
      let totalSeconds = 0
      if (currentSong.value) {
        logInfo('前端 当前歌曲:', currentSong.value.title, 'duration:', currentSong.value.duration)
        // 如果时长为"未知"，只更新位置，不计算进度百分比
        if (currentSong.value.duration === '未知') {
          currentPosition.value = positionInSeconds
          progress.value = 0
          logInfo('前端 时长为未知，不计算进度百分比')
          return
        }
        
        const parts = currentSong.value.duration.split(':')
        logInfo('前端 duration parts:', parts)
        if (parts.length === 2) {
          const minutes = parseInt(parts[0])
          const seconds = parseInt(parts[1])
          totalSeconds = minutes * 60 + seconds
          logInfo('前端 计算总秒数:', totalSeconds, '秒')
          if (totalSeconds > 0) {
            // 确保位置值不超过总长度
            if (positionInSeconds > totalSeconds) {
              positionInSeconds = totalSeconds
              logInfo('前端 限制位置值: 从', actualPosition, '秒限制为', positionInSeconds, '秒')
            }
          }
        }
      }
      
      // 调试播放完成检测
      if (currentSong.value && currentSong.value.duration !== '未知' && totalSeconds > 0) {
        logInfo('前端 播放完成检测: positionInSeconds=', positionInSeconds, 'totalSeconds=', totalSeconds, '条件:', positionInSeconds >= totalSeconds - 0.5)
      }

      // 直接赋值更新
      currentPosition.value = positionInSeconds

      logInfo('前端 赋值后 currentPosition.value:', currentPosition.value, '类型:', typeof currentPosition.value)

      // 强制触发响应式更新
      await nextTick()

      // 计算进度百分比
      if (currentSong.value && currentSong.value.duration !== '未知') {
        const parts = currentSong.value.duration.split(':')
        if (parts.length === 2) {
          const minutes = parseInt(parts[0])
          const seconds = parseInt(parts[1])
          const totalSeconds = minutes * 60 + seconds
          if (totalSeconds > 0) {
            // 确保进度不会超过100%
            const calculatedProgress = (positionInSeconds / totalSeconds) * 100
            progress.value = Math.min(calculatedProgress, 100)
            logInfo('前端 更新进度: position=' + positionInSeconds.toFixed(1) + 's, total=' + totalSeconds + 's, progress=' + progress.value.toFixed(2) + '%')
            
            // 预转码下一首歌曲（在剩余15秒时开始，且启用了转码功能）
            const remainingTime = totalSeconds - positionInSeconds
            if (enableTranscode.value && remainingTime <= 15 && !hasPretranscodedNextSong && nextSong.value) {
              logInfo('[预转码] 剩余时间:', remainingTime.toFixed(1), '秒，开始预转码下一首:', nextSong.value.title)
              hasPretranscodedNextSong = true
              
              // 在后台静默开始转码，不等待结果
              invoke('pretranscode_audio', { path: nextSong.value.path, force_transcode: forceTranscode.value }).catch((error) => {
                logError('[预转码] 预转码失败:', error)
              })
            }
          }
        }
      }

      // 同步歌词显示
      syncLyrics()

      // 输出更新后的值和歌词状态
      logInfo('前端 更新后 currentPosition.value:', currentPosition.value, 'formatTime:', formatTime(currentPosition.value), 'formattedCurrentPosition:', formattedCurrentPosition.value)
      
      // 每50次更新输出一次歌词状态
      if (updateProgressCallCount % 50 === 0 && lyrics.value.length > 0) {
        logInfo('【歌词状态】歌词总数:', lyrics.value.length, '当前索引:', currentLyricIndex.value, 'showLyrics:', showLyrics.value)
        if (currentLyricIndex.value >= 0 && currentLyricIndex.value < lyrics.value.length) {
          logInfo('【歌词状态】当前歌词:', lyrics.value[currentLyricIndex.value].text)
        }
      }

      // 尝试直接读取DOM
      const timeElements = document.querySelectorAll('.progress-info span')
      if (timeElements.length > 0) {
        logInfo('DOM中的时间显示:', timeElements[0].textContent)
      }
    } catch (error) {
      logError('从后端获取位置失败,使用本地计算:', error)
      // 备用方案：使用本地计算
      const now = Date.now()
      const elapsedSeconds = (now - playbackStartTime.value) / 1000 - pausedDuration.value
      
      let positionInSeconds = elapsedSeconds
      let totalSeconds = 0
      if (currentSong.value) {
        // 如果时长为"未知"，只更新位置，不计算进度百分比
        if (currentSong.value.duration === '未知') {
          currentPosition.value = positionInSeconds
          progress.value = 0
          syncLyrics()
          return
        }
        
        const parts = currentSong.value.duration.split(':')
        if (parts.length === 2) {
          const minutes = parseInt(parts[0])
          const seconds = parseInt(parts[1])
          totalSeconds = minutes * 60 + seconds
          if (totalSeconds > 0) {
            if (positionInSeconds > totalSeconds) {
              positionInSeconds = totalSeconds
            }
          }
        }
      }
      
      currentPosition.value = positionInSeconds
      
      if (currentSong.value && totalSeconds > 0) {
        const calculatedProgress = (positionInSeconds / totalSeconds) * 100
        progress.value = Math.min(calculatedProgress, 100)
      }
      
      syncLyrics()
    }
  } catch (error) {
    logError('更新进度失败:', error)
  }
}

// 播放完成标志，防止重复触发
let isPlaybackFinished = false

// 播放完成检测定时器ID
let playbackTimerId: number | null = null

const handlePlaybackFinished = async () => {
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
  
  // 立即设置播放状态为false,防止重复触发
  isPlaying.value = false
  logInfo('前端 播放状态已设置为false')

  logInfo('前端 播放完成,处理下一首', {
    isRepeating: isRepeating.value,
    playbackMode: playbackMode.value,
    autoPlayNext: autoPlayNext.value,
    crossfadeEnabled: crossfadeEnabled.value,
    crossfadeDuration: crossfadeDuration.value,
    currentSong: currentSong.value?.title
  })

  // 计算延迟时间：如果启用了交叉淡入淡出，延迟时间为淡出时间，否则为100ms
  const delay = (crossfadeEnabled.value && crossfadeDuration.value > 0) 
    ? crossfadeDuration.value * 1000 
    : 100

  logInfo('前端 播放完成延迟时间:', delay, 'ms')

  // 使用 setTimeout 确保状态更新后再处理下一首
  setTimeout(async () => {
    logInfo('前端 延迟后处理下一首')
    if (isRepeating.value) {
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
})

// 异步初始化
;(async () => {
  // 加载保存的歌曲
  const savedSongs = await localStorageService.getSongs()
  if (savedSongs.length > 0) {
    songs.value = savedSongs as Song[]
  }
  
  // 加载歌单和收藏
  favorites.value = await localStorageService.getFavorites()
  playlists.value = await localStorageService.getPlaylists()
  
  // 更新歌曲的收藏状态
  songs.value.forEach(song => {
    song.isFavorite = favorites.value.includes(song.id)
  })
  
  // 加载保存的播放进度
  const savedProgress = await localStorageService.getPlaybackProgress()
  if (savedProgress) {
    // 查找对应的歌曲
    const savedSong = songs.value.find(song => song.id === savedProgress.songId)
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
    }
  }

  // 自动播放上次的歌曲
  if (currentSong.value && songs.value.length > 0) {
    // 延迟一小段时间确保应用完全加载
    setTimeout(async () => {
      try {
        await invoke('play_song', { path: currentSong.value?.path })
        // 如果有保存的播放位置,跳转到该位置
        if (currentPosition.value > 0) {
          await invoke('seek_song', { position: currentPosition.value })
        }
        isPlaying.value = true
      } catch (error) {
        logError('自动播放失败:', error)
        // 如果自动播放失败,播放第一首歌曲
        if (songs.value.length > 0) {
          await playSong(songs.value[0])
        }
      }
    }, 500)
  } else if (songs.value.length > 0) {
    // 如果没有保存的歌曲,播放第一首
    setTimeout(async () => {
      await playSong(songs.value[0])
    }, 500)
  }

  // 加载保存的设置
  const savedSettings = await localStorageService.getSettings()
  volume.value = savedSettings.volume
  playbackMode.value = savedSettings.playbackMode
  currentPreset.value = savedSettings.equalizerPreset
  equalizerBands.value = savedSettings.equalizerBands
  theme.value = savedSettings.theme || 'dark'
  crossfadeEnabled.value = savedSettings.crossfadeEnabled ?? false
  crossfadeDuration.value = savedSettings.crossfadeDuration ?? 1
  autoPlayNext.value = savedSettings.autoPlayNext ?? true
  showLyrics.value = savedSettings.showLyrics ?? true
  enableTranscode.value = savedSettings.enableTranscode ?? true
  forceTranscode.value = savedSettings.forceTranscode ?? false

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
})()

// 进度更新定时器
let progressTimer: number | null = null

// 监听播放状态变化，动态控制定时器
watch(isPlaying, (playing) => {
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

// 监听设置变化，自动保存
watch([volume, playbackMode, currentPreset, equalizerBands, theme, crossfadeEnabled, crossfadeDuration, autoPlayNext, showLyrics, enableTranscode, forceTranscode], 
  async ([newVolume, newPlaybackMode, newPreset, newBands, newTheme, newCrossfadeEnabled, newCrossfadeDuration, newAutoPlayNext, newShowLyrics, newEnableTranscode, newForceTranscode]) => {
  try {
    // 确保所有数据都是可克隆的
    const serializableSettings = {
      theme: newTheme,
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
      enableTranscode: newEnableTranscode,
      forceTranscode: newForceTranscode
    }
    await localStorageService.saveSettings(serializableSettings)
  } catch (error) {
    logError('保存设置失败:', error)
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
})
</script>

<style scoped>
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
}

.tplayer-container.light {
  --text-primary: #333333;
  --text-secondary: #666666;
  --bg-secondary: #ffffff;
  --bg-hover: #e0e0e0;
  --border-color: rgba(0, 0, 0, 0.2);
  --btn-secondary-bg: #e0e0e0;
  --btn-secondary-hover: #d0d0d0;
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

.app-logo,
.window-controls {
  /* Tauri 窗口拖动排除区域属性 */
  -webkit-app-region: no-drag;
  app-region: no-drag;
}

.app-logo h1 {
  margin: 0;
  font-size: 18px;
  font-weight: bold;
  color: #4CAF50;
}

.window-controls {
  display: flex;
  gap: 10px;
}

.control-btn {
  width: 30px;
  height: 30px;
  border: none;
  border-radius: 4px;
  background-color: transparent;
  color: #ffffff;
  font-size: 14px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background-color 0.2s;
}

.control-btn:hover {
  background-color: rgba(255, 255, 255, 0.1);
}

.control-btn.close:hover {
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
  color: #ffffff;
  font-size: 16px;
  cursor: pointer;
  margin-right: 10px;
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
  background-color: rgba(76, 175, 80, 0.2);
  border-left: 3px solid #4CAF50;
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

.btn {
  padding: 10px 15px;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 14px;
  transition: background-color 0.2s;
}

.btn.primary {
  background-color: #4CAF50;
  color: #ffffff;
}

.btn.primary:hover {
  background-color: #45a049;
}

.btn.secondary {
  background-color: #3a3a3a;
  color: #ffffff;
}

.btn.secondary:hover {
  background-color: #4a4a4a;
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
  align-items: flex-start;
  margin-bottom: 15px;
}

.filter-title {
  display: flex;
  flex-direction: column;
  gap: 5px;
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
}

/* 歌曲列表 */
.song-list-container {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  min-height: 0; /* 确保flex子元素可以正确收缩 */
  max-width: 100%;
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
  overflow: hidden;
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
  background-color: rgba(76, 175, 80, 0.1);
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
  color: #ff4757;
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
  color: #ffffff;
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
  background: #4CAF50;
  border-radius: 50%;
  cursor: pointer;
}

.band input[type="range"]::-moz-range-thumb {
  width: 16px;
  height: 16px;
  background: #4CAF50;
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
  align-items: flex-start;
  gap: 20px;
  transition: height 0.3s ease;
  height: 200px;
}

.player-controls.expanded {
  height: 240px;
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
}

.song-cover img {
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
}

.playback-controls {
  display: flex;
  align-items: center;
  gap: 15px;
}

.playback-controls .control-btn {
  font-size: 18px;
}

.playback-controls .control-btn.play {
  width: 40px;
  height: 40px;
  font-size: 20px;
  background-color: #4CAF50;
  border-radius: 50%;
}

.playback-controls .control-btn.play:hover {
  background-color: #45a049;
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
  background: #4CAF50;
  border-radius: 50%;
  cursor: pointer;
}

.progress-bar input[type="range"]::-moz-range-thumb {
  width: 16px;
  height: 16px;
  background: #4CAF50;
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
  background: linear-gradient(135deg, #4CAF50, #45a049);
  color: white;
  border: none;
  border-radius: 4px;
  padding: 3px 8px;
  font-size: 10px;
  font-weight: 500;
  cursor: pointer;
  margin-top: 4px;
  transition: all 0.2s ease;
  box-shadow: 0 1px 3px rgba(76, 175, 80, 0.3);
}

.skip-next-btn:hover {
  background: linear-gradient(135deg, #45a049, #3d8b40);
  transform: translateY(-1px);
  box-shadow: 0 2px 6px rgba(76, 175, 80, 0.4);
}

.skip-next-btn:active {
  transform: translateY(0);
  box-shadow: 0 1px 2px rgba(76, 175, 80, 0.3);
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
}

.crossfade-duration input[type="range"]::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 12px;
  height: 12px;
  background: #4CAF50;
  border-radius: 50%;
  cursor: pointer;
}

.crossfade-duration input[type="range"]::-moz-range-thumb {
  width: 12px;
  height: 12px;
  background: #4CAF50;
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
  background: #4CAF50;
  border-radius: 50%;
  cursor: pointer;
}

.volume-control input[type="range"]::-moz-range-thumb {
  width: 12px;
  height: 12px;
  background: #4CAF50;
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
  padding: 6px 12px;
  background-color: #4a90e2;
  border: none;
  border-radius: 4px;
  color: #ffffff;
  cursor: pointer;
  transition: background-color 0.2s;
}

.match-btn:hover {
  background-color: #357abd;
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
  color: #cccccc;
  cursor: pointer;
  transition: all 0.2s;
  border-bottom: 2px solid transparent;
}

.tab-button:hover {
  color: #ffffff;
}

.tab-button.active {
  color: #4a90e2;
  border-bottom-color: #4a90e2;
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
  padding: 0 12px;
  background-color: #3a3a3a;
  border: none;
  border-radius: 4px;
  color: #ffffff;
  cursor: pointer;
  transition: background-color 0.2s;
  white-space: nowrap;
}

.copy-btn:hover {
  background-color: #4a4a4a;
}

/* 歌词操作 */
.lyric-actions {
  margin-top: 12px;
  display: flex;
  gap: 10px;
}

.action-btn {
  padding: 6px 12px;
  background-color: #3a3a3a;
  border: none;
  border-radius: 4px;
  color: #ffffff;
  cursor: pointer;
  transition: background-color 0.2s;
}

.action-btn:hover {
  background-color: #4a4a4a;
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
  gap: 6px;
  width: 100%;
  padding: 5px 0;
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
  color: #4CAF50;
  font-weight: bold;
  opacity: 1;
  transform: scale(1.08);
  text-shadow: 0 0 8px rgba(76, 175, 80, 0.5);
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
  color: #4CAF50;
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
  background-color: rgba(76, 175, 80, 0.15);
  border-left: 3px solid #4CAF50;
  font-weight: 500;
}

.tplayer-container.light .sidebar-footer {
  border-top: 1px solid #e0e0e0;
}

.tplayer-container.light .btn.primary {
  background-color: #4CAF50;
  color: #ffffff;
}

.tplayer-container.light .btn.primary:hover {
  background-color: #45a049;
}

.tplayer-container.light .btn.secondary {
  background-color: #e0e0e0;
  color: #333333;
}

.tplayer-container.light .btn.secondary:hover {
  background-color: #d0d0d0;
}

.tplayer-container.light .content-area {
  background-color: #f8f9fa;
}

.tplayer-container.light .search-box input {
  border: 1px solid #e0e0e0;
  background-color: #ffffff;
  color: #333333;
}

.tplayer-container.light .search-box input:focus {
  border-color: #4CAF50;
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
  color: #4CAF50;
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
  background-color: rgba(76, 175, 80, 0.12);
  border-left: 3px solid #4CAF50;
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
  background: #4CAF50;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  transition: all 0.2s ease;
}

.tplayer-container.light .progress-bar input[type="range"]::-webkit-slider-thumb:hover {
  transform: scale(1.1);
  box-shadow: 0 4px 8px rgba(76, 175, 80, 0.4);
}

.tplayer-container.light .progress-bar input[type="range"]::-moz-range-thumb {
  background: #4CAF50;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  border: none;
  transition: all 0.2s ease;
}

.tplayer-container.light .progress-bar input[type="range"]::-moz-range-thumb:hover {
  transform: scale(1.1);
  box-shadow: 0 4px 8px rgba(76, 175, 80, 0.4);
}

.tplayer-container.light .volume-control input[type="range"] {
  background: #e0e0e0;
  height: 4px;
  border-radius: 2px;
}

.tplayer-container.light .volume-control input[type="range"]::-webkit-slider-thumb {
  background: #4CAF50;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  transition: all 0.2s ease;
}

.tplayer-container.light .volume-control input[type="range"]::-webkit-slider-thumb:hover {
  transform: scale(1.1);
  box-shadow: 0 2px 6px rgba(76, 175, 80, 0.4);
}

.tplayer-container.light .volume-control input[type="range"]::-moz-range-thumb {
  background: #4CAF50;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  border: none;
  transition: all 0.2s ease;
}

.tplayer-container.light .volume-control input[type="range"]::-moz-range-thumb:hover {
  transform: scale(1.1);
  box-shadow: 0 2px 6px rgba(76, 175, 80, 0.4);
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
  border-color: #4CAF50;
  outline: none;
}

.tplayer-container.light .band input[type="range"] {
  background: #e0e0e0;
  height: 4px;
  border-radius: 2px;
}

.tplayer-container.light .band input[type="range"]::-webkit-slider-thumb {
  background: #4CAF50;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  transition: all 0.2s ease;
}

.tplayer-container.light .band input[type="range"]::-webkit-slider-thumb:hover {
  transform: scale(1.1);
  box-shadow: 0 2px 6px rgba(76, 175, 80, 0.4);
}

.tplayer-container.light .band input[type="range"]::-moz-range-thumb {
  background: #4CAF50;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  border: none;
  transition: all 0.2s ease;
}

.tplayer-container.light .band input[type="range"]::-moz-range-thumb:hover {
  transform: scale(1.1);
  box-shadow: 0 2px 6px rgba(76, 175, 80, 0.4);
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
  border-color: #4CAF50;
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
  background-color: #4CAF50;
  color: #ffffff;
  box-shadow: 0 2px 4px rgba(76, 175, 80, 0.3);
}

.tplayer-container.light .form-actions button.primary:hover {
  background-color: #45a049;
  transform: translateY(-1px);
  box-shadow: 0 4px 8px rgba(76, 175, 80, 0.4);
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
  border: 2px solid #4CAF50;
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
  border: 2px solid #4CAF50;
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

.tplayer-container.light .filter-header h2 {
  color: #333333;
  font-size: 18px;
  font-weight: 600;
  margin: 0 0 16px 0;
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

.tplayer-container.light .playback-controls .control-btn {
  color: #666666;
  font-size: 16px;
  margin: 0 8px;
}

.tplayer-container.light .playback-controls .control-btn:hover {
  color: #333333;
}

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

.tplayer-container.light .skip-next-btn {
  background: linear-gradient(135deg, #4CAF50, #45a049);
  color: white;
}

.tplayer-container.light .skip-next-btn:hover {
  background: linear-gradient(135deg, #45a049, #3d8b40);
}

.tplayer-container.light .modal-header .close-btn {
  color: #666666;
  font-size: 20px;
  cursor: pointer;
}

.tplayer-container.light .modal-header .close-btn:hover {
  color: #333333;
}

/* 浅色主题 - 标签页 */
.tplayer-container.light .tab-buttons {
  border-bottom: 1px solid #e0e0e0;
}

.tplayer-container.light .tab-button {
  color: #666666;
}

.tplayer-container.light .tab-button:hover {
  color: #333333;
}

.tplayer-container.light .tab-button.active {
  color: #4CAF50;
  border-bottom-color: #4CAF50;
}

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
  border-color: #4CAF50;
  outline: none;
}

/* 浅色主题 - 匹配区域 */
.tplayer-container.light .match-section {
  background-color: rgba(76, 175, 80, 0.1);
  border: 1px solid rgba(76, 175, 80, 0.3);
}

.tplayer-container.light .match-btn {
  background-color: #4CAF50;
}

.tplayer-container.light .match-btn:hover {
  background-color: #45a049;
}

/* 浅色主题 - 按钮 */
.tplayer-container.light .btn-cancel {
  background-color: #e0e0e0;
  color: #333333;
}

.tplayer-container.light .btn-cancel:hover {
  background-color: #d0d0d0;
}

.tplayer-container.light .btn-save {
  background-color: #4CAF50;
}

.tplayer-container.light .btn-save:hover {
  background-color: #45a049;
}

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
  color: #999999;
}

.tplayer-container.light .action-btn.favorite:hover {
  color: #666666;
}

.tplayer-container.light .action-btn.favorite.active {
  color: #ff4757;
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
  border-bottom: 2px solid rgba(76, 175, 80, 0.3);
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

.tplayer-container.light .cover-modal-btn {
  background-color: rgba(0, 0, 0, 0.1);
  color: #333;
}

.tplayer-container.light .cover-modal-btn:hover {
  background-color: rgba(0, 0, 0, 0.2);
}

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
  background-color: rgba(255, 255, 255, 0.1);
  color: #fff;
  font-size: 14px;
  border-radius: 4px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
}

.cover-modal-btn:hover {
  background-color: rgba(255, 255, 255, 0.2);
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

.btn-cancel {
  padding: 8px 16px;
  background-color: #3a3a3a;
  border: none;
  border-radius: 4px;
  color: #ffffff;
  cursor: pointer;
  transition: background-color 0.2s;
}

.btn-cancel:hover {
  background-color: #4a4a4a;
}

.btn-save {
  padding: 8px 16px;
  background-color: #4a90e2;
  border: none;
  border-radius: 4px;
  color: #ffffff;
  cursor: pointer;
  transition: background-color 0.2s;
}

.btn-save:hover {
  background-color: #357abd;
}

/* 滚动条样式 - 隐藏滚动条但保留滚动功能 */
.tplayer-container ::-webkit-scrollbar,
.tplayer-container ::-webkit-scrollbar-horizontal,
.tplayer-container ::-webkit-scrollbar-vertical {
  width: 0px;
  height: 0px;
  display: none;
}

/* Firefox */
.tplayer-container {
  scrollbar-width: none;
}

/* IE 和 Edge */
.tplayer-container {
  -ms-overflow-style: none;
}

/* 确保body和html不显示滚动条 */
body, html {
  overflow: hidden !important;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .sidebar {
    width: 200px;
  }
  
  .sidebar.collapsed {
    width: 50px;
  }
  
  .content-area {
    padding: 10px;
  }
  
  .player-controls {
    padding: 10px;
  }
  
  .player-left {
    flex: 1;
  }
  
  .player-center {
    flex: 1;
  }
  
  .player-right {
    flex: 1;
  }
  
  .volume-control {
    width: 100px;
  }
}
</style>