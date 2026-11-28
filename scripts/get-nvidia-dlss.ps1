# Fetches the NVIDIA files DLSS and its frame generation need from NVIDIA's own GitHub
# (github.com/NVIDIA-RTX/Streamline, release v2.14.16) into the data folder, outside this
# checkout: <data_dir>\nvidia\streamline-v2.14.16\bin\x64, where `leonida run` and
# `leonida dlss-check` look for them. They are NVIDIA's, under NVIDIA's licences (saved beside
# them: read them); this repository does not and may not carry them. Asks before downloading.
# Nothing of NVIDIA's is needed to build Leonida. `leonida run` runs this script only after a
# yes at its start question; the game itself never goes online.
#
#   powershell -ExecutionPolicy Bypass -File scripts\get-nvidia-dlss.ps1 [-Yes] [-To <folder>]
#
# Ported from trustno1-coder/VIctory-NVIDIA (959dcaf, scripts/get-nvidia-sdks.ps1, work of
# readmet3xt-lab); frame generation's four files (LEONIDA_FRAMEGEN) after the same, fc8b9ca.
param(
    [string]$StreamlineTag = 'v2.14.16',
    # Another folder than the data folder's nvidia\streamline-v2.14.16 (then set
    # LEONIDA_STREAMLINE_DIR to its bin\x64).
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

$sl = if ($To) { $To } else { Join-Path (Get-DataDir) "nvidia\streamline-$($StreamlineTag)" }
$sl = [IO.Path]::GetFullPath($sl)
if (($sl.TrimEnd('\') + '\').StartsWith($repo.TrimEnd('\') + '\', 'OrdinalIgnoreCase')) {
    throw "$sl is inside this checkout; NVIDIA's files must stay outside it (pass -To)."
}
$bin = Join-Path $sl 'bin\x64'
# What is loaded when the game runs with LEONIDA_DLSS: Streamline's interposer, its common
# plugin, the DLSS plugin and DLSS itself; with LEONIDA_FRAMEGEN also frame generation's
# plugin and model, Reflex and the latency markers' plugin. All eight are signed by NVIDIA.
$dlls = 'sl.interposer.dll', 'sl.common.dll', 'sl.dlss.dll', 'nvngx_dlss.dll', 'sl.dlss_g.dll',
    'nvngx_dlssg.dll', 'sl.reflex.dll', 'sl.pcl.dll'

# One file from a URL, with curl's progress line where Windows has curl.
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
    Move-Item $part $To -Force
}

# Bytes first..last of a URL.
function Get-Range([string]$Url, [long]$First, [long]$Last) {
    $q = [Net.HttpWebRequest]::Create($Url)
    $q.UserAgent = 'get-nvidia-dlss'
    $q.AddRange($First, $Last)
    $r = $q.GetResponse()
    try {
        if ([int]$r.StatusCode -ne 206) { throw 'the server does not give parts of the file' }
        $m = New-Object IO.MemoryStream
        $r.GetResponseStream().CopyTo($m)
        , $m.ToArray()
    } finally { $r.Close() }
}

# Takes the named files out of a zip on a server without downloading the whole zip: its
# directory is at its end, and each file in it can be asked for by its place. $Want: a regex
# on the paths in the zip. Each goes into $Into under the part of its path after the SDK's top
# folder.
function Get-FromZip([string]$Url, [string]$Want, [string]$Into) {
    $h = [Net.HttpWebRequest]::Create($Url)
    $h.Method = 'HEAD'
    $h.UserAgent = 'get-nvidia-dlss'
    $r = $h.GetResponse()
    $real = $r.ResponseUri.AbsoluteUri
    $length = $r.ContentLength
    $r.Close()
    $tailLen = [Math]::Min($length, 2MB)
    $tail = Get-Range $real ($length - $tailLen) ($length - 1)
    $end = -1
    for ($i = $tail.Length - 22; $i -ge 0; $i--) {
        if ($tail[$i] -eq 0x50 -and $tail[$i + 1] -eq 0x4b -and $tail[$i + 2] -eq 5 -and
            $tail[$i + 3] -eq 6) { $end = $i; break }
    }
    if ($end -lt 0) { throw 'no zip directory found' }
    $dirSize = [BitConverter]::ToUInt32($tail, $end + 12)
    $dirAt = [BitConverter]::ToUInt32($tail, $end + 16)
    if ($dirAt -eq [uint32]::MaxValue) { throw 'a zip64 archive' }
    $dir = if ($dirAt -ge $length - $tailLen) {
        $from = [int]($dirAt - ($length - $tailLen))
        , $tail[$from..($from + $dirSize - 1)]
    } else { Get-Range $real $dirAt ($dirAt + $dirSize - 1) }
    $at = 0
    while ($at + 46 -le $dir.Length -and [BitConverter]::ToUInt32($dir, $at) -eq 0x02014b50) {
        $method = [BitConverter]::ToUInt16($dir, $at + 10)
        $packed = [BitConverter]::ToUInt32($dir, $at + 20)
        $size = [BitConverter]::ToUInt32($dir, $at + 24)
        $nameLen = [BitConverter]::ToUInt16($dir, $at + 28)
        $extraLen = [BitConverter]::ToUInt16($dir, $at + 30)
        $noteLen = [BitConverter]::ToUInt16($dir, $at + 32)
        $headerAt = [BitConverter]::ToUInt32($dir, $at + 42)
        $name = [Text.Encoding]::UTF8.GetString($dir, $at + 46, $nameLen)
        $at += 46 + $nameLen + $extraLen + $noteLen
        if ($name -notmatch $Want) { continue }
        # The file's own header (its extra field may be longer than the directory's), then its
        # bytes.
        $piece = Get-Range $real $headerAt ($headerAt + 30 + $nameLen + 2048 + $packed - 1)
        if ([BitConverter]::ToUInt32($piece, 0) -ne 0x04034b50) { throw "no file header for $name" }
        $dataAt = 30 + [BitConverter]::ToUInt16($piece, 26) + [BitConverter]::ToUInt16($piece, 28)
        if ($dataAt + $packed -gt $piece.Length) {
            throw "the header of $name is longer than expected"
        }
        $out = [IO.MemoryStream]::new()
        if ($method -eq 0) { $out.Write($piece, $dataAt, [int]$packed) }
        elseif ($method -eq 8) {
            $in = [IO.MemoryStream]::new($piece, $dataAt, [int]$packed)
            $inflate = [IO.Compression.CompressionMode]::Decompress
            $z = [IO.Compression.DeflateStream]::new($in, $inflate)
            $z.CopyTo($out)
            $z.Close()
        } else { throw "$name is packed in a way this script does not know ($method)" }
        if ($out.Length -ne $size) { throw "$name came out $($out.Length) bytes long, not $size" }
        $rel = ($name -replace '^.*?(?=(bin/x64/|license\.txt$))', '') -replace '/', '\'
        $to = Join-Path $Into $rel
        New-Item -ItemType Directory -Force (Split-Path $to) | Out-Null
        [IO.File]::WriteAllBytes($to, $out.ToArray())
        Write-Host ('  {0}  {1:N1} MB' -f $rel, ($size / 1MB))
    }
}

# Every DLL must carry a valid Authenticode signature by NVIDIA; one that does not is deleted.
function Test-Signed {
    $bad = @()
    foreach ($name in $dlls) {
        $path = Join-Path $bin $name
        $sig = Get-AuthenticodeSignature -LiteralPath $path
        if ($sig.Status -ne 'Valid' -or $sig.SignerCertificate.Subject -notmatch 'NVIDIA') {
            Remove-Item -LiteralPath $path -Force
            $bad += "$name ($($sig.Status))"
        }
    }
    if ($bad) { throw "Not validly signed by NVIDIA, deleted: $($bad -join ', ')" }
}

Write-Host "NVIDIA Streamline $StreamlineTag with DLSS and DLSS Frame Generation"
Write-Host "  from: github.com/NVIDIA-RTX/Streamline (release $StreamlineTag)"
Write-Host "  to:   $bin"
Write-Host "  what: $($dlls -join ', ') and NVIDIA's licence files (about 70 MB; the whole"
Write-Host '        archive, 263 MB, only if taking the files out of it in place fails)'
Write-Host "  licence: NVIDIA's (Streamline licence.txt and the DLSS licence in bin\x64)"
$missing = @($dlls | Where-Object { -not (Test-Path (Join-Path $bin $_)) })
if (-not $missing) {
    Write-Host '  all there already.'
} else {
    if (-not $Yes) {
        $answer = Read-Host 'Download them now? [y/N]'
        if ($answer -notmatch '^(y|yes)$') { Write-Host 'Nothing downloaded.'; exit 1 }
    }
    $url = "https://github.com/NVIDIA-RTX/Streamline/releases/download/$StreamlineTag/" +
        "streamline-sdk-$StreamlineTag.zip"
    $names = ($dlls | ForEach-Object { [regex]::Escape($_) }) -join '|'
    $want = "(^|/)bin/x64/($names|[^/]*license[^/]*\.txt)$|^([^/]+/)?license\.txt$"
    try { Get-FromZip $url $want $sl }
    catch {
        $why = $_.Exception.Message
        Write-Host "  Taking the files out of the archive in place did not work ($why)."
    }
    if (@($dlls | Where-Object { -not (Test-Path (Join-Path $bin $_)) })) {
        Write-Host '  Getting the whole archive instead (263 MB) ...'
        $zip = Join-Path $sl "streamline-sdk-$StreamlineTag.zip"
        Get-File $url $zip
        $into = Join-Path $sl 'streamline-unpacked'
        Expand-Archive -Path $zip -DestinationPath $into -Force
        $found = Get-ChildItem $into -Recurse -Filter 'sl.interposer.dll' |
            Where-Object {
                $_.FullName -match '\\bin\\x64\\sl\.interposer\.dll$' -and
                $_.FullName -notmatch '\\development\\'
            } |
            Select-Object -First 1
        if (-not $found) { throw 'The Streamline archive has no bin\x64\sl.interposer.dll.' }
        $top = Split-Path (Split-Path (Split-Path $found.FullName))
        New-Item -ItemType Directory -Force $bin | Out-Null
        Get-ChildItem (Join-Path $top 'bin\x64') -File |
            Where-Object { $dlls -contains $_.Name -or $_.Name -match 'license.*\.txt$' } |
            Copy-Item -Destination $bin -Force
        Copy-Item (Join-Path $top 'license.txt') $sl -Force -ErrorAction SilentlyContinue
        Remove-Item $into -Recurse -Force -ErrorAction SilentlyContinue
        Remove-Item $zip -Force
    }
}
$still = @($dlls | Where-Object { -not (Test-Path (Join-Path $bin $_)) })
if ($still) { throw "Not there after all: $($still -join ', ')" }
Test-Signed
Write-Host "  licences: $(Join-Path $sl 'license.txt') and the *license*.txt files in $bin"
Write-Host ''
Write-Host 'Done. Check them with `leonida dlss-check`, then set LEONIDA_DLSS=quality (or balanced,'
Write-Host 'performance, dlaa) for a run. On an RTX 40 or 50 card LEONIDA_FRAMEGEN=1 adds frame'
Write-Host 'generation (or use the dlss-framegen profile).'
