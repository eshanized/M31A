# M31A Standalone Binary Installer for Windows (PowerShell)
# Usage: irm https://raw.githubusercontent.com/eshanized/M31A/master/scripts/install.ps1 | iex

$ErrorActionPreference = "Stop"

$repo = "eshanized/M31A"
$tag = if ($env:M31A_VERSION) { $env:M31A_VERSION } else { "v0.1.3" }

$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
switch ($arch) {
    "X64"   { $target = "windows-x64" }
    "Arm64" { $target = "windows-arm64" }
    Default { Write-Error "Unsupported Windows architecture: $arch"; exit 1 }
}

# Release archives are versioned per the release workflow:
#   m31a-<VERSION>-windows-x64.zip (plus m31a-<VERSION>-windows-x64.msi)
$version = $tag.TrimStart("v")
$pkgName = "m31a-$version-$target"
$archive = "$pkgName.zip"
$url = "https://github.com/$repo/releases/download/$tag/$archive"
$checksumUrl = "$url.sha256"

$tempDir = Join-Path $env:TEMP "m31a-install-$(Get-Random)"
New-Item -ItemType Directory -Path $tempDir -Force | Out-Null

try {
    Write-Host "==> Downloading M31A $tag for $target..."
    $zipPath = Join-Path $tempDir $archive
    $checksumPath = Join-Path $tempDir "$archive.sha256"
    
    Invoke-WebRequest -Uri $url -OutFile $zipPath
    Invoke-WebRequest -Uri $checksumUrl -OutFile $checksumPath

    Write-Host "==> Verifying SHA-256 checksum..."
    $expectedHash = (Get-Content $checksumPath).Split(" ")[0].Trim().ToLower()
    $actualHash = (Get-FileHash -Path $zipPath -Algorithm SHA256).Hash.ToLower()

    if ($expectedHash -ne $actualHash) {
        Write-Error "Checksum verification failed! Expected: $expectedHash, got: $actualHash"
        exit 1
    }

    Write-Host "==> Extracting binary..."
    Expand-Archive -Path $zipPath -DestinationPath $tempDir -Force

    $binSource = Join-Path $tempDir (Join-Path $pkgName "m31a.exe")
    $installDir = Join-Path $env:LOCALAPPDATA "Programs\m31a"
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    $binDest = Join-Path $installDir "m31a.exe"

    Copy-Item -Path $binSource -Destination $binDest -Force

    # Ensure install dir is in user PATH
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($userPath -notlike "*$installDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$installDir;$userPath", "User")
        Write-Host "==> Added $installDir to user PATH."
    }

    Write-Host "`n==> Successfully installed M31A to $binDest!"
    & $binDest --version
}
finally {
    Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
}
