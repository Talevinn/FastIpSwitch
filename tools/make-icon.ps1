# Génère assets\icon.ico, assets\tray32.rgba et assets\logo24.rgba
# à partir des PNG fournis dans assets\logo.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$logoDir = Join-Path $root 'assets\logo'
$assets = Join-Path $root 'assets'

function Write-Rgba([string]$source, [int]$size, [string]$destination) {
    $src = [System.Drawing.Bitmap]::new($source)
    $bmp = [System.Drawing.Bitmap]::new($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.DrawImage($src, 0, 0, $size, $size)
    $g.Dispose()
    $src.Dispose()

    $bytes = [byte[]]::new($size * $size * 4)
    $i = 0
    for ($y = 0; $y -lt $size; $y++) {
        for ($x = 0; $x -lt $size; $x++) {
            $c = $bmp.GetPixel($x, $y)
            $bytes[$i] = $c.R
            $bytes[$i + 1] = $c.G
            $bytes[$i + 2] = $c.B
            $bytes[$i + 3] = $c.A
            $i += 4
        }
    }
    $bmp.Dispose()
    [System.IO.File]::WriteAllBytes($destination, $bytes)
}

$sizes = 16, 20, 24, 32, 40, 48, 64, 128, 256
$entries = @()
foreach ($size in $sizes) {
    $png = Join-Path $logoDir "ip-swap-$size.png"
    if (-not (Test-Path $png)) { continue }
    $entries += [PSCustomObject]@{ Size = $size; Data = [System.IO.File]::ReadAllBytes($png) }
}

$icoPath = Join-Path $assets 'icon.ico'
$fs = [System.IO.File]::Create($icoPath)
$bw = [System.IO.BinaryWriter]::new($fs)
$bw.Write([UInt16]0)
$bw.Write([UInt16]1)
$bw.Write([UInt16]$entries.Count)

$offset = 6 + 16 * $entries.Count
foreach ($entry in $entries) {
    $dim = if ($entry.Size -ge 256) { 0 } else { $entry.Size }
    $bw.Write([Byte]$dim)
    $bw.Write([Byte]$dim)
    $bw.Write([Byte]0)
    $bw.Write([Byte]0)
    $bw.Write([UInt16]1)
    $bw.Write([UInt16]32)
    $bw.Write([UInt32]$entry.Data.Length)
    $bw.Write([UInt32]$offset)
    $offset += $entry.Data.Length
}
foreach ($entry in $entries) { $bw.Write($entry.Data) }
$bw.Flush()
$bw.Dispose()

Write-Rgba (Join-Path $logoDir 'ip-swap-32.png') 32 (Join-Path $assets 'tray32.rgba')
Write-Rgba (Join-Path $logoDir 'ip-swap-24.png') 24 (Join-Path $assets 'logo24.rgba')

Get-ChildItem $assets -File | Select-Object Name, Length
