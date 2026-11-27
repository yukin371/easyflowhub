param(
    [switch]$Json,
    [int]$SampleSeconds = 0,
    [string]$Label = "",
    [string]$OutFile = ""
)

# Footprint baseline for EasyFlowHub.
#
# Read-only: never launches, stops, or mutates the app. It reports the live
# process tree (host + WebView2 children + Go sidecar) and the static disk
# footprint, so memory goals in docs/plans/active/2026-11-24-native-runtime-redesign.md
# can be checked with the same tool before and after the native migration.
#
# Usage:
#   powershell -NoProfile -File scripts\measure-footprint.ps1
#   powershell -NoProfile -File scripts\measure-footprint.ps1 -SampleSeconds 30
#   powershell -NoProfile -File scripts\measure-footprint.ps1 -Json
#   powershell -NoProfile -File scripts\measure-footprint.ps1 -Label idle -Json `
#       -OutFile docs\releases\footprint\0.2.0\idle-20261124-2300.json
#
# -Label tags the pose (idle / launcher / manager-2notes) so archived snapshots
# stay comparable; -OutFile also writes the JSON next to stdout (the script
# remains read-only — writing the report is not a mutation of the app).

$ErrorActionPreference = "Stop"

function Convert-ToMb {
    param([double]$Bytes)
    if ($null -eq $Bytes) { return 0 }
    return [math]::Round($Bytes / 1MB, 1)
}

function Get-TreeSizeMb {
    param([string]$Path)
    if ([string]::IsNullOrWhiteSpace($Path) -or -not (Test-Path -LiteralPath $Path)) { return $null }
    $sum = (Get-ChildItem -LiteralPath $Path -Recurse -Force -ErrorAction SilentlyContinue |
        Measure-Object -Property Length -Sum).Sum
    if ($null -eq $sum) { return 0 }
    return Convert-ToMb $sum
}

function Get-MonitorList {
    # Per-monitor resolution + effective DPI. Mixed-DPI setups are the reason this
    # matters: a launcher panel is one buffer per scale, the font atlas is per scale,
    # so "how many scales are live" changes both memory and first-frame cost.
    # Falls back to a single WMI/registry reading when EnumDisplayMonitors is unavailable.
    $fallback = {
        $dpi = 96
        $metrics = Get-ItemProperty -Path 'HKCU:\Control Panel\Desktop\WindowMetrics' -Name AppliedDPI -ErrorAction SilentlyContinue
        if ($metrics -and $metrics.AppliedDPI) { $dpi = [int]$metrics.AppliedDPI }
        $vc = Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue |
            Where-Object { $_.CurrentHorizontalResolution -gt 0 } | Select-Object -First 1
        if (-not $vc) { return @() }
        return @([pscustomobject]@{
            Device   = 'primary'
            Width    = [int]$vc.CurrentHorizontalResolution
            Height   = [int]$vc.CurrentVerticalResolution
            Dpi      = $dpi
            ScalePct = [int][math]::Round($dpi / 96 * 100)
            Primary  = $true
        })
    }

    if (-not ('EfhMonitors' -as [type])) {
        $source = @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public class EfhMonitors {
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct MONITORINFOEX {
        public int cbSize;
        public RECT rcMonitor;
        public RECT rcWork;
        public uint dwFlags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string szDevice;
    }

    public delegate bool MonitorEnumProc(IntPtr hMonitor, IntPtr hdc, ref RECT lprc, IntPtr data);

    [DllImport("user32.dll")]
    static extern bool EnumDisplayMonitors(IntPtr hdc, IntPtr clip, MonitorEnumProc proc, IntPtr data);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern bool GetMonitorInfo(IntPtr hMonitor, ref MONITORINFOEX info);

    [DllImport("shcore.dll")]
    static extern int GetDpiForMonitor(IntPtr hmonitor, int dpiType, out uint dpiX, out uint dpiY);

    [DllImport("shcore.dll")]
    static extern int SetProcessDpiAwareness(int value);

    [DllImport("user32.dll")]
    static extern bool SetProcessDPIAware();

    // A DPI-unaware process sees virtualized coordinates (3840x2160 @150% reports as
    // 2560x1440 @100%), so the reading would silently understate both pixels and
    // per-monitor scale. Make this process DPI aware before enumerating.
    static void EnsureDpiAware() {
        try { if (SetProcessDpiAwareness(2) == 0) { return; } } catch { }
        try { SetProcessDPIAware(); } catch { }
    }

    public class Row {
        public string Device;
        public int Width, Height, Dpi, WorkWidth, WorkHeight;
        public bool Primary;
    }

    public static List<Row> Get() {
        EnsureDpiAware();
        var rows = new List<Row>();
        MonitorEnumProc proc = delegate(IntPtr h, IntPtr hdc, ref RECT r, IntPtr d) {
            var info = new MONITORINFOEX();
            info.cbSize = Marshal.SizeOf(typeof(MONITORINFOEX));
            if (GetMonitorInfo(h, ref info)) {
                var row = new Row();
                row.Device = info.szDevice;
                row.Width = info.rcMonitor.Right - info.rcMonitor.Left;
                row.Height = info.rcMonitor.Bottom - info.rcMonitor.Top;
                row.WorkWidth = info.rcWork.Right - info.rcWork.Left;
                row.WorkHeight = info.rcWork.Bottom - info.rcWork.Top;
                row.Primary = (info.dwFlags & 1) != 0;
                uint dx, dy;
                row.Dpi = 96;
                if (GetDpiForMonitor(h, 0, out dx, out dy) == 0 && dx > 0) { row.Dpi = (int)dx; }
                rows.Add(row);
            }
            return true;
        };
        EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, proc, IntPtr.Zero);
        return rows;
    }
}
'@
        try { Add-Type -TypeDefinition $source -ErrorAction Stop } catch { return (& $fallback) }
    }

    $rows = @()
    try {
        $rows = @([EfhMonitors]::Get())
    } catch {
        return (& $fallback)
    }
    if ($rows.Count -eq 0) { return (& $fallback) }

    return @($rows | ForEach-Object {
        [pscustomobject]@{
            Device    = $_.Device
            Width     = $_.Width
            Height    = $_.Height
            Dpi       = $_.Dpi
            ScalePct  = [int][math]::Round($_.Dpi / 96 * 100)
            WorkArea  = ('{0}x{1}' -f $_.WorkWidth, $_.WorkHeight)
            Primary   = $_.Primary
        }
    })
}

