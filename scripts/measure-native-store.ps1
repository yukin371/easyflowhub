# native S3 data-compatibility gate
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/measure-native-store.ps1
#   ... -Label windows-144dpi -Sqlite3 'C:\path\to\sqlite3.exe'
#
# Answers the S3 question with evidence instead of assertion: can the native shell read
# and write the product's notes.db without breaking it for the Tauri build that already
# owns it?
#
# Every run works on a COPY. The live database is fingerprinted before and after, and a
# change fails the gate (G-1) -- an experiment must not be able to touch user data.
#
# Three independent layers have to agree before this prints PASS:
#   L1 native reads and writes the copy            (hub-core::store)
#   L2 the Tauri code reads and writes the result  (cargo test --ignored in src-tauri)
#   L3 a different SQLite build reads it           (sqlite3 CLI: integrity + row compare)
#
# Exit codes: 0 pass (or SKIP when the native app is not built), 1 any failure.
#
# The work tree is `%TEMP%\efh-s3-gate\s3-gate-<stamp>` (override with -WorkRoot). It
# lives outside the repo on purpose: the copy needs a journal, and a checkout that
# cannot be written to turns that into `attempt to write a readonly database` -- a red
# gate about the checkout, not about the migrator. L2 shells out to `cargo test` in the
# Tauri crate, so CARGO_TARGET_DIR is inherited; set it to a writable directory when
# the repo itself is not writable.
param(
    [string]$Label,
    [string]$OutDir,
    [string]$Exe,
    [string]$Sqlite3,
    [string]$TauriDir,
    [string]$WorkRoot,
    [int]$HoldShort = 400,
    [int]$HoldLong = 7000,
    [switch]$KeepWork
)

$ErrorActionPreference = 'Stop'
$scriptRoot = $PSScriptRoot
$repoRoot = Split-Path -Parent $scriptRoot

if (-not $Exe) { $Exe = Join-Path $repoRoot 'native\target\release\easyflowhub-native.exe' }
if (-not $TauriDir) { $TauriDir = Join-Path $repoRoot 'easyflowhub-app\src-tauri' }
if (-not $OutDir) { $OutDir = Join-Path $repoRoot 'docs\releases\native-s3' }
if (-not $Label) { $Label = 'windows' }
# See the header: the work tree must be writable, and `%TEMP%` is by construction.
if (-not $WorkRoot) { $WorkRoot = Join-Path ([System.IO.Path]::GetTempPath()) 'efh-s3-gate' }

if (-not (Test-Path $Exe)) {
    Write-Host 'SKIP native store: native/target/release/easyflowhub-native.exe not built.'
    Write-Host '     build it with: cargo build --release -p app --manifest-path native/Cargo.toml'
    exit 0
}

$liveRoot = Join-Path $env:APPDATA 'com.yukin.easyflowhub'
$liveDb = Join-Path $liveRoot 'notes.db'
if (-not (Test-Path $liveDb)) {
    Write-Host ("SKIP native store: no live database at {0}" -f $liveDb)
    exit 0
}

function Get-FileFingerprint([string]$path) {
    if (-not (Test-Path $path)) { return $null }
    $item = Get-Item $path
    $hash = (Get-FileHash -Path $path -Algorithm SHA256).Hash
    return [ordered]@{ bytes = $item.Length; mtime = $item.LastWriteTimeUtc.ToString('o'); sha256 = $hash }
}

# L3 is optional: a machine without the sqlite3 CLI can still run L1 + L2. It is
# reported as `skipped`, never as `ok`, so the archive never claims a check that did
# not happen.
if (-not $Sqlite3) {
    $found = Get-Command sqlite3 -ErrorAction SilentlyContinue
    if ($found) { $Sqlite3 = $found.Source }
}

$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$work = Join-Path $WorkRoot ("s3-gate-{0}" -f $stamp)
$root = Join-Path $work 'root'
$art = Join-Path $work 'art'
New-Item -ItemType Directory -Path $root -Force | Out-Null
New-Item -ItemType Directory -Path $art -Force | Out-Null

$liveBefore = Get-FileFingerprint $liveDb
Copy-Item $liveDb (Join-Path $root 'notes.db')
Write-Host ("native store: gate run {0} on a copy of {1} ({2:N0} bytes)" -f $stamp, $liveDb, $liveBefore.bytes)

