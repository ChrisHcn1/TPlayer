<#
.SYNOPSIS
Sign MSIX package with a test certificate

.DESCRIPTION
This script creates a self-signed certificate and signs the MSIX package for testing purposes.
For production/Microsoft Store submission, use the certificate provided by Microsoft.

.PARAMETER MsixPath
Path to the MSIX package

.PARAMETER CertificatePassword
Password for the certificate

.EXAMPLE
.\Sign-MSIX.ps1 -MsixPath "src-tauri/target/release/bundle/msix/TPlayer_1.0.2.0.msix" -CertificatePassword "test1234"
#>

param(
    [string]$MsixPath = "src-tauri/target/release/bundle/msix/TPlayer_1.0.2.0.msix",
    [string]$CertificatePassword = "test1234"
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

try {
    Write-Host "`n==============================================" -ForegroundColor $InfoColor
    Write-Host "        MSIX Signing Script (Test Only)" -ForegroundColor $InfoColor
    Write-Host "==============================================`n" -ForegroundColor $InfoColor

    if (-not (Test-Path -Path $MsixPath)) {
        throw "MSIX package not found: $MsixPath"
    }
    Write-Success "MSIX package: Found"

    Write-Host "`n[1/3] Creating self-signed certificate..." -ForegroundColor $InfoColor
    
    $certSubject = "CN=TPlayer Test Publisher"
    $cert = New-SelfSignedCertificate -Type Custom -Subject $certSubject -KeyUsage DigitalSignature -FriendlyName "TPlayer Test Certificate" -CertStoreLocation "Cert:\CurrentUser\My" -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3")
    
    if (-not $cert) {
        throw "Failed to create certificate"
    }
    Write-Success "Certificate created: $($cert.Thumbprint)"

    Write-Host "`n[2/3] Exporting certificate to PFX..." -ForegroundColor $InfoColor
    
    $certPath = "test-cert.pfx"
    if (Test-Path -Path $certPath) {
        Remove-Item -Path $certPath -Force
    }
    
    $securePassword = ConvertTo-SecureString $CertificatePassword -AsPlainText -Force
    Export-PfxCertificate -Cert $cert -FilePath $certPath -Password $securePassword
    Write-Success "Certificate exported: $certPath"

    Write-Host "`n[3/3] Signing MSIX package..." -ForegroundColor $InfoColor
    
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
    
    $signArgs = "sign /f `"$certPath`" /p `"$CertificatePassword`" /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 `"$MsixPath`""
    $signProcess = Start-Process -FilePath $signToolPath -ArgumentList $signArgs -Wait -NoNewWindow -PassThru
    
    if ($signProcess.ExitCode -eq 0) {
        Write-Success "MSIX package signed successfully"
    } else {
        throw "Signing failed. Exit code: $($signProcess.ExitCode)"
    }

    Write-Host "`n==============================================" -ForegroundColor $SuccessColor
    Write-Success "MSIX Signing Completed!"
    Write-Host "----------------------------------------------" -ForegroundColor $InfoColor
    Write-Host "Package: $MsixPath" -ForegroundColor $InfoColor
    Write-Host "Certificate: $certPath" -ForegroundColor $InfoColor
    Write-Host "`nNote: This is a TEST certificate. For Microsoft Store submission," -ForegroundColor $WarningColor
    Write-Host "use the certificate provided by Microsoft Partner Center." -ForegroundColor $WarningColor
    Write-Host "==============================================`n" -ForegroundColor $SuccessColor

} catch {
    Write-Error "Error during signing: $_"
    exit 1
}