function Get-ScreenDescription {
    param($Monitors)
    # Resolution and DPI are recorded next to the memory numbers on purpose:
    # font atlas and first-frame texture scale with DPI and physical pixels, so
    # a 65 MB reading at 100% is not comparable to the same reading at 150%.
    if (-not $Monitors) { $Monitors = Get-MonitorList }
    if (-not $Monitors -or @($Monitors).Count -eq 0) { return "unknown" }
    if (@($Monitors).Count -eq 1) {
        $m = @($Monitors)[0]
        return ("{0}x{1} @{2}%" -f $m.Width, $m.Height, $m.ScalePct)
    }
    $parts = @($Monitors | ForEach-Object { "{0}x{1}@{2}%" -f $_.Width, $_.Height, $_.ScalePct })
    return ("{0} monitors: {1}" -f @($Monitors).Count, ($parts -join " + "))
}

function Get-ProcessSnapshot {
    $all = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue)
    $children = @{}
    foreach ($p in $all) {
        $parentId = [int]$p.ParentProcessId
        if (-not $children.ContainsKey($parentId)) {
            $children[$parentId] = New-Object System.Collections.Generic.List[object]
        }
        $children[$parentId].Add($p)
    }

    $roots = @($all | Where-Object { $_.Name -eq "easyflowhub.exe" })

    $tree = New-Object System.Collections.Generic.List[object]
    $queue = New-Object System.Collections.Generic.Queue[object]
    $seen = @{}

    foreach ($root in $roots) {
        $queue.Enqueue($root)
        $seen[[int]$root.ProcessId] = $true
    }

    while ($queue.Count -gt 0) {
        $item = $queue.Dequeue()
        $proc = Get-Process -Id ([int]$item.ProcessId) -ErrorAction SilentlyContinue
        $tree.Add([pscustomobject]@{
            Name        = [string]$item.Name
            Id          = [int]$item.ProcessId
            ParentId    = [int]$item.ParentProcessId
            WorkingMb   = if ($proc) { Convert-ToMb $proc.WorkingSet64 } else { 0 }
            PrivateMb   = if ($proc) { Convert-ToMb $proc.PrivateMemorySize64 } else { 0 }
        })
        foreach ($child in $children[[int]$item.ProcessId]) {
            $childId = [int]$child.ProcessId
            if (-not $seen.ContainsKey($childId)) {
                $seen[$childId] = $true
                $queue.Enqueue($child)
            }
        }
    }

    # The Go sidecar is a direct child today, but keep the name filter as a
    # safety net for orphaned processes after a crash.
    $sidecar = @($all | Where-Object { $_.Name -like "scriptmgr*.exe" })

    return [pscustomobject]@{
        Running = ($roots.Count -gt 0)
        Tree    = $tree
        Sidecar = $sidecar
    }
}

