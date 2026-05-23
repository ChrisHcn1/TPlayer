<#
.SYNOPSIS
Create MSIX package for TPlayer

.DESCRIPTION
This script packages TPlayer into MSIX format for Microsoft Store publishing

.PARAMETER Version
Application version in x.x.x.x format (default: 1.0.2.0)

.PARAMETER OutputDir
Output directory path (default: src-tauri/target/release/bundle/msix)

.PARAMETER CertificatePath
Signing certificate path (optional)

.PARAMETER CertificatePassword
Certificate password (optional)

.EXAMPLE
.\Create-MSIX.ps1
Create unsigned MSIX package (for testing)

.EXAMPLE
.\Create-MSIX.ps1 -CertificatePath "cert.pfx" -CertificatePassword "password"
Create signed MSIX package (for publishing)
#>

param(
    [string]$Version = "1.0.2.0",
    [string]$OutputDir = "src-tauri/target/release/bundle/msix",
    [string]$CertificatePath,
    [string]$CertificatePassword
)

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

try {
    Write-Host "`n==============================================" -ForegroundColor $InfoColor
    Write-Host "          TPlayer MSIX Packaging Script" -ForegroundColor $InfoColor
    Write-Host "==============================================`n" -ForegroundColor $InfoColor

    $packageName = "TPlayer"
    $publisher = "CN=4A6BC8B4-7E26-46D4-8F71-B56966D06EB0"
    $msixFileName = "$($packageName)_$Version.msix"
    $msixPath = Join-Path -Path $OutputDir -ChildPath $msixFileName

    Write-Info "Version: $Version"
    Write-Info "Publisher: $publisher"
    Write-Info "Output Path: $msixPath"
    Write-Info "Certificate: $(if ($CertificatePath) { $CertificatePath } else { 'Not specified' })"

    Write-Host "`n[1/6] Validating input files..." -ForegroundColor $InfoColor
    
    $appExePath = "src-tauri/target/release/app.exe"
    if (-not (Test-Path -Path $appExePath)) {
        throw "Application executable not found: $appExePath`nPlease run 'npm run tauri build' first"
    }
    Write-Success "Application executable: Found"

    $distPath = "dist"
    if (-not (Test-Path -Path $distPath)) {
        throw "Frontend assets not found: $distPath`nPlease run 'npm run build' first"
    }
    Write-Success "Frontend assets: Found"

    Write-Host "`n[2/6] Preparing output directory..." -ForegroundColor $InfoColor
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
    Write-Success "Output directory: $OutputDir"

    Write-Host "`n[3/6] Creating temporary workspace..." -ForegroundColor $InfoColor
    $tempDir = Join-Path -Path $env:TEMP -ChildPath "MSIXBuild_$([guid]::NewGuid())"
    New-Item -ItemType Directory -Path $tempDir -Force | Out-Null
    Write-Success "Temporary directory: $tempDir"

    Write-Host "`n[4/6] Creating app manifest..." -ForegroundColor $InfoColor
    
    $manifestContent = @"
<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:uap2="http://schemas.microsoft.com/appx/manifest/uap/windows10/2"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap uap2 rescap">

  <Identity
    Name="D57E920A.TPlayer"
    Publisher="$publisher"
    Version="$Version" />

  <Properties>
    <DisplayName>TPlayer</DisplayName>
    <PublisherDisplayName>&#26684;&#21147;&#26862;</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
    <Description>A modern music player with DSD support and advanced lyric features.</Description>
  </Properties>

  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.22621.0" />
  </Dependencies>

  <Resources>
    <Resource Language="en-US" />
    <Resource Language="zh-CN" />
  </Resources>

  <Applications>
    <Application Id="App" Executable="VFS/ProgramFilesX64/TPlayer/app.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements
        DisplayName="TPlayer"
        Description="A modern music player with DSD support"
        Square150x150Logo="Assets\Square150x150Logo.png"
        Square44x44Logo="Assets\Square44x44Logo.png"
        BackgroundColor="#1a1a2e">
        <uap:DefaultTile Wide310x150Logo="Assets\Wide310x150Logo.png" Square310x310Logo="Assets\Square310x310Logo.png" />
        <uap:SplashScreen Image="Assets\SplashScreen.png" BackgroundColor="#1a1a2e" />
        <uap:InitialRotationPreference>
          <uap:Rotation Preference="portrait" />
          <uap:Rotation Preference="landscape" />
          <uap:Rotation Preference="portraitFlipped" />
          <uap:Rotation Preference="landscapeFlipped" />
        </uap:InitialRotationPreference>
      </uap:VisualElements>
      <Extensions>
        <uap:Extension Category="windows.fileTypeAssociation">
          <uap:FileTypeAssociation Name="audio">
            <uap:DisplayName>Audio File</uap:DisplayName>
            <uap:SupportedFileTypes>
              <uap:FileType>.mp3</uap:FileType>
              <uap:FileType>.flac</uap:FileType>
              <uap:FileType>.wav</uap:FileType>
              <uap:FileType>.aac</uap:FileType>
              <uap:FileType>.ogg</uap:FileType>
              <uap:FileType>.m4a</uap:FileType>
              <uap:FileType>.wma</uap:FileType>
              <uap:FileType>.dsf</uap:FileType>
              <uap:FileType>.dff</uap:FileType>
            </uap:SupportedFileTypes>
          </uap:FileTypeAssociation>
        </uap:Extension>
      </Extensions>
    </Application>
  </Applications>

  <Capabilities>
    <Capability Name="internetClient" />
    <Capability Name="privateNetworkClientServer" />
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>

</Package>
"@

    $manifestPath = Join-Path -Path $tempDir -ChildPath "AppxManifest.xml"
    $manifestContent | Out-File -FilePath $manifestPath -Encoding UTF8
    Write-Success "App manifest: Created"

    Write-Host "`n[5/6] Copying icon assets..." -ForegroundColor $InfoColor
    
    $assetsDir = Join-Path -Path $tempDir -ChildPath "Assets"
    New-Item -ItemType Directory -Path $assetsDir -Force | Out-Null
    
    # 同时复制到 icons 目录（用于向后兼容）
    $iconsDir = Join-Path -Path $tempDir -ChildPath "icons"
    New-Item -ItemType Directory -Path $iconsDir -Force | Out-Null

    $iconFiles = @(
        @{ Source = "src-tauri/icons/Square44x44Logo.png"; Dest = "Square44x44Logo.png" },
        @{ Source = "src-tauri/icons/Square150x150Logo.png"; Dest = "Square150x150Logo.png" },
        @{ Source = "src-tauri/icons/Square310x310Logo.png"; Dest = "Square310x310Logo.png" },
        @{ Source = "src-tauri/icons/Square310x310Logo.png"; Dest = "Wide310x150Logo.png" },
        @{ Source = "src-tauri/icons/Square310x310Logo.png"; Dest = "SplashScreen.png" },
        @{ Source = "src-tauri/icons/StoreLogo.png"; Dest = "StoreLogo.png" }
    )

    foreach ($icon in $iconFiles) {
        if (Test-Path -Path $icon.Source) {
            # 复制到 Assets 目录
            Copy-Item -Path $icon.Source -Destination (Join-Path -Path $assetsDir -ChildPath $icon.Dest) -Force
            Write-Success "Copied icon to Assets: $($icon.Dest)"
            
            # 同时复制到 icons 目录（用于兼容性）
            Copy-Item -Path $icon.Source -Destination (Join-Path -Path $iconsDir -ChildPath $icon.Dest) -Force
        } else {
            Write-Warning "Icon file not found: $($icon.Source)"
        }
    }

    Write-Host "`n[6/6] Copying application files..." -ForegroundColor $InfoColor
    
    $appDir = Join-Path -Path $tempDir -ChildPath "VFS\ProgramFilesX64\TPlayer"
    New-Item -ItemType Directory -Path $appDir -Force | Out-Null

    Copy-Item -Path $appExePath -Destination $appDir -Force
    Write-Success "Copied main executable: app.exe"

    $distDest = Join-Path -Path $appDir -ChildPath "dist"
    New-Item -ItemType Directory -Path $distDest -Force | Out-Null
    Copy-Item -Path (Join-Path -Path $distPath -ChildPath "*") -Destination $distDest -Recurse -Force
    Write-Success "Copied frontend assets: dist/"

    $binSrc = "src-tauri/bin"
    if (Test-Path -Path $binSrc) {
        $binDest = Join-Path -Path $appDir -ChildPath "bin"
        New-Item -ItemType Directory -Path $binDest -Force | Out-Null
        Copy-Item -Path (Join-Path -Path $binSrc -ChildPath "*") -Destination $binDest -Recurse -Force
        Write-Success "Copied binaries: bin/"
    }

    Write-Host "`n==============================================" -ForegroundColor $InfoColor
    Write-Info "Creating MSIX package..."
    
    $makeAppxPath = "MakeAppx.exe"
    if (-not (Get-Command $makeAppxPath -ErrorAction SilentlyContinue)) {
        $sdkPaths = @(
            "C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\MakeAppx.exe",
            "C:\Program Files (x86)\Windows Kits\10\bin\10.0.22000.0\x64\MakeAppx.exe"
        )
        
        foreach ($path in $sdkPaths) {
            if (Test-Path -Path $path) {
                $makeAppxPath = $path
                break
            }
        }
        
        if (-not (Test-Path -Path $makeAppxPath)) {
            throw "MakeAppx.exe not found. Please install Windows SDK or ensure it's in PATH"
        }
    }

    Write-Info "Using tool: $makeAppxPath"
    
    if (Test-Path -Path $msixPath) {
        Remove-Item -Path $msixPath -Force
        Write-Info "Removed existing MSIX file"
    }

    $process = Start-Process -FilePath $makeAppxPath -ArgumentList "pack /d `"$tempDir`" /p `"$msixPath`" /o" -Wait -NoNewWindow -PassThru
    
    if ($process.ExitCode -eq 0) {
        Write-Success "MSIX package created successfully"
    } else {
        throw "Failed to create MSIX package. Exit code: $($process.ExitCode)"
    }

    if ($CertificatePath -and $CertificatePassword) {
        Write-Host "`n==============================================" -ForegroundColor $InfoColor
        Write-Info "Signing MSIX package..."
        
        $signToolPath = "signtool.exe"
        if (-not (Get-Command $signToolPath -ErrorAction SilentlyContinue)) {
            $sdkSignToolPaths = @(
                "C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\signtool.exe",
                "C:\Program Files (x86)\Windows Kits\10\bin\10.0.22000.0\x64\signtool.exe"
            )
            
            foreach ($path in $sdkSignToolPaths) {
                if (Test-Path -Path $path) {
                    $signToolPath = $path
                    break
                }
            }
        }

        Write-Info "Using sign tool: $signToolPath"
        
        $signArgs = "sign /f `"$CertificatePath`" /p `"$CertificatePassword`" /tr http://timestamp.digicert.com /td SHA256 `"$msixPath`""
        $signProcess = Start-Process -FilePath $signToolPath -ArgumentList $signArgs -Wait -NoNewWindow -PassThru
        
        if ($signProcess.ExitCode -eq 0) {
            Write-Success "MSIX package signed successfully"
        } else {
            Write-Warning "Signing failed. Exit code: $($signProcess.ExitCode)"
        }
    } else {
        Write-Warning "No certificate provided. MSIX package is not signed (for testing only)"
    }

    Write-Host "`n==============================================" -ForegroundColor $InfoColor
    Write-Info "Cleaning up temporary directory..."
    Remove-Item -Path $tempDir -Recurse -Force
    Write-Success "Temporary directory cleaned"

    Write-Host "`n==============================================" -ForegroundColor $SuccessColor
    Write-Success "MSIX Packaging Completed!"
    Write-Host "----------------------------------------------" -ForegroundColor $InfoColor
    Write-Host "Output File: $msixPath" -ForegroundColor $InfoColor
    Write-Host "File Size: $(Get-Item $msixPath | Select-Object -ExpandProperty Length)" -ForegroundColor $InfoColor
    Write-Host "`nNext Steps:" -ForegroundColor $InfoColor
    Write-Host "1. Upload MSIX package to Microsoft Store" -ForegroundColor $InfoColor
    Write-Host "2. Sign package with Store certificate (recommended)" -ForegroundColor $InfoColor
    Write-Host "3. Test installation on target machines" -ForegroundColor $InfoColor
    Write-Host "==============================================`n" -ForegroundColor $SuccessColor

} catch {
    Write-Error "Error during packaging: $_"
    
    if ($tempDir -and (Test-Path -Path $tempDir)) {
        Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    
    exit 1
}