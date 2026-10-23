# build-store.ps1 -- Build the Microsoft Store submission package (signed, offline MSI).
#
# Tauri v2 does NOT produce MSIX. For the Microsoft Store, a Tauri app is submitted as an
# "EXE or MSI app": an offline, silently-installable, code-signed installer. This script:
#   1. Ensures a self-signed code-signing certificate exists in CurrentUser\My (for local
#      testing / producing a submittable package). For real publication, replace it with a
#      CA-issued code-signing cert (see the Store checklist in chat).
#   2. Reads src-tauri/tauri.microsoftstore.conf.json and injects the cert thumbprint into a
#      temporary override config (machine-specific; not committed).
#   3. Runs `tauri build --config <override>` which builds the offline (WebView2 bundled) MSI.
#
# The resulting MSI supports silent install via `msiexec /i TPlayer.msi /quiet` (Partner
# Center silent-install argument). It is code-signed so Windows SmartScreen / Store cert
# steps are satisfiable.
#
# NOTE: run from the repository root.

$ErrorActionPreference = 'Stop'

$storeConfig = Join-Path $PSScriptRoot '..\src-tauri\tauri.microsoftstore.conf.json'
$override   = Join-Path $PSScriptRoot '..\src-tauri\tauri.store.override.json'

# 1. Reuse or create the self-signed code-signing certificate.
$certSubject = 'CN=TPlayer Store Packaging'
$existing = Get-ChildItem Cert:\CurrentUser\My | Where-Object {
    $_.Subject -eq $certSubject -and $_.EnhancedKeyUsageList -match 'Code Signing'
}
if ($existing) {
    $cert = $existing | Select-Object -First 1
    Write-Host "[store] Reusing packaging cert: $certSubject ($($cert.Thumbprint))"
} else {
    Write-Host "[store] Creating self-signed code-signing cert: $certSubject"
    $cert = New-SelfSignedCertificate `
        -Subject $certSubject `
        -Type CodeSigning `
        -CertStoreLocation Cert:\CurrentUser\My `
        -KeyUsage DigitalSignature `
        -NotAfter (Get-Date).AddYears(5)
    Write-Host "[store] Created cert, thumbprint: $($cert.Thumbprint)"
}
$thumb = $cert.Thumbprint

# 2. Inject the thumbprint into a temporary override config (merge over the static store config).
$cfg = Get-Content $storeConfig -Raw | ConvertFrom-Json
if (-not $cfg.bundle) { $cfg | Add-Member -NotePropertyName bundle -NotePropertyValue ([pscustomobject]::new()) }
if (-not $cfg.bundle.windows) { $cfg.bundle | Add-Member -NotePropertyName windows -NotePropertyValue ([pscustomobject]::new()) }
$cfg.bundle.windows | Add-Member -NotePropertyName certificateThumbprint -NotePropertyValue $thumb -Force
$cfg | ConvertTo-Json -Depth 10 | Set-Content -Path $override -Encoding utf8
Write-Host "[store] Wrote override config: $override (certificateThumbprint=$thumb)"

# 3. Build the offline MSI.
try {
    Write-Host '[store] Building Store MSI (release + frontend + WebView2 offline bundle, takes a while)...'
    npm run tauri -- build --config src-tauri/tauri.store.override.json
    Write-Host '[store] MSI build done. Artifact in src-tauri\target\release\bundle\msi\'
} finally {
    if (Test-Path $override) { Remove-Item $override -Force }
    Write-Host '[store] Cleaned up temporary override config.'
}
