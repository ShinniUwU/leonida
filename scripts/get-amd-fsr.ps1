# Fetches the AMD files FSR 3 frame generation needs from AMD's own GitHub (the
# FidelityFX SDK, github.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK, release v2.4.0) into the
# data folder, outside this checkout: <data_dir>\amd\fidelityfx-v2.4.0, where `leonida run` looks
# for them. They are AMD's, under AMD's licence (saved beside them: read it); this repository does
# not and may not carry them. Asks before downloading. Each file is checked against the SHA-256 of
# the copy VIctory-NVIDIA tested and must carry AMD's valid signature; the game checks the
# signature again before it loads them. `leonida run` runs this script only after a yes at its
# start question; the game itself never goes online.
#
#   powershell -ExecutionPolicy Bypass -File scripts\get-amd-fsr.ps1 [-Yes] [-To <folder>]
#
# The release has no files of its own: its notes name the tag's source archive (about 200 MB) as
# the download, with the signed DLLs in Kits\FidelityFX\signedbin. The two files are taken from
# that tag directly (the same bytes).
#
# Ported from trustno1-coder/VIctory-NVIDIA (10862a6, scripts/get-amd-fsr.ps1).
param(
    [string]$Tag = 'v2.4.0',
    # Another folder than the data folder's amd\fidelityfx-v2.4.0 (then set
    # LEONIDA_FIDELITYFX_DIR to it).
    [string]$To = '',
    # Do not ask.
    [switch]$Yes
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'   # Invoke-WebRequest's progress bar slows it down a lot
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor
    [Net.SecurityProtocolType]::Tls12
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

# The data folder as Leonida resolves it: `leonida config` if it is built, else
# LEONIDA_DATA_DIR, else the default.
function Get-DataDir {
    foreach ($exe in 'target\release\leonida.exe', 'target\debug\leonida.exe') {
        $path = Join-Path $repo $exe
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            $line = & $path config 2>$null | Select-String -Pattern '^data_dir\s*:\s*(.+)$'
            if ($line) { return $line.Matches[0].Groups[1].Value.Trim() }
        }
    }
    if ($env:LEONIDA_DATA_DIR) { return $env:LEONIDA_DATA_DIR }
    Join-Path $env:LOCALAPPDATA 'leonida\data'
}

$bin = if ($To) { $To } else { Join-Path (Get-DataDir) "amd\fidelityfx-$($Tag)" }
$bin = [IO.Path]::GetFullPath($bin)
if (($bin.TrimEnd('\') + '\').StartsWith($repo.TrimEnd('\') + '\', 'OrdinalIgnoreCase')) {
    throw "$bin is inside this checkout; AMD's files must stay outside it (pass -To)."
}
$base = "https://raw.githubusercontent.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK/" +
    "$Tag/Kits/FidelityFX"
# What the game loads, and the SHA-256 of the copies tested (FidelityFX SDK v2.4.0).
$files = [ordered]@{}
$files['amd_fidelityfx_loader_dx12.dll'] =
    'E2191D3C0828BE45EB8AFFD4E3EC0378F2293ADC345B772A7170EDCF3B021ECB'
$files['amd_fidelityfx_framegeneration_dx12.dll'] =
    'E6843252F58B0341A0D80F6A20BC0D840643EE95BC48C22CAC08745344577BB6'
$licence = Join-Path $bin 'amd-fidelityfx-license.md'

# One file from a URL into a .part beside its place, with curl's progress line where Windows has
# curl; the .part's path.
function Get-File([string]$Url, [string]$To) {
    New-Item -ItemType Directory -Force (Split-Path $To) | Out-Null
    $part = "$To.part"
    if (Get-Command curl.exe -ErrorAction SilentlyContinue) {
        & curl.exe --location --fail --silent --show-error --progress-bar --output $part $Url
        if ($LASTEXITCODE -ne 0) {
            Remove-Item $part -ErrorAction SilentlyContinue
            throw "could not get $Url"
        }
    } else {
        Invoke-WebRequest -Uri $Url -OutFile $part -UseBasicParsing
    }
    $part
}

# Whether a file is there with the SHA-256 tested.
function Test-Pinned([string]$Path, [string]$Hash) {
    (Test-Path -LiteralPath $Path) -and
        (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash -eq $Hash
}

Write-Host "AMD FSR 3 frame generation (FidelityFX SDK $Tag)"
Write-Host "  from: github.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK (release $Tag)"
Write-Host "  to:   $bin"
Write-Host "  what: $($files.Keys -join ', ') and AMD's licence"
Write-Host "  licence: AMD's (amd-fidelityfx-license.md beside them)"
$missing = @($files.Keys | Where-Object { -not (Test-Pinned (Join-Path $bin $_) $files[$_]) })
if (-not $missing) {
    Write-Host '  all there already.'
} else {
    if (-not $Yes) {
        $answer = Read-Host 'Download them now? [y/N]'
        if ($answer -notmatch '^(y|yes)$') { Write-Host 'Nothing downloaded.'; exit 1 }
    }
    New-Item -ItemType Directory -Force $bin | Out-Null
    foreach ($name in $missing) {
        $to = Join-Path $bin $name
        $part = Get-File "$base/signedbin/$name" $to
        $hash = (Get-FileHash -LiteralPath $part -Algorithm SHA256).Hash
        if ($hash -ne $files[$name]) {
            Remove-Item -LiteralPath $part -Force
            throw "$name is not the file this build was tested with (SHA-256 $hash); not kept"
        }
        $sig = Get-AuthenticodeSignature -LiteralPath $part
        $by = if ($sig.SignerCertificate) { $sig.SignerCertificate.Subject } else { 'nobody' }
        if ($sig.Status -ne 'Valid' -or $by -notmatch 'Advanced Micro Devices|\bAMD\b') {
            Remove-Item -LiteralPath $part -Force
            throw "$name is not validly signed by AMD ($($sig.Status), $by); not kept"
        }
        Move-Item -LiteralPath $part $to -Force
        Write-Host ('  {0}  {1:N1} MB' -f $name, ((Get-Item -LiteralPath $to).Length / 1MB))
    }
}
if (-not (Test-Path -LiteralPath $licence)) {
    try {
        $part = Get-File "$base/docs/license.md" $licence
        Move-Item -LiteralPath $part $licence -Force
    } catch {
        Write-Host "  (AMD's licence text could not be fetched: $($_.Exception.Message))"
    }
}
if (Test-Path -LiteralPath $licence) { Write-Host "  licence: $licence" }
Write-Host ''
Write-Host 'Done. They are used from the next start with LEONIDA_FSR_FRAMEGEN=1 and DLSS on (the'
Write-Host 'dlss-framegen profile sets both).'
