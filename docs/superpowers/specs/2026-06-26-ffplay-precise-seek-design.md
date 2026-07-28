# FFplay精确Seek智能混合方案设计

**日期**: 2026-06-26  
**版本**: 1.0  
**状态**: 已批准

## 一、背景与目标

### 1.1 问题分析

当前FFplay seek功能存在以下问题：
1. 方向键seek只能10秒步进，精度有限
2. 暂停状态下seek效果不佳
3. Seek后状态同步问题（位置被重置）

### 1.2 设计目标

实现FFplay精确Seek功能，确保：
- Seek精度达到毫秒级
- 暂停和播放状态下都有效
- 状态同步准确可靠
- 用户体验流畅

## 二、架构设计

### 2.1 核心组件

**Seek策略选择器**
- **职责**: 根据seek参数和当前状态决定使用哪种seek方式
- **接口**: `decide_seek_strategy(current_pos: f64, target_pos: f64, is_playing: bool) -> SeekStrategy`
- **策略规则**:
  - 距离 < 5秒 → `NoSeek`（差值太小，无需seek）
  - 5秒 ≤ 距离 < 10秒 + 播放状态 → `StdinSeek`（方向键快速调整）
  - 距离 ≥ 10秒 或 暂停状态 → `ProcessRestartSeek`（重启进程精确定位）

**Seek执行器**
- **职责**: 执行具体的seek操作
- **两种实现**:
  1. `StdinSeekExecutor`: 发送方向键命令，更新偏移量
  2. `ProcessRestartSeekExecutor`: 重启ffplay进程，使用`-ss`参数

**状态同步器**
- **职责**: 确保seek后前端和后端状态一致
- **关键机制**:
  - 更新`FFPLAY_START_OFFSET`（起始偏移量）
  - 重置`FFPLAY_START_INSTANT`（起始时刻）
  - 更新`FFPLAY_STATUS.position`（当前位置）
  - 触发前端状态更新事件

## 三、数据流设计

### 3.1 Seek操作完整流程

```
前端拖动进度条 → seek_ffplay(position) →
├─ 获取当前状态 → { current_pos, is_playing }
├─ 计算seek距离 → diff = target_pos - current_pos
├─ 决策seek策略 → decide_seek_strategy()
│   ├─ diff < 5秒 → NoSeek（返回成功，无需操作）
│   ├─ 5秒 ≤ diff < 10秒 + 播放 → StdinSeek
│   └─ diff ≥ 10秒 或 暂停 → ProcessRestartSeek
│
├─ 执行seek操作 →
│   ├─ StdinSeek →
│   │   ├─ 计算方向键次数 → times = diff/10
│   │   ├─ 发送stdin命令 → stdin.write(方向键序列)
│   │   ├─ 更新偏移量 → FFPLAY_START_OFFSET = estimated_pos
│   │   └─ 返回成功 → { success: true, position: estimated_pos }
│   │
│   └─ ProcessRestartSeek →
│   │   ├─ 停止当前进程 → stop_ffplay()
│   │   ├─ 启动新进程 → play_with_ffplay(path, ss=target_pos)
│   │   ├─ 设置偏移量 → FFPLAY_START_OFFSET = target_pos
│   │   ├─ 如果之前暂停 → pause_ffplay()
│   │   └─ 返回成功 → { success: true, position: target_pos }
│
└─ 更新前端状态 →
    ├─ 更新进度条 → currentPosition.value = result.position
    └─ 更新播放状态 → isPlaying.value = result.is_playing
```

### 3.2 状态同步机制

**偏移量计算公式**:
```
绝对位置 = FFPLAY_START_OFFSET + 输出解析线程的相对时间
```

**状态同步时序**:

1. **StdinSeek**:
   - 立即更新`FFPLAY_START_OFFSET`和`status.position`
   - 输出解析线程自动修正实际位置

2. **ProcessRestartSeek**:
   - 重启进程前保存`is_playing`状态
   - 启动新进程时设置`FFPLAY_START_OFFSET = target_pos`
   - 如果之前暂停，立即发送暂停命令
   - 输出解析线程从0开始计算相对时间

## 四、错误处理

### 4.1 错误场景

1. **FFplay进程不存在** → 返回错误提示，前端显示友好提示
2. **stdin发送失败** → 自动降级到重启进程方式
3. **进程重启失败** → 返回错误，前端恢复到之前状态
4. **文件路径无效** → 拒绝seek操作，保持当前位置

### 4.2 降级策略

```
StdinSeek失败 → 尝试ProcessRestartSeek →
ProcessRestartSeek失败 → 返回错误 → 前端恢复原状态
```

## 五、边界情况

### 5.1 极端场景

1. **Seek到文件末尾附近**（position > duration - 1秒）:
   - 直接定位到duration - 1秒
   - 防止超过文件长度

2. **Seek到文件开头**（position < 0）:
   - 定位到0秒

3. **连续快速Seek**（用户快速拖动进度条）:
   - 添加Seek防抖机制（200ms延迟）
   - 只执行最后一次seek操作

4. **Seek过程中用户停止播放**:
   - 立即取消seek操作
   - 清理中间状态

5. **输出解析线程冲突**:
   - 使用Mutex保护状态更新
   - 避免并发写入冲突

## 六、测试策略

### 6.1 单元测试

**Seek策略选择器测试**:
- 测试不同距离的决策逻辑（<5秒、5-10秒、>10秒）
- 测试播放/暂停状态的决策差异
- 测试边界值（4.9秒、5.0秒、9.9秒、10.0秒）

**StdinSeek执行器测试**:
- 测试方向键命令发送
- 测试偏移量更新准确性
- 测试stdin失败时的降级逻辑

**ProcessRestartSeek执行器测试**:
- 测试进程重启逻辑
- 测试`-ss`参数传递正确性
- 测试暂停状态恢复

### 6.2 集成测试

**基础seek功能**:
- 从头seek到中间位置（30秒、60秒、120秒）
- 从中间位置seek到其他位置
- Seek到文件末尾附近

**暂停状态seek**:
- 暂停时seek，验证位置正确且保持暂停
- 暂停时seek，恢复播放，验证从正确位置开始

**连续操作**:
- 快速连续seek（测试防抖机制）
- Seek后立即暂停/恢复
- Seek后切换歌曲

### 6.3 性能测试

**响应时间测试**:
- StdinSeek响应时间（预期<100ms）
- ProcessRestartSeek响应时间（预期<300ms）

**资源消耗测试**:
- 内存使用是否稳定
- 进程重启次数是否合理

## 七、实施计划

### 7.1 实施步骤

1. **后端改造**（src-tauri/src/ffmpeg_transcoder.rs）:
   - 实现Seek策略选择器
   - 优化StdinSeek执行器
   - 完善ProcessRestartSeek执行器
   - 增强状态同步机制

2. **前端适配**（src/App.vue）:
   - 添加Seek防抖机制
   - 完善错误处理逻辑
   - 优化用户体验提示

3. **测试验证**:
   - 执行单元测试
   - 执行集成测试
   - 执行性能测试

### 7.2 预期效果

- Seek精度提升至毫秒级
- 暂停状态下seek功能完善
- 状态同步准确可靠
- 用户体验显著提升