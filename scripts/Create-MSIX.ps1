# TPlayer MSIX Build Script
param(
    [string]$OutputPath = "E:\TPlayer\msix-output"
)

Write-Host "Starting MSIX packaging..." -ForegroundColor Green

# Check dependencies
Write-Host "Checking Windows SDK tools..." -ForegroundColor Cyan
$makeAppxPath = Get-ChildItem -Path "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\makeappx.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
$signToolPath = Get-ChildItem -Path "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue | Select-Object -First 1

if (-not $makeAppxPath) {
    Write-Host "Error: MakeAppx tool not found" -ForegroundColor Red
    exit 1
}

if (-not $signToolPath) {
    Write-Host "Error: SignTool not found" -ForegroundColor Red
    exit 1
}

Write-Host "Dependencies check completed" -ForegroundColor Green

# Clean output directory
Write-Host "Cleaning output directory..." -ForegroundColor Cyan
if (Test-Path $OutputPath) {
    Remove-Item -Path "$OutputPath\*" -Recurse -Force -ErrorAction SilentlyContinue
} else {
    New-Item -ItemType Directory -Path $OutputPath -Force | Out-Null
}

# Prepare MSIX package content
Write-Host "Preparing MSIX package content..." -ForegroundColor Cyan
$appxPath = Join-Path $OutputPath "Appx"
New-Item -ItemType Directory -Path $appxPath -Force | Out-Null

# Copy executable file
Write-Host "Copying executable file..." -ForegroundColor Cyan
$releasePath = "E:\TPlayer\src-tauri\target\release"
if (-not (Test-Path "$releasePath\app.exe")) {
    Write-Host "Error: Executable file not found" -ForegroundColor Red
    exit 1
}

Copy-Item -Path "$releasePath\app.exe" -Destination "$appxPath\app.exe" -Force

# Copy resource files
Write-Host "Copying resource files..." -ForegroundColor Cyan
if (Test-Path "$releasePath\bin") {
    Copy-Item -Path "$releasePath\bin" -Destination "$appxPath\bin" -Recurse -Force
}

# Copy icon resources
Write-Host "Copying icon resources..." -ForegroundColor Cyan
$iconsPath = "E:\TPlayer\src-tauri\icons"
if (Test-Path $iconsPath) {
    New-Item -ItemType Directory -Path "$appxPath\icons" -Force | Out-Null
    Copy-Item -Path "$iconsPath\*.png" -Destination "$appxPath\icons" -Force -ErrorAction SilentlyContinue
    Copy-Item -Path "$iconsPath\*.ico" -Destination "$appxPath\icons" -Force -ErrorAction SilentlyContinue
}

# Copy manifest file
Write-Host "Copying manifest file..." -ForegroundColor Cyan
Copy-Item -Path "E:\TPlayer\src-tauri\AppxManifest.xml" -Destination "$appxPath\AppxManifest.xml" -Force

# Create MSIX package
Write-Host "Creating MSIX package..." -ForegroundColor Cyan
$msixPath = Join-Path $OutputPath "TPlayer_1.0.4.0_x64.msix"
$arguments = @("pack", "/d", $appxPath, "/p", $msixPath, "/o")

$process = Start-Process -FilePath $makeAppxPath.FullName -ArgumentList $arguments -Wait -PassThru -NoNewWindow

if ($process.ExitCode -ne 0) {
    Write-Host "Error: Failed to create MSIX package" -ForegroundColor Red
    exit 1
}

Write-Host "MSIX package created: $msixPath" -ForegroundColor Green

# Check file size
$fileSize = (Get-Item $msixPath).Length
$fileSizeMB = [math]::Round($fileSize / 1MB, 2)
Write-Host "MSIX package size: $fileSizeMB MB" -ForegroundColor Cyan

Write-Host "MSIX packaging completed!" -ForegroundColor Green
Write-Host "Package location: $msixPath" -ForegroundColor Cyan
Write-Host "Version: 1.0.4.0" -ForegroundColor Cyan
Write-Host "Architecture: x64" -ForegroundColor Cyan