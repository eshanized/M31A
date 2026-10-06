#!/usr/bin/env pwsh
# M31A Cross-Platform Verification Script (PowerShell 7+)
#
# Non-negotiable principle:
# - Host OS != PowerShell shell layer.
# - Distinguishes between shell portability and operating-system portability.

$ErrorActionPreference = "Stop"

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  M31A Architecture & Runtime Verification Suite  " -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

# 1. Platform Detection
$osType = "Unknown"
if ($IsWindows) {
    $osType = "Windows (Native)"
} elseif ($IsLinux) {
    $osType = "Linux (Host development environment)"
} elseif ($IsMacOS) {
    $osType = "macOS (Native)"
}

Write-Host "Host Operating System : $osType" -ForegroundColor Yellow
Write-Host "PowerShell Version    : $($PSVersionTable.PSVersion)" -ForegroundColor Yellow
Write-Host "Rust Toolchain        : $(rustc --version)" -ForegroundColor Yellow
Write-Host "Working Directory     : $PWD" -ForegroundColor Yellow
Write-Host ""

# 2. Format check
Write-Host "==> [1/4] Checking code formatting (cargo fmt --check)..." -ForegroundColor Green
cargo fmt --check
if ($LASTEXITCODE -ne 0) {
    Write-Error "Code formatting check failed. Run 'cargo fmt' to fix."
    exit 1
}

# 3. Type & trait check
Write-Host "==> [2/4] Checking compilation (cargo check)..." -ForegroundColor Green
cargo check --all-targets
if ($LASTEXITCODE -ne 0) {
    Write-Error "Cargo check failed."
    exit 1
}

# 4. Strict clippy analysis
Write-Host "==> [3/4] Running linter (cargo clippy)..." -ForegroundColor Green
cargo clippy --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Error "Clippy linter found warnings/errors."
    exit 1
}

# 5. Test execution
Write-Host "==> [4/4] Executing test suite..." -ForegroundColor Green
cargo test --lib
if ($LASTEXITCODE -ne 0) {
    Write-Error "Library unit tests failed."
    exit 1
}

cargo test --test platform -- --test-threads 1
if ($LASTEXITCODE -ne 0) {
    Write-Error "Platform contract tests failed."
    exit 1
}

# 6. Platform-specific execution audit
Write-Host ""
Write-Host "==> Platform Verification Assessment:" -ForegroundColor Cyan
if ($IsWindows) {
    Write-Host "    [✓] Native Windows runtime tests executed natively on Windows host." -ForegroundColor Green
    Write-Host "    [✓] Win32 Job Objects, ConPTY, and ACL security verified live." -ForegroundColor Green
} else {
    Write-Host "    [✓] Linux/macOS host execution verified." -ForegroundColor Green
    Write-Host "    [✓] Windows pure reasoning contract tests verified (path security, job limits, quoting, identity)." -ForegroundColor Green
    Write-Host "    [i] Native Win32 API calls are verified by Windows CI (GitHub Actions windows-latest)." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Verification succeeded cleanly." -ForegroundColor Green
