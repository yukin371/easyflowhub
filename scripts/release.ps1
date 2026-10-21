<#
.SYNOPSIS
  Bump the EasyFlowHub version consistently, run the release gate, tag and push.
  GitHub Actions then builds, signs (via the TAURI_SIGNING_PRIVATE_KEY secret)
  and publishes the release together with a signed updater manifest.

  The script does NOT decide the version semantics - you pass the version. It
  guarantees the three version files stay in sync, the new version is a real
  bump, the tree is clean, the gate passes, and the tag matches.

.DESCRIPTION
  Version files kept in sync:
    - easyflowhub-app/package.json
    - easyflowhub-app/src-tauri/tauri.conf.json
    - easyflowhub-app/src-tauri/Cargo.toml
    - easyflowhub-app/src-tauri/Cargo.lock   (the easyflowhub package entry)

  A version without a hyphen (e.g. 0.2.1) is published as a formal release and
  becomes visible to the in-app updater (releases/latest). A version with a
  hyphen (e.g. 0.3.0-beta.1) is published as a pre-release and is NOT offered
  to the in-app updater.

.EXAMPLE
  .\scripts\release.ps1 0.2.1            # formal release
  .\scripts\release.ps1 0.3.0-beta.1    # pre-release
  .\scripts\release.ps1 0.2.1 -DryRun    # show the plan, change nothing
  .\scripts\release.ps1 0.2.1 -SkipSmoke # skip the scripted smoke gate
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$Version,

    [switch]$SkipSmoke,   # skip the scripted smoke gate
    [switch]$Force,       # skip the confirmation prompt and the clean-tree check
    [switch]$DryRun       # print the plan and exit without changing anything
)

$ErrorActionPreference = 'Stop'

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot   = Split-Path -Parent $scriptRoot

# ---------- semver helpers ----------

function Get-SemverParts {
    param([string]$V)
    $core = ($V -split '-', 2)[0]
    $pre  = if ($V.Contains('-')) { ($V -split '-', 2)[1] } else { $null }
    return @($core, $pre)
}

# Proper semver precedence: core first, then no-prerelease > prerelease, then
# dot-separated identifiers (numeric compares numerically, numeric < alphanumeric).
function Compare-Semver {
    param([string]$A, [string]$B)
    $aCore, $aPre = Get-SemverParts $A
    $bCore, $bPre = Get-SemverParts $B

    if ([version]$aCore -gt [version]$bCore) { return 1 }
    if ([version]$aCore -lt [version]$bCore) { return -1 }

    if (-not $aPre -and $bPre) { return 1 }
    if ($aPre -and -not $bPre) { return -1 }
    if (-not $aPre -and -not $bPre) { return 0 }

    $aIds = $aPre -split '\.'
    $bIds = $bPre -split '\.'
    $len = [Math]::Max($aIds.Count, $bIds.Count)
    for ($i = 0; $i -lt $len; $i++) {
        if ($i -ge $aIds.Count) { return -1 }
        if ($i -ge $bIds.Count) { return 1 }
        $ai = $aIds[$i]; $bi = $bIds[$i]
        $aNum = 0; $bNum = 0
        $aIsNum = [int]::TryParse($ai, [ref]$aNum)
        $bIsNum = [int]::TryParse($bi, [ref]$bNum)
        if ($aIsNum -and $bIsNum) {
            if ($aNum -gt $bNum) { return 1 }
            if ($aNum -lt $bNum) { return -1 }
        }
        elseif ($aIsNum) { return -1 }
        elseif ($bIsNum) { return 1 }
        else {
            $cmp = [string]::Compare($ai, $bi, [StringComparison]::Ordinal)
            if ($cmp -ne 0) { return [Math]::Sign($cmp) }
        }
    }
    return 0
}

# Replace the first regex match's captured version (group 1) with $NewVersion.
# Uses byte-faithful read/write so only the version text changes.
function Set-FileVersion {
    param([string]$Path, [string]$Pattern, [string]$NewVersion)
    $raw = [System.IO.File]::ReadAllText($Path)
    $m = [regex]::Match($raw, $Pattern)
    if (-not $m.Success) { throw "No version match in $(Split-Path $Path -Leaf) (pattern: $Pattern)" }
    $g = $m.Groups[1]
    $abs = $m.Index + $g.Index
    $updated = $raw.Remove($abs, $g.Length).Insert($abs, $NewVersion)
    [System.IO.File]::WriteAllText($Path, $updated)
    return $g.Value
}

