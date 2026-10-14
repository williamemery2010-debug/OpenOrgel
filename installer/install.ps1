<#
.SYNOPSIS
    OpenOrgel Windows Application Installer
.DESCRIPTION
    Installs OpenOrgel Virtual Pipe Organ Synthesis Engine, creates Desktop
    and Start Menu shortcuts, and registers OpenOrgel in Windows Installed Apps.
#>

[CmdletBinding()]
param(
    [string]$InstallPath = "$env:LOCALAPPDATA\OpenOrgel",
    [switch]$NoShortcuts,
    [switch]$Silent
)

$ErrorActionPreference = "Stop"

Write-Host "=================================================================" -ForegroundColor Cyan
Write-Host "             OpenOrgel Windows Application Installer             " -ForegroundColor White
Write-Host "         Virtual Pipe Organ Synthesis Engine (v2.0.x)           " -ForegroundColor Cyan
Write-Host "=================================================================" -ForegroundColor Cyan
Write-Host ""

# Determine source directory where installer is running from
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$ProjectRoot = Split-Path -Parent $ScriptDir

# Locate executable
$ExeSource = $null
$PotentialPaths = @(
    Join-Path $ScriptDir "openorgelsynth.exe",
    Join-Path $ProjectRoot "rust\target\x86_64-pc-windows-gnullvm\release\openorgelsynth.exe",
    Join-Path $ProjectRoot "rust\target\release\openorgelsynth.exe",
    "C:\Users\$env:USERNAME\OneDrive - St Helena Secondary College\Desktop\openorgelsynth.exe"
)

foreach ($path in $PotentialPaths) {
    if (Test-Path $path) {
        $ExeSource = $path
        break
    }
}

if (-not $ExeSource) {
    Write-Error "Could not find 'openorgelsynth.exe'. Please ensure the release binary is compiled before installing."
    exit 1
}

Write-Host "Source binary: $ExeSource" -ForegroundColor Gray
Write-Host "Target directory: $InstallPath" -ForegroundColor Green

# Create installation directory
if (-not (Test-Path $InstallPath)) {
    New-Item -ItemType Directory -Path $InstallPath -Force | Out-Null
}

# Copy binary
Write-Host "Installing executable..." -ForegroundColor Yellow
$TargetExe = Join-Path $InstallPath "openorgelsynth.exe"
Copy-Item -Path $ExeSource -Destination $TargetExe -Force

# Copy audio samples directory if found
$SamplesSrc = Join-Path $ProjectRoot "samples"
if (Test-Path $SamplesSrc) {
    Copy-Item -Path $SamplesSrc -Destination $InstallPath -Recurse -Force
}
foreach ($sample in @("clarion.wav", "stoppedflue.wav")) {
    $src = Join-Path $ProjectRoot $sample
    if (Test-Path $src) {
        Copy-Item -Path $src -Destination (Join-Path $InstallPath $sample) -Force
    }
}

# Copy documentation
foreach ($doc in @("LICENSE", "README.md", "unit_tests_report.md")) {
    $src = Join-Path $ProjectRoot $doc
    if (Test-Path $src) {
        Copy-Item -Path $src -Destination (Join-Path $InstallPath $doc) -Force
    }
}

# Install uninstaller script into installation directory
$UninstallScriptPath = Join-Path $InstallPath "uninstall.ps1"
$UninstallScriptContent = @"
# OpenOrgel Uninstaller
`$InstallPath = "$InstallPath"
Write-Host "Uninstalling OpenOrgel from `$InstallPath..." -ForegroundColor Yellow

# Remove shortcuts
`$DesktopShortcut = Join-Path `([Environment]::GetFolderPath("Desktop")`) "OpenOrgel.lnk"
if (Test-Path `$DesktopShortcut) { Remove-Item `$DesktopShortcut -Force }

`$StartMenuDir = Join-Path `([Environment]::GetFolderPath("Programs")`) "OpenOrgel"
if (Test-Path `$StartMenuDir) { Remove-Item `$StartMenuDir -Recurse -Force }

