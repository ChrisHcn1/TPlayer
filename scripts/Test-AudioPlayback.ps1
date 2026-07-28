# 测试音频播放功能的PowerShell脚本

$ErrorColor = "Red"
$SuccessColor = "Green"
$InfoColor = "Cyan"
$WarningColor = "Yellow"

function Write-Info($message) {
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] INFO: $message" -ForegroundColor $InfoColor
}

function Write-Success($message) {
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] SUCCESS: $message" -ForegroundColor $SuccessColor
}

function Write-Error($message) {
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] ERROR: $message" -ForegroundColor $ErrorColor
}

function Write-Warning($message) {
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] WARNING: $message" -ForegroundColor $WarningColor
}

function Test-FFmpegPresence {
    param(
        [string]$SearchPath
    )
    
    Write-Info "检查FFmpeg文件是否存在于: $SearchPath"
    
    $ffmpegExe = Join-Path -Path $SearchPath -ChildPath "ffmpeg.exe"
    $ffprobeExe = Join-Path -Path $SearchPath -ChildPath "ffprobe.exe"
    $ffplayExe = Join-Path -Path $SearchPath -ChildPath "ffplay.exe"
    
    $allFound = $true
    
    if (Test-Path -Path $ffmpegExe) {
        Write-Success "ffmpeg.exe: 存在"
    } else {
        Write-Warning "ffmpeg.exe: 缺失"
        $allFound = $false
    }
    
    if (Test-Path -Path $ffprobeExe) {
        Write-Success "ffprobe.exe: 存在"
    } else {
        Write-Warning "ffprobe.exe: 缺失"
        $allFound = $false
    }
    
    if (Test-Path -Path $ffplayExe) {
        Write-Success "ffplay.exe: 存在"
    } else {
        Write-Warning "ffplay.exe: 缺失"
        $allFound = $false
    }
    
    return $allFound
}

Write-Host "==============================================" -ForegroundColor $InfoColor
Write-Host "          TPlayer 音频播放测试脚本" -ForegroundColor $InfoColor
Write-Host "==============================================" -ForegroundColor $InfoColor

Write-Host ""
Write-Host "[开发环境测试]" -ForegroundColor $InfoColor

$devBinPath = Join-Path -Path $PWD.Path -ChildPath "src-tauri\bin"
$allFfmpegFound = Test-FFmpegPresence -SearchPath $devBinPath

if (-not $allFfmpegFound) {
    Write-Warning "FFmpeg文件缺失，这将导致MSIX打包后音频播放失败！"
    Write-Warning "请按照 src-tauri/bin/README.md 的说明放置FFmpeg文件"
} else {
    Write-Success "所有FFmpeg文件都已准备就绪"
}

Write-Host ""
Write-Host "检查应用构建状态..." -ForegroundColor $InfoColor

$appExePath = Join-Path -Path $PWD.Path -ChildPath "src-tauri\target\release\app.exe"
if (Test-Path -Path $appExePath) {
    Write-Success "应用可执行文件存在"
} else {
    Write-Warning "应用可执行文件不存在，请先运行 npm run tauri build"
}

$distPath = Join-Path -Path $PWD.Path -ChildPath "dist"
if (Test-Path -Path $distPath) {
    Write-Success "前端资源目录存在"
} else {
    Write-Warning "前端资源目录不存在，请先运行 npm run build"
}

Write-Host ""
Write-Host "==============================================" -ForegroundColor $InfoColor
if ($allFfmpegFound) {
    Write-Success "测试完成！"
} else {
    Write-Warning "测试完成，但存在问题需要修复"
}
Write-Host "==============================================" -ForegroundColor $InfoColor