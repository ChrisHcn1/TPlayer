# FFplay 播放优化实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 优化 FFplay 播放完成检测逻辑，修复随机播放和进度同步问题

**架构：** 采用方案 A - 简化播放完成检测，移除对 `is_playing` 状态的依赖，仅通过位置检测播放完成。同时修复随机播放逻辑和 seek 状态管理。

**技术栈：** Vue 3, TypeScript, Tauri, Rust

---

## 文件结构

**修改的文件：**
- `src/App.vue` - 前端播放逻辑、状态管理
- `src-tauri/src/ffmpeg_transcoder.rs` - 后端 FFplay 状态管理（已修改，需验证）

**测试方式：**
- 手动测试：播放 FFplay 支持的格式（DTS、DSD 等），验证播放完成后自动播放下一首
- 手动测试：随机播放模式，验证随机性
- 手动测试：拖动进度条，验证前后端同步

---

## 任务 1：优化播放完成检测逻辑

**文件：**
- 修改：`src/App.vue:2167-2176`

**需求：**
当前播放完成检测依赖 `status.is_playing === false`，但后端在进程结束后会保持状态 2.5 秒。需要移除对 `is_playing` 的依赖，仅检测位置。

- [ ] **步骤 1：定位播放完成检测代码**

在 `src/App.vue` 中找到以下代码（约 2167-2176 行）：

```typescript
// 检查播放是否完成
// 只有当满足以下所有条件时才认为播放完成：
// 1. is_playing 为 false (FFplay 已停止)
// 2. duration > 0 (确保已获取到有效时长)
// 3. position >= duration - 1 (播放到接近结尾，允许 1 秒误差)
// 4. ffplayDuration.value > 0 (确保前端也获取到了时长)
// 注意：不再检查 isFFplayPlaying.value，因为定时器启动时已经检查过
const isPlaybackComplete =
  status.is_playing === false &&
  status.duration > 0 &&
  status.position >= status.duration - 1 &&
  ffplayDuration.value > 0
```

- [ ] **步骤 2：修改检测条件**

替换为：

```typescript
// 检查播放是否完成（方案 A：仅检测位置）
// 满足以下条件时认为播放完成：
// 1. duration > 0 (确保已获取到有效时长)
// 2. position >= duration - 1 (播放到接近结尾，允许 1 秒误差)
// 3. ffplayDuration.value > 0 (确保前端也获取到了时长)
// 4. !isSeeking.value (确保不在 seek 状态，避免误判)
const isPlaybackComplete =
  status.duration > 0 &&
  status.position >= status.duration - 1 &&
  ffplayDuration.value > 0 &&
  !isSeeking.value
```

**修改说明：**
- 移除 `status.is_playing === false` 条件
- 添加 `!isSeeking.value` 条件，防止 seek 期间误判

- [ ] **步骤 3：验证修改**

运行应用，播放 FFplay 支持的格式，等待播放完成，观察日志：
- 预期：看到 "播放完成检测" 日志，`isPlaybackComplete=true`
- 预期：看到 "FFplay 播放完成" 日志
- 预期：自动播放下一首（如果启用了 autoPlayNext）

---

## 任务 2：修复随机播放逻辑

**文件：**
- 修改：`src/App.vue:3376-3385`

**需求：**
当前随机播放逻辑已经使用了 `randomNextIndex.value`，但需要确保在播放完成后正确更新这个值。

- [ ] **步骤 1：检查随机播放逻辑**

查看 `playNext()` 函数中的随机播放部分（约 3376-3385 行）：

```typescript
if (playbackMode.value === 'random') {
  // 随机播放 - 使用预先确定的下一首
  if (randomNextIndex.value !== null && randomNextIndex.value >= 0 && randomNextIndex.value < songs.value.length) {
    targetIndex = randomNextIndex.value
    logInfo('随机播放模式，使用预先确定的索引:', targetIndex)
  } else {
    // 如果没有预先确定，随机选择一首
    targetIndex = Math.floor(Math.random() * songs.value.length)
    logInfo('随机播放模式，实时选择索引:', targetIndex)
  }
}
```

**当前逻辑已正确**，但需要确保在 `playSong()` 中预先确定下一首。

- [ ] **步骤 2：验证预先确定逻辑**

检查 `playSong()` 函数末尾（约 2237-2243 行）：

```typescript
// 预先确定下一首歌曲（用于随机播放模式）
if (playbackMode.value === 'random' && songs.value.length > 1) {
  let nextIndex
  do {
    nextIndex = Math.floor(Math.random() * songs.value.length)
  } while (nextIndex === currentIndex && songs.value.length > 1)
  randomNextIndex.value = nextIndex
```

**确认此代码存在且正确执行。**

- [ ] **步骤 3：测试随机播放**

1. 切换到随机播放模式
2. 播放多首歌曲
3. 观察日志，确认每次播放都使用了不同的 `randomNextIndex.value`
4. 确认不会重复播放同一首（除非随机到）

---

## 任务 3：修复 Seek 状态管理

**文件：**
- 修改：`src/App.vue:3569-3700`

**需求：**
确保 `isSeeking` 状态在 seek 开始、完成、错误时都被正确重置。

- [ ] **步骤 1：检查 isSeeking 设置位置**

在 `seek()` 函数中，查找 `isSeeking.value = true` 的设置位置。

**当前问题：** 可能缺少 `isSeeking.value = false` 的重置。

- [ ] **步骤 2：添加 finally 块**

在 `seek()` 函数的 try-catch 结构中添加 finally 块：

