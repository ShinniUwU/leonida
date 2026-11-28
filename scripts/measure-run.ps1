# One unattended run of the diagnostics build: the game is started with the profiles of
# gpu-profiles.json (plus the ones -Profile names, from gpu-profiles.local.json too), Enter is
# pressed every 8 s from 20 s on to get through the menus into the world, frame shots are taken a
# fixed number of frames after the world's first frame (so two runs can be compared shot by shot),
# the game is closed after -Seconds, and the steady part of the run is summed up in summary.txt.
# With -SkipScenes, Enter is also held in the world while the picture is letterboxed, to skip
# the opening's cutscenes and measure play (LEONIDA_SKIP_SCENES=1).
#
# Everything it writes goes under %LOCALAPPDATA%\leonida\runs (outside the checkout). Logs and
# shots come from your game: keep them on your PC, post only the figures of summary.txt.
#
#   cargo build --profile diag -p leonida --features diagnostics
#   powershell -ExecutionPolicy Bypass -File scripts\measure-run.ps1 -Label 01-base
#   powershell -ExecutionPolicy Bypass -File scripts\measure-run.ps1 -Label 02-levels -Profile my-test
#
# -Root       folder of a series of runs (each run is a folder in it, and summary.txt collects all)
# -Label      this run's folder name
# -Profile    LEONIDA_PROFILE for this run: base and the profiles named, whatever the card
# -Seconds    the most the run may take before the game is closed
# -From       seconds after the start from which frame lines are counted (the world is reached
#             at about 70 s with the key presses below; its first minute still streams)
# -ShotAfter  frames after the world's first frame to save (world-<n>.png, and world-<n>.view.jpg,
#             960 wide, for an agent to look at). At 8 fps a 330 s run has about 1,400 of them.
# -Console    series-x or series-s
# -SkipScenes    hold Enter while a cutscene is letterboxed, to get to play

