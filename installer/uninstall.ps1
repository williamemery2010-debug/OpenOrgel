<#
.SYNOPSIS
    OpenOrgel Windows Application Uninstaller
#>

[CmdletBinding()]
param(
    [switch]$KeepSettings
)

$InstallPath = "$env:LOCALAPPDATA\OpenOrgel"
Write-Host "=================================================================" -ForegroundColor Cyan
Write-Host "           OpenOrgel Windows Application Uninstaller             " -ForegroundColor White
Write-Host "=================================================================" -ForegroundColor Cyan

# Remove shortcuts
$DesktopShortcut = Join-Path ([Environment]::GetFolderPath("Desktop")) "OpenOrgel.lnk"
if (Test-Path $DesktopShortcut) {
    Remove-Item $DesktopShortcut -Force
    Write-Host "Removed Desktop shortcut." -ForegroundColor Gray
}

$StartMenuDir = Join-Path ([Environment]::GetFolderPath("Programs")) "OpenOrgel"
if (Test-Path $StartMenuDir) {
    Remove-Item $StartMenuDir -Recurse -Force
    Write-Host "Removed Start Menu shortcuts." -ForegroundColor Gray
}

# Remove Registry uninstall entry
$RegPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenOrgel"
if (Test-Path $RegPath) {
    Remove-Item $RegPath -Recurse -Force
    Write-Host "Unregistered from Windows Installed Apps." -ForegroundColor Gray
}

# Remove install directory files
if (Test-Path $InstallPath) {
    if ($KeepSettings -and (Test-Path (Join-Path $InstallPath "settings.json"))) {
        $temp = Join-Path $env:TEMP "openorgel_settings_backup.json"
        Copy-Item (Join-Path $InstallPath "settings.json") $temp -Force
        Remove-Item $InstallPath -Recurse -Force
        New-Item -ItemType Directory -Path $InstallPath -Force | Out-Null
        Copy-Item $temp (Join-Path $InstallPath "settings.json") -Force
        Write-Host "Removed application files (settings preserved)." -ForegroundColor Yellow
    } else {
        Remove-Item $InstallPath -Recurse -Force
        Write-Host "Removed application files." -ForegroundColor Yellow
    }
}

Write-Host "OpenOrgel has been successfully uninstalled from your computer." -ForegroundColor Green
Write-Host "=================================================================" -ForegroundColor Cyan
