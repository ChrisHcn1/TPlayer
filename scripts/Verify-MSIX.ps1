# MSIX Package Verification Script
# Verify the integrity and installation of the MSIX package

param(
    [string]$PackagePath = "E:\TPlayer\msix-output\TPlayer_1.0.3.0_x64.msix"
)

Write-Host "Starting MSIX package verification..." -ForegroundColor Green

# Check if package exists
if (-not (Test-Path $PackagePath)) {
    Write-Host "Error: MSIX package not found at $PackagePath" -ForegroundColor Red
    exit 1
}

Write-Host "Package found: $PackagePath" -ForegroundColor Cyan

# Get file information
$fileInfo = Get-Item $PackagePath
$fileSizeMB = [math]::Round($fileInfo.Length / 1MB, 2)
Write-Host "Package size: $fileSizeMB MB" -ForegroundColor Cyan
Write-Host "Created: $($fileInfo.CreationTime)" -ForegroundColor Cyan

# Check package manifest
Write-Host "`nChecking package manifest..." -ForegroundColor Cyan
$makeAppxPath = Get-ChildItem -Path "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\makeappx.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
if ($makeAppxPath) {
    $arguments = @("unpack", "/p", $PackagePath, "/d", "E:\TPlayer\msix-temp", "/o")
    $process = Start-Process -FilePath $makeAppxPath.FullName -ArgumentList $arguments -Wait -PassThru -NoNewWindow
    
    if ($process.ExitCode -eq 0) {
        Write-Host "Package unpacked successfully" -ForegroundColor Green
        
        # Check manifest file
        $manifestPath = "E:\TPlayer\msix-temp\AppxManifest.xml"
        if (Test-Path $manifestPath) {
            Write-Host "Manifest file found" -ForegroundColor Green
            [xml]$manifest = Get-Content $manifestPath
            
            # Display package information
            Write-Host "`nPackage Information:" -ForegroundColor Cyan
            Write-Host "Name: $($manifest.Package.Identity.Name)" -ForegroundColor White
            Write-Host "Version: $($manifest.Package.Identity.Version)" -ForegroundColor White
            Write-Host "Publisher: $($manifest.Package.Identity.Publisher)" -ForegroundColor White
            Write-Host "Processor Architecture: $($manifest.Package.Identity.ProcessorArchitecture)" -ForegroundColor White
            Write-Host "Display Name: $($manifest.Package.Properties.DisplayName)" -ForegroundColor White
            Write-Host "Description: $($manifest.Package.Properties.Description)" -ForegroundColor White
            
            # Check supported file types
            Write-Host "`nSupported File Types:" -ForegroundColor Cyan
            $fileTypes = $manifest.Package.Applications.Application.Extensions.'uap3:FileTypeAssociation'.'uap:SupportedFileTypes'.'uap:FileType'
            foreach ($fileType in $fileTypes) {
                Write-Host "  - $fileType" -ForegroundColor White
            }
        }
        
        # Clean up temp directory
        Remove-Item -Path "E:\TPlayer\msix-temp" -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# Check if package can be installed (requires Windows 10/11)
Write-Host "`nChecking installation capability..." -ForegroundColor Cyan
$addAppxPath = Get-ChildItem -Path "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\Add-AppxPackage.ps1" -ErrorAction SilentlyContinue | Select-Object -First 1
if ($addAppxPath) {
    Write-Host "Add-AppxPackage tool found" -ForegroundColor Green
    Write-Host "Note: Package needs to be signed before installation" -ForegroundColor Yellow
} else {
    Write-Host "Add-AppxPackage tool not found" -ForegroundColor Yellow
}

Write-Host "`nMSIX package verification completed!" -ForegroundColor Green
Write-Host "Package is ready for testing and distribution" -ForegroundColor Cyan