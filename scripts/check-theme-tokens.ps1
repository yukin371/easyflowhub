param(
    [string]$AppRoot = "easyflowhub-app"
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

$managerRoot = Resolve-RepoPath (Join-Path $AppRoot "src\components\manager")
$sourceRoot = Resolve-RepoPath (Join-Path $AppRoot "src")
$sharedStylesPath = Join-Path $sourceRoot "styles.css"
$launcherStylesPath = Join-Path $sourceRoot "features\launcher\LauncherPanel.css"
$appearanceHookPath = Join-Path $sourceRoot "hooks\useAppearance.ts"
$relativeTargets = @(
    "SettingsPanel.tsx",
    "settings\GeneralSection.tsx",
    "settings\ModulesSection.tsx",
    "settings\ShortcutsSection.tsx",
    "settings\TranslationSection.tsx",
    "settings\TranscriptionSection.tsx",
    "settings\DeveloperSection.tsx",
    "settings\QuickNoteSection.tsx",
    "settings\TodoSection.tsx",
    "settings\TrashSection.tsx",
    "settings\EditorSection.tsx",
    "settings\NumberSettingInput.tsx",
    "mcp\McpPanel.tsx",
    "tasks\TasksPanel.tsx",
    "notes\StatusBar.tsx",
    "notes\NoteEditor.tsx",
    "BeautificationPanel.tsx",
    "ComponentPreviewPanel.tsx",
    "TrashPanel.tsx",
    "shared\SettingsSectionCard.tsx",
    "..\shared\EditorTextarea.tsx"
)

$forbiddenPatterns = @(
    "bg-white",
    "bg-gray-",
    "text-gray-",
    "bg-amber",
    "text-amber",
    "bg-red",
    "text-red",
    "bg-green",
    "text-green"
)

$violations = @()
foreach ($relativeTarget in $relativeTargets) {
    $path = Join-Path $managerRoot $relativeTarget
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Theme audit target not found: $path"
    }

    $violations += Select-String -LiteralPath $path -Pattern $forbiddenPatterns -SimpleMatch
}

if ($violations.Count -gt 0) {
    $violations | ForEach-Object { Write-Host $_.ToString() -ForegroundColor Red }
    throw "Found $($violations.Count) legacy hard-coded theme surface/status token(s)."
}

foreach ($path in @($sharedStylesPath, $launcherStylesPath, $appearanceHookPath)) {
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Shared theme audit target not found: $path"
    }
}

$sharedStyles = Get-Content -LiteralPath $sharedStylesPath -Raw
$launcherStyles = Get-Content -LiteralPath $launcherStylesPath -Raw
$appearanceHook = Get-Content -LiteralPath $appearanceHookPath -Raw
$launcherShell = [regex]::Match($launcherStyles, '(?s)\.launcher-shell\s*\{(?<body>[^}]*)\}')
if (-not $launcherShell.Success) {
    throw "Launcher theme token block not found: $launcherStylesPath"
}

$launcherTokens = $launcherShell.Groups["body"].Value -split "`r?`n" | Where-Object { $_ -match '^\s*--launcher-[\w-]+\s*:' }
$independentLauncherTokens = @($launcherTokens | Where-Object { $_ -notmatch 'var\(--manager-' })
if ($independentLauncherTokens.Count -gt 0) {
    throw "Launcher palette must alias shared manager tokens:`n$($independentLauncherTokens -join "`n")"
}

$quickTokenNames = @(
    "--quick-bg", "--quick-surface", "--quick-surface-raised", "--quick-input",
    "--quick-border", "--quick-text", "--quick-muted", "--quick-accent",
    "--quick-accent-soft", "--quick-shadow"
)
foreach ($tokenName in $quickTokenNames) {
    $pattern = "(?m)^\s*$([regex]::Escape($tokenName))\s*:\s*var\(--manager-[^)]+\);"
    if ([regex]::Matches($sharedStyles, $pattern).Count -lt 2) {
        throw "Quick Note token must use the shared palette in both themes: $tokenName"
    }
}

if ($sharedStyles -notmatch 'html\[data-blur=[''"]off[''"]\]') {
    throw "An explicit no-blur opaque surface fallback is required in $sharedStylesPath"
}
$noBlurRule = [regex]::Match(
    $sharedStyles,
    "html\[data-blur='off'\]\s+\.manager-backdrop,\s*html\[data-blur='off'\]\s+\.launcher-panel\s*\{(?<body>[^}]*)\}"
)
if (-not $noBlurRule.Success -or
    $noBlurRule.Groups["body"].Value -notmatch 'backdrop-filter:\s*none;' -or
    $noBlurRule.Groups["body"].Value -notmatch '-webkit-backdrop-filter:\s*none;') {
    throw "Turning blur off must disable backdrop filters for Manager and Launcher surfaces in $sharedStylesPath"
}
if ($sharedStyles -notmatch '@supports not \(backdrop-filter:\s*blur\(1px\)\)') {
    throw "Unsupported backdrop-filter needs an opaque fallback in $sharedStylesPath"
}
if ($sharedStyles -notmatch '@media\s*\(forced-colors:\s*active\)') {
    throw "Windows forced-colors token mapping is required in $sharedStylesPath"
}
if ($appearanceHook -notmatch 'dataset\.blur\s*=') {
    throw "Appearance hook must propagate the blur preference to window styles: $appearanceHookPath"
}

Write-Host "Theme token audit passed for $($relativeTargets.Count) Manager components, Launcher, Quick Note and fallback states." -ForegroundColor Green
