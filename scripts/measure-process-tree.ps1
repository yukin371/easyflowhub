<#
.SYNOPSIS
  Measure the resident cost of a process tree, in three columns that disagree.

.DESCRIPTION
  One WebView2 app is spread over 6-9 processes, and its GPU process alone can
  outweigh an entire native binary. Task Manager's "Memory" column is private
  working set; commit charge ("Private Bytes", what S1 recorded) is typically
  2x that. Quoting one against the other makes a 500 MB app look like a 200 MB
  app, so this prints all three and names which is which.

  Walks the tree by ParentProcessId rather than by image name: msedgewebview2.exe
  is shared with every other Chromium-based app on the machine, and summing by
  name would attribute someone else's browser to this product.

.PARAMETER ProcessName
  Root process, with or without .exe (e.g. easyflowhub, app).

.PARAMETER Samples
  Number of samples, default 3. Sampling repeatedly costs 10 s and catches a
  number that has not settled, which a single reading would report as fact.

.PARAMETER IntervalSeconds
  Seconds between samples, default 5.

.EXAMPLE
  powershell -NoProfile -File scripts/measure-process-tree.ps1 -ProcessName easyflowhub

.EXAMPLE
  # Native shell, comparing against S1's 130.1 MB commit figure
  powershell -NoProfile -File scripts/measure-process-tree.ps1 -ProcessName app
#>
param(
    [Parameter(Mandatory = $true)][string]$ProcessName,
    [int]$Samples = 3,
    [int]$IntervalSeconds = 5
)

$ErrorActionPreference = 'Stop'

# No ternary in PowerShell 5.1; a statement can be assigned though.
if ($ProcessName -match '\.exe$') { $rootName = $ProcessName } else { $rootName = "$ProcessName.exe" }

function Get-ProcessTree([string]$Name) {
    $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name, CommandLine
    $set = @{}
    foreach ($r in ($all | Where-Object { $_.Name -eq $Name })) { $set[[int]$r.ProcessId] = $true }
    if ($set.Count -eq 0) { return $null }

    $changed = $true
    while ($changed) {
        $changed = $false
        foreach ($p in $all) {
            if ($set.ContainsKey([int]$p.ProcessId)) { continue }
            if ($set.ContainsKey([int]$p.ParentProcessId)) {
                $set[[int]$p.ProcessId] = $true
                $changed = $true
            }
        }
    }
    # WebView2 keeps a single browser process for all children, so depth is 2-3
    # and the fixed point above is enough; no recursion needed.
    return ($all | Where-Object { $set.ContainsKey([int]$_.ProcessId) })
}

$tree = Get-ProcessTree $rootName
if ($null -eq $tree) {
    Write-Output "not running: $rootName"
    exit 1
}

# WorkingSetPrivate is what Task Manager's "Memory" column shows. It is not on
# System.Diagnostics.Process, hence the perf counter class (KB there, MB here).
function Get-PrivateWorkingSet() {
    $map = @{}
    foreach ($p in (Get-CimInstance Win32_PerfFormattedData_PerfProc_Process)) {
        if ($p.IDProcess -gt 0) { $map[[int]$p.IDProcess] = $p.WorkingSetPrivate }
    }
    return $map
}

$roots = @($tree | Where-Object { $_.Name -eq $rootName } | ForEach-Object { $_.ProcessId })
$started = Get-Process -Id $roots[0] -ErrorAction SilentlyContinue
if ($started) {
    $uptimeMin = [math]::Round(((Get-Date) - $started.StartTime).TotalMinutes, 1)
    Write-Output "root: $rootName (pid $($roots -join ','))  uptime: $uptimeMin min  processes: $($tree.Count)"
}

$lastTotals = $null
$unstable = $false

for ($i = 1; $i -le $Samples; $i++) {
    $pws = Get-PrivateWorkingSet
    $rows = foreach ($p in $tree) {
        $m = Get-Process -Id $p.ProcessId -ErrorAction SilentlyContinue
        if (-not $m) { continue }

        # Command-line switches survive WMI, and they are the only way to tell
        # the GPU process from a renderer without guessing from memory size.
        $type = 'host'
        if ($p.CommandLine -match '--type=([a-zA-Z-]+)') { $type = $Matches[1] }

        $privateWs = 0.0
        if ($pws.ContainsKey([int]$p.ProcessId)) { $privateWs = $pws[[int]$p.ProcessId] / 1MB }

        [pscustomobject]@{
            Pid         = $p.ProcessId
            Type        = $type
            WS_MB       = [math]::Round($m.WorkingSet64 / 1MB, 1)
            PrivateWS_MB = [math]::Round($privateWs, 1)
            Commit_MB   = [math]::Round($m.PrivateMemorySize64 / 1MB, 1)
        }
    }

    $rows = @($rows | Sort-Object Commit_MB -Descending)
    if ($i -eq 1) { $rows | Format-Table -AutoSize | Out-String -Width 120 | Write-Output }

    $tws = ($rows | Measure-Object WS_MB -Sum).Sum
    $tpws = ($rows | Measure-Object PrivateWS_MB -Sum).Sum
    $tc = ($rows | Measure-Object Commit_MB -Sum).Sum

    $gpu = @($rows | Where-Object { $_.Type -eq 'gpu-process' })
    if ($gpu.Count -gt 0) {
        Write-Output ("t{0}: WS {1,7:N1}  PrivateWS {2,7:N1}  Commit {3,7:N1} MB   | GPU alone: PrivateWS {4:N1}" -f $i, $tws, $tpws, $tc, $gpu[0].PrivateWS_MB)
    }
    else {
        Write-Output ("t{0}: WS {1,7:N1}  PrivateWS {2,7:N1}  Commit {3,7:N1} MB" -f $i, $tws, $tpws, $tc)
    }

    if ($null -ne $lastTotals) {
        $drift = [math]::Abs($tc - $lastTotals)
        # 2 MB between samples is normal allocator noise; more than that means
        # the app is still doing something and the number is not a resting value.
        if ($drift -gt 2.0) { $unstable = $true }
    }
    $lastTotals = $tc

    if ($i -lt $Samples) { Start-Sleep -Seconds $IntervalSeconds }
}

Write-Output "`ncolumns: WS = working set (shared pages included); PrivateWS = Task Manager 'Memory'; Commit = S1's 'Private Bytes'"
if ($unstable) {
    Write-Output "WARNING: totals moved between samples; this is not a resting figure"
    exit 2
}
exit 0
