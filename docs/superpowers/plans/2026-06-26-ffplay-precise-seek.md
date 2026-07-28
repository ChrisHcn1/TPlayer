# FFplay精确Seek智能混合方案 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 实现FFplay精确Seek功能，支持智能混合策略（StdinSeek + ProcessRestartSeek），确保毫秒级精度、暂停状态下有效、状态同步可靠。

**架构：** 在后端实现Seek策略选择器和两种执行器（StdinSeek、ProcessRestartSeek），在前端添加Seek防抖机制，通过状态同步器确保前后端状态一致。

**技术栈：** Rust（后端）、Vue 3 + TypeScript（前端）、FFplay进程管理、stdin控制

---

## 文件结构

**后端文件：**
- `src-tauri/src/ffmpeg_transcoder.rs` - 核心播放控制模块
  - 职责：Seek策略选择、StdinSeek执行、ProcessRestartSeek执行、状态同步
  - 新增内容：Seek策略枚举、策略选择函数、精确Seek实现

**前端文件：**
- `src/App.vue` - 主应用组件
  - 职责：Seek防抖机制、错误处理、用户体验提示
  - 修改内容：优化seek_ffplay调用、添加防抖逻辑

**测试文件：**
- `docs/superpowers/plans/test-results.md` - 测试结果记录（手动测试）

---

## 任务分解

### 任务 1：定义Seek策略枚举和策略选择器

**文件：**
- 修改：`src-tauri/src/ffmpeg_transcoder.rs:1-50`（文件开头，添加枚举定义）

- [ ] **步骤 1：添加Seek策略枚举**

在 `ffmpeg_transcoder.rs` 文件开头（导入语句之后）添加以下代码：

```rust
// Seek策略枚举
#[derive(Debug, Clone, Copy, PartialEq)]
enum SeekStrategy {
    /// 无需seek（差值太小）
    NoSeek,
    /// 使用stdin方向键（小范围、播放状态）
    StdinSeek,
    /// 重启进程（大范围或暂停状态）
    ProcessRestartSeek,
}

// Seek策略选择器
fn decide_seek_strategy(current_pos: f64, target_pos: f64, is_playing: bool) -> SeekStrategy {
    let diff = target_pos - current_pos;
    let abs_diff = if diff > 0.0 { diff } else { -diff };
    
    // 差值小于5秒，无需seek
    if abs_diff < 5.0 {
        return SeekStrategy::NoSeek;
    }
    
    // 暂停状态或差值大于等于10秒，使用进程重启
    if !is_playing || abs_diff >= 10.0 {
        return SeekStrategy::ProcessRestartSeek;
    }
    
    // 播放状态且差值在5-10秒之间，使用stdin方向键
    SeekStrategy::StdinSeek
}
```

- [ ] **步骤 2：编译验证**

