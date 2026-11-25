# Runs the game unattended and takes frame shots (LEONIDA_SHOT) at fixed frames, then closes it.
# Uses the diagnostics build, like measure-gta6.ps1. Shots go under %LOCALAPPDATA%\leonida\shots:
# per shot, the frame's commands (events.txt), the card's clock (stamps.txt) and the picture each
# pass left (snap-*.png). They come from your game: keep them on your PC, never post them.
#
#   powershell -ExecutionPolicy Bypass -File .\shot-run.ps1 -Keys '30:Enter,35:Enter' -ShotAt '300,600'
#
# -Keys      LEONIDA_KEYS: <seconds>:<key>,... pressed after start, to get from the menus into play
# -ShotAt    frames to shoot, counted from the first frame of 500 draws or more (the world)
# -Seconds   the most the run may take before the game is closed

param(
    [string]$Keys = '',
    [string]$ShotAt = '300,600',
    [int]$Seconds = 600,
    [string]$ShotDir = (Join-Path $env:LOCALAPPDATA 'leonida\shots')
)

$ErrorActionPreference = 'Stop'

Push-Location $PSScriptRoot
try {
    cargo build --profile diag -p leonida --features diagnostics
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE); is leonida.exe still running?" }
    $exe = Join-Path $PSScriptRoot 'target\diag\leonida.exe'

    $line = & $exe config | Where-Object { $_ -match '^game_dir\s*:' } | Select-Object -First 1
    if ($line -notmatch '^game_dir\s*:\s*(.+?)\s*$' -or $Matches[1] -match '\(not set\)|\(does not exist\)') {
        throw 'no game folder saved: run run-gta6.ps1 once and let it remember the folder'
    }

    $run = Join-Path $ShotDir (Get-Date -Format 'yyyyMMdd-HHmmss')
    New-Item -ItemType Directory -Force $run | Out-Null
    $wanted = @($ShotAt -split ',' | Where-Object { $_.Trim() -ne '' }).Count

    $env:LEONIDA_SHOT = $run
    $env:LEONIDA_SHOT_AT = $ShotAt
    if ($Keys -ne '') { $env:LEONIDA_KEYS = $Keys }
    $env:LEONIDA_LOG = 'warn'
    $env:LEONIDA_ASYNC_SUBMIT = '1'
    $env:LEONIDA_QUEUE2 = '1'
    $env:LEONIDA_INDIRECT_WINDOW = '1'
    $env:LEONIDA_COPY_PLAIN = '1'
    $env:LEONIDA_JOIN_PARTS = '1'
    $env:LEONIDA_BACK_OWN_VIEW = '1'
    $env:LEONIDA_INDIRECT_GATHER = '1'
    $env:LEONIDA_CARD_REHASH_RUNS = '1'

    Write-Host "shots go to $run; the game is closed after $Seconds s or when $wanted shot(s) are written"
    $log = Join-Path $run 'run.log'
    $game = Start-Process -FilePath $exe -ArgumentList 'run', '--execute' -PassThru -NoNewWindow `
        -RedirectStandardError $log -RedirectStandardOutput (Join-Path $run 'stdout.log')
    $began = Get-Date
    while (-not $game.HasExited -and ((Get-Date) - $began).TotalSeconds -lt $Seconds) {
        $done = @(Get-ChildItem -Path $run -Recurse -Filter 'done' -ErrorAction SilentlyContinue).Count
        if ($done -ge $wanted) { break }
        Start-Sleep -Seconds 5
    }
    if (-not $game.HasExited) { Stop-Process -Id $game.Id -Force }
    $done = @(Get-ChildItem -Path $run -Recurse -Filter 'done' -ErrorAction SilentlyContinue).Count
    Write-Host "$done of $wanted shot(s) written to $run"
} finally {
    Pop-Location
}
