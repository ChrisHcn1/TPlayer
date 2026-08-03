# TPlayer MSIX Build Script v1.0.5
# Creates MSIX package for Windows 10/11 deployment
param(
    [string]$OutputPath = "E:\TPlayer\msix-output"
)

$ErrorActionPreference = "Stop"
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

$releasePath = "E:\TPlayer\src-tauri\target\release"
$projectPath = "E:\TPlayer\src-tauri"
$version = "1.0.6.0"
$appName = "D57E920A.TPlayer"
$msixFileName = "$appName`_$version`_x64.msix"

Write-Host "`n==============================================" -ForegroundColor $InfoColor
Write-Host "     TPlayer MSIX Package Creator v$version" -ForegroundColor $InfoColor
Write-Host "==============================================`n" -ForegroundColor $InfoColor

# Step 1: Check dependencies
Write-Info "Step 1: Checking Windows SDK tools..."

$makeAppxPath = Get-ChildItem -Path "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\makeappx.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $makeAppxPath) {
    Write-Error "MakeAppx tool not found. Please install Windows SDK 10."
    exit 1
}
Write-Success "MakeAppx found: $($makeAppxPath.FullName)"

# Step 2: Verify build artifacts
Write-Info "Step 2: Verifying build artifacts..."

if (-not (Test-Path "$releasePath\app.exe")) {
    Write-Error "app.exe not found at $releasePath\app.exe. Run 'cargo build --release' first."
    exit 1
}
$appExeInfo = Get-Item "$releasePath\app.exe"
Write-Success "app.exe found: $([math]::Round($appExeInfo.Length / 1MB, 2)) MB"

# Step 3: Verify bin directory
Write-Info "Step 3: Verifying FFmpeg binaries..."

$requiredBinFiles = @("ffmpeg.exe", "ffplay.exe", "ffprobe.exe", "avcodec-62.dll", "avformat-62.dll", "avutil-60.dll", "swresample-6.dll", "swscale-9.dll")
$binPath = "$releasePath\bin"

if (-not (Test-Path $binPath)) {
    Write-Warning "bin directory not found in release. Checking src-tauri/bin..."
    $binPath = "$projectPath\bin"
}

$allBinFilesPresent = $true
foreach ($file in $requiredBinFiles) {
    $filePath = Join-Path $binPath $file
    if (Test-Path $filePath) {
        Write-Success "  $file"
    } else {
        Write-Error "  $file - MISSING"
        $allBinFilesPresent = $false
    }
}
if (-not $allBinFilesPresent) {
    Write-Error "Required FFmpeg files are missing. Cannot continue."
    exit 1
}

# Step 4: Verify icons
Write-Info "Step 4: Verifying icon resources..."

$iconsPath = "$projectPath\icons"
$requiredIcons = @("StoreLogo.png", "Square44x44Logo.png", "Square150x150Logo.png", "Square310x310Logo.png")
foreach ($icon in $requiredIcons) {
    $iconPath = Join-Path $iconsPath $icon
    if (Test-Path $iconPath) {
        Write-Success "  $icon"
    } else {
        Write-Warning "  $icon - MISSING"
    }
}

# Step 5: Verify AppxManifest.xml
Write-Info "Step 5: Verifying AppxManifest.xml..."

$manifestPath = "$projectPath\AppxManifest.xml"
if (-not (Test-Path $manifestPath)) {
    Write-Error "AppxManifest.xml not found at $manifestPath"
    exit 1
}
Write-Success "AppxManifest.xml found"

# Step 6: Find WebView2Loader.dll
Write-Info "Step 6: Locating WebView2Loader.dll..."

$webviewLoaderPath = Get-ChildItem -Path "$releasePath\build" -Filter "WebView2Loader.dll" -Recurse -ErrorAction SilentlyContinue | Where-Object { $_.FullName -match "x64" } | Select-Object -First 1
if ($webviewLoaderPath) {
    Write-Success "WebView2Loader.dll found: $($webviewLoaderPath.FullName)"
} else {
    Write-Warning "WebView2Loader.dll not found. MSIX will use downloadBootstrapper mode."
}