# Remove Registry uninstall entry
`$RegPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenOrgel"
if (Test-Path `$RegPath) { Remove-Item `$RegPath -Recurse -Force }

# Remove install directory files
if (Test-Path `$InstallPath) {
    Remove-Item `$InstallPath -Recurse -Force
}

Write-Host "OpenOrgel has been successfully uninstalled." -ForegroundColor Green
"@
Set-Content -Path $UninstallScriptPath -Value $UninstallScriptContent -Encoding UTF8

# Create Shortcuts
if (-not $NoShortcuts) {
    Write-Host "Creating shortcuts..." -ForegroundColor Yellow
    $WshShell = New-Object -ComObject WScript.Shell

    # Desktop shortcut
    $DesktopDir = [Environment]::GetFolderPath("Desktop")
    $DesktopShortcutPath = Join-Path $DesktopDir "OpenOrgel.lnk"
    $Shortcut = $WshShell.CreateShortcut($DesktopShortcutPath)
    $Shortcut.TargetPath = $TargetExe
    $Shortcut.WorkingDirectory = $InstallPath
    $Shortcut.Description = "OpenOrgel - Virtual Pipe Organ Synthesis Engine"
    $Shortcut.Save()

    # Start Menu shortcut
    $ProgramsDir = [Environment]::GetFolderPath("Programs")
    $StartMenuDir = Join-Path $ProgramsDir "OpenOrgel"
    if (-not (Test-Path $StartMenuDir)) {
        New-Item -ItemType Directory -Path $StartMenuDir -Force | Out-Null
    }

    $StartShortcutPath = Join-Path $StartMenuDir "OpenOrgel.lnk"
    $Shortcut = $WshShell.CreateShortcut($StartShortcutPath)
    $Shortcut.TargetPath = $TargetExe
    $Shortcut.WorkingDirectory = $InstallPath
    $Shortcut.Description = "OpenOrgel - Virtual Pipe Organ Synthesis Engine"
    $Shortcut.Save()

    # Start Menu Uninstaller shortcut
    $UninstallerShortcutPath = Join-Path $StartMenuDir "Uninstall OpenOrgel.lnk"
    $Shortcut = $WshShell.CreateShortcut($UninstallerShortcutPath)
    $Shortcut.TargetPath = "powershell.exe"
    $Shortcut.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$UninstallScriptPath`""
    $Shortcut.WorkingDirectory = $InstallPath
    $Shortcut.Description = "Uninstall OpenOrgel"
    $Shortcut.Save()
}

# Register in Windows Add/Remove Programs (HKCU uninstall key requires no admin privileges)
Write-Host "Registering in Windows Installed Apps..." -ForegroundColor Yellow
$RegKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenOrgel"
if (-not (Test-Path $RegKey)) {
    New-Item -Path $RegKey -Force | Out-Null
}
Set-ItemProperty -Path $RegKey -Name "DisplayName" -Value "OpenOrgel Virtual Pipe Organ" -Force
Set-ItemProperty -Path $RegKey -Name "DisplayVersion" -Value "2.0.5" -Force
Set-ItemProperty -Path $RegKey -Name "Publisher" -Value "William Emery" -Force
Set-ItemProperty -Path $RegKey -Name "InstallLocation" -Value $InstallPath -Force
Set-ItemProperty -Path $RegKey -Name "DisplayIcon" -Value "$TargetExe,0" -Force
Set-ItemProperty -Path $RegKey -Name "UninstallString" -Value "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$UninstallScriptPath`"" -Force
Set-ItemProperty -Path $RegKey -Name "NoModify" -Value 1 -Type DWord -Force
Set-ItemProperty -Path $RegKey -Name "NoRepair" -Value 1 -Type DWord -Force

Write-Host ""
Write-Host "Installation Complete!" -ForegroundColor Green
Write-Host "OpenOrgel is now installed at: $InstallPath" -ForegroundColor White
Write-Host "You can launch OpenOrgel from your Desktop or Start Menu." -ForegroundColor White
Write-Host "=================================================================" -ForegroundColor Cyan