# PowerShell 5.1 turns each line of native stderr into a RemoteException; the spike logs
# to stderr on purpose. Under 'Continue' those records are collected too, and
# `ToString()` keeps the message -- the lock test needs to read "database is locked".
function Invoke-Native {
    param([string[]]$Arguments)
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $out = & $Exe @Arguments 2>&1
        $code = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previous
    }
    $lines = @()
    foreach ($line in $out) {
        if ($line -is [string]) { $lines += $line }
        elseif ($null -ne $line) { $lines += $line.ToString() }
    }
    $text = ($lines -join "`n")
    $json = $null
    if ($text -match '(?s)---STORE-JSON---\s*(\{.*?\})\s*$') { $json = $Matches[1] | ConvertFrom-Json }
    return [pscustomobject]@{ Text = $text; Exit = $code; Json = $json }
}

# ---------------------------------------------------------------- L1: read
$read = Invoke-Native @('--store-bench', '--store-root', $root)
if (-not $read.Json) {
    Write-Host ("native store: read pass produced no measurement (exit {0})" -f $read.Exit)
    Write-Host $read.Text
    exit 1
}
$r = $read.Json
Write-Host (($read.Text -split '---STORE-JSON---')[0].TrimEnd())

# The live database's own numbers, read independently, so "40 notes" is not something
# our reader told us about itself.
$cliCounts = $null
if ($Sqlite3) {
    $cliCounts = [ordered]@{
        notes          = [int](& $Sqlite3 (Join-Path $root 'notes.db') 'SELECT COUNT(*) FROM notes;')
        settings       = [int](& $Sqlite3 (Join-Path $root 'notes.db') 'SELECT COUNT(*) FROM settings;')
    }
}

$checks = [ordered]@{}
$checks['db bytes match the live file'] = ($r.db_bytes -eq $liveBefore.bytes)
$checks['notes read = live count'] = if ($cliCounts) { $r.counts_before.notes -eq $cliCounts.notes } else { $r.counts_before.notes -gt 0 }
$checks['settings read = live count'] = if ($cliCounts) { $r.counts_before.settings -eq $cliCounts.settings } else { $r.counts_before.settings -gt 0 }
$checks['all notes readable'] = ($r.notes.live -gt 0) -and ($r.notes.with_deleted -ge $r.notes.live)
$checks['sort matches the product query'] = ($r.notes.sorted -eq $true)
$checks['pinned notes first'] = ($r.notes.pinned_first -eq $true)
$checks['updated_at descending'] = ($r.notes.updated_desc -eq $true)
$checks['todo inbox located'] = ($r.todo.found -eq $true)
$checks['settings decode as JSON'] = (($r.settings.objects + $r.settings.scalars) -eq $r.settings.keys.Count)
$checks['autostart shape read'] = ($null -ne $r.settings.autostart)
$checks['journal_mode untouched'] = ($r.contract.journal_mode -eq 'delete')
$checks['user_version untouched'] = ($r.contract.user_version -eq 0)
$checks['page_size untouched'] = ($r.contract.page_size -eq 4096)
$checks['integrity ok (read)'] = ($r.integrity.before -eq $true)

# ---------------------------------------------------------------- L1: write
$write = Invoke-Native @('--store-bench', '--store-root', $root, '--store-write', '--store-artifacts', $art)
if (-not $write.Json) {
    Write-Host ("native store: write pass produced no measurement (exit {0})" -f $write.Exit)
    Write-Host $write.Text
    exit 1
}
$w = $write.Json

$checks['native inserted a note'] = ($w.writes.created -eq $true)
$checks['native updated it'] = ($w.writes.updated_at_changed -eq $true)
$checks['native soft-deleted a note'] = ($w.writes.soft_deleted -eq $true)
$checks['native wrote an attachment'] = ($w.writes.attachment_row -eq $true) -and ($w.writes.attachment_bytes -eq 1024)
$checks['counts grew by 2 notes'] = ($w.counts_after.notes -eq ($w.counts_before.notes + 2))
$checks['attachment row appeared'] = ($w.counts_after.inbox_attachments -eq ($w.counts_before.inbox_attachments + 1))
$checks['integrity ok (write)'] = ($w.integrity.after -eq $true)
$checks['schema unchanged by writes'] = ($w.schema_same -eq $true)
$checks['journal_mode still delete'] = ($w.contract.journal_mode -eq 'delete')
$checks['user_version still 0'] = ($w.contract.user_version -eq 0)

$db = Join-Path $root 'notes.db'