# Push with retries; bypass a possibly-stale local http proxy that would
# otherwise route github.com through an unreachable 127.0.0.1 listener.
function Push-WithRetry {
    param([string[]]$Refs, [string]$Label)
    for ($i = 1; $i -le 8; $i++) {
        $reachable = Test-NetConnection -ComputerName github.com -Port 443 -InformationLevel Quiet -WarningAction SilentlyContinue
        if ($reachable) {
            git -c http.proxy= -c https.proxy= push @Refs
            if ($LASTEXITCODE -eq 0) {
                Write-Host "  pushed $Label" -ForegroundColor Green
                return $true
            }
        }
        Write-Host "  $Label push attempt $i failed; retrying in 15s..." -ForegroundColor DarkYellow
        Start-Sleep -Seconds 15
    }
    Write-Host "  $Label push FAILED after 8 attempts" -ForegroundColor Red
    return $false
}

# ---------- validate version ----------

$normalized = $Version.Trim() -replace '^v', ''
if ($normalized -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$') {
    Write-Host "Invalid version '$Version'. Expected X.Y.Z or X.Y.Z-suffix (e.g. 0.2.1, 0.3.0-beta.1)." -ForegroundColor Red
    exit 1
}
$newVersion = $normalized
$tag = "v$newVersion"
$isPreRelease = $newVersion.Contains('-')

# ---------- paths ----------

$pkgJson   = Join-Path $repoRoot 'easyflowhub-app\package.json'
$tauriConf = Join-Path $repoRoot 'easyflowhub-app\src-tauri\tauri.conf.json'
$cargoToml = Join-Path $repoRoot 'easyflowhub-app\src-tauri\Cargo.toml'
$cargoLock = Join-Path $repoRoot 'easyflowhub-app\src-tauri\Cargo.lock'

# tauri.conf.json is the source of truth for the current app version.
$tauriRaw = [System.IO.File]::ReadAllText($tauriConf)
$curMatch = [regex]::Match($tauriRaw, '"version"\s*:\s*"([^"]+)"')
if (-not $curMatch.Success) { Write-Host "Cannot find version in tauri.conf.json." -ForegroundColor Red; exit 1 }
$currentVersion = $curMatch.Groups[1].Value

if ((Compare-Semver $newVersion $currentVersion) -le 0) {
    Write-Host "New version $newVersion is not greater than current $currentVersion per semver." -ForegroundColor Red
    exit 1
}

# ---------- safety checks ----------