```typescript
const seek = async () => {
  console.log('【SEEK】========== seek 函数开始 ==========')
  // ... 现有代码 ...
  
  try {
    // FFplay seek 逻辑
    if (shouldUseFFplay && currentSong.value) {
      // ... seek 逻辑 ...
    } else {
      // 原生 seek 逻辑
    }
  } catch (error) {
    console.error('【SEEK】seek 失败:', error)
    logError('seek 失败:', error)
  } finally {
    // 确保重置 isSeeking 状态
    console.log('【SEEK】finally 块：重置 isSeeking')
    logInfo('finally 块：重置 isSeeking')
    isSeeking.value = false
  }
}
```

- [ ] **步骤 3：验证超时重置**

确认已有的 500ms 超时重置逻辑（约 3700 行后）：

```typescript
// 500ms 后自动重置 isSeeking，防止卡住
setTimeout(() => {
  if (isSeeking.value) {
    console.log('【SEEK】超时重置 isSeeking 状态')
    logInfo('超时重置 isSeeking 状态')
    isSeeking.value = false
  }
}, 500)
```

**确认此代码存在。**

- [ ] **步骤 4：测试 seek 功能**

1. 播放 FFplay 支持的格式
2. 拖动进度条到不同位置
3. 观察日志，确认：
   - seek 开始时 `isSeeking=true`
   - seek 完成后 `isSeeking=false`
   - 进度条立即更新到新位置
   - 播放从新位置继续

---

## 任务 4：验证后端状态保持

**文件：**
- 验证：`src-tauri/src/ffmpeg_transcoder.rs:1285-1315`

**需求：**
确认后端在 FFplay 进程结束后保持状态 2.5 秒的逻辑正确。

- [ ] **步骤 1：检查后端代码**

查看 FFplay 监控线程（约 1285-1315 行）：

```rust
Ok(Some(exit_status)) => {
    // 进程已结束，更新状态
    println!("[FFplay] 进程已结束，退出状态：{:?}", exit_status);
    drop(process);
    
    // 设置播放完成状态
    let mut status = FFPLAY_STATUS.lock().unwrap();
    status.is_playing = false;
    status.position = status.duration; // 设置位置为时长，触发前端播放完成检测
    let duration = status.duration; // 保存时长值
    println!("[FFplay] 进程结束，设置播放完成状态：position={:.2}, duration={:.2}", status.position, duration);
    drop(status);
    
    // 停止 FFplay（清理资源）
    let _ = stop_ffplay();
    
    // 不要立即退出，继续循环几次让前端获取到最终状态
    // 设置一个计数器，继续循环 5 次（2.5 秒）后退出
    println!("[FFplay] 等待前端获取最终状态...");
    for _ in 0..5 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        println!("[FFplay] 保持最终状态：position={:.2}, duration={:.2}", duration, duration);
    }
    
    // 现在退出监控线程
    println!("[FFplay] 退出状态监控线程");
    return;
}
```

**确认此代码存在且正确。**

- [ ] **步骤 2：测试后端状态保持**

1. 播放 FFplay 支持的格式
2. 等待播放完成
3. 观察后端日志：
   - 预期：看到 "进程已结束，退出状态：..."
   - 预期：看到 "设置播放完成状态：position=xxx, duration=xxx"
   - 预期：看到 5 次 "保持最终状态" 日志
   - 预期：看到 "退出状态监控线程"

---

## 任务 5：清理冗余代码

**文件：**
- 修改：`src/App.vue` 多处

**需求：**
删除已注释的代码和冗余日志。

- [ ] **步骤 1：查找并删除注释代码**

搜索以下模式并删除：
- `// currentPosition.value = status.position // seek 后跳过更新`
- 其他已注释的无用代码

- [ ] **步骤 2：合并冗余日志**

在 FFplay 定时器中（约 2103-2105 行），合并重复的日志：

```typescript
ffplayStatusInterval = window.setInterval(() => {
  console.log('【FFplay 定时器】定时器触发，isFFplayPlaying=', isFFplayPlaying.value)
  logInfo('FFplay 定时器触发')
  // 删除重复的 logInfo('FFplay 状态监控定时器触发')
  // ...
})
```

---

## 任务 6：完整测试

**测试场景：**

- [ ] **场景 1：FFplay 播放完成自动播放下一首**
  1. 播放 DTS 格式文件
  2. 等待播放完成
  3. 预期：自动播放下一首
  4. 预期：前端日志显示播放完成检测成功
  5. 预期：后端日志显示状态保持 2.5 秒

- [ ] **场景 2：随机播放**
  1. 切换到随机播放模式
  2. 播放 5 首以上歌曲
  3. 预期：每首歌曲的索引不同
  4. 预期：日志显示使用 `randomNextIndex.value`

- [ ] **场景 3：拖动进度条**
  1. 播放 FFplay 支持的格式
  2. 拖动进度条到 50% 位置
  3. 预期：进度条立即跳转到 50%
  4. 预期：播放从 50% 位置继续
  5. 预期：前后端进度一致

- [ ] **场景 4：非 FFplay 格式播放**
  1. 播放 MP3 格式
  2. 等待播放完成
  3. 预期：自动播放下一首
  4. 预期：没有 FFplay 相关日志

---

## 验收标准

1. ✅ FFplay 播放完成后，前端能检测到并自动播放下一首
2. ✅ 随机播放模式正常工作，不会重复播放同一首
3. ✅ 拖动进度条后，前后端进度同步
4. ✅ 没有冗余的 FFplay 日志输出（非 FFplay 播放时）
5. ✅ 代码整洁，无注释的无用代码

---

## 执行方式

**选择执行方式：**

1. **子代理驱动（推荐）** - 每个任务调度一个新的子代理，任务间进行审查，快速迭代
2. **内联执行** - 在当前会话中使用 executing-plans 执行任务，批量执行并设有检查点

**选哪种方式？**
