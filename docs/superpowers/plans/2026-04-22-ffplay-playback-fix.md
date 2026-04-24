# FFplay 播放问题修复实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 修复 FFplay 播放时前端进度不更新、invoke 调用不返回、应用退出时 FFplay 不停止等问题

**架构：** 
- 后端：将 `play_with_ffplay` 改为同步函数，在播放开始时重置状态
- 前端：添加时间保护机制（1 秒内不更新 position），添加多重退出事件监听，在播放前重置进度

**技术栈：** Rust (Tauri 后端), Vue 3 + TypeScript (前端)

---

## 文件结构

**修改的文件：**
1. `src-tauri/src/ffmpeg_transcoder.rs` - 后端 FFplay 播放逻辑
2. `src/App.vue` - 前端播放控制和进度更新逻辑

**测试文件：**
- 无需创建新测试文件，使用现有手动测试流程

---

## 任务 1：后端重置 FFPLAY_STATUS

**文件：**
- 修改：`src-tauri/src/ffmpeg_transcoder.rs:985-1080`

- [ ] **步骤 1：在 play_with_ffplay 开始时重置状态**

在 `play_with_ffplay` 函数的第 986 行（函数开始处，stop_ffplay 调用之后）添加状态重置代码：

```rust
// 重置 FFplay 状态，避免旧状态影响新曲目
{
    let mut status = FFPLAY_STATUS.lock().unwrap();
    status.is_playing = false;
    status.position = 0.0;
    status.duration = 0.0;
    status.volume = 100.0;
    drop(status);
    println!("[FFplay] 已重置 FFPLAY_STATUS");
}
```

- [ ] **步骤 2：验证代码编译**

运行：
```bash
cd e:\TPlayer
npm run tauri dev
```

预期：编译成功，无错误

- [ ] **步骤 3：Commit**

```bash
cd e:\TPlayer
git add src-tauri/src/ffmpeg_transcoder.rs
git commit -m "fix: 在 play_with_ffplay 开始时重置 FFPLAY_STATUS"
```

---

## 任务 2：前端重置 currentPosition

**文件：**
- 修改：`src/App.vue:1930-1945`（调用 invoke 之前）

- [ ] **步骤 1：在调用 invoke 前重置 currentPosition**

找到以下代码（约第 1935 行）：
```typescript
console.log('【FFplay】准备调用 invoke play_with_ffplay')
logInfo('准备调用 invoke play_with_ffplay')
```

在其**之前**添加：
```typescript
// 重置播放进度，避免显示旧曲目的位置
currentPosition.value = 0
frontendPosition = 0
console.log('【FFplay】重置 currentPosition 为 0')
logInfo('重置 currentPosition 为 0')
```

- [ ] **步骤 2：验证应用启动**

运行：
```bash
cd e:\TPlayer
npm run tauri dev
```

预期：应用正常启动，播放 FFplay 格式歌曲时前端日志显示"重置 currentPosition 为 0"

- [ ] **步骤 3：Commit**

```bash
cd e:\TPlayer
git add src/App.vue
git commit -m "fix: 在调用 play_with_ffplay 前重置 currentPosition"
```

---

## 任务 3：前端添加时间保护机制

**文件：**
- 修改：`src/App.vue:2130-2200`（get_ffplay_status 调用后的 status 处理逻辑）

- [ ] **步骤 1：找到 status 更新逻辑**

定位到以下代码（约第 2150 行）：
```typescript
if (status.is_playing) {
  // 更新播放进度
  if (status.position !== undefined && status.position !== null) {
    currentPosition.value = status.position
    frontendPosition = status.position
  }
}
```

- [ ] **步骤 2：添加时间保护机制**

替换为：
```typescript
if (status.is_playing) {
  // 只有当 status.position 有效时才更新 currentPosition
  // FFplay 启动后的 1 秒内，不更新 currentPosition，保持 start_position
  // 避免后端旧状态覆盖前端已设置的初始位置
  const timeSinceFFplayStart = Date.now() - ffplayStartTime
  if (status.position !== undefined && status.position !== null && timeSinceFFplayStart > 1000) {
    console.log('【FFplay】立即更新 currentPosition 前:', currentPosition.value, '更新后:', status.position)
    logInfo('立即更新 currentPosition 前:', currentPosition.value, '更新后:', status.position)
    if (!isSeeking.value) {
      currentPosition.value = status.position
      frontendPosition = status.position
    } else {
      console.log('【FFplay】seek 期间，跳过 currentPosition 更新')
      logInfo('seek 期间，跳过 currentPosition 更新')
    }
    console.log('【FFplay】立即更新播放进度:', currentPosition.value, '秒，时长:', ffplayDuration.value)
    logInfo('立即更新播放进度:', currentPosition.value, '秒，时长:', ffplayDuration.value)
  } else if (timeSinceFFplayStart <= 1000) {
    console.log('【FFplay】FFplay 刚启动 (<1s)，保持 startPosition:', start_position, 'currentPosition:', currentPosition.value)
    logInfo('FFplay 刚启动 (<1s)，保持 startPosition:', start_position, 'currentPosition:', currentPosition.value)
  }
}
```