param(
    [string]$Root = (Join-Path $env:LOCALAPPDATA ('leonida\runs\series-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))),
    [Parameter(Mandatory)] [string]$Label,
    [string]$Profile = '',
    [int]$Seconds = 330,
    [int]$From = 170,
    [string]$ShotAfter = '600,1000',
    [ValidateSet('series-x', 'series-s')] [string]$Console = 'series-x',
    [switch]$SkipScenes
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'leonida-settings.ps1')
$environmentBefore = Save-LeonidaEnvironment

function Get-Median([double[]]$xs) {
    if ($xs.Count -eq 0) { return [double]::NaN }
    $s = $xs | Sort-Object
    $n = $s.Count
    if ($n % 2) { $s[($n - 1) / 2] } else { ($s[$n / 2 - 1] + $s[$n / 2]) / 2 }
}

$repo = Split-Path -Parent $PSScriptRoot
if (Get-Process leonida -ErrorAction SilentlyContinue) { throw 'leonida.exe is already running: close it first' }
$exe = Join-Path $repo 'target\diag\leonida.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw 'no diagnostics build: run cargo build --profile diag -p leonida --features diagnostics' }
$run = Join-Path $Root $Label
New-Item -ItemType Directory -Force (Join-Path $run 'cap') | Out-Null

$crashBefore = (@(Get-ChildItem (Join-Path $env:LOCALAPPDATA 'leonida\data\*\crashes.log') -ErrorAction SilentlyContinue) | Measure-Object Length -Sum).Sum

# leonida.toml is read from the current folder.
Push-Location $repo
try {
    if ($PSBoundParameters.ContainsKey('Profile')) {
        if ($Profile) { $env:LEONIDA_PROFILE = $Profile }
        else { Remove-Item Env:LEONIDA_PROFILE -ErrorAction SilentlyContinue }
    }
    # Not the player's own choices from the F1 menu or the launcher: runs compare like with
    # like.
    $env:LEONIDA_SAVED_SETTINGS = '0'
    $graphics = Get-LeonidaGraphics $exe
    Show-LeonidaGraphics $graphics
    $switches = @(Get-LeonidaGraphicsSwitches $graphics -Short)
    $env:LEONIDA_KEYS = ((0..20) | ForEach-Object { '{0}:Enter' -f (20 + 8 * $_) }) -join ','
    # The taps get through the start screen; a cutscene wants Enter held, and for how long
    # depends on the frame rate, so the game's picture says when (letterbox bars).
    if ($SkipScenes) { $env:LEONIDA_SKIP_SCENES = '1' }
    else { Remove-Item Env:LEONIDA_SKIP_SCENES -ErrorAction SilentlyContinue }
    $env:LEONIDA_CAPTURE = Join-Path $run 'cap'
    # Shots a fixed number of frames after the world's first frame; the every-8th-frame world
    # pictures are pushed out of reach (each costs about 235 ms of its frame).
    $env:LEONIDA_CAPTURE_WORLD = '1'
    $env:LEONIDA_CAPTURE_WORLD_EVERY = '1000000'
    $env:LEONIDA_CAPTURE_WORLD_ALSO = $ShotAfter
    $env:LEONIDA_LOG = 'info'

    $start = (Get-Date).ToUniversalTime()
    $p = Start-Process $exe -PassThru -NoNewWindow -ArgumentList 'run', '--execute', '--console', $Console `
        -RedirectStandardError (Join-Path $run 'stderr.log') -RedirectStandardOutput (Join-Path $run 'stdout.log')
    $null = $p.Handle
    if ($p.WaitForExit($Seconds * 1000)) { $end = "exit $($p.ExitCode)" } else { taskkill /pid $p.Id /t /f | Out-Null; $end = 'timeout (closed)' }
    $null = $p.WaitForExit(15000)
} finally {
    Restore-LeonidaEnvironment $environmentBefore
    Pop-Location
}

$since = $start.AddSeconds($From)
$fps = @(); $ms = @(); $gpu = @(); $prep = @(); $back = @(); $draws = @(); $usual = @(); $median = @()
$world = $null; $bad = @(); $held = 0
foreach ($raw in [System.IO.File]::ReadLines((Join-Path $run 'stderr.log'))) {
    $line = $raw -replace "$([char]27)\[[0-9;]*m", ''
    if ($line -notmatch '^(\d{4}-\d\d-\d\dT[\d:.]+)Z') { continue }
    $t = [datetime]::Parse($Matches[1], [Globalization.CultureInfo]::InvariantCulture)
    if ($line -match 'device removed|DEVICE_REMOVED|DEVICE_HUNG|panicked' -or ($line -match 'leonida_gpu::drvwatch' -and $line -notmatch 'are watched')) {
        if ($bad.Count -lt 5) { $bad += $line.Substring(0, [Math]::Min(200, $line.Length)) }
    }
    if ($line -match 'cutscene skip: holding Enter') { $held++ }
    if ($line -match '([\d.]+) fps, ([\d.]+) ms/frame \(([\d.]+) preparing:[^;]*; ([\d.]+) on the GPU; ([\d.]+) copying back[^)]*\), (\d+) draws/frame') {
        if (-not $world -and [int]$Matches[6] -ge 500) { $world = ($t - $start).TotalSeconds }
        if ($t -ge $since) {
            $fps += [double]$Matches[1]; $ms += [double]$Matches[2]; $prep += [double]$Matches[3]
            $gpu += [double]$Matches[4]; $back += [double]$Matches[5]; $draws += [double]$Matches[6]
        }
    } elseif ($t -ge $since -and $line -match 'usual frame \(the mean of \d+ not long\): (\d+) ms') {
        $usual += [double]$Matches[1]
    } elseif ($t -ge $since -and $line -match 'frame times: \d+ frames in [\d.]+ s: shortest \d+ ms, median (\d+)') {
        $median += [double]$Matches[1]
    }
}

$crashAfter = (@(Get-ChildItem (Join-Path $env:LOCALAPPDATA 'leonida\data\*\crashes.log') -ErrorAction SilentlyContinue) | Measure-Object Length -Sum).Sum

# The capture names shots by frame number; the log names the world's first frame ("frames
# <first+2> and <first+6> are captured"). Each wanted shot is copied to world-<n>.png.
$cap = Join-Path $run 'cap'
$first = $null
$said = Select-String -Path (Join-Path $run 'stderr.log') -Pattern 'frames (\d+) and \d+ are captured' | Select-Object -First 1
if ($said) { $first = [long]$said.Matches[0].Groups[1].Value - 2 }
$shots = @()
if ($null -ne $first) {
    # (Not $from: PowerShell names are case-blind, and $From is an [int] parameter.)
    foreach ($k in @($ShotAfter -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ })) {
        $png = Join-Path $cap ('frame-{0}.png' -f ($first + [long]$k))
        if (Test-Path -LiteralPath $png) {
            Copy-Item -LiteralPath $png (Join-Path $cap "world-$k.png") -Force
            $shots += "world-$k.png"
        }
    }
    if ($shots.Count -gt 0) {
        $null = & (Join-Path $PSScriptRoot 'make-view.ps1') -Path $cap -Filter 'world-*.png'
    }
}

$summary = [ordered]@{
    label = $Label
    profile = if (@($graphics.profiles).Count) { $graphics.profiles -join ', ' } else { 'none' }
    console = $Console
    end = $end
    world_at_s = if ($world) { [Math]::Round($world) } else { 'never' }
    cutscenes_held = if (-not $SkipScenes) { 'off' } else { $held }
    lines = $fps.Count
    fps = [Math]::Round((Get-Median $fps), 1)
    ms_frame = [Math]::Round((Get-Median $ms), 1)
    usual_frame_ms = [Math]::Round((Get-Median $usual), 1)
    median_frame_ms = [Math]::Round((Get-Median $median), 1)
    gpu_ms = [Math]::Round((Get-Median $gpu), 1)
    preparing_ms = [Math]::Round((Get-Median $prep), 1)
    copying_back_ms = [Math]::Round((Get-Median $back), 1)
    draws = [Math]::Round((Get-Median $draws))
    crashes_log_grew = [bool]($crashAfter -gt $crashBefore)
    problems = $bad -join ' || '
    shots = $shots -join ', '
    switches = $switches -join ' '
}
$text = ($summary.GetEnumerator() | ForEach-Object { '{0}: {1}' -f $_.Key, $_.Value }) -join [Environment]::NewLine
$text | Out-File -Encoding utf8 (Join-Path $run 'summary.txt')
Add-Content -Encoding utf8 (Join-Path $Root 'summary.txt') ($text + [Environment]::NewLine)
$text
