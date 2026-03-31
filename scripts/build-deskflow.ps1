# build-deskflow.ps1 - Build DeskFlow with scriptmgr integration
#
# Usage: .\scripts\build-deskflow.ps1

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Split-Path -Parent $ScriptDir
$SCRIPTMGR_DIR = Join-Path $RepoRoot "scriptmgr-go"
$DESKFLOW_DIR = Join-Path $RepoRoot "deskflow"
$BUNDLE_DIR = Join-Path $DESKFLOW_DIR "src-tauri\target\release\bundle"

Write-Host "=== Building DeskFlow ===" -ForegroundColor Cyan

# Step 1: Build scriptmgr.exe
Write-Host "[1/3] Building scriptmgr.exe..." -ForegroundColor Yellow
Set-Location $SCRIPTMGR_DIR

$needBuild = $false
if (-not (Test-Path "scriptmgr.exe")) {
    $needBuild = $true
} else {
    $scriptMgrMain = Join-Path $SCRIPTMGR_DIR "cmd\scriptmgr\main.go"
    if ((Get-Item $scriptMgrMain).LastWriteTime -gt (Get-Item "scriptmgr.exe").LastWriteTime) {
        $needBuild = $true
    }
}

if ($needBuild) {
    go build -ldflags="-s -w" -o scriptmgr.exe ./cmd/scriptmgr
    Write-Host "      scriptmgr.exe built" -ForegroundColor Green
} else {
    Write-Host "      scriptmgr.exe already up-to-date" -ForegroundColor Gray
}

# Step 2: Copy scriptmgr.exe to deskflow resources
Write-Host "[2/3] Copying scriptmgr.exe to deskflow..." -ForegroundColor Yellow
$scriptmgrDest = Join-Path $DESKFLOW_DIR "scriptmgr"
if (-not (Test-Path $scriptmgrDest)) {
    New-Item -ItemType Directory -Path $scriptmgrDest | Out-Null
}
Copy-Item "scriptmgr.exe" -Destination $scriptmgrDest -Force

# Step 3: Build DeskFlow
Write-Host "[3/3] Building DeskFlow..." -ForegroundColor Yellow
Set-Location (Join-Path $DESKFLOW_DIR "deskflow-app")
bun run tauri build

Write-Host ""
Write-Host "=== Build Complete ===" -ForegroundColor Cyan
Write-Host "Output: $BUNDLE_DIR" -ForegroundColor Gray

if (Test-Path (Join-Path $BUNDLE_DIR "nsis")) {
    Write-Host "NSIS installer:" -ForegroundColor Green
    Get-ChildItem (Join-Path $BUNDLE_DIR "nsis") -Filter "*.exe" | ForEach-Object {
        Write-Host "  $($_.Name)" -ForegroundColor White
    }
}

if (Test-Path (Join-Path $BUNDLE_DIR "msi")) {
    Write-Host "MSI installer:" -ForegroundColor Green
    Get-ChildItem (Join-Path $BUNDLE_DIR "msi") -Filter "*.msi" | ForEach-Object {
        Write-Host "  $($_.Name)" -ForegroundColor White
    }
}