- [ ] **步骤 3：验证时间保护机制**

播放 FFplay 格式歌曲，观察前端日志：
- 第 1 秒内应显示"FFplay 刚启动 (<1s)，保持 startPosition"
- 1 秒后应显示"立即更新 currentPosition 前/后"

- [ ] **步骤 4：Commit**

```bash
cd e:\TPlayer
git add src/App.vue
git commit -m "feat: 添加 FFplay 时间保护机制，启动后 1 秒内不更新 position"
```

---

## 任务 4：前端添加退出事件监听

**文件：**
- 修改：`src/App.vue`（应用初始化部分，约第 800-1000 行）

- [ ] **步骤 1：找到初始化代码位置**

定位到 `onMounted` 或 `initializeApp` 函数（约第 900 行）

- [ ] **步骤 2：添加 beforeunload 事件监听**

在初始化函数的末尾添加：
```typescript
// 添加页面关闭监听器，确保前端退出时停止 FFplay 播放
window.addEventListener('beforeunload', async () => {
  console.log('【页面关闭】正在关闭页面，停止 FFplay 播放')
  try {
    await window.__TAURI__.invoke('stop_ffplay')
    console.log('【页面关闭】FFplay 已停止')
  } catch (error) {
    console.error('【页面关闭】停止 FFplay 失败:', error)
  }
})
```

- [ ] **步骤 3：添加 tauri://close-requested 事件监听**

在同一个初始化函数中添加：
```typescript
// 监听 Tauri 应用关闭事件
if (window.__TAURI__ && window.__TAURI__.event) {
  window.__TAURI__.event.listen('tauri://close-requested', async () => {
    console.log('【Tauri 关闭】收到关闭请求，停止 FFplay 播放')
    try {
      await window.__TAURI__.invoke('stop_ffplay')
      console.log('【Tauri 关闭】FFplay 已停止')
    } catch (error) {
      console.error('【Tauri 关闭】停止 FFplay 失败:', error)
    }
    // 允许应用关闭
    window.__TAURI__.window.getCurrent().close()
  })
}
```

- [ ] **步骤 4：验证退出事件监听**

1. 播放 FFplay 格式歌曲
2. 关闭应用窗口
3. 观察后端日志应显示"【页面关闭】正在关闭页面，停止 FFplay 播放"
4. 检查 FFplay 进程是否已停止（任务管理器中无 ffplay.exe）

- [ ] **步骤 5：Commit**

```bash
cd e:\TPlayer
git add src/App.vue
git commit -m "feat: 添加页面关闭和 Tauri 关闭事件监听，确保退出时停止 FFplay"
```

---

## 任务 5：验证所有修复

- [ ] **步骤 1：测试 invoke 返回**

播放 FFplay 格式歌曲，前端日志应显示：
- "【FFplay】invoke 调用成功，返回结果:"
- "【FFplay】继续执行后续代码"
- "【FFplay】启动 FFplay 状态监控定时器"

- [ ] **步骤 2：测试进度更新**

播放 FFplay 格式歌曲，观察：
- 进度条从 0:00 开始
- 进度条实时更新（每 0.5 秒更新一次）
- 前端日志显示"立即更新播放进度"

- [ ] **步骤 3：测试切换曲目**

播放一首歌曲后切换到另一首 FFplay 格式歌曲：
- 新曲目开始时进度条应重置为 0:00
- 不应显示旧曲目的结束位置

- [ ] **步骤 4：测试应用退出**

播放 FFplay 格式歌曲时关闭应用：
- FFplay 进程应被停止
- 任务管理器中无残留的 ffplay.exe 进程

- [ ] **步骤 5：运行类型检查**

```bash
cd e:\TPlayer
npx vue-tsc --noEmit
```

预期：无类型错误

- [ ] **步骤 6：运行构建**

```bash
cd e:\TPlayer
npm run build
```

预期：构建成功

---

## 测试检查清单

完成所有任务后，执行以下手动测试：

- [ ] 播放 DSD 格式歌曲（.dff/.dsf），进度条正常显示
- [ ] 播放 DTS 格式歌曲（.dts），进度条正常显示
- [ ] 播放普通 MP3 歌曲，进度条正常显示
- [ ] 切换不同格式的曲目，进度条正确重置
- [ ] 拖动进度条跳转，进度正常更新
- [ ] 暂停/恢复播放，进度正常更新
- [ ] 关闭应用，FFplay 进程被停止
- [ ] 播放完成后自动播放下一首

---

## 回滚方案

如果修复后出现问题，可以通过以下命令回滚：

```bash
cd e:\TPlayer
git log --oneline  # 找到修复相关的 commit
git revert <commit-hash>  # 逐个回滚 commit
```

---

## 注意事项

1. **时间保护机制的 1 秒阈值**：如果测试发现 1 秒不够，可以调整为 1.5 秒或 2 秒
2. **日志输出**：修复过程中添加了详细日志，便于调试。修复完成后可根据需要移除部分日志
3. **性能影响**：时间保护机制和退出事件监听对性能影响极小，可忽略不计
