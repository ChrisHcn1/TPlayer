---
name: tplayer-dev-launch
description: Launch TPlayer Next dev on Windows — free port 3100, kill stale tplayer-next, run npm run tauri dev in background unsandboxed, verify the window. Use when user says 启动测试/启动/重新启动 or asks to start/relaunch tauri dev. Not for production builds.
---

# TPlayer Next 开发环境启动

项目：`e:\TPlayerNext`（Tauri 2 + Vue 3 + Rust，Windows）。把 dev 环境拉起到可用状态。

## 固定事实

- 前端 Vite 端口：**3100**
- 应用进程名：**tplayer-next**
- 启动命令：`npm run tauri dev`（同时拉起 Vite 与 Rust 编译/窗口）
- 该命令**必须** `dangerouslyDisableSandbox: true` 且 `run_in_background: true`（Tauri 要起窗口、长驻进程，沙箱内会失败）
- 增量编译通常 1–3 秒；改了 Rust 依赖时可能 30–60 秒，等待时间要相应加长
- 窗口关闭默认只是隐藏到托盘；只有托盘"退出"或进程被杀，dev 后台任务才会结束（exit code 0）
- 严禁触碰旧版 `E:\TPlayer`

## 步骤

1. **检查现状**（一次 Shell 调用并行查）：
   ```powershell
   Get-Process -Name tplayer-next -ErrorAction SilentlyContinue | Select-Object Id, StartTime | Format-Table -AutoSize
   Get-NetTCPConnection -LocalPort 3100 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1 LocalPort, OwningProcess | Format-Table -AutoSize
   ```
   - 两者都在 → 无需重启，直接告知用户已在运行（给 PID），结束。
   - 否则继续。

2. **清理残留**（tplayer-next 已退出但 vite 可能还在占端口；反之亦然）：
   ```powershell
   Get-NetTCPConnection -LocalPort 3100 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty OwningProcess | ForEach-Object { Stop-Process -Id $_ -Force }
   Get-Process -Name tplayer-next -ErrorAction SilentlyContinue | Stop-Process -Force
   ```

3. **后台启动**，`cwd` = `e:\TPlayerNext`：
   - Shell: `npm run tauri dev`
   - `dangerouslyDisableSandbox: true`、`run_in_background: true`
   - 记录返回的 command_id 与 output.log 路径。

4. **等待并验证**（默认 20 秒；若近期改过 Cargo.toml/依赖，等 60 秒）：
   ```powershell
   Start-Sleep -Seconds 20
   Get-Content "<output.log>" -Tail 4
   Get-Process -Name tplayer-next -ErrorAction SilentlyContinue | Select-Object Id, StartTime | Format-Table -AutoSize
   ```
   成功判据：日志出现 `Running target\debug\tplayer-next.exe`，且进程列表有 tplayer-next。
   失败处理：日志若有 `error[`/`error:`/`panicked`，把错误段原文贴给用户，不要自行猜测性改动 Rust 代码；只是还在 `Compiling` 则再等一轮。

5. **汇报**：告知已启动（PID + 端口），不主动跑浏览器回归，除非用户要求。

## 注意

- 不要前台执行启动命令——它永不退出，会耗尽命令超时。
- 不要重复启动多个实例；步骤 1 的现状检查不可省。
- 用户说"启动测试"且近期改动包含前端时，启动后可顺手提示用户在 Tauri 窗口验证；是否做浏览器回归由用户明确要求决定。