# ---------------------------------------------------------------- L3: independent reader
$independent = 'skipped: sqlite3 not found'
if ($Sqlite3) {
    $integrity = (& $Sqlite3 $db 'PRAGMA integrity_check;') -join ''
    $row = & $Sqlite3 -separator '|' $db "SELECT title, tags, updated_at, is_pinned FROM notes WHERE id = 's3-probe-created';"
    $hidden = & $Sqlite3 $db "SELECT COUNT(*) FROM notes WHERE id = 's3-probe-deleted' AND deleted_at IS NULL;"
    $attachment = & $Sqlite3 -separator '|' $db "SELECT filename, size_bytes FROM inbox_attachments WHERE item_id = 's3-probe-item';"

    $checks['sqlite3 CLI integrity ok'] = ($integrity -eq 'ok')
    $checks['sqlite3 CLI sees the probe row'] = ($row -match '^S3 探针\|')
    $checks['soft-deleted row hidden from list'] = ($hidden -eq '0')
    $checks['sqlite3 CLI sees the attachment row'] = ($null -ne $attachment -and $attachment -match '\.bin\|1024$')
    $independent = 'ran: integrity + row + attachment'

    # Differential dump: our reader's view of every row against the CLI's view of the
    # same file. Written through `.output` so the bytes stay UTF-8 instead of being
    # decoded through the console codepage.
    $cliDump = Join-Path $work 'cli-notes.json'
    & $Sqlite3 -json $db ".output $cliDump" "SELECT id, title, content, tags, created_at, updated_at, is_pinned, deleted_at FROM notes ORDER BY id;" | Out-Null
    $compare = Join-Path $work 'compare.js'
    @'
const fs = require('fs');
const [ours, theirs] = process.argv.slice(2);
const a = JSON.parse(fs.readFileSync(ours, 'utf8')).notes_dump;
const b = JSON.parse(fs.readFileSync(theirs, 'utf8'));
const key = (n) => [n.id, n.title, n.content, n.tags, n.created_at, n.updated_at, n.is_pinned ? 1 : 0, n.deleted_at === undefined ? null : n.deleted_at].join('\u001f');
const norm = (list) => list.map(key).sort();
const left = norm(a), right = norm(b);
if (left.length !== right.length) {
  console.log(`row-count-mismatch ours=${left.length} cli=${right.length}`);
  process.exit(1);
}
for (let i = 0; i < left.length; i += 1) {
  if (left[i] !== right[i]) {
    console.log(`row-mismatch at ${i}:\n  ours: ${left[i]}\n  cli : ${right[i]}`);
    process.exit(1);
  }
}
console.log(`rows-identical ${left.length}`);
'@ | Set-Content -Path $compare -Encoding ASCII
    $dumpResult = (& node $compare (Join-Path $art 's3-notes-dump.json') $cliDump) -join ' '
    Write-Host ("  differential dump: {0}" -f $dumpResult)
    $checks['dump identical to sqlite3 CLI'] = ($dumpResult -match 'rows-identical')
}

# ---------------------------------------------------------------- L2: the Tauri build itself
$artifactsForTauri = 'not run'
if (Test-Path (Join-Path $TauriDir 'Cargo.toml')) {
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        Push-Location $TauriDir
        $env:EFH_S3_DB = $db
        $tauriOut = & cargo test --lib -- --ignored reads_and_writes_a_database_produced_by_the_native_shell 2>&1 | Where-Object { $_ -is [string] }
        $tauriExit = $LASTEXITCODE
        Pop-Location
    }
    finally {
        Remove-Item Env:\EFH_S3_DB -ErrorAction SilentlyContinue
        $ErrorActionPreference = $previous
    }
    $tauriText = ($tauriOut -join "`n")
    $checks['Tauri code reads native rows'] = ($tauriExit -eq 0)
    $artifactsForTauri = if ($tauriExit -eq 0) { 'passed' } else { 'failed' }
    if ($tauriExit -ne 0) {
        Write-Host $tauriText
    }
}
else {
    $checks['Tauri code reads native rows'] = $false
    Write-Host ("  Tauri crate not found at {0}: L2 cannot run, treating as failure" -f $TauriDir)
}

# The Tauri test wrote into the copy; the native reader must see that write (C3). The
# row dump only goes to the artifacts file, so ask for one.
$reverseArt = Join-Path $work 'art-reverse'
$reverse = Invoke-Native @('--store-bench', '--store-root', $root, '--store-artifacts', $reverseArt)
$reverseOk = $false
$reverseDump = Join-Path $reverseArt 's3-notes-dump.json'
if (Test-Path $reverseDump) {
    $rows = Get-Content -Raw -Encoding UTF8 $reverseDump | ConvertFrom-Json
    $probe = $rows.notes_dump | Where-Object { $_.id -eq 's3-probe-created' }
    $reverseOk = ($null -ne $probe) -and ($probe.content -like '*Tauri*')
    if (-not $reverseOk -and $null -ne $probe) {
        Write-Host ("  reverse read saw: {0}" -f $probe.content)
    }
}
$checks['native reads the Tauri write'] = $reverseOk

