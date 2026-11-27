<#
.SYNOPSIS
    Responsiveness gate: measure hotkey -> first frame latency.

.DESCRIPTION
    The Native Runtime track (ADR-0009, decision 8) makes responsiveness the primary
    release gate, not memory footprint. This script produces that number.

    Mode 1 - log analysis (default, read-only):
        Pairs every "[timing] hotkey_down" with the next "[timing] first_frame"
        emitted by the app and reports p50 / p95 / p99 / max.

    Mode 2 - cold start (-ColdStart, launches the app once):
        Measures wall time from process start until a main window handle appears.
        Refuses to run if the app is already running (single-instance makes it
        meaningless) and never kills processes.

.LOG CONTRACT
    The app must emit these two lines (source tag must be "timing"):

        2026-11-24 19:04:01.234 [INFO] [timing] hotkey_down
        2026-11-24 19:04:01.290 [INFO] [timing] first_frame

    Only the timestamp and the event name are used; trailing fields are ignored,
    so "first_frame_ms=56" is also accepted. Until the app emits these lines the
    script prints a SKIP notice and exits 0 so CI is not broken mid-migration.

.PARAMETER BudgetMs
    p95 budget in milliseconds. Defaults: 60 for standard, 120 for -Lite.

.EXAMPLE
    powershell -NoProfile -File scripts\measure-latency.ps1

.EXAMPLE
    powershell -NoProfile -File scripts\measure-latency.ps1 -Lite -LogPath C:\tmp\app.log -Json

.EXAMPLE
    powershell -NoProfile -File scripts\measure-latency.ps1 -ColdStart -ProcessPath dist\easyflowhub.exe -BudgetMs 400
