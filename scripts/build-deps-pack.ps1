<#
.SYNOPSIS
    构建独立解码组件依赖包（ffmpeg / ffprobe / sacd_extract）。

.DESCRIPTION
    解码组件已从主安装包剥离，改为独立可选依赖：
      - 主程序（NSIS）不再分发这三个二进制，首次运行时引导用户安装；
      - 依赖包安装到公共目录（应用数据目录下的 deps/packs/<版本>），
        位于安装目录之外 -> 主程序覆盖安装 / 卸载都不会删除它，直接复用。

    本脚本把仓库 bin\ 下的三个工具打成一个 zip，并生成包内 deps.json
    （逐文件 SHA256 + 大小），供应用安装时强制校验：
      - 只允许白名单内的文件名落盘；
      - 解压拒绝路径穿越（Zip Slip）；
      - 逐文件 SHA256 不符即拒绝安装并清理暂存目录。

.PARAMETER Version
    依赖包版本号（默认读取 src-tauri\deps-manifest.json 的 packVersion）。

.PARAMETER OutputDir
    输出目录（默认 src-tauri\target\deps）。

.EXAMPLE
    pwsh scripts\build-deps-pack.ps1
    pwsh scripts\build-deps-pack.ps1 -Version 1.0.1
#>
param(
    [string]$Version = '',
    [string]$OutputDir = ''
)

$ErrorActionPreference = 'Stop'

$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$binDir = Join-Path $root 'bin'
$manifestPath = Join-Path $root 'src-tauri\deps-manifest.json'

if (-not $OutputDir) { $OutputDir = Join-Path $root 'src-tauri\target\deps' }

# ---- 版本号：优先参数，其次内置清单 ---------------------------------------
if (-not $Version) {
    if (-not (Test-Path $manifestPath)) {
        throw "找不到 $manifestPath ，请用 -Version 显式指定依赖包版本。"
    }
    $Version = (ConvertFrom-Json ([IO.File]::ReadAllText($manifestPath))).packVersion
}
if (-not $Version) { throw '依赖包版本号为空。' }

Write-Host "[deps] 版本         : $Version" -ForegroundColor Cyan
Write-Host "[deps] 工具来源目录 : $binDir" -ForegroundColor DarkGray

# ---- 收集工具文件 ---------------------------------------------------------
$tools = @('ffmpeg.exe', 'ffprobe.exe', 'sacd_extract.exe')
$files = @()
foreach ($tool in $tools) {
    $path = Join-Path $binDir $tool
    if (-not (Test-Path $path)) {
        throw "缺少 $path 。请把 $tool 放到仓库根 bin\ 目录后再构建依赖包。"
    }
    $files += [pscustomobject]@{
        Name   = $tool
        Path   = $path
        Size   = (Get-Item $path).Length
        Sha256 = (Get-FileHash -Path $path -Algorithm SHA256).Hash.ToLower()
    }
}
foreach ($f in $files) {
    Write-Host ("[deps]   {0,-18} {1,10:N0} bytes  {2}" -f $f.Name, $f.Size, $f.Sha256.Substring(0, 16)) -ForegroundColor DarkGray
}

# ---- 生成包内清单 deps.json ------------------------------------------------
$entries = @{}
foreach ($f in $files) { $entries[$f.Name] = @{ sha256 = $f.Sha256; size = $f.Size } }
$packManifest = @{
    packVersion = $Version
    files       = $entries
}
$manifestJson = $packManifest | ConvertTo-Json -Depth 5

# ---- 打包 ------------------------------------------------------------------
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
$zipPath = Join-Path $OutputDir "tplayer-deps-windows-x86_64-$Version.zip"
# 上次的产物可能被占用（ antivirus / 资源管理器预览），删除失败不阻断本次构建
if (Test-Path $zipPath) { Remove-Item $zipPath -Force -ErrorAction SilentlyContinue }

# 用临时目录收集待打包内容，避免把中间文件留在仓库
$stage = Join-Path $env:TEMP "tplayer-deps-stage-$Version"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $stage | Out-Null
try {
    foreach ($f in $files) { Copy-Item $f.Path (Join-Path $stage $f.Name) -Force }
    # deps.json 必须是 UTF-8 无 BOM（Rust 侧 serde_json 解析）
    [IO.File]::WriteAllText((Join-Path $stage 'deps.json'), $manifestJson, (New-Object Text.UTF8Encoding($false)))
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zipPath -CompressionLevel Optimal
} finally {
    Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
}

$zipHash = (Get-FileHash -Path $zipPath -Algorithm SHA256).Hash.ToLower()
$zipSize = (Get-Item $zipPath).Length

Write-Host "[deps] 依赖包       : $zipPath" -ForegroundColor Green
Write-Host ("[deps] 大小         : {0:N0} bytes" -f $zipSize) -ForegroundColor Green
Write-Host "[deps] zip SHA256  : $zipHash" -ForegroundColor Green

# ---- 同步更新内置清单的 url / sha256 / size --------------------------------
# 内置清单（src-tauri\deps-manifest.json）被编译进程序，指向依赖包的下载地址。
# 构建后把当前包的 sha256 / size 写回清单，保证「下载后整体校验」可用。
try {
    $builtin = ConvertFrom-Json ([IO.File]::ReadAllText($manifestPath))
    $entry = $builtin.packs | Where-Object { $_.platform -eq 'windows-x86_64' }
    if ($entry) {
        $entry.sha256 = $zipHash
        $entry.size = $zipSize
        $json = ($builtin | ConvertTo-Json -Depth 5)
        [IO.File]::WriteAllText($manifestPath, $json, (New-Object Text.UTF8Encoding($false)))
        Write-Host "[deps] 已更新内置清单 windows-x86_64 的 sha256 / size" -ForegroundColor DarkGray
    }
} catch {
    Write-Warning "[deps] 更新内置清单失败（不影响本次构建）：$_"
}

Write-Host ''
Write-Host '[deps] 发布提示：把该 zip 作为 Release 资产上传（tag 形如 deps-v1.0.0），' -ForegroundColor Cyan
Write-Host '[deps] 并把内置清单中的 url 指向该资产地址。' -ForegroundColor Cyan