# ---------------------------------------------------------------- C1: two processes, one file
$short = Join-Path $work 'hold-short.log'
$long = Join-Path $work 'hold-long.log'

# Start-Process on PS 5.1 throws "key HTTPS_PROXY already added" when the environment
# contains both cases of a variable, so the child is started through the .NET API and
# PowerShell never enumerates the environment block.
function Start-Holder([int]$holdMs) {
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $Exe
    $info.Arguments = ('--store-hold {0} --store-root "{1}"' -f $holdMs, $root)
    $info.UseShellExecute = $false
    $info.RedirectStandardOutput = $true
    $info.CreateNoWindow = $true
    return [System.Diagnostics.Process]::Start($info)
}

$holder = Start-Holder $HoldShort
Start-Sleep -Milliseconds 150
$during = Invoke-Native @('--store-bench', '--store-root', $root, '--store-write')
$holder.WaitForExit()
$checks['write succeeds while another writer holds briefly'] = ($during.Exit -eq 0)

$holderLong = Start-Holder $HoldLong
Start-Sleep -Milliseconds 150
$blocked = Invoke-Native @('--store-bench', '--store-root', $root, '--store-write')
$holderLong.WaitForExit()
$blockedText = $blocked.Text.ToLower()
$checks['long lock is refused, not corrupted'] = ($blocked.Exit -ne 0) -and ($blockedText -match 'locked|busy')

$afterArt = Join-Path $work 'art-after-lock'
$afterLock = Invoke-Native @('--store-bench', '--store-root', $root, '--store-artifacts', $afterArt)
$checks['integrity ok after the lock fight'] = ($afterLock.Json -and $afterLock.Json.integrity.before -eq $true)
$heldRow = $false
$afterDump = Join-Path $afterArt 's3-notes-dump.json'
if (Test-Path $afterDump) {
    $dump = Get-Content -Raw -Encoding UTF8 $afterDump | ConvertFrom-Json
    $heldRow = $null -ne ($dump.notes_dump | Where-Object { $_.id -eq 's3-hold-row' })
}
$checks['held row survived the lock fight'] = $heldRow
$checks['no half-written note after a refused write'] = ($afterLock.Json -and ($afterLock.Json.notes.with_deleted -eq $afterLock.Json.counts_before.notes))

# ---------------------------------------------------------------- G-1: user data untouched
$liveAfter = Get-FileFingerprint $liveDb
$checks['live database untouched (bytes)'] = ($liveAfter.bytes -eq $liveBefore.bytes)
$checks['live database untouched (sha256)'] = ($liveAfter.sha256 -eq $liveBefore.sha256)
$checks['live database untouched (mtime)'] = ($liveAfter.mtime -eq $liveBefore.mtime)

# ---------------------------------------------------------------- report
$failed = @()
foreach ($name in $checks.Keys) {
    if ($checks[$name]) { Write-Host ("  ok   {0}" -f $name) }
    else {
        Write-Host ("  FAIL {0}" -f $name)
        $failed += $name
    }
}
Write-Host ("independent reader: {0} | tauri compat: {1}" -f $independent, $artifactsForTauri)
Write-Host ("sqlite native={0}" -f $w.sqlite)

if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Path $OutDir -Force | Out-Null }
$archivePath = Join-Path $OutDir ('store-{0}-{1}.json' -f $Label, $stamp)
$record = [ordered]@{
    kind              = 'native-s3-store-archive'
    label             = $Label
    timestamp         = (Get-Date).ToString('o')
    live              = $liveBefore
    independent       = $independent
    tauri_compat      = $artifactsForTauri
    checks            = $checks
    failed            = $failed
    sample            = $w
}
$jsonText = $record | ConvertTo-Json -Depth 10
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($archivePath, $jsonText, $utf8NoBom)
Write-Host ("archived: {0}" -f $archivePath)

if (-not $KeepWork) { Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue }
else { Write-Host ("work dir kept: {0}" -f $work) }

if ($failed.Count -gt 0) {
    Write-Host ("RESULT: FAIL ({0})" -f ($failed -join ', '))
    exit 1
}
Write-Host 'RESULT: PASS'
exit 0
