param(
    [switch]$CheckReleaseArtifacts,
    [string]$ReleaseDir = "easyflowhub-app\src-tauri\target\release"
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptRoot

$steps = New-Object System.Collections.Generic.List[object]
$smokeFailed = $false

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
        # Native commands (bun / cargo / go) report failure through the exit
        # code, not through a PowerShell exception, so reset it and check it
        # explicitly. Without this the gate would report "passed" on a red
        # test suite.
        $global:LASTEXITCODE = 0
        # Redirecting native stderr into the pipeline keeps incidental
        # warnings (cargo incremental lock cleanup, bun notices) from becoming
        # terminating errors under $ErrorActionPreference = "Stop", which
        # previously failed healthy steps in milliseconds.
        $ErrorActionPreference = 'Continue'
        $output = & $Action 2>&1
        $ErrorActionPreference = 'Stop'
        $output | ForEach-Object {
            if ($_ -is [System.Management.Automation.ErrorRecord]) {
                # PowerShell 5.1 wraps merged native stderr in RemoteException;
                # print the underlying message so real failures stay readable.
                $text = $_.Exception.Message
                if ([string]::IsNullOrWhiteSpace($text)) {
                    $text = $_.ToString()
                }
                Write-Host "    $text" -ForegroundColor DarkYellow
            }
            else {
                Write-Host "    $_"
            }
        }
        $exitCode = $LASTEXITCODE
        if ($null -ne $exitCode -and $exitCode -ne 0) {
            throw "step exited with code $exitCode"
        }
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
        Write-Host ("    " + $_.Exception.Message) -ForegroundColor Red
        $script:smokeFailed = $true
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

Invoke-Step -Name "Production build" -WorkingDirectory "easyflowhub-app" -Action {
    bun run build
}

Invoke-Step -Name "Manager theme token audit" -WorkingDirectory "." -Action {
    & (Join-Path $scriptRoot "check-theme-tokens.ps1")
}

Invoke-Step -Name "Cargo check" -WorkingDirectory "easyflowhub-app" -Action {
    cargo check --manifest-path src-tauri/Cargo.toml
}

Invoke-Step -Name "Rust library tests" -WorkingDirectory "easyflowhub-app" -Action {
    cargo test --manifest-path src-tauri/Cargo.toml --lib
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

if ($smokeFailed) {
    Write-Host ""
    Write-Host "Smoke FAILED: at least one gate did not pass." -ForegroundColor Red
    exit 1
}