Push-Location $repoRoot
try {
    if (-not $Force) {
        $dirty = git status --porcelain
        if ($dirty) {
            Write-Host "Working tree is not clean. Commit or stash first (or pass -Force):" -ForegroundColor Red
            $dirty | Select-Object -First 20 | ForEach-Object { Write-Host "  $_" }
            exit 1
        }
    }
    if (git tag --list $tag) {
        Write-Host "Tag $tag already exists locally. Choose a different version or delete the tag." -ForegroundColor Red
        exit 1
    }
    if (git ls-remote --exit-code --tags origin "refs/tags/$tag") {
        Write-Host "Tag $tag already exists on origin." -ForegroundColor Red
        exit 1
    }

    # ---------- plan ----------

    Write-Host ""
    Write-Host "Release plan" -ForegroundColor Yellow
    Write-Host "  version:   $currentVersion -> $newVersion$(if ($isPreRelease) { '  (pre-release)' })"
    Write-Host "  tag:       $tag"
    Write-Host "  publish:   $(if ($isPreRelease) { 'pre-release (hidden from in-app updater)' } else { 'formal (visible to in-app updater)' })"
    Write-Host "  files:     package.json, tauri.conf.json, Cargo.toml, Cargo.lock"
    Write-Host "  gate:      $(if ($SkipSmoke) { 'skipped' } else { 'run-smoke.ps1' })"
    Write-Host "  then:      commit -> tag -> push; CI builds, signs and publishes"
    Write-Host ""

    if ($DryRun) {
        Write-Host "[DryRun] No changes made." -ForegroundColor Cyan
        return
    }

    if (-not $Force) {
        $answer = Read-Host "Proceed? [y/N]"
        if ($answer -notmatch '^[Yy]') { Write-Host "Aborted."; return }
    }

    # ---------- bump versions ----------

    Write-Host ""
    Write-Host "==> Bumping versions" -ForegroundColor Cyan
    $eOld = [regex]::Escape($currentVersion)
    $bumped = @(
        Set-FileVersion $pkgJson   "`"version"\s*:\s*`"$eOld`"" $newVersion
        Set-FileVersion $tauriConf "`"version"\s*:\s*`"$eOld`"" $newVersion
        Set-FileVersion $cargoToml "(?m)^version\s*=\s*`"$eOld`"" $newVersion
        Set-FileVersion $cargoLock "name = `"easyflowhub`"\r?\nversion = `"($eOld)`"" $newVersion
    )
    foreach ($f in @($pkgJson, $tauriConf, $cargoToml)) {
        Write-Host "  $(Split-Path $f -Leaf): $($bumped[[array]::IndexOf(@($pkgJson, $tauriConf, $cargoToml, $cargoLock), $f)]) -> $newVersion"
    }

    # Belt and suspenders: confirm all three app files now read $newVersion.
    foreach ($f in @($pkgJson, $tauriConf)) {
        $check = [regex]::Match([System.IO.File]::ReadAllText($f), '"version"\s*:\s*"([^"]+)"').Groups[1].Value
        if ($check -ne $newVersion) { throw "$(Split-Path $f -Leaf) did not bump to $newVersion (got $check)" }
    }
    $cargoCheck = [regex]::Match([System.IO.File]::ReadAllText($cargoToml), '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
    if ($cargoCheck -ne $newVersion) { throw "Cargo.toml did not bump to $newVersion (got $cargoCheck)" }
    Write-Host "  verified all three app files agree on $newVersion" -ForegroundColor Green

    # ---------- gate ----------

    if (-not $SkipSmoke) {
        Write-Host ""
        Write-Host "==> Release gate (run-smoke.ps1)" -ForegroundColor Cyan
        & (Join-Path $scriptRoot 'run-smoke.ps1')
        if ($LASTEXITCODE -ne 0) {
            Write-Host ""
            Write-Host "Gate failed. Reverting version bump." -ForegroundColor Red
            git checkout -- $pkgJson $tauriConf $cargoToml $cargoLock
            exit 1
        }
    }

    # ---------- commit + tag ----------

    Write-Host ""
    Write-Host "==> Committing and tagging" -ForegroundColor Cyan
    git add $pkgJson $tauriConf $cargoToml $cargoLock
    $commitMsg = "release: $tag`n`nBump version $currentVersion -> $newVersion across package.json, tauri.conf.json and Cargo.toml (Cargo.lock synced). Tagged for the Release workflow, which builds, signs with the GitHub secret and publishes the updater manifest."
    git commit -m $commitMsg
    if ($LASTEXITCODE -ne 0) { Write-Host "Commit failed." -ForegroundColor Red; exit 1 }
    git tag -a $tag -m "EasyFlowHub $tag"

    # ---------- push ----------

    Write-Host ""
    Write-Host "==> Pushing commit and tag" -ForegroundColor Cyan
    if (-not (Push-WithRetry -Refs @('origin', 'main') -Label 'main')) { exit 1 }
    if (-not (Push-WithRetry -Refs @('origin', $tag) -Label $tag)) { exit 1 }

    # ---------- done ----------

    Write-Host ""
    Write-Host "Tag $tag pushed." -ForegroundColor Green
    Write-Host "GitHub Actions is now building, signing and publishing $tag."
    Write-Host "  Watch:   gh run watch  (or the Actions tab)"
    Write-Host "  Release: https://github.com/yukin371/easyflowhub/releases/tag/$tag"
    if ($isPreRelease) {
        Write-Host "  Note:    pre-releases are hidden from the in-app updater." -ForegroundColor DarkYellow
    }
}
finally {
    Pop-Location
}