$snapshot = Get-ProcessSnapshot

$installCandidates = @(
    (Join-Path $env:LOCALAPPDATA "EasyFlowHub"),
    (Join-Path $env:PROGRAMFILES "EasyFlowHub")
)
$installed = $installCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1

$monitors = Get-MonitorList

$report = [pscustomobject]@{
    MeasuredAt          = (Get-Date).ToString("s")
    Pose                = $Label
    Screen              = (Get-ScreenDescription -Monitors $monitors)
    Monitors            = @($monitors)
    PrimaryGpu          = (Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue |
        Where-Object { $_.CurrentHorizontalResolution -gt 0 } | Select-Object -First 1).Name
    AppRunning          = $snapshot.Running
    HostProcesses       = @($snapshot.Tree | Where-Object { $_.Name -eq "easyflowhub.exe" }).Count
    WebviewProcesses    = @($snapshot.Tree | Where-Object { $_.Name -eq "msedgewebview2.exe" }).Count
    WorkingSetMb        = Convert-ToMb (($snapshot.Tree | Measure-Object -Property WorkingMb -Sum).Sum * 1MB)
    PrivateMb           = Convert-ToMb (($snapshot.Tree | Measure-Object -Property PrivateMb -Sum).Sum * 1MB)
    SidecarProcesses    = @($snapshot.Sidecar).Count
    SidecarPrivateMb    = Convert-ToMb ((@($snapshot.Sidecar) | ForEach-Object {
        $p = Get-Process -Id ([int]$_.ProcessId) -ErrorAction SilentlyContinue
        if ($p) { $p.PrivateMemorySize64 } else { 0 }
    } | Measure-Object -Sum).Sum)
    InstallDir          = $installed
    InstallDirMb        = Get-TreeSizeMb $installed
    ExeMb               = Get-TreeSizeMb (Join-Path $installed "easyflowhub.exe")
    SidecarExeMb        = Get-TreeSizeMb (Join-Path $installed "scriptmgr.exe")
    WebviewProfileMb    = Get-TreeSizeMb (Join-Path $env:LOCALAPPDATA "com.yukin.easyflowhub")
    WebviewRuntimeMb    = Get-TreeSizeMb (Join-Path ${env:ProgramFiles(x86)} "Microsoft\EdgeWebView\Application")
    WebviewRuntimeVer   = (Get-ChildItem -LiteralPath (Join-Path ${env:ProgramFiles(x86)} "Microsoft\EdgeWebView\Application") -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' } |
        Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1).Name
}

if ($SampleSeconds -gt 0 -and $snapshot.Running) {
    # Peak sampling: useful while opening Manager / several quick notes by hand.
    $peakPrivate = 0
    $peakProcesses = 0
    $deadline = (Get-Date).AddSeconds($SampleSeconds)
    while ((Get-Date) -lt $deadline) {
        $s = Get-ProcessSnapshot
        $sum = Convert-ToMb (($s.Tree | Measure-Object -Property PrivateMb -Sum).Sum * 1MB)
        if ($sum -gt $peakPrivate) { $peakPrivate = $sum }
        if ($s.Tree.Count -gt $peakProcesses) { $peakProcesses = $s.Tree.Count }
        Start-Sleep -Milliseconds 800
    }
    $report | Add-Member -NotePropertyName PeakPrivateMb -NotePropertyValue $peakPrivate
    $report | Add-Member -NotePropertyName PeakProcesses -NotePropertyValue $peakProcesses
}