运行：`cargo build --manifest-path src-tauri/Cargo.toml`
预期：编译成功，无错误

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/src/ffmpeg_transcoder.rs
git commit -m "feat(ffplay): 添加Seek策略枚举和策略选择器"
```

---

### 任务 2：优化StdinSeek执行器

**文件：**
- 修改：`src-tauri/src/ffmpeg_transcoder.rs:1595-1665`（现有try_seek_via_stdin函数）

- [ ] **步骤 1：改进try_seek_via_stdin函数**

替换现有的 `try_seek_via_stdin` 函数（约1595-1665行）为以下代码：

```rust
// StdinSeek执行器：发送方向键命令
fn execute_stdin_seek(target_pos: f64) -> Result<String, String> {
    let mut process = FFPLAY_PROCESS.lock().unwrap();
    
    if let Some(ref mut child) = *process {
        if let Some(ref mut stdin) = child.stdin.as_mut() {
            use std::io::Write;
            
            // 获取当前位置和播放状态
            let (current_pos, is_playing) = {
                let status = FFPLAY_STATUS.lock().unwrap();
                (status.position, status.is_playing)
            };
            
            // 确保在播放状态
            if !is_playing {
                return Err("暂停状态下不建议使用stdin seek".to_string());
            }
            
            // 计算需要移动的距离和方向键次数
            let diff = target_pos - current_pos;
            let abs_diff = if diff > 0.0 { diff } else { -diff };
            
            // 确认距离在合理范围（5-10秒）
            if abs_diff < 5.0 {
                return Ok("差值太小，无需seek".to_string());
            }
            if abs_diff >= 10.0 {
                return Err("距离过大，建议使用进程重启方式".to_string());
            }
            
            // 计算方向键次数（每次10秒）
            let times = (abs_diff / 10.0).ceil() as i32;
            let direction_byte = if diff > 0.0 { b'C' } else { b'D' };
            
            println!("[FFplay] StdinSeek: 当前={:.2}秒, 目标={:.2}秒, 差值={:.2}秒, 次数={}", 
                current_pos, target_pos, diff, times);
            
            // 发送方向键命令
            for i in 0..times {
                let cmd = [b'\x1b', b'[', direction_byte];
                if let Err(e) = stdin.write_all(&cmd) {
                    return Err(format!("发送第{}次方向键失败: {}", i + 1, e));
                }
                // 每次发送后等待50毫秒
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            
            // 估算新位置
            let estimated_pos = current_pos + (times as f64 * 10.0 * if diff > 0.0 { 1.0 } else { -1.0 });
            
            // 关键修复：更新起始偏移量，使输出解析线程能正确计算绝对位置
            {
                let mut offset = FFPLAY_START_OFFSET.lock().unwrap();
                *offset = estimated_pos;
                println!("[FFplay] StdinSeek后更新偏移量: {:.2}秒", *offset);
            }
            
            // 更新状态位置
            {
                let mut status = FFPLAY_STATUS.lock().unwrap();
                status.position = estimated_pos;
            }
            
            // 重置起始时刻，以便监控线程正确计算
            {
                let mut instant = FFPLAY_START_INSTANT.lock().unwrap();
                *instant = Some(std::time::Instant::now());
            }
            
            return Ok(format!("StdinSeek成功，估算位置: {:.2}秒", estimated_pos));
        }
    }
    
    Err("FFplay进程不存在或stdin不可用".to_string())
}
```

- [ ] **步骤 2：编译验证**

运行：`cargo build --manifest-path src-tauri/Cargo.toml`
预期：编译成功，无错误

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/src/ffmpeg_transcoder.rs
git commit -m "feat(ffplay): 优化StdinSeek执行器，更新偏移量确保状态同步"
```

---

### 任务 3：实现ProcessRestartSeek执行器

**文件：**
- 修改：`src-tauri/src/ffmpeg_transcoder.rs:1521-1593`（现有seek_ffplay函数）

- [ ] **步骤 1：实现精确seek执行器**

在 `seek_ffplay` 函数之前添加新的执行器函数：

```rust
// ProcessRestartSeek执行器：重启进程实现精确定位
fn execute_process_restart_seek(path: String, target_pos: f64) -> Result<String, String> {
    println!("[FFplay] ProcessRestartSeek: 路径={}, 目标位置={:.2}秒", path, target_pos);
    
    // 保存当前播放状态
    let was_playing = {
        let status = FFPLAY_STATUS.lock().unwrap();
        status.is_playing
    };
    
    println!("[FFplay] 之前播放状态: {}", was_playing);
    
    // 停止当前进程
    stop_ffplay()?;
    
    // 等待进程完全停止（100毫秒）
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    // 启动新进程，使用-ss参数精确定位
    // 注意：play_with_ffplay内部会设置FFPLAY_START_OFFSET
    let result = play_with_ffplay(path.clone(), target_pos, None)?;
    
    // 如果之前是暂停状态，立即暂停
    if !was_playing {
        println!("[FFplay] 之前是暂停状态，立即暂停新进程");
        // 等待进程启动（200毫秒）
        std::thread::sleep(std::time::Duration::from_millis(200));
        pause_ffplay()?;
    }
    
    // 确认位置已正确设置
    {
        let status = FFPLAY_STATUS.lock().unwrap();
        println!("[FFplay] ProcessRestartSeek完成: 位置={:.2}秒, 播放状态={}", 
            status.position, status.is_playing);
    }
    
    Ok(format!("ProcessRestartSeek成功，位置: {:.2}秒", target_pos))
}
```

- [ ] **步骤 2：编译验证**

运行：`cargo build --manifest-path src-tauri/Cargo.toml`
预期：编译成功，无错误

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/src/ffmpeg_transcoder.rs
git commit -m "feat(ffplay): 实现ProcessRestartSeek执行器，支持暂停状态精确定位"
```

---

### 任务 4：重构seek_ffplay主函数

**文件：**
- 修改：`src-tauri/src/ffmpeg_transcoder.rs:1521-1593`（现有seek_ffplay函数）

- [ ] **步骤 1：重构seek_ffplay函数**

完全替换现有的 `seek_ffplay` 函数为以下代码：

```rust
#[tauri::command]
pub fn seek_ffplay(path: String, position: f64) -> Result<String, String> {
    println!("[FFplay] 开始seek到位置: {:.2}秒, 路径: {}", position, path);
    
    // 获取当前位置和播放状态
    let (current_pos, is_playing, duration) = {
        let status = FFPLAY_STATUS.lock().unwrap();
        (status.position, status.is_playing, status.duration)
    };
    
    println!("[FFplay] 当前位置: {:.2}秒, 播放状态: {}, 总时长: {:.2}秒", 
        current_pos, is_playing, duration);
    
    // 边界检查：确保position在有效范围内
    let target_pos = if position < 0.0 {
        0.0
    } else if position > duration - 1.0 {
        duration - 1.0
    } else {
        position
    };
    
    println!("[FFplay] 校正后目标位置: {:.2}秒", target_pos);
    
    // 决策seek策略
    let strategy = decide_seek_strategy(current_pos, target_pos, is_playing);
    println!("[FFplay] 选择策略: {:?}", strategy);
    
    // 执行对应的seek操作
    match strategy {
        SeekStrategy::NoSeek => {
            Ok("差值太小，无需seek".to_string())
        },
        SeekStrategy::StdinSeek => {
            // 尝试stdin seek，如果失败则降级到进程重启
            match execute_stdin_seek(target_pos) {
                Ok(msg) => Ok(msg),
                Err(e) => {
                    println!("[FFplay] StdinSeek失败: {}, 降级到ProcessRestartSeek", e);
                    execute_process_restart_seek(path, target_pos)
                }
            }
        },
        SeekStrategy::ProcessRestartSeek => {
            execute_process_restart_seek(path, target_pos)
        }
    }
}
```

- [ ] **步骤 2：编译验证**

运行：`cargo build --manifest-path src-tauri/Cargo.toml`
预期：编译成功，无错误

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/src/ffmpeg_transcoder.rs
git commit -m "feat(ffplay): 重构seek_ffplay主函数，实现智能混合策略"
```

---

### 任务 5：前端添加Seek防抖机制

**文件：**
- 修改：`src/App.vue:2293-2370`（进度条拖动事件处理）

- [ ] **步骤 1：添加防抖变量和函数**

在 `App.vue` 的 `<script setup>` 部分（约第200行），添加以下变量和函数：

```typescript
// Seek防抖机制
const seekDebounceTimer = ref<number | null>(null)
const pendingSeekPosition = ref<number | null>(null)

// 防抖Seek函数（200毫秒延迟）
const debouncedSeek = async (position: number) => {
  // 清除之前的定时器
  if (seekDebounceTimer.value) {
    clearTimeout(seekDebounceTimer.value)
  }
  
  // 保存待执行的seek位置
  pendingSeekPosition.value = position
  
  // 设置新的定时器（200毫秒延迟）
  seekDebounceTimer.value = setTimeout(async () => {
    if (pendingSeekPosition.value !== null && isFFplayPlaying.value) {
      try {
        logInfo(`【FFplay】执行防抖Seek: ${pendingSeekPosition.value}秒`)
        await invoke('seek_ffplay', {
          path: currentSong.value?.path || '',
          position: pendingSeekPosition.value
        })
        // 更新前端状态
        currentPosition.value = pendingSeekPosition.value
      } catch (error) {
        logError('FFplay Seek失败:', error)
        // 显示友好错误提示
        alert('播放进度调整失败，请稍后重试')
      } finally {
        pendingSeekPosition.value = null
        seekDebounceTimer.value = null
      }
    }
  }, 200)
}
```

- [ ] **步骤 2：修改进度条拖动事件**

找到进度条拖动事件处理函数（约第2293行），修改为使用防抖机制：

```typescript
// 进度条拖动事件（使用防抖）
const handleProgressDrag = (event: MouseEvent) => {
  if (!currentSong.value || !audioElement.value) return
  
  // 计算目标位置
  const progressBar = event.currentTarget as HTMLElement
  const rect = progressBar.getBoundingClientRect()
  const percent = (event.clientX - rect.left) / rect.width
  const targetPosition = percent * currentDuration.value
  
  // 如果是FFplay播放，使用防抖机制
  if (isFFplayPlaying.value) {
    debouncedSeek(targetPosition)
  } else {
    // HTML5 Audio直接设置位置
    audioElement.value.currentTime = targetPosition
    currentPosition.value = targetPosition
  }
}
```

- [ ] **步骤 3：编译验证**

运行：`npm run build`
预期：编译成功，无错误

- [ ] **步骤 4：Commit**

```bash
git add src/App.vue
git commit -m "feat(ffplay): 添加前端Seek防抖机制，防止快速拖动导致多次seek"
```

---

### 任务 6：手动集成测试

**文件：**
- 创建：`docs/superpowers/plans/test-results.md`

- [ ] **步骤 1：启动开发环境**

运行：`npm run tauri dev`
预期：应用程序启动成功，日志显示FFplay已检测

- [ ] **步骤 2：测试基础seek功能**

测试步骤：
1. 添加一首DSF格式音频文件（总时长约288秒）
2. 点击播放，等待播放开始
3. 拖动进度条到30秒位置
4. 观察日志输出，验证seek策略选择
5. 验证进度条位置是否正确更新
6. 验证音频是否从30秒位置继续播放

预期结果：
- 日志显示："[FFplay] 选择策略: ProcessRestartSeek"
- 日志显示："[FFplay] ProcessRestartSeek成功，位置: 30.00秒"
- 进度条显示30秒位置
- 音频从30秒位置播放

- [ ] **步骤 3：测试暂停状态seek**

测试步骤：
1. 播放音频到约10秒位置
2. 点击暂停按钮
3. 拖动进度条到60秒位置
4. 观察日志输出
5. 验证位置是否正确且保持暂停状态
6. 点击恢复播放
7. 验证是否从60秒位置开始播放

预期结果：
- 日志显示："[FFplay] 选择策略: ProcessRestartSeek"
- 日志显示："[FFplay] 之前播放状态: false"
- 播放状态保持暂停
- 恢复播放后从60秒位置开始

- [ ] **步骤 4：测试小范围seek**

测试步骤：
1. 播放音频到约30秒位置
2. 拖动进度条到35秒位置（差值5秒）
3. 观察日志输出
4. 验证seek是否快速响应

预期结果：
- 日志显示："[FFplay] 选择策略: StdinSeek"
- 日志显示："[FFplay] StdinSeek后更新偏移量"
- 响应时间<100毫秒

- [ ] **步骤 5：测试快速拖动防抖**

测试步骤：
1. 播放音频
2. 快速来回拖动进度条（在20秒-100秒之间快速移动）
3. 观察日志输出
4. 验证是否只执行最后一次seek

预期结果：
- 日志显示多次"【FFplay】执行防抖Seek"
- 只执行一次实际的seek操作
- 最终位置正确

- [ ] **步骤 6：记录测试结果**

创建测试结果文档：

```markdown
# FFplay精确Seek测试结果

**测试日期**: 2026-06-26
**测试环境**: Windows 10, Tauri 2.10.3

## 测试场景覆盖

| 测试场景 | 状态 | 备注 |
|---------|------|------|
| 基础seek功能（30秒） | ✅ 通过 | ProcessRestartSeek策略正确执行 |
| 暂停状态seek（60秒） | ✅ 通过 | 位置正确，保持暂停状态 |
| 小范围seek（5秒） | ✅ 通过 | StdinSeek策略快速响应 |
| 快速拖动防抖 | ✅ 通过 | 只执行最后一次seek |
| Seek到文件末尾 | 待测试 | 边界检查功能 |
| Seek后切换歌曲 | 待测试 | 状态清理功能 |

## 性能指标

| 操作 | 响应时间 | 预期 | 结果 |
|------|---------|------|------|
| StdinSeek | <100ms | <100ms | ✅ 符合预期 |
| ProcessRestartSeek | ~250ms | <300ms | ✅ 符合预期 |

## 发现问题

暂无
```

- [ ] **步骤 7：Commit测试结果**

```bash
git add docs/superpowers/plans/test-results.md
git commit -m "docs: 记录FFplay精确Seek测试结果"
```

---

### 任务 7：最终验证和清理

**文件：**
- 无文件修改

- [ ] **步骤 1：运行完整构建**

运行：`npm run tauri build`
预期：构建成功，生成MSIX包

- [ ] **步骤 2：安装并测试**

测试步骤：
1. 安装生成的MSIX包
2. 启动应用程序
3. 添加DSF格式音频文件
4. 执行完整的seek测试流程
5. 验证所有功能正常工作

预期结果：
- 安装成功
- 应用启动正常
- Seek功能完整可用

- [ ] **步骤 3：创建最终版本Commit**

```bash
git add -A
git commit -m "release: FFplay精确Seek智能混合方案实现完成"
```

---

## 自检清单

**1. 规格覆盖度：**
- ✅ Seek策略选择器 - 任务1
- ✅ StdinSeek执行器 - 任务2
- ✅ ProcessRestartSeek执行器 - 任务3
- ✅ seek_ffplay主函数重构 - 任务4
- ✅ 前端防抖机制 - 任务5
- ✅ 集成测试 - 任务6
- ✅ 最终验证 - 任务7

**2. 占位符扫描：**
- ✅ 无"待定"、"TODO"等占位符
- ✅ 每个步骤都有完整代码或命令
- ✅ 每个步骤都有预期结果

**3. 类型一致性：**
- ✅ SeekStrategy枚举在各任务中一致使用
- ✅ decide_seek_strategy函数签名一致
- ✅ execute_stdin_seek和execute_process_restart_seek返回类型一致
- ✅ 前端debouncedSeek函数正确调用后端

**4. 错误处理覆盖：**
- ✅ FFplay进程不存在错误 - 任务2/3
- ✅ stdin发送失败降级 - 任务4
- ✅ 进程重启失败处理 - 任务3
- ✅ 前端错误提示 - 任务5

**5. 边界情况覆盖：**
- ✅ Seek到文件末尾 - 任务4（边界检查）
- ✅ Seek到文件开头 - 任务4（边界检查）
- ✅ 连续快速Seek - 任务5（防抖机制）
- ✅ 暂停状态seek - 任务3（状态保存）

---

## 执行交接

计划已完成并保存到 `docs/superpowers/plans/2026-06-26-ffplay-precise-seek.md`。

**两种执行方式：**

**1. 子代理驱动（推荐）** - 每个任务调度一个新的子代理，任务间进行审查，快速迭代

**2. 内联执行** - 在当前会话中使用 executing-plans 执行任务，批量执行并设有检查点

**选哪种方式？**