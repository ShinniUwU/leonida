# Builds Leonida Legacy and starts the game with the frame-rate switches measured to help
# (all off by default). Remove LEONIDA_INDIRECT_WINDOW if the device gets lost.
#
# The first time, it asks for the folder of your copy of the game (the one with
# MicrosoftGame.config in it) and offers to remember it in leonida.toml, which git ignores.
#
#   powershell -ExecutionPolicy Bypass -File .\run-gta6.ps1
#   powershell -ExecutionPolicy Bypass -File .\run-gta6.ps1 -GameDir 'D:\Games\GTA VI'   # this run only
#   powershell -ExecutionPolicy Bypass -File .\run-gta6.ps1 -WarmUp yes   # warm up the shader cache without asking
#
# To change the remembered folder: target\release\leonida.exe config --game-dir <folder>

param(
    [string]$GameDir = '',
    # yes: warm up the shader cache before starting, no: skip it, ask: ask (default).
    [ValidateSet('ask', 'yes', 'no')]
    [string]$WarmUp = 'ask'
)

$ErrorActionPreference = 'Stop'

function Test-GameDir([string]$dir) {
    $dir -ne '' -and (Test-Path -LiteralPath (Join-Path $dir 'MicrosoftGame.config') -PathType Leaf)
}

# leonida.toml is read from the current folder.
Push-Location $PSScriptRoot
try {
    cargo build --release -p leonida
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE); is leonida.exe still running?" }
    $exe = Join-Path $PSScriptRoot 'target\release\leonida.exe'

    if ($GameDir -ne '') {
        if (-not (Test-GameDir $GameDir)) { throw "no MicrosoftGame.config in $GameDir" }
        $dir = $GameDir
    } else {
        # What leonida.toml or LEONIDA_GAME_DIR already says.
        $out = & $exe config
        if ($LASTEXITCODE -ne 0) { throw "leonida config failed (exit $LASTEXITCODE); check leonida.toml" }
        $dir = ''
        $line = $out | Where-Object { $_ -match '^game_dir\s*:' } | Select-Object -First 1
        if ($line -match '^game_dir\s*:\s*(.+?)\s*$') { $dir = $Matches[1] -replace '\s+\(does not exist\)$', '' }

        if (-not (Test-GameDir $dir)) {
            if ($dir -ne '' -and $dir -ne '(not set)') { Write-Host "The saved game folder has no MicrosoftGame.config: $dir" }
            do {
                $dir = (Read-Host 'Folder of your copy of the game (the one with MicrosoftGame.config in it)').Trim().Trim('"')
                if (-not (Test-GameDir $dir)) { Write-Host "No MicrosoftGame.config in '$dir', try again." }
            } until (Test-GameDir $dir)

            $answer = Read-Host 'Remember this folder for next time? [Y/n]'
            if ($answer.Trim() -match '^(y|yes)?$') {
                & $exe config --game-dir $dir | Out-Null
                if ($LASTEXITCODE -ne 0) { throw "saving the game folder failed (exit $LASTEXITCODE)" }
                Write-Host 'Saved to leonida.toml.'
            }
        }
    }
    $env:LEONIDA_GAME_DIR = $dir

    $env:LEONIDA_ASYNC_SUBMIT = '1'
    $env:LEONIDA_QUEUE2 = '1'
    $env:LEONIDA_INDIRECT_WINDOW = '1'
    $env:LEONIDA_COPY_PLAIN = '1'
    $env:LEONIDA_JOIN_PARTS = '1'
    $env:LEONIDA_BACK_OWN_VIEW = '1'
    $env:LEONIDA_INDIRECT_GATHER = '1'
    $env:LEONIDA_CARD_REHASH_RUNS = '1'

    # Shaders not in the cache are translated the first time the game uses them, mid-play.
    if ($WarmUp -eq 'ask') {
        Write-Host ''
        Write-Host 'Warm up the shader cache first? It translates every shader before the game starts:'
        Write-Host 'about 1 to 2 minutes the first time (and after an update), about 30 s when the cache is full.'
        Write-Host 'WARNING: without it, the game translates shaders while you play, so it stutters and runs'
        Write-Host 'slower until the cache fills up.'
        $answer = Read-Host 'Warm up now? [Y/n]'
        $WarmUp = if ($answer.Trim() -match '^(y|yes)?$') { 'yes' } else { 'no' }
    }
    if ($WarmUp -eq 'yes') {
        & $exe translate --warm all
        if ($LASTEXITCODE -ne 0) { Write-Warning "the warm-up failed (exit $LASTEXITCODE); starting anyway" }
    } else {
        Write-Warning 'starting without a warm-up: expect stutter until the shader cache fills up'
    }

    # Only warnings and errors in the console; set LEONIDA_LOG=info to see the GPU layer's status lines.
    if (-not $env:LEONIDA_LOG) { $env:LEONIDA_LOG = 'warn' }

    & $exe run --execute
} finally {
    Pop-Location
}
