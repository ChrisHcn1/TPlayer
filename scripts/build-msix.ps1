# build-msix.ps1 -- Build a Microsoft Store submittable .msix package for TPlayer.
#
# WHY MSIX (and not the MSI we built earlier):
#   The Store has two submission types with different rules.
#     - "Packaged app (MSIX/AppX)": you UPLOAD the package; the Store re-signs it
#       with its own Microsoft certificate -> NO CA code-signing certificate needed.
#     - "EXE or MSI app": you do NOT upload a file, you provide an HTTPS download URL,
#       and the binary MUST be signed with a certificate chaining to a CA in the
#       Microsoft Trusted Root Program -> a self-signed cert is rejected.
#   Tauri v2 cannot emit MSIX (tauri-bundler has no msix module), so we build the
#   package by hand with the Windows SDK's makeappx.exe, using the same on-disk
#   layout the MSI/NSIS installer produces:
#       TPlayer.exe            (frontend assets are embedded into the exe)
#       bin\ffmpeg.exe         (bundle resources, mapped by tauri.conf.json)
#       bin\ffprobe.exe
#       bin\sacd_extract.exe
#       Assets\*.png           (icon set already ships UWP/MSIX-sized PNGs)
#       AppxManifest.xml
#
# Partner Center must already have RESERVED the app for this to be accepted:
#   - Identity/Name     = the reserved app name (e.g. TPlayer)
#   - Identity/Publisher = "CN=<publisher GUID>" shown in Partner Center > App
#     management > App identity. It MUST match the Store account exactly.
#   The package identity is what the Store checks, so rebuild with the real values
#   before submitting (see -IdentityName / -Publisher below).
#
# Usage (from repo root):
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\build-msix.ps1 `
#       -IdentityName TPlayer -Publisher "CN=1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d" `
#       -Version 2.0.3.0
#
# Output: src-tauri\target\release\bundle\msix\TPlayer_<version>_x64.msix

[CmdletBinding()]
param(
    # Store app name (must equal the reserved name in Partner Center)
    [string]$IdentityName = 'TPlayer',
    # "CN=<publisher GUID>" from Partner Center > App management > App identity
    [string]$Publisher = 'CN=00000000-0000-0000-0000-000000000000',
    # 4-part numeric version, e.g. 2.0.3.0
    [string]$Version = '2.0.3.0',
    [string]$DisplayName = 'TPlayer',
    [string]$PublisherDisplayName = 'TPlayer'
)

$ErrorActionPreference = 'Stop'
$root       = Split-Path -Parent $PSScriptRoot
$srcTauri   = Join-Path $root 'src-tauri'
$target     = Join-Path $srcTauri 'target'
$releaseDir = Join-Path $target 'release'
$payloadDir = Join-Path $target 'msix-payload'
$outDir     = Join-Path $releaseDir 'bundle\msix'
$iconsDir   = Join-Path $srcTauri 'icons'
$binDir     = Join-Path $root 'bin'

# ---------------------------------------------------------------- preflight --
function Fail($msg) { Write-Host "[msix] ERROR: $msg" -ForegroundColor Red; exit 1 }

$sdkBins = @(
    'C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64',
    'C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64',
    'C:\Program Files (x86)\Windows Kits\10\bin\10.0.17134.0\x64'
)
$makeappx = $null; $signtool = $null
foreach ($b in $sdkBins) {
    if (-not $makeappx -and (Test-Path (Join-Path $b 'makeappx.exe'))) { $makeappx = Join-Path $b 'makeappx.exe' }
    if (-not $signtool -and (Test-Path (Join-Path $b 'signtool.exe'))) { $signtool = Join-Path $b 'signtool.exe' }
}
if (-not $makeappx) { Fail 'makeappx.exe not found (install "Windows SDK" with the MSIX tooling)' }
if (-not $signtool) { Fail 'signtool.exe not found (install "Windows SDK" with the MSIX tooling)' }
if ($Publisher -match '00000000-0000-0000-0000-000000000000') {
    Write-Host '[msix] WARNING: using the PLACEHOLDER publisher. The Store will reject this package.' -ForegroundColor Yellow
    Write-Host '[msix]          Pass -Publisher "CN=<your Partner Center publisher GUID>" before submitting.' -ForegroundColor Yellow
}

