param(
    [switch]$CheckReleaseArtifacts,
    [string]$ReleaseDir = "easyflowhub-app\src-tauri\target\release"
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptRoot

$steps = New-Object System.Collections.Generic.List[object]

function Invoke-Step {
    param(
        [string]$Name,
        [string]$WorkingDirectory,
        [scriptblock]$Action
    )

    $resolvedWorkingDirectory = Join-Path $repoRoot $WorkingDirectory
    $startedAt = Get-Date

    Write-Host ""
    Write-Host "==> $Name" -ForegroundColor Cyan
    Write-Host "    cwd: $resolvedWorkingDirectory" -ForegroundColor DarkGray

    Push-Location $resolvedWorkingDirectory
    try {
        & $Action
        $duration = [math]::Round(((Get-Date) - $startedAt).TotalSeconds, 1)
        $steps.Add([pscustomobject]@{
            Name = $Name
            Status = "passed"
            DurationSeconds = $duration
        }) | Out-Null
        Write-Host "    passed (${duration}s)" -ForegroundColor Green
    }
    catch {
        $duration = [math]::Round(((Get-Date) - $startedAt).TotalSeconds, 1)
        $steps.Add([pscustomobject]@{
            Name = $Name
            Status = "failed"
            DurationSeconds = $duration
        }) | Out-Null
        Write-Host "    failed (${duration}s)" -ForegroundColor Red
        throw
    }
    finally {
        Pop-Location
    }
}

Write-Host "Running EasyFlowHub scripted smoke..." -ForegroundColor Yellow
Write-Host "Repo root: $repoRoot" -ForegroundColor DarkGray

Invoke-Step -Name "Frontend tests" -WorkingDirectory "easyflowhub-app" -Action {
    bun run test
}

Invoke-Step -Name "Type check" -WorkingDirectory "easyflowhub-app" -Action {
    bunx tsc --noEmit
}

Invoke-Step -Name "Cargo check" -WorkingDirectory "easyflowhub-app" -Action {
    cargo check --manifest-path src-tauri/Cargo.toml
}

Invoke-Step -Name "Go tests" -WorkingDirectory "scriptmgr-go" -Action {
    go test ./...
}

if ($CheckReleaseArtifacts) {
    Invoke-Step -Name "Release artifact preflight" -WorkingDirectory "." -Action {
        & (Join-Path $scriptRoot "check-release-artifacts.ps1") -ReleaseDir $ReleaseDir
    }
}

Write-Host ""
Write-Host "Smoke summary" -ForegroundColor Yellow
$steps | ForEach-Object {
    Write-Host ("  {0,-28} {1,-6} {2,6}s" -f $_.Name, $_.Status, $_.DurationSeconds)
}

Write-Host ""
Write-Host "Next manual step: follow docs/checklists/windows-package-smoke.md when validating packaged Windows behavior." -ForegroundColor Yellow
