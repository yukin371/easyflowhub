# native S2 typing gate
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/measure-native-editor.ps1
#   ... -Samples 400 -Label scale-1.5
#
# Runs the native spike's text-island benchmark, checks the S2 acceptance budget
# (type p95 <= 16 ms, p99 <= 25 ms) plus the structural evidence that makes the
# number meaningful (dark theme reached the surface, the caret scrolled, geometry
# follows the layout, Esc/Tab are intercepted) and archives the sample.
#
# Why the structural checks are part of the exit code: a fast editor that paints
# nothing, or one whose keys never arrive, is not a passing editor. Timing alone
# cannot tell those apart.
#
# Exit codes: 0 pass (or SKIP when the app is not built), 1 budget or evidence failure.
param(
    [int]$Samples = 200,
    [string]$Label,
    [string]$OutDir,
    [string]$Exe,
    [double]$BudgetP95 = 16,
    [double]$BudgetP99 = 25
)

$ErrorActionPreference = 'Stop'
$scriptRoot = $PSScriptRoot
$repoRoot = Split-Path -Parent $scriptRoot

if (-not $Exe) {
    $Exe = Join-Path $repoRoot 'native\target\release\easyflowhub-native.exe'
}
if (-not (Test-Path $Exe)) {
    Write-Host 'SKIP native editor: native/target/release/easyflowhub-native.exe not built.'
    Write-Host '     build it with: cargo build --release -p app --manifest-path native/Cargo.toml'
    exit 0
}
if (-not $OutDir) {
    $OutDir = Join-Path $repoRoot 'docs\releases\native-s2'
}
if (-not $Label) { $Label = 'windows' }

$arguments = @(
    '--editor-bench', $Samples.ToString(),
    '--editor-budget-p95', $BudgetP95.ToString([Globalization.CultureInfo]::InvariantCulture),
    '--editor-budget-p99', $BudgetP99.ToString([Globalization.CultureInfo]::InvariantCulture)
)

Write-Host ("native editor: running {0} samples ({1}) ..." -f $Samples, $Label)

# PowerShell 5.1 turns each line of native stderr into a RemoteException, which
# aborts the run under $ErrorActionPreference='Stop'. The spike logs to stderr on
# purpose, so only string records — i.e. stdout — are kept.
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

$editorJson = $null
if ($text -match '(?s)---EDITOR-JSON---\s*(\{.*?\})\s*$') {
    $editorJson = $Matches[1]
}
if (-not $editorJson) {
    Write-Host 'native editor: no measurement produced (island likely failed to create).'
    exit 1
}

$sample = $editorJson | ConvertFrom-Json
Write-Host (($text -split '---EDITOR-JSON---')[0].TrimEnd())

# Structural evidence. Each line states what breaks in the product if it regresses.
$checks = [ordered]@{
    'class is RICHEDIT50W'      = ($sample.class -eq 'RICHEDIT50W')
    'text colour painted'       = ($sample.pixels.fg_typed -gt 0)
    'empty doc has no ink'      = ($sample.pixels.fg_empty -eq 0)
    'caret scrolled (10k)'      = ($sample.scroll.large -gt 0)
    'short doc does not scroll' = ($sample.scroll.small -eq 0)
    'rect moves on change'      = ($sample.resize.changed_moved -eq $true)
    'rect idle when unchanged'  = ($sample.resize.same_moved -eq $false)
    'undo restores text'        = ($sample.undo.ok -eq $true)
    'redo restores text'        = ($sample.redo.ok -eq $true)
    'undo stack has depth'      = ($sample.undo.grouped -gt 1)
    'Esc reaches shell'         = ($sample.keys.esc -eq $true)
    'Tab inserts, no focus loss' = ($sample.keys.tab -eq $true)
    'unhandled key falls through' = ($sample.keys.fallthrough -eq $true)
    'IME context present'       = ($sample.keys.imm -eq $true)
    '10k load < 200 ms'         = ($sample.load_ms.doc_10k -lt 200)
    '10k paste < 150 ms'        = ($sample.load_ms.paste_10k -lt 150)
    'burst < 400 ms'            = ($sample.burst_ms -lt 400)
}

$failed = @()
foreach ($name in $checks.Keys) {
    if ($checks[$name]) {
        Write-Host ("  ok   {0}" -f $name)
    }
    else {
        Write-Host ("  FAIL {0}" -f $name)
        $failed += $name
    }
}

$typeP95 = [math]::Round($sample.doc_1k.type_us.p95 / 1000, 2)
$typeP99 = [math]::Round($sample.doc_1k.type_us.p99 / 1000, 2)
$tenP95 = [math]::Round($sample.doc_10k.type_us.p95 / 1000, 2)
Write-Host ("type p95={0} ms p99={1} ms (budget {2}/{3}) | 10k p95={4} ms" -f $typeP95, $typeP99, $BudgetP95, $BudgetP99, $tenP95)

if (-not $sample.pass) { $failed += 'type budget' }

if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Path $OutDir -Force | Out-Null }
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$archivePath = Join-Path $OutDir ('editor-{0}-{1}.json' -f $Label, $stamp)

$record = [ordered]@{
    kind      = 'native-s2-editor-archive'
    label     = $Label
    samples   = $Samples
    timestamp = (Get-Date).ToString('o')
    budget    = [ordered]@{ p95 = $BudgetP95; p99 = $BudgetP99 }
    checks    = $checks
    failed    = $failed
    sample    = $sample
}
$jsonText = $record | ConvertTo-Json -Depth 8
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($archivePath, $jsonText, $utf8NoBom)
Write-Host ("archived: {0}" -f $archivePath)

if ($failed.Count -gt 0) {
    Write-Host ("RESULT: FAIL ({0})" -f ($failed -join ', '))
    exit 1
}
Write-Host 'RESULT: PASS'
exit 0
