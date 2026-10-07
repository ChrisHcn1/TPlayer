<#
.SYNOPSIS
    TPlayer Next Windows 一键打包（NSIS 安装包 + 自动升级签名产物）。

.DESCRIPTION
    执行 `tauri build`，产物输出到 src-tauri/target/release/bundle/nsis/：
      - TPlayer.Next_<版本>_x64-setup.exe        用户安装包（同时也是自动升级包）
      - TPlayer.Next_<版本>_x64-setup.exe.sig    升级签名（minisign）
    发布时还需把 latest.json 与 setup.exe/.sig 一起上传到 GitHub Releases
    （tauri-action 会自动生成 latest.json 并上传全部产物）。

    签名密钥（minisign）二选一提供：
      1. 环境变量 TAURI_SIGNING_PRIVATE_KEY（私钥字符串或 .key 文件路径）
         环境变量 TAURI_SIGNING_PRIVATE_KEY_PASSWORD（私钥密码，无密码可省）
      2. 本地文件 src-tauri/.tauri-signing/TPlayerNext.key（已 gitignore，不入库）

.EXAMPLE
    # 用环境变量
    $env:TAURI_SIGNING_PRIVATE_KEY = "C:\secrets\TPlayerNext.key"
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "密钥密码"
    npm run build:win

.EXAMPLE
    # 直接运行（回落使用本地 .tauri-signing 密钥，密码仍需环境变量）
    powershell -File scripts/build-win.ps1
#>
[CmdletBinding()]
param(
    # 跳过依赖安装（CI 或已手动 npm ci 时使用）
    [switch]$SkipInstall
)

$ErrorActionPreference = 'Stop'

$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$localKey = Join-Path $root 'src-tauri\.tauri-signing\TPlayerNext.key'

# 前置检查：FFmpeg 运行时工具会随安装包分发（tauri.conf.json bundle.resources），
# 缺失时打出来的包安装后无法播放 wma/ape/dsf/dff/dts 等格式
foreach ($tool in @('ffmpeg.exe', 'ffprobe.exe')) {
    $toolPath = Join-Path $root (Join-Path 'bin' $tool)
    if (-not (Test-Path $toolPath)) {
        throw "缺少运行时依赖 $toolPath 。请把 $tool 放到仓库根 bin\ 目录后再打包。"
    }
}

if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
    if (Test-Path $localKey) {
        $env:TAURI_SIGNING_PRIVATE_KEY = (Resolve-Path $localKey).Path
        Write-Host "[build] 未设置 TAURI_SIGNING_PRIVATE_KEY，回落使用本地密钥: $localKey" -ForegroundColor DarkGray
    } else {
        throw @(
            '未找到升级签名密钥。请任选一种方式提供：'
            '  1) 设置环境变量 TAURI_SIGNING_PRIVATE_KEY（私钥字符串或 .key 文件路径）；'
            '  2) 放置私钥到 src-tauri\.tauri-signing\TPlayerNext.key。'
            '私钥有密码时还需设置 TAURI_SIGNING_PRIVATE_KEY_PASSWORD。'
            '可用 `npm run tauri signer generate` 生成密钥对（公钥写入 tauri.conf.json）。'
        ) -join [Environment]::NewLine
    }
}

# 无密码密钥可留空；有密码必须在环境变量中提供
if ($null -eq $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
    Write-Warning '[build] TAURI_SIGNING_PRIVATE_KEY_PASSWORD 未设置；若私钥有密码，签名会失败。'
}

Push-Location $root
try {
    if (-not $SkipInstall) {
        Write-Host '[build] 安装前端依赖 (npm ci)…' -ForegroundColor Cyan
        npm ci
        if ($LASTEXITCODE -ne 0) { throw 'npm ci 失败' }
    }

    Write-Host '[build] 开始 Tauri 构建（NSIS + 升级签名）…' -ForegroundColor Cyan
    npm run tauri build
    if ($LASTEXITCODE -ne 0) { throw 'tauri build 失败' }

    $bundleDir = Join-Path $root 'src-tauri\target\release\bundle\nsis'
    Write-Host ''
    Write-Host '[build] 构建完成，产物目录:' -ForegroundColor Green
    Write-Host "  $bundleDir" -ForegroundColor Green
    if (Test-Path $bundleDir) {
        Get-ChildItem $bundleDir | ForEach-Object { Write-Host "  - $($_.Name)" }
    }
    Write-Host ''
    Write-Host '发布到 GitHub Releases 时，请同时上传 *-setup.exe、*-setup.exe.sig，并更新 latest.json。' -ForegroundColor Yellow
} finally {
    Pop-Location
}
