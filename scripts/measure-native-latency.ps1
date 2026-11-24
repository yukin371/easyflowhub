# native S1 latency gate
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/measure-native-latency.ps1
#   ... -Cycles 200 -Label scale-1.5
#
# Runs the native spike's automated reveal cycles, checks the distribution against
# the S1 acceptance budget (p95 <= 60 ms, p99 <= 100 ms) and archives the result
# next to the memory samples so the G0 decision has evidence instead of a memory
# of a conversation.
#
# Exit codes: 0 pass (or SKIP when the spike is not built), 1 budget failure.
param(
    [int]$Cycles = 200,
    [double]$Scale = 0,
    [string]$Label,
    [string]$OutDir,
    [string]$Exe,
    [double]$BudgetP95 = 60,
    [double]$BudgetP99 = 100
)

$ErrorActionPreference = 'Stop'
$scriptRoot = $PSScriptRoot
$repoRoot = Split-Path -Parent $scriptRoot

if (-not $Exe) {
    $Exe = Join-Path $repoRoot 'native\target\release\easyflowhub-native.exe'
}
if (-not (Test-Path $Exe)) {
    Write-Host 'SKIP native latency: native/target/release/easyflowhub-native.exe not built.'
    Write-Host '     build it with: cargo build --release -p app --manifest-path native/Cargo.toml'
    exit 0
}
if (-not $OutDir) {
    $OutDir = Join-Path $repoRoot 'docs\releases\native-s1'
}
if (-not $Label) {
    if ($Scale -gt 0) { $Label = ('scale-{0}' -f $Scale.ToString('0.0', [Globalization.CultureInfo]::InvariantCulture)) }
    else { $Label = 'scale-system' }
}

$arguments = @('--bench', $Cycles.ToString(), '--budget-p95', $BudgetP95.ToString(), '--budget-p99', $BudgetP99.ToString())
if ($Scale -gt 0) {
    $arguments += @('--scale', $Scale.ToString('0.0', [Globalization.CultureInfo]::InvariantCulture))
}

Write-Host ("native latency: running {0} cycles ({1}) ..." -f $Cycles, $Label)

# PowerShell 5.1 turns each line of native stderr into a RemoteException, which
# aborts the whole run under $ErrorActionPreference='Stop'. The spike logs to stderr
# on purpose (timing marks), so only the string records — i.e. stdout — are kept.
$previousPreference = $ErrorActionPreference
$ErrorActionPreference = 'Continue'
try {
    $raw = & $Exe @arguments 2>&1 | Where-Object { $_ -is [string] }
    $exit = $LASTEXITCODE
}
finally {
    $ErrorActionPreference = $previousPreference
}
$text = ($raw -join "`n")

# The spike prints: report, ---JSON---, latency json, ---LEAK---, leak json
$latencyJson = $null
$leakJson = $null
if ($text -match '(?s)---JSON---\s*(\{.*?\})\s*---LEAK---\s*(\{.*?\})') {
    $latencyJson = $Matches[1]
    $leakJson = $Matches[2]
}

if (-not $latencyJson) {
    Write-Host 'native latency: no measurement produced (hotkey likely already registered).'
    exit 1
}

$latency = $latencyJson | ConvertFrom-Json
$leak = $null
if ($leakJson) { $leak = $leakJson | ConvertFrom-Json }

$report = ($text -split '---JSON---')[0].TrimEnd()
Write-Host $report

if ($leak) {
    Write-Host ("leak: cycles={0} private {1} -> {2} MB (delta {3}) handles {4} -> {5} (delta {6})" -f `
        $leak.cycles, $leak.start_private_mb, $leak.end_private_mb, $leak.private_delta_mb, `
        $leak.start_handles, $leak.end_handles, $leak.handles_delta)
}

# Archive: one file per run, same shape as the footprint samples.
if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Path $OutDir -Force | Out-Null }
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$archivePath = Join-Path $OutDir ('latency-{0}-{1}.json' -f $Label, $stamp)

$record = [ordered]@{
    kind        = 'native-s1-latency-archive'
    label       = $Label
    cycles      = $Cycles
    scale       = $Scale
    timestamp   = (Get-Date).ToString('o')
    shown       = $latency.shown
    refresh     = $latency.refresh
    budget      = $latency.budget
    leak        = $leak
}
$jsonText = $record | ConvertTo-Json -Depth 6
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($archivePath, $jsonText, $utf8NoBom)
Write-Host ("archived: {0}" -f $archivePath)

if ($latency.budget.p95_ok -and $latency.budget.p99_ok) {
    Write-Host 'RESULT: PASS'
    exit 0
}
Write-Host 'RESULT: FAIL'
exit 1
