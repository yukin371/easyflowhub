#Requires -Version 5.1
<#
.SYNOPSIS
    Collects the environment facts the C2 IME acceptance run needs, as Markdown.

.DESCRIPTION
    C2 (docs/plans/active/2026-11-25-c2-ime-acceptance.md) asks for three input methods
    typing into the RichEdit island for 30 minutes each. The result is only
    evidence if it says *which* input methods and *which* binary were used --
    "I tested Sogou" is not reproducible, "Sogou 16.10 on build 26300 with exe
    sha256 6a1f..." is.

    Prints one Markdown block meant to be pasted into section 7.1, plus a short
    readiness verdict about whether the run can start at all.

.NOTES
    Deliberately avoids Add-Type: this environment blocks runtime compilation and
    none of these facts need it. Output is assembled as lines and emitted once,
    because per-line output loses the blank lines that Markdown depends on.
#>
[CmdletBinding()]
param(
    # Path to the binary under test. Defaults to the release build in this repo.
    [string]$Exe,

    # Where to write the Markdown. Defaults to docs/releases/native-c2/environment.md.
    # Written to a file rather than stdout because a console renderer collapses the
    # blank lines that make the Markdown copy-pasteable.
    [string]$OutFile
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not $Exe) {
    $Exe = Join-Path $repoRoot 'native\target\release\easyflowhub-native.exe'
}
if (-not $OutFile) {
    $OutFile = Join-Path $repoRoot 'docs\releases\native-c2\environment.md'
}

$lines = New-Object System.Collections.Generic.List[string]
function Add-Line {
    param([string]$Text = '')
    $lines.Add($Text)
}
function Format-Mb {
    param([double]$Bytes)
    return ('{0:N1} MB' -f ($Bytes / 1MB))
}

$now = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
$tick = [char]96

# ---------------------------------------------------------------- OS / session
$os = Get-CimInstance Win32_OperatingSystem
$sessionName = $env:SESSIONNAME
if (-not $sessionName) { $sessionName = 'unknown' }
$remoteNote = '(local)'
if ($sessionName -match '^RDP|^ICA') {
    $remoteNote = '**remote** - candidate window geometry is not trustworthy over a compressed stream'
}

Add-Line '### C2 environment'
Add-Line
Add-Line ('Collected: {0}{1}{2}' -f $tick, $now, $tick)
Add-Line
Add-Line '| Item | Value |'
Add-Line '|---|---|'
Add-Line ('| OS | {0} build **{1}** ({2}) |' -f $os.Caption, $os.BuildNumber, $os.OSArchitecture)
Add-Line ('| Session | {0}{1}{2} {3} |' -f $tick, $sessionName, $tick, $remoteNote)
Add-Line ('| User | {0} |' -f $env:USERNAME)

# ------------------------------------------------------------------- displays
$gpus = @(Get-CimInstance Win32_VideoController)
$monitors = @(Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction SilentlyContinue)
Add-Line ('| Displays (WMI monitor entries) | {0} |' -f $monitors.Count)
foreach ($g in $gpus) {
    if ($g.CurrentHorizontalResolution) {
        Add-Line ('| Adapter {0}{1}{2} | {3}x{4} |' -f $tick, $g.Name, $tick, $g.CurrentHorizontalResolution, $g.CurrentVerticalResolution)
    }
}

# ------------------------------------------------------------ input methods
# Identity is the TIP CLSID, never the display name.
#
# On 2026-11-25 this script reported Sogou and WeChat input as "not installed"
# while both were installed and enabled. Two independent causes, both fixed:
#   * WeChat input registers its text service as **WeType**, so a /wechat|微信/
#     name match could never hit it. Its binary is wetype_tip.dll.
#   * A TIP key's Description lives under HKLM; the HKCU mirror of the same key
#     usually has none. Reading one hive means seeing a nameless service.
# Presence is now decided from the same list the language bar itself uses, and
# every verdict below prints what it matched, so a wrong answer is auditable
# instead of plausible.
$KnownTips = [ordered]@{
    '{81d4e9c9-1d3b-41bc-9e6c-4b40bf79e35e}' = 'Microsoft Pinyin'
    '{e7ea138e-69f8-11d7-a6ea-00065b844310}' = 'Sogou Pinyin'
    '{86598fb9-66a2-463e-b9c2-aeb906d477ad}' = 'WeChat input (WeType)'
}

