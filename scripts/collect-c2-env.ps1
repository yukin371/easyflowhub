#Requires -Version 5.1
<#
.SYNOPSIS
    Collects the environment facts the C2 IME acceptance run needs, as Markdown.

.DESCRIPTION
    C2 (docs/plans/2026-11-25-c2-ime-acceptance.md) asks for three input methods
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
$enabled = @()
foreach ($lang in (Get-WinUserLanguageList)) {
    foreach ($tip in $lang.InputMethodTips) {
        $enabled += [pscustomobject]@{ Lang = $lang.LanguageTag; Tip = $tip }
    }
}

$registered = @()
$tipRoot = 'HKLM:\SOFTWARE\Microsoft\CTF\TIP'
if (Test-Path $tipRoot) {
    foreach ($key in (Get-ChildItem $tipRoot -ErrorAction SilentlyContinue)) {
        $profileRoot = Join-Path $key.PSPath 'LanguageProfile\0x00000804'
        if (-not (Test-Path $profileRoot)) { continue }
        foreach ($profile in (Get-ChildItem $profileRoot -ErrorAction SilentlyContinue)) {
            $desc = (Get-ItemProperty $profile.PSPath -ErrorAction SilentlyContinue).Description
            $registered += [pscustomobject]@{
                Clsid   = $key.PSChildName.ToLower()
                Profile = $profile.PSChildName.ToLower()
                Desc    = $desc
            }
        }
    }
}

Add-Line
Add-Line '**Input methods enabled in the language list:**'
Add-Line
if ($enabled.Count -eq 0) {
    Add-Line '- (none)'
} else {
    foreach ($e in $enabled) {
        $match = $registered | Where-Object { $e.Tip -like ('*' + $_.Clsid + '*') } | Select-Object -First 1
        $label = 'unknown profile'
        if ($match) { $label = $match.Desc }
        Add-Line ('- {0}{1}{2} {3} -- {0}{4}{0}' -f $tick, $e.Lang, $tick, $label, $e.Tip)
    }
}

Add-Line
Add-Line '**Registered zh-CN text services (enabled or not):**'
Add-Line
if ($registered.Count -eq 0) {
    Add-Line '- (none)'
} else {
    foreach ($r in $registered) {
        # TIP child keys are already named `{CLSID}`; do not wrap them again.
        Add-Line ('- {0} -- {1}{2}{1}' -f $r.Desc, $tick, $r.Clsid)
    }
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
$hasSogou = @($registered | Where-Object { $_.Desc -match 'sogou|搜狗' }).Count -gt 0
$hasWechat = @($registered | Where-Object { $_.Desc -match 'wechat|微信' }).Count -gt 0
$hasPinyin = @($registered | Where-Object { $_.Desc -match 'Microsoft Pinyin|微软拼音' }).Count -gt 0

function Say-Presence {
    param([bool]$Present, [string]$MissingNote)
    if ($Present) { return 'present' }
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
