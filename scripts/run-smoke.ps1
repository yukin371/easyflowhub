param(
    [switch]$CheckReleaseArtifacts,
    [string]$ReleaseDir = "easyflowhub-app\src-tauri\target\release",
    # The native reveal gate needs a release build of native/ and takes ~15 s at
    # 40 cycles, so it is opt-in. Labeled 200-cycle runs belong in the archive.
    [switch]$NativeBench,
    # C1 (font footprint) needs the same release build, a desktop session and
    # ~90 s: three probe runs per mode plus three pixel captures. Opt-in for the
    # same reason as the bench, and skipped (exit 0) when the exe is not built.
    [switch]$NativeFont,
    # T-Gate 1: watches the running build (0 WebView2 children, second launch
    # forwards, Manager created on demand). Opt-in because it needs the session
    # to itself: the single-instance mutex is keyed by the app identifier, so it
    # would quit the copy the user is running. Also needs a fresh release build.
    [switch]$TauriResident
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

Invoke-Step -Name "Native architecture guard" -WorkingDirectory "." -Action {
    node (Join-Path $scriptRoot "check-native-architecture.mjs")
}

# A gate that has only ever been seen to pass is not evidence. This runs the
# guard against fixture trees with one rule broken each, so a refactor that
# silently disables a rule turns this red instead of quietly passing everything.
Invoke-Step -Name "Native architecture guard self-check" -WorkingDirectory "." -Action {
    node --test (Join-Path $scriptRoot "check-native-architecture.selfcheck.mjs")
}

# The 60 ms budget is judged on the native shell (measure-native-latency.ps1
# below). This Tauri build is the control group and currently measures ~101 ms
# p95, so it runs with -NoFail: it still prints "Verdict: FAIL" against the 60 ms
# target every run, but it must not paint the whole smoke red for months, or the
# other nine steps stop meaning anything. Until P4 retires this shell, treat a
# jump past ~130 ms as a real regression.
Invoke-Step -Name "Tauri latency baseline (report)" -WorkingDirectory "." -Action {
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $scriptRoot "measure-latency.ps1") -NoFail
}

# Same budget, native shell. SKIPs with exit 0 when native/ is not built, so this
# never fails a fresh clone; -NativeBench turns it on for a normal run. The run
# archives into a temp dir on purpose - only labeled, full-length runs belong in
# docs/releases/native-s1.
if ($NativeBench) {
    Invoke-Step -Name "Responsiveness (native)" -WorkingDirectory "." -Action {
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $scriptRoot "measure-native-latency.ps1") `
            -Cycles 40 -Label smoke -OutDir (Join-Path ([IO.Path]::GetTempPath()) "easyflowhub-native-smoke")
    }
}

# C1: the CJK font must cost the process nothing (it is mapped, not read into the
# heap) and must still render -- both halves are checked, because "commit went
# down" also happens when the font silently fails to load and the panel shows
# tofu. Skipped, not failed, when native/ has never been built.
if ($NativeFont) {
    $nativeExe = Join-Path $repoRoot "native\target\release\easyflowhub-native.exe"
    if (Test-Path $nativeExe) {
        Invoke-Step -Name "Font footprint (native)" -WorkingDirectory "." -Action {
            python (Join-Path $scriptRoot "check-native-font.py") --exe $nativeExe
        }
    }
    else {
        Write-Host "SKIP  Font footprint (native): build native/ first" -ForegroundColor DarkYellow
    }
}

# T-Gate 1: "0 webviews" is a claim about a running process, which --shell-report
# cannot answer. This observes the process instead (process tree + windows), so it
# needs a built binary and an exclusive session; it refuses rather than measuring
# the user's copy when one is already running.
if ($TauriResident) {
    $tauriExe = Join-Path $repoRoot (Join-Path "easyflowhub-app\src-tauri\target\release" "easyflowhub.exe")
    if (Test-Path $tauriExe) {
        Invoke-Step -Name "Resident shape (tauri)" -WorkingDirectory "." -Action {
            python (Join-Path $scriptRoot "check-tauri-resident.py") --exe $tauriExe
        }
    }
    else {
        Write-Host "SKIP  Resident shape (tauri): build easyflowhub-app/src-tauri first" -ForegroundColor DarkYellow
    }
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