$enabledClsids = New-Object System.Collections.Generic.List[string]
foreach ($lang in (Get-WinUserLanguageList)) {
    foreach ($tip in $lang.InputMethodTips) {
        # Format is `0804:{CLSID}{PROFILE}`; the first GUID identifies the service.
        # The braces are inside the capture group on purpose: the table below is
        # keyed by `{CLSID}`, and a brace-less capture silently compares unequal.
        if ($tip -match '(\{[0-9A-Fa-f\-]{36}\})') {
            $enabledClsids.Add($Matches[1].ToLower())
        }
    }
}

function Get-TipDescription {
    param([string]$Clsid)
    foreach ($root in @('HKLM:\SOFTWARE\Microsoft\CTF\TIP', 'HKCU:\SOFTWARE\Microsoft\CTF\TIP')) {
        $profiles = Join-Path (Join-Path $root $Clsid) 'LanguageProfile'
        if (-not (Test-Path $profiles)) { continue }
        foreach ($langKey in (Get-ChildItem $profiles -ErrorAction SilentlyContinue)) {
            foreach ($profileKey in (Get-ChildItem $langKey.PSPath -ErrorAction SilentlyContinue)) {
                $desc = (Get-ItemProperty $profileKey.PSPath -ErrorAction SilentlyContinue).Description
                if ($desc) { return $desc }
            }
        }
    }
    return $null
}

function Get-TipBinary {
    param([string]$Clsid)
    foreach ($root in @('HKLM:\SOFTWARE\Classes\CLSID', 'HKCU:\SOFTWARE\Classes\CLSID')) {
        $server = Join-Path (Join-Path $root $Clsid) 'InprocServer32'
        if (-not (Test-Path $server)) { continue }
        $dll = (Get-ItemProperty $server -ErrorAction SilentlyContinue).'(default)'
        if ($dll) { return $dll }
    }
    return $null
}

Add-Line
Add-Line '**Input methods under test** (identity is the TIP CLSID; version comes from the service binary):'
Add-Line
Add-Line '| Input method | TIP CLSID | Enabled | Version | Binary |'
Add-Line '|---|---|---|---|---|'
foreach ($entry in $KnownTips.GetEnumerator()) {
    $clsid = $entry.Key
    $label = $entry.Value
    $desc = Get-TipDescription $clsid
    # Keep the localized name when the label does not already carry it.
    if ($desc -and $label -notlike ('*' + $desc + '*')) {
        $label = '{0} ({1})' -f $label, $desc
    }
    $on = 'no'
    if ($enabledClsids -contains $clsid) { $on = '**yes**' }
    $version = 'unknown'
    $binary = 'not found'
    $dll = Get-TipBinary $clsid
    if ($dll) {
        $binary = Split-Path -Leaf $dll
        if (Test-Path $dll) {
            $fileVersion = (Get-Item $dll).VersionInfo.FileVersion
            if ($fileVersion) { $version = $fileVersion }
        }
    } elseif ($enabledClsids -contains $clsid) {
        # Inbox services (Microsoft Pinyin) have no InprocServer32: nothing to read
        # a version from, which is not the same thing as "not installed".
        $binary = 'inbox service (no InprocServer32)'
        $version = 'n/a'
    }
    Add-Line ("| $label | $tick$clsid$tick | $on | $version | $binary |")
}

