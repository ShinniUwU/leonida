# Renames what comes in from upstream (fxceless/victory-gta6 or VIctory) to this fork's names:
# VIctory -> Leonida Legacy, VICTORY -> LEONIDA, victory -> leonida, in the tracked text files and
# in tracked paths. Credits keep upstream's name: links, upstream commit and issue references,
# other forks' names (Marvisak/VIctory, VIctory-NVIDIA, fxceless/victory-fork), the
# README's credits and its copy of upstream's README. Running it on an already renamed tree changes
# nothing, so it is safe to run after every merge or cherry-pick from upstream:
#
#   git merge upstream/main   (or git cherry-pick <hash>)
#   powershell -ExecutionPolicy Bypass -File scripts\rename-from-upstream.ps1
#   cargo check --workspace --all-targets
#
# -DryRun lists what would change; -Also names untracked files to rewrite too (run-gta6.ps1).

param(
    [switch]$DryRun,
    [string[]]$Also = @(),
    [string]$Root = ''
)

$ErrorActionPreference = 'Stop'

if ($Root -eq '') {
    $Root = (git -C $PSScriptRoot rev-parse --show-toplevel).Trim()
}
$Root = (Resolve-Path -LiteralPath $Root).Path

$protectAll = @(
    'https?://[^\s)>\]]+',
    '\[victory-amd\]',
    'victory-gta6',
    '[Uu]pstream VIctory',
    'VIctory(?= #\d)',
    'VIctory(?= [0-9a-f]{7}\b)',
    # Other forks and repositories: VIctory-NVIDIA, Marvisak/VIctory, fxceless/victory-fork. A
    # lower-case owner/victory-name is a repository unless the owner is one of our own folders.
    '[\w.-]+/VIctory[\w-]*',
    'VIctory-[\w-]+',
    '(?<![\w./\-])(?!(?:crates|apps|src|docs|scripts|xtask|target)/)[\w.-]+/victory-[\w-]+',
    'victory-[\w-]+(?= #\d)',
    # Another fork's own switches, named as its: "foxhound's `VICTORY_WC_BAR`".
    '(?<=\w''s `)VICTORY_\w+'
)
$protectReadme = @('VIctory(?![A-Za-z])')

# This fork's own files, which name upstream on purpose.
$keepAsIs = @('CLAUDE.md', 'TODO.md')

function Convert-Name([string]$text, [string[]]$protect) {
    $saved = New-Object System.Collections.Generic.List[string]
    foreach ($p in $protect) {
        $text = [regex]::Replace($text, $p, {
            param($m)
            $saved.Add($m.Value)
            "`0$($saved.Count - 1)`0"
        })
    }
    $text = [regex]::Replace($text, 'VIctory(?=[A-Z])', 'Leonida')
    $text = $text.Replace('VIctory', 'Leonida Legacy').Replace('VICTORY', 'LEONIDA').Replace('victory', 'leonida')
    [regex]::Replace($text, "`0(\d+)`0", { param($m) $saved[[int]$m.Groups[1].Value] })
}

$self = (Resolve-Path -LiteralPath $PSCommandPath).Path
$strict = New-Object System.Text.UTF8Encoding($false, $true)
$tracked = @(git -C $Root -c core.quotepath=off ls-files)
if ($LASTEXITCODE -ne 0) { throw "git ls-files failed in $Root" }

$rewritten = 0
foreach ($rel in $tracked + $Also) {
    $path = Join-Path $Root $rel
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { continue }
    if ((Resolve-Path -LiteralPath $path).Path -eq $self) { continue }
    if ($keepAsIs -contains $rel) { continue }
    $bytes = [System.IO.File]::ReadAllBytes($path)
    if ([Array]::IndexOf($bytes, [byte]0) -ge 0) { continue }
    try { $text = $strict.GetString($bytes) } catch { Write-Host "not UTF-8, left alone: $rel"; continue }
    $protect = $protectAll
    if ($rel -eq 'README.md') { $protect = $protectAll + $protectReadme }
    $new = Convert-Name $text $protect
    if ($new -cne $text) {
        $rewritten++
        if ($DryRun) { Write-Host "would rewrite $rel" }
        else { [System.IO.File]::WriteAllBytes($path, $strict.GetBytes($new)) }
    }
}

$moved = 0
$emptied = @{}
foreach ($rel in $tracked) {
    if ($rel -cnotmatch 'victory') { continue }
    $dest = $rel.Replace('victory', 'leonida')
    if (Test-Path -LiteralPath (Join-Path $Root $dest)) {
        Write-Host "not moved, $dest already exists: $rel"
        continue
    }
    $moved++
    if ($DryRun) { Write-Host "would move $rel -> $dest"; continue }
    $parent = Split-Path -Parent (Join-Path $Root $dest)
    if (-not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent | Out-Null }
    git -C $Root mv -- $rel $dest
    if ($LASTEXITCODE -ne 0) { throw "git mv failed: $rel -> $dest" }
    $emptied[(Split-Path -Parent (Join-Path $Root $rel))] = $true
}

# git mv leaves the old folders behind once they are empty.
foreach ($dir in ($emptied.Keys | Sort-Object Length -Descending)) {
    $d = $dir
    while ($d.Length -gt $Root.Length -and (Test-Path -LiteralPath $d) -and
           -not (Get-ChildItem -LiteralPath $d -Force | Select-Object -First 1)) {
        Remove-Item -LiteralPath $d
        $d = Split-Path -Parent $d
    }
}

$verb = 'rewrote'
if ($DryRun) { $verb = 'would rewrite' }
Write-Host "$verb $rewritten files, moved $moved paths"