# Step 7: Prepare MSIX staging directory
Write-Info "Step 7: Preparing MSIX staging directory..."

if (Test-Path $OutputPath) {
    Remove-Item -Path "$OutputPath\*" -Recurse -Force -ErrorAction SilentlyContinue
} else {
    New-Item -ItemType Directory -Path $OutputPath -Force | Out-Null
}

$appxPath = Join-Path $OutputPath "Appx"
New-Item -ItemType Directory -Path $appxPath -Force | Out-Null

# Copy executable
Write-Info "Copying app.exe..."
Copy-Item -Path "$releasePath\app.exe" -Destination "$appxPath\app.exe" -Force
Write-Success "app.exe copied"

# Copy bin directory
Write-Info "Copying FFmpeg binaries..."
Copy-Item -Path "$binPath" -Destination "$appxPath\bin" -Recurse -Force
Write-Success "FFmpeg binaries copied"

# Copy icons
Write-Info "Copying icon resources..."
New-Item -ItemType Directory -Path "$appxPath\icons" -Force | Out-Null
Get-ChildItem -Path $iconsPath -Filter "*.png" | Copy-Item -Destination "$appxPath\icons" -Force
Get-ChildItem -Path $iconsPath -Filter "*.ico" | Copy-Item -Destination "$appxPath\icons" -Force -ErrorAction SilentlyContinue
Write-Success "Icons copied"

# Copy manifest
Write-Info "Copying AppxManifest.xml..."
Copy-Item -Path $manifestPath -Destination "$appxPath\AppxManifest.xml" -Force
Write-Success "AppxManifest.xml copied"

# Copy WebView2Loader.dll if found
if ($webviewLoaderPath) {
    Write-Info "Copying WebView2Loader.dll..."
    Copy-Item -Path $webviewLoaderPath.FullName -Destination "$appxPath\WebView2Loader.dll" -Force
    Write-Success "WebView2Loader.dll copied"
}

# Step 8: Create MSIX package
Write-Info "Step 8: Creating MSIX package..."

$msixPath = Join-Path $OutputPath $msixFileName
$arguments = @("pack", "/d", $appxPath, "/p", $msixPath, "/o")

$process = Start-Process -FilePath $makeAppxPath.FullName -ArgumentList $arguments -Wait -PassThru -NoNewWindow

if ($process.ExitCode -ne 0) {
    Write-Error "Failed to create MSIX package (exit code: $($process.ExitCode))"
    exit 1
}

# Step 9: Verify the created package
if (Test-Path $msixPath) {
    $fileSize = (Get-Item $msixPath).Length
    $fileSizeMB = [math]::Round($fileSize / 1MB, 2)
    
    Write-Success "MSIX package created successfully!"
    Write-Host ""
    Write-Host "==============================================" -ForegroundColor $SuccessColor
    Write-Host "  MSIX Package: $msixPath" -ForegroundColor $SuccessColor
    Write-Host "  Size: $fileSizeMB MB" -ForegroundColor $InfoColor
    Write-Host "  Version: $version" -ForegroundColor $InfoColor
    Write-Host "  Architecture: x64" -ForegroundColor $InfoColor
    Write-Host "  Product: $appName" -ForegroundColor $InfoColor
    Write-Host "==============================================" -ForegroundColor $SuccessColor
    Write-Host ""
    Write-Warning "IMPORTANT: The MSIX package must be signed before installation."
    Write-Warning "Use scripts\Sign-MSIX.ps1 to sign with a test certificate,"
    Write-Warning "or use the certificate from Microsoft Partner Center for Store submission."
} else {
    Write-Error "MSIX package file not found after creation attempt."
    exit 1
}