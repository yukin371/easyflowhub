param(
    [string]$ReleaseDir = "easyflowhub-app\src-tauri\target\release"
)

$ErrorActionPreference = "Stop"

function Resolve-RepoPath {
    param([string]$PathValue)

    $candidate = $PathValue
    if (-not [System.IO.Path]::IsPathRooted($candidate)) {
        $candidate = Join-Path (Get-Location) $candidate
    }

    return [System.IO.Path]::GetFullPath($candidate)
}

function Require-Artifacts {
    param(
        [string]$Label,
        [string[]]$Patterns
    )

    $matches = @()
    foreach ($pattern in $Patterns) {
        $matches += Get-ChildItem -Path $pattern -ErrorAction SilentlyContinue
    }

    $uniqueMatches = $matches | Sort-Object FullName -Unique
    if (-not $uniqueMatches) {
        throw "Missing expected artifact for $Label. Checked: $($Patterns -join ', ')"
    }

    Write-Host "[ok] $Label" -ForegroundColor Green
    $uniqueMatches | ForEach-Object {
        Write-Host "     $($_.FullName)" -ForegroundColor Gray
    }
}

$resolvedReleaseDir = Resolve-RepoPath $ReleaseDir
if (-not (Test-Path -LiteralPath $resolvedReleaseDir)) {
    throw "Release directory not found: $resolvedReleaseDir"
}

$bundleDir = Join-Path $resolvedReleaseDir "bundle"

Write-Host "Checking EasyFlowHub release artifacts..." -ForegroundColor Cyan
Write-Host "Release dir: $resolvedReleaseDir" -ForegroundColor Gray

Require-Artifacts -Label "NSIS installer" -Patterns @(
    (Join-Path $bundleDir "nsis\*.exe")
)

Require-Artifacts -Label "MSI installer" -Patterns @(
    (Join-Path $bundleDir "msi\*.msi")
)

Require-Artifacts -Label "Portable executable" -Patterns @(
    (Join-Path $resolvedReleaseDir "easyflowhub.exe"),
    (Join-Path $resolvedReleaseDir "EasyFlowHub.exe")
)

Write-Host ""
Write-Host "Artifact preflight passed." -ForegroundColor Green
Write-Host "Next step: follow docs/checklists/windows-package-smoke.md for manual Windows smoke." -ForegroundColor Yellow