$archiveNote = ""
if (-not [string]::IsNullOrWhiteSpace($OutFile)) {
    # NB: `$json` would collide with the `-Json` switch (PowerShell variable names
    # are case-insensitive), so the payload variable must be named differently.
    $jsonText = $report | ConvertTo-Json -Depth 4
    $dir = Split-Path -Parent $OutFile
    if ($dir -and -not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    $full = [System.IO.Path]::GetFullPath($OutFile)
    # WriteAllText with a BOM-less UTF8 encoder: `Out-File -Encoding utf8` emits a
    # BOM under PowerShell 5.1, which trips strict JSON readers.
    [System.IO.File]::WriteAllText($full, $jsonText, (New-Object System.Text.UTF8Encoding($false)))
    $archiveNote = "archived -> $full"
}

if ($Json) {
    $report | ConvertTo-Json -Depth 4
    exit 0
}

# Output is emitted as ONE success-stream write instead of many Write-Host calls.
# Write-Host writes to the host stream, so `| Out-File` / `>` would capture nothing,
# and `Write-Host ""` emits no newline at all under PowerShell 5.1 (blank lines get
# swallowed and the next header glues onto the previous value: "564 MBDisk").
$out = New-Object System.Collections.Generic.List[string]
$out.Add("EasyFlowHub footprint @ $($report.MeasuredAt)")
if (-not [string]::IsNullOrWhiteSpace($report.Pose)) {
    $out.Add("pose=$($report.Pose)")
}
if ($archiveNote) {
    $out.Add($archiveNote)
}
$out.Add(("screen={0}  gpu={1}" -f $report.Screen, $report.PrimaryGpu))
if (@($report.Monitors).Count -gt 0) {
    foreach ($m in $report.Monitors) {
        $mark = ''
        if ($m.Primary) { $mark = ' (primary)' }
        $out.Add(("  monitor {0,-14} {1}x{2} @{3}%  work={4}{5}" -f $m.Device, $m.Width, $m.Height, $m.ScalePct, $m.WorkArea, $mark))
    }
}
$out.Add(("running={0}  host={1}  webview2={2}  sidecar={3}" -f $report.AppRunning, $report.HostProcesses, $report.WebviewProcesses, $report.SidecarProcesses))
$out.Add(("private={0} MB  workingset={1} MB" -f $report.PrivateMb, $report.WorkingSetMb))
if ($report.PSObject.Properties.Name -contains "PeakPrivateMb") {
    $out.Add(("peak private={0} MB  peak processes={1}" -f $report.PeakPrivateMb, $report.PeakProcesses))
}
$out.Add("")
$out.Add("Disk")
$out.Add(("install dir      {0} MB  ({1})" -f $report.InstallDirMb, $report.InstallDir))
$out.Add(("easyflowhub.exe  {0} MB" -f $report.ExeMb))
$out.Add(("scriptmgr.exe    {0} MB" -f $report.SidecarExeMb))
$out.Add(("webview profile  {0} MB  %LOCALAPPDATA%\com.yukin.easyflowhub" -f $report.WebviewProfileMb))
$out.Add(("webview runtime  {0} MB  v{1} (system shared)" -f $report.WebviewRuntimeMb, $report.WebviewRuntimeVer))

if ($snapshot.Running) {
    $out.Add("")
    $out.Add("Process tree (tree walk from easyflowhub.exe; NOT a system-wide name count)")
    $out.Add(("  {0,-20} {1,7} {2,9} {3,11} {4,11}" -f 'Name', 'Id', 'ParentId', 'WorkingMb', 'PrivateMb'))
    $treeTotalWorking = 0.0
    $treeTotalPrivate = 0.0
    foreach ($row in ($snapshot.Tree | Sort-Object PrivateMb -Descending)) {
        $treeTotalWorking += $row.WorkingMb
        $treeTotalPrivate += $row.PrivateMb
        $out.Add(("  {0,-20} {1,7} {2,9} {3,11:N1} {4,11:N1}" -f $row.Name, $row.Id, $row.ParentId, $row.WorkingMb, $row.PrivateMb))
    }
    $out.Add(("  {0,-20} {1,7} {2,9} {3,11:N1} {4,11:N1}" -f 'TOTAL', '', '', $treeTotalWorking, $treeTotalPrivate))
}

Write-Output ($out -join [Environment]::NewLine)