#>
[CmdletBinding()]
param(
    [string]$LogPath,
    [int]$BudgetMs = 0,
    [switch]$Lite,
    [switch]$ColdStart,
    [string]$ProcessPath,
    [int]$TimeoutMs = 15000,
    [switch]$Json,
    [switch]$NoFail
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($BudgetMs -le 0) {
    if ($Lite) { $BudgetMs = 120 } else { $BudgetMs = 60 }
}

function Get-Percentile {
    param([double[]]$Values, [double]$P)
    if ($null -eq $Values -or $Values.Count -eq 0) { return $null }
    $sorted = @($Values | Sort-Object)
    $rank = [int][math]::Ceiling(($P / 100.0) * $sorted.Count)
    if ($rank -lt 1) { $rank = 1 }
    if ($rank -gt $sorted.Count) { $rank = $sorted.Count }
    return [double]$sorted[$rank - 1]
}

function Resolve-DefaultLogPath {
    $candidates = @(
        (Join-Path $env:APPDATA 'com.yukin.easyflowhub\logs\app.log'),
        (Join-Path $env:LOCALAPPDATA 'com.yukin.easyflowhub\logs\app.log')
    )
    foreach ($c in $candidates) {
        if (Test-Path -LiteralPath $c) { return $c }
    }
    return $null
}

function Read-TimingSamples {
    param([string]$Path)

    $pattern = '^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d{3})\s+\[\w+\]\s+\[timing\]\s+(hotkey_down|first_frame)\b'
    $samples = New-Object System.Collections.Generic.List[double]
    $pending = $null
    $pairs = 0

    foreach ($line in (Get-Content -LiteralPath $Path -Encoding UTF8 -ReadCount 0)) {
        $m = [regex]::Match($line, $pattern)
        if (-not $m.Success) { continue }

        $stamp = [datetime]::ParseExact(
            $m.Groups[1].Value, 'yyyy-MM-dd HH:mm:ss.fff',
            [System.Globalization.CultureInfo]::InvariantCulture)
        $event = $m.Groups[2].Value

        if ($event -eq 'hotkey_down') {
            # A second downbeat without a frame overwrites the first: the user
            # pressed the hotkey again instead of waiting, which is not a sample.
            $pending = $stamp
        }
        elseif ($null -ne $pending) {
            $delta = ($stamp - $pending).TotalMilliseconds
            if ($delta -ge 0 -and $delta -lt 10000) {
                $samples.Add($delta)
                $pairs++
            }
            $pending = $null
        }
    }

    return [pscustomobject]@{ Samples = $samples; Pairs = $pairs }
}

function Measure-ColdStart {
    param([string]$ExePath, [int]$Timeout)

    $resolved = Resolve-Path -LiteralPath $ExePath
    $name = [System.IO.Path]::GetFileNameWithoutExtension($resolved.Path)
    $running = @(Get-Process -Name $name -ErrorAction SilentlyContinue)
    if ($running.Count -gt 0) {
        throw "App '$name' is already running (pid $($running[0].Id)). Cold start requires a clean state; single-instance would short-circuit the measurement."
    }

    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    # `--legacy-ui` is required from T-Gate 0 on: the default build serves only
    # an on-demand Manager, so it has no main window to wait for and no launcher
    # webview to time. This script measures the Tauri baseline, which is now the
    # fallback path -- the native shell has its own gate
    # (`measure-native-latency.ps1`).
    $proc = Start-Process -FilePath $resolved.Path -ArgumentList '--legacy-ui' -PassThru
    $deadline = $sw.ElapsedMilliseconds + $Timeout
    $seen = $false

    while ($sw.ElapsedMilliseconds -lt $deadline) {
        $proc.Refresh()
        if ($proc.MainWindowHandle -ne [IntPtr]::Zero -and $proc.MainWindowHandle -ne 0) {
            $seen = $true
            break
        }
        Start-Sleep -Milliseconds 5
    }
    $sw.Stop()

    if (-not $seen) {
        return [pscustomobject]@{
            Ok = $false
            Ms = $sw.Elapsed.TotalMilliseconds
            Note = "no main window handle within $Timeout ms"
            Pid = $proc.Id
        }
    }
    return [pscustomobject]@{
        Ok = $true
        Ms = $sw.Elapsed.TotalMilliseconds
        Note = 'main window handle observed'
        Pid = $proc.Id
    }
}

# ---------------------------------------------------------------- cold start

if ($ColdStart) {
    if (-not $ProcessPath) {
        throw '-ColdStart requires -ProcessPath pointing at the built executable.'
    }
    $cold = Measure-ColdStart -ExePath $ProcessPath -Timeout $TimeoutMs
    $pass = $cold.Ok -and ($cold.Ms -le $BudgetMs)

    if ($Json) {
        [pscustomobject]@{
            Mode = 'cold-start'
            MeasuredAt = (Get-Date).ToString('s')
            BudgetMs = $BudgetMs
            ValueMs = [math]::Round($cold.Ms, 1)
            Ok = $cold.Ok
            Pass = $pass
            Note = $cold.Note
            Pid = $cold.Pid
        } | ConvertTo-Json -Depth 4
    }
    else {
        $out = @(
            ''
            "Cold start @ $((Get-Date).ToString('yyyy-MM-dd HH:mm:ss'))"
            "  exe     : $ProcessPath"
            "  window  : $($cold.Note)"
            ("  elapsed : {0:N1} ms   [budget {1} ms]" -f $cold.Ms, $BudgetMs)
            ''
        )
        if ($pass) { $out += 'Verdict: PASS' } else { $out += 'Verdict: FAIL' }
        $out += ''
        Write-Output ($out -join [Environment]::NewLine)
    }

    if (-not $pass -and -not $NoFail) { exit 1 }
    exit 0
}

# ------------------------------------------------------------ log analysis

if (-not $LogPath) { $LogPath = Resolve-DefaultLogPath }
if (-not $LogPath) {
    Write-Output 'SKIP: no log file found (pass -LogPath). Nothing measured.'
    exit 0
}
if (-not (Test-Path -LiteralPath $LogPath)) {
    Write-Output "SKIP: log not found: $LogPath"
    exit 0
}

$result = Read-TimingSamples -Path $LogPath
$values = @($result.Samples.ToArray())

if ($values.Count -eq 0) {
    Write-Output 'SKIP: no [timing] hotkey_down / first_frame pairs in log.'
    Write-Output '      The app must emit these lines before this gate can judge anything:'
    Write-Output '        2026-11-24 19:04:01.234 [INFO] [timing] hotkey_down'
    Write-Output '        2026-11-24 19:04:01.290 [INFO] [timing] first_frame'
    exit 0
}

$p50 = Get-Percentile -Values $values -P 50
$p95 = Get-Percentile -Values $values -P 95
$p99 = Get-Percentile -Values $values -P 99
$max = ($values | Measure-Object -Maximum).Maximum
$pass = ($p95 -le $BudgetMs)

if ($Json) {
    [pscustomobject]@{
        Mode = 'hotkey-to-first-frame'
        MeasuredAt = (Get-Date).ToString('s')
        Log = $LogPath
        Samples = $values.Count
        BudgetMs = $BudgetMs
        P50Ms = [math]::Round($p50, 1)
        P95Ms = [math]::Round($p95, 1)
        P99Ms = [math]::Round($p99, 1)
        MaxMs = [math]::Round($max, 1)
        Pass = $pass
    } | ConvertTo-Json -Depth 4
}
else {
    $tag = 'standard'
    if ($Lite) { $tag = 'lite' }
    $out = @(
        ''
        "Responsiveness @ $((Get-Date).ToString('yyyy-MM-dd HH:mm:ss'))  [$tag]"
        "  log     : $LogPath"
        "  samples : $($values.Count) hotkey_down / first_frame pairs"
        ''
        '  hotkey -> first frame'
        ("    p50   {0,7:N1} ms" -f $p50)
        ("    p95   {0,7:N1} ms   [budget {1} ms]" -f $p95, $BudgetMs)
        ("    p99   {0,7:N1} ms" -f $p99)
        ("    max   {0,7:N1} ms" -f $max)
        ''
    )
    if ($pass) { $out += 'Verdict: PASS' } else { $out += 'Verdict: FAIL' }
    $out += ''
    Write-Output ($out -join [Environment]::NewLine)
}

if (-not $pass -and -not $NoFail) { exit 1 }
exit 0
