# Builds Leonida Legacy and starts the game with the frame-rate switches measured to help
# (all off by default). Remove LEONIDA_INDIRECT_WINDOW if the device gets lost.
#
# The first time, it asks for the folder of your copy of the game (the one with
# MicrosoftGame.config in it) and offers to remember it in leonida.toml, which git ignores.
#
#   powershell -ExecutionPolicy Bypass -File .\run-gta6.ps1
#   powershell -ExecutionPolicy Bypass -File .\run-gta6.ps1 -GameDir 'D:\Games\GTA VI'   # this run only
#
# To change the remembered folder: target\release\leonida.exe config --game-dir <folder>

param(
    [string]$GameDir = ''
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

    & $exe run --execute
} finally {
    Pop-Location
}