$appExe = Join-Path $releaseDir 'TPlayer.exe'
if (-not (Test-Path $appExe)) { Fail "TPlayer.exe not found in $releaseDir -- run 'npm run tauri -- build' first" }
foreach ($tool in @('ffmpeg.exe', 'ffprobe.exe', 'sacd_extract.exe')) {
    if (-not (Test-Path (Join-Path $binDir $tool))) { Fail "missing bundle resource bin\$tool" }
}

# ------------------------------------------------------------------ payload --
Write-Host '[msix] Preparing payload...'
if (Test-Path $payloadDir) { Remove-Item $payloadDir -Recurse -Force }
New-Item -ItemType Directory -Force -Path $payloadDir | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $payloadDir 'bin') | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $payloadDir 'Assets') | Out-Null

Copy-Item $appExe (Join-Path $payloadDir 'TPlayer.exe') -Force
foreach ($tool in @('ffmpeg.exe', 'ffprobe.exe', 'sacd_extract.exe')) {
    Copy-Item (Join-Path $binDir $tool) (Join-Path $payloadDir "bin\$tool") -Force
}

# Icon set: the repo already ships MSIX-named PNGs (Square44x44Logo, StoreLogo, ...).
$assetNames = @(
    'Square44x44Logo.png', 'Square71x71Logo.png', 'Square89x89Logo.png',
    'Square107x107Logo.png', 'Square142x142Logo.png', 'Square150x150Logo.png',
    'Square284x284Logo.png', 'Square310x310Logo.png', 'StoreLogo.png'
)
foreach ($a in $assetNames) {
    $src = Join-Path $iconsDir $a
    if (Test-Path $src) { Copy-Item $src (Join-Path $payloadDir "Assets\$a") -Force }
}
# Wide310x150Logo is not shipped in the repo -> render it from the main icon.
$wide = Join-Path $payloadDir 'Assets\Wide310x150Logo.png'
if (-not (Test-Path $wide)) {
    $ffmpeg = Join-Path $binDir 'ffmpeg.exe'
    & $ffmpeg -y -loglevel error -i (Join-Path $iconsDir 'icon.png') `
        -vf "scale=310:150:force_original_aspect_ratio=decrease,pad=310:150:(ow-iw)/2:(oh-ih)/2:color=black@0,format=rgba" `
        -frames:v 1 $wide
}

# ------------------------------------------------------------------ manifest --
$description = 'TPlayer 是本地优先的桌面音乐播放器：曲库数据与在线能力解耦，在线模块默认关闭且默认不上传本地曲库信息。'
$manifest = @"
<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap rescap">
  <Identity Name="$IdentityName" Publisher="$Publisher" Version="$Version" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>$DisplayName</DisplayName>
    <PublisherDisplayName>$PublisherDisplayName</PublisherDisplayName>
    <Description>$description</Description>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.22621.0" />
  </Dependencies>
  <Resources>
    <Resource Language="zh-CN" />
    <Resource Language="en-US" />
  </Resources>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
  <Applications>
    <Application Id="App" Executable="TPlayer.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements
        DisplayName="$DisplayName"
        Description="$description"
        BackgroundColor="transparent"
        Square44x44Logo="Assets\Square44x44Logo.png"
        Square150x150Logo="Assets\Square150x150Logo.png">
        <uap:DefaultTile
          Square71x71Logo="Assets\Square71x71Logo.png"
          Square310x310Logo="Assets\Square310x310Logo.png"
          Wide310x150Logo="Assets\Wide310x150Logo.png" />
      </uap:VisualElements>
    </Application>
  </Applications>
</Package>
"@
$manifestPath = Join-Path $payloadDir 'AppxManifest.xml'
# BOM would break the XML parser in makeappx
[System.IO.File]::WriteAllText($manifestPath, $manifest, (New-Object System.Text.UTF8Encoding($false)))

# ---------------------------------------------------------------------- pack --
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$msixPath = Join-Path $outDir ("TPlayer_{0}_x64.msix" -f $Version)
if (Test-Path $msixPath) { Remove-Item $msixPath -Force }

Write-Host "[msix] Packing -> $msixPath"
& $makeappx pack /d $payloadDir /p $msixPath /o
if ($LASTEXITCODE -ne 0) { Fail "makeappx pack failed (exit $LASTEXITCODE)" }

