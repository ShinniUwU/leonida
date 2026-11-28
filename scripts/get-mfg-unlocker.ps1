# Fetches MFG Unlocker into %LOCALAPPDATA%\leonida\mfg-unlocker, outside this checkout. It is
# someone else's mod (github.com/dashdogy/MFG-Unlocker, MIT licence, about 5 MB) that lets DLSS
# Frame Generation run on GeForce RTX 20 and 30 cards. NVIDIA does not support that, it is not part
# of this repository, and it was not tested with this game. leonida.exe loads it only with
# LEONIDA_MFG_UNLOCKER=1 (F1, FRAME GENERATION, MFG UNLOCKER), from this folder under its own
# name: nothing is renamed or put beside the game. Asks before downloading.
#
#   powershell -ExecutionPolicy Bypass -File scripts\get-mfg-unlocker.ps1 [-Yes] [-To <folder>]
#
# Its release v1.7.2 is pinned: the zip and the DLL in it are checked against the SHA-256 its
# author published with that release (SHA256SUMS.txt); leonida.exe checks the DLL's again before
# it loads it. Its licence and notices are kept beside it (the Intel XeSS data in it is under
# Intel's licence). `leonida run` runs it only with LEONIDA_MFG_UNLOCKER on (turning it on was
# the yes) and the DLL not there yet.
#
# Ported from trustno1-coder/VIctory-NVIDIA (0cef07e, scripts/get-mfg-unlocker.ps1).
param(
    # Another folder than %LOCALAPPDATA%\leonida\mfg-unlocker (leonida.exe looks only there).
    [string]$To = '',
    # Do not ask.
    [switch]$Yes
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'   # Invoke-WebRequest's progress bar slows it down a lot
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor
    [Net.SecurityProtocolType]::Tls12
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

$dir = if ($To) { $To } else { Join-Path $env:LOCALAPPDATA 'leonida\mfg-unlocker' }
$dir = [IO.Path]::GetFullPath($dir)
if (($dir.TrimEnd('\') + '\').StartsWith($repo.TrimEnd('\') + '\', 'OrdinalIgnoreCase')) {
    throw "$dir is inside this checkout; the mod must stay outside it (pass -To)."
}
$version = '1.7.2'
# From the release's SHA256SUMS.txt (also GitHub's own digest of the zip). apps\leonida\src\mfg.rs
# holds the DLL's too.
$package = 'E002D624E3E7BA5CEDAB7211043C096FE8F555C30FF43500235A0686C7893F22'
$module = 'A9B1C01727FA72763BEF8F604E44EC165CE1C98272DE00719572B766A11CF28F'
$site = 'https://github.com/dashdogy/MFG-Unlocker'
$url = "$site/releases/download/v$version/MFGUnlocker-v$version.zip"
$target = Join-Path $dir 'MFGUnlocker.dll'

Write-Host "MFG Unlocker $version, someone else's mod (MIT licence), not part of Leonida Legacy"
Write-Host "  from: github.com/dashdogy/MFG-Unlocker (release v$version, about 5 MB)"
Write-Host "  to:   $dir"
Write-Host '  what: DLSS Frame Generation on RTX 20 and 30 cards. NVIDIA does not support it, and it'
Write-Host '        was not tested with this game. If the game fails, turn it off: F1, MFG UNLOCKER.'
if ((Test-Path -LiteralPath $target) -and
    (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash -eq $module) {
    Write-Host '  already there.'
    exit 0
}
if (-not $Yes) {
    $answer = Read-Host 'Download it now? [y/N]'
    if ($answer -notmatch '^(y|yes)$') { Write-Host 'Nothing downloaded.'; exit 1 }
}
New-Item -ItemType Directory -Force $dir | Out-Null
$part = Join-Path $dir "MFGUnlocker-v$version.zip.part"
try {
    if (Get-Command curl.exe -ErrorAction SilentlyContinue) {
        & curl.exe --location --fail --silent --show-error --progress-bar --output $part $url
        if ($LASTEXITCODE -ne 0) { throw "could not get $url" }
    } else {
        Invoke-WebRequest -Uri $url -OutFile $part -UseBasicParsing
    }
    $hash = (Get-FileHash -LiteralPath $part -Algorithm SHA256).Hash
    if ($hash -ne $package) { throw "the zip is not the v$version release (SHA-256 $hash); not kept" }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead($part)
    try {
        $entry = $zip.Entries | Where-Object Name -eq 'MFGUnlocker.dll' | Select-Object -First 1
        if (-not $entry) { throw 'the zip has no MFGUnlocker.dll' }
        $dll = "$target.part"
        [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $dll, $true)
        # Its licence and notices, kept beside it.
        foreach ($name in 'LICENSE', 'LICENSE.txt', 'LICENSE.md', 'NOTICE.md', 'NOTICE.txt',
                'README.md', 'README.txt') {
            $doc = $zip.Entries | Where-Object Name -eq $name | Select-Object -First 1
            if ($doc) {
                [IO.Compression.ZipFileExtensions]::ExtractToFile($doc, (Join-Path $dir $name), $true)
            }
        }
    } finally { $zip.Dispose() }
    $hash = (Get-FileHash -LiteralPath $dll -Algorithm SHA256).Hash
    if ($hash -ne $module) {
        Remove-Item -LiteralPath $dll -Force
        throw "MFGUnlocker.dll is not the v$version file (SHA-256 $hash); not kept"
    }
    Move-Item -LiteralPath $dll $target -Force
} finally {
    Remove-Item -LiteralPath $part -Force -ErrorAction SilentlyContinue
}
# Without a licence file in the zip: the one in the repository at that tag.
if (-not (Get-ChildItem -LiteralPath $dir -Filter 'LICENSE*')) {
    try {
        $licence = Join-Path $dir 'LICENSE'
        $raw = "https://raw.githubusercontent.com/dashdogy/MFG-Unlocker/v$version/LICENSE"
        Invoke-WebRequest -Uri $raw -OutFile $licence -UseBasicParsing
    } catch {
        Write-Host "  (its licence text could not be fetched: $($_.Exception.Message); MIT, see $site)"
    }
}
$mb = (Get-Item -LiteralPath $target).Length / 1MB
Write-Host ('  MFGUnlocker.dll  {0:N1} MB, v{1}, checked against the published SHA-256' -f $mb, $version)
Write-Host "  in $dir (its licence and notices beside it)"
Write-Host ''
Write-Host 'Done. It is loaded from the next start with LEONIDA_MFG_UNLOCKER=1 (F1, MFG UNLOCKER), DLSS'
Write-Host 'and DLSS frame generation on (the dlss-framegen profile sets both).'