# Everything else registered for zh-CN, so "not in the three above" is checkable
# rather than an assertion.
Add-Line
Add-Line '**Other zh-CN text services registered on this machine** (not under test):'
Add-Line
$otherRows = New-Object System.Collections.Generic.List[string]
foreach ($root in @('HKLM:\SOFTWARE\Microsoft\CTF\TIP', 'HKCU:\SOFTWARE\Microsoft\CTF\TIP')) {
    if (-not (Test-Path $root)) { continue }
    foreach ($key in (Get-ChildItem $root -ErrorAction SilentlyContinue)) {
        $clsid = $key.PSChildName.ToLower()
        if ($KnownTips.Contains($clsid)) { continue }
        $desc = Get-TipDescription $clsid
        if (-not $desc) { continue }
        $row = "- $desc -- $tick$clsid$tick"
        if (-not $otherRows.Contains($row)) { $otherRows.Add($row) }
    }
}
if ($otherRows.Count -eq 0) {
    Add-Line '- (none)'
} else {
    foreach ($row in $otherRows) { Add-Line $row }
}

# ------------------------------------------------------------- binary under test
Add-Line
Add-Line '**Binary under test:**'
Add-Line
if (Test-Path $Exe) {
    $file = Get-Item $Exe
    $hash = (Get-FileHash $Exe -Algorithm SHA256).Hash.ToLower()
    Add-Line ('- Path: {0}{1}{0}' -f $tick, $file.FullName)
    Add-Line ('- Built: {0}{1}{0}  Size: {0}{2}{0}' -f $tick, $file.LastWriteTime.ToString('yyyy-MM-dd HH:mm:ss'), (Format-Mb $file.Length))
    Add-Line ('- SHA256: {0}{1}{0}' -f $tick, $hash)
} else {
    Add-Line ('- **MISSING**: {0}{1}{0}' -f $tick, $Exe)
    Add-Line '- Build it: cargo build --release -p app, from native/'
}

# ------------------------------------------------------------ what is running
$running = @(Get-Process -Name 'easyflowhub*' -ErrorAction SilentlyContinue)
Add-Line
Add-Line '**Product processes running right now:**'
Add-Line
if ($running.Count -eq 0) {
    Add-Line '- (none)'
} else {
    foreach ($p in $running) {
        Add-Line ('- {0}{1}{0} pid {2}, {3}' -f $tick, $p.ProcessName, $p.Id, (Format-Mb $p.WorkingSet64))
    }
}

# ------------------------------------------------------------------- verdict
# Counted from the language bar's own list, by CLSID. A name regex would have
# said "missing" for an input method the user had been typing with all day.
$hasPinyin = $enabledClsids -contains '{81d4e9c9-1d3b-41bc-9e6c-4b40bf79e35e}'
$hasSogou = $enabledClsids -contains '{e7ea138e-69f8-11d7-a6ea-00065b844310}'
$hasWechat = $enabledClsids -contains '{86598fb9-66a2-463e-b9c2-aeb906d477ad}'

function Say-Presence {
    param([bool]$Present, [string]$MissingNote)
    if ($Present) { return 'present and enabled' }
    return ('**MISSING** - ' + $MissingNote)
}

Add-Line
Add-Line '**Readiness for C2 (needs Microsoft Pinyin + Sogou + WeChat):**'
Add-Line
Add-Line ('- Microsoft Pinyin: {0}' -f (Say-Presence $hasPinyin 'install first'))
Add-Line ('- Sogou: {0}' -f (Say-Presence $hasSogou 'install before the run'))
Add-Line ('- WeChat input: {0}' -f (Say-Presence $hasWechat 'install before the run'))
Add-Line
Add-Line '> DPI / scale is not guessed here on purpose: take native_ppp= from the startup log line set_ppp,'
Add-Line '> which is the value the panel was actually laid out at.'

$output = $lines -join [Environment]::NewLine

$outDir = Split-Path -Parent $OutFile
if (-not (Test-Path $outDir)) {
    New-Item -ItemType Directory -Force -Path $outDir | Out-Null
}
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($OutFile, $output + [Environment]::NewLine, $utf8NoBom)

Write-Host ('Wrote {0}' -f $OutFile)
Write-Host ''
Write-Host 'Readiness:'
Write-Host ('  Microsoft Pinyin : {0}' -f (Say-Presence $hasPinyin 'install first'))
Write-Host ('  Sogou            : {0}' -f (Say-Presence $hasSogou 'install before the run'))
Write-Host ('  WeChat input     : {0}' -f (Say-Presence $hasWechat 'install before the run'))