# --------------------------------------------------------------------- sign --
# The Store re-signs MSIX packages with its own certificate, but the package we
# upload must already carry a valid signature. A self-signed cert is fine here
# (it only has to satisfy the package format); it would NOT be fine for the
# "EXE or MSI app" route, which demands a CA-issued certificate.
# The APPX signing API requires the signer certificate Subject to match
# Identity/Publisher exactly -- so the cert Subject is derived from -Publisher.
$certSubject = $Publisher
$cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object {
    $_.Subject -eq $certSubject -and $_.EnhancedKeyUsageList -match 'Code Signing'
} | Select-Object -First 1
if (-not $cert) {
    Write-Host "[msix] Creating self-signed packaging cert: $certSubject"
    $cert = New-SelfSignedCertificate -Subject $certSubject -Type CodeSigning `
        -CertStoreLocation Cert:\CurrentUser\My -KeyUsage DigitalSignature `
        -NotAfter (Get-Date).AddYears(5)
}
$pfx = Join-Path $target 'msix-packaging.pfx'
$pfxPass = 'tplayer-msix'
if (Test-Path $pfx) { Remove-Item $pfx -Force }
Export-PfxCertificate -Cert $cert -FilePath $pfx -Password (ConvertTo-SecureString $pfxPass -AsPlainText -Force) | Out-Null

Write-Host '[msix] Signing package (self-signed; the Store re-signs on submission)...'
# Sign straight from the certificate store (thumbprint-pinned) -- more reliable than
# the .pfx round-trip, which intermittently fails with "SignerSign() failed".
$signed = $false
$signVariants = @(
    @('/fd', 'SHA256', '/td', 'SHA256', '/tr', 'http://timestamp.digicert.com'),
    @('/fd', 'SHA256')
)
foreach ($signArgs in $signVariants) {
    & $signtool sign @signArgs /a /sha1 $cert.Thumbprint $msixPath
    if ($LASTEXITCODE -eq 0) { $signed = $true; break }
    Write-Host '[msix] Signing attempt failed -- trying the next variant...' -ForegroundColor Yellow
}
if (-not $signed) {
    # last resort: the exported .pfx
    & $signtool sign /fd SHA256 /f $pfx /p $pfxPass $msixPath
    if ($LASTEXITCODE -ne 0) { Fail "signtool failed (exit $LASTEXITCODE)" }
}

# ------------------------------------------------------------------ verify --
# NOTE: this SDK build of makeappx has no `validate` sub-command, so verification is
# done with signtool (signature chain + timestamp) plus a package content listing.
# makeappx pack already performed manifest schema validation and checked that every
# file referenced by the manifest exists.
Write-Host '[msix] Verifying signature...'
# signtool always walks the certificate chain, and the packaging cert is
# self-signed, so it reports "not trusted by the trust provider". That is
# EXPECTED here: the Store re-signs the package with its own Microsoft
# certificate on submission. Only a genuinely broken signature is a problem.
$prevEA = $ErrorActionPreference
$ErrorActionPreference = 'Continue'   # signtool writes diagnostics to stderr
$verifyOut = (& $signtool verify /v $msixPath 2>&1 | Out-String)
$verifyExit = $LASTEXITCODE
$ErrorActionPreference = $prevEA
$verifyOut -split "`r?`n" | Where-Object { $_ -match 'SignTool|warning|error|timestamped' } | ForEach-Object { Write-Host "         $_" }
if ($verifyExit -ne 0 -and $verifyOut -notmatch 'not trusted by the trust provider') {
    Write-Host '[msix] WARNING: signature verification failed for a reason other than the untrusted self-signed root.' -ForegroundColor Yellow
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::OpenRead($msixPath)
$entries = $zip.Entries | ForEach-Object { $_.FullName }
$zip.Dispose()

Write-Host '[msix] package content:'
$entries | ForEach-Object { Write-Host "         $_" }
foreach ($required in @('AppxManifest.xml', 'AppxBlockMap.xml', '[Content_Types].xml', 'AppxSignature.p7x', 'TPlayer.exe')) {
    if ($entries -notcontains $required) {
        Write-Host "[msix] WARNING: package is missing $required" -ForegroundColor Yellow
        $verifyExit = 1
    }
}

Write-Host ("[msix] package size     : {0} MB" -f [math]::Round((Get-Item $msixPath).Length / 1MB, 1))
Write-Host ("[msix] package          : {0}" -f $msixPath)
Write-Host '[msix] Done.'
