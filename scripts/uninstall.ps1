# M31A Uninstaller for Windows (PowerShell)
# Usage: scripts\uninstall.ps1 [-Channel production|development|all] [-Purge] [-Yes]
#
# Removes the `m31a.exe` binary installed by scripts/install.ps1 from
# %LOCALAPPDATA%\Programs\m31a and cleans the user PATH entry.
# (Development-channel `m31a-dev.exe` is removed from the same directory
# when present and the channel selection includes it.)
#
# User state (%APPDATA%\m31a[-dev], %LOCALAPPDATA%\m31a[-dev]) is PRESERVED
# by default. Pass -Purge to delete it (prompts unless -Yes).
# Project-local `.m31a\` workspace directories are never touched.

param(
    [ValidateSet("production", "development", "all")]
    [string]$Channel = "all",
    [switch]$Purge,
    [switch]$Yes
)

$ErrorActionPreference = "Stop"

$installDir = Join-Path $env:LOCALAPPDATA "Programs\m31a"
$removed = 0

Write-Host "==> Removing M31A binaries (channel=$Channel)..."
$binNames = @()
if ($Channel -eq "production" -or $Channel -eq "all") { $binNames += "m31a.exe" }
if ($Channel -eq "development" -or $Channel -eq "all") { $binNames += "m31a-dev.exe" }

foreach ($bin in $binNames) {
    $binPath = Join-Path $installDir $bin
    if (Test-Path $binPath) {
        Remove-Item -Path $binPath -Force
        Write-Host "    removed $binPath"
        $removed++
    }
}

# Remove the install dir from the user PATH if it is now empty of our binaries.
$remaining = @(Get-ChildItem -Path $installDir -Filter "m31a*.exe" -ErrorAction SilentlyContinue)
if ($removed -gt 0 -and $remaining.Count -eq 0) {
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($userPath -like "*$installDir*") {
        $entries = $userPath -split ";" | Where-Object { $_ -ne "" -and $_ -ne $installDir }
        [Environment]::SetEnvironmentVariable("Path", ($entries -join ";"), "User")
        Write-Host "    removed $installDir from user PATH."
    }
    if ((Test-Path $installDir) -and @(Get-ChildItem -Path $installDir -Force | Where-Object { $_.Name -notlike "m31a*" }).Count -eq 0 -and (Get-ChildItem -Path $installDir -Force).Count -eq 0) {
        Remove-Item -Path $installDir -Force
        Write-Host "    removed empty directory $installDir."
    }
}

if ($removed -eq 0) {
    Write-Host "    no M31A binaries found at $installDir."
}

if ($Purge) {
    $appDirs = @()
    if ($Channel -eq "production" -or $Channel -eq "all") { $appDirs += "m31a" }
    if ($Channel -eq "development" -or $Channel -eq "all") { $appDirs += "m31a-dev" }

    $targets = @()
    foreach ($leaf in $appDirs) {
        foreach ($parent in @($env:APPDATA, $env:LOCALAPPDATA)) {
            $candidate = Join-Path $parent $leaf
            if (Test-Path $candidate) { $targets += $candidate }
        }
    }
    # Channel env overrides (exact directories only).
    foreach ($var in @("M31A_CONFIG_DIR", "M31A_DATA_DIR", "M31A_CACHE_DIR", "M31A_STATE_DIR",
                       "M31A_DEV_CONFIG_DIR", "M31A_DEV_DATA_DIR", "M31A_DEV_CACHE_DIR", "M31A_DEV_STATE_DIR")) {
        $val = [Environment]::GetEnvironmentVariable($var)
        if ($val -and (Test-Path $val)) { $targets += $val }
    }
    $targets = @($targets | Select-Object -Unique)

    if ($targets.Count -eq 0) {
        Write-Host "==> -Purge: no M31A state directories found."
    } else {
        Write-Host "==> -Purge: the following state directories will be permanently deleted:"
        foreach ($t in $targets) { Write-Host "    $t" }
        if (-not $Yes) {
            $answer = Read-Host "Delete these directories? [y/N]"
            if ($answer -notin @("y", "Y", "yes", "YES")) {
                Write-Host "Aborted; binaries were still removed, state preserved."
                return
            }
        }
        foreach ($t in $targets) {
            Remove-Item -Path $t -Recurse -Force
            Write-Host "    purged $t."
        }
    }
} else {
    Write-Host "User state preserved (config/data/cache/state untouched). Re-run with -Purge to remove it."
}

Write-Host "`n==> Uninstall complete."
