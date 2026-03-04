#!/usr/bin/env pwsh
# scripts/preflight.ps1
# Pre-flight validation script for GitHub Export.
# Runs all CI checks locally before pushing to GitHub.
# Exit code 0 = all checks passed.  Exit code 1 = one or more checks failed.

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $PSScriptRoot
$CargoRoot   = Join-Path $ProjectRoot "src-tauri"

$Results     = [ordered]@{}
$AnyFailed   = $false

function Invoke-Check {
    param(
        [string] $Name,
        [scriptblock] $Body
    )

    Write-Host "`n==> $Name" -ForegroundColor Cyan
    try {
        & $Body
        if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
            throw "Command exited with code $LASTEXITCODE"
        }
        $Results[$Name] = "PASS"
        Write-Host "    PASS" -ForegroundColor Green
    } catch {
        $Results[$Name] = "FAIL — $_"
        Write-Host "    FAIL — $_" -ForegroundColor Red
        $script:AnyFailed = $true
    }
}

# ── File Existence Checks ─────────────────────────────────────────────────────

Invoke-Check "flake.nix exists" {
    $path = Join-Path $ProjectRoot "flake.nix"
    if (-not (Test-Path $path)) {
        throw "flake.nix not found at $path"
    }
    Write-Host "    Found: $path"
}

Invoke-Check "src-tauri/Cargo.lock exists" {
    $path = Join-Path $CargoRoot "Cargo.lock"
    if (-not (Test-Path $path)) {
        throw "Cargo.lock not found at $path — run 'cargo generate-lockfile' inside src-tauri/ and commit it"
    }
    Write-Host "    Found: $path"
}

# ── Rust Checks (run from src-tauri/) ────────────────────────────────────────

Push-Location $CargoRoot

Invoke-Check "cargo build (debug)" {
    cargo build 2>&1 | Write-Host
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }
}

Invoke-Check "cargo clippy -- -D warnings" {
    cargo clippy -- -D warnings 2>&1 | Write-Host
    if ($LASTEXITCODE -ne 0) { throw "cargo clippy failed with exit code $LASTEXITCODE" }
}

Invoke-Check "cargo test" {
    cargo test 2>&1 | Write-Host
    if ($LASTEXITCODE -ne 0) { throw "cargo test failed with exit code $LASTEXITCODE" }
}

Pop-Location

# ── Summary ───────────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "══════════════════════════════════════════════════" -ForegroundColor White
Write-Host "  Preflight Summary" -ForegroundColor White
Write-Host "══════════════════════════════════════════════════" -ForegroundColor White

foreach ($key in $Results.Keys) {
    $value  = $Results[$key]
    $color  = if ($value -eq "PASS") { "Green" } else { "Red" }
    $marker = if ($value -eq "PASS") { "[PASS]" } else { "[FAIL]" }
    Write-Host "  $marker  $key" -ForegroundColor $color
    if ($value -ne "PASS") {
        Write-Host "           $value" -ForegroundColor Red
    }
}

Write-Host "══════════════════════════════════════════════════" -ForegroundColor White

if ($AnyFailed) {
    Write-Host ""
    Write-Host "  PREFLIGHT FAILED — fix the issues above before pushing." -ForegroundColor Red
    Write-Host ""
    exit 1
} else {
    Write-Host ""
    Write-Host "  All checks passed. Code is ready to push to GitHub." -ForegroundColor Green
    Write-Host ""
    exit 0
}
