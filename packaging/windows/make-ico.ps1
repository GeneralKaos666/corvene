# Writes packaging/windows/corvane.ico from assets/icon/Corvane-1024.png:
# the sizes Windows asks for (16 to 256 px), each stored as a PNG.
# Run again when the icon changes; the .ico is checked in.
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
$root = Resolve-Path "$PSScriptRoot\..\.."
$source = [System.Drawing.Image]::FromFile("$root\assets\icon\Corvane-1024.png")
$sizes = 16, 20, 24, 32, 40, 48, 64, 256
$images = foreach ($size in $sizes) {
    $bitmap = New-Object System.Drawing.Bitmap $size, $size
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $graphics.DrawImage($source, 0, 0, $size, $size)
    $graphics.Dispose()
    $stream = New-Object System.IO.MemoryStream
    $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
    $bitmap.Dispose()
    , $stream.ToArray()
}
$source.Dispose()

$out = New-Object System.IO.MemoryStream
$writer = New-Object System.IO.BinaryWriter $out
# ICONDIR: reserved, type 1 (icon), count
$writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    # ICONDIRENTRY: a width / height of 0 means 256
    $dimension = if ($sizes[$i] -ge 256) { 0 } else { $sizes[$i] }
    $writer.Write([byte]$dimension); $writer.Write([byte]$dimension)
    $writer.Write([byte]0); $writer.Write([byte]0)
    $writer.Write([uint16]1); $writer.Write([uint16]32)
    $writer.Write([uint32]$images[$i].Length); $writer.Write([uint32]$offset)
    $offset += $images[$i].Length
}
foreach ($image in $images) { $writer.Write($image) }
$writer.Flush()
[System.IO.File]::WriteAllBytes("$PSScriptRoot\corvane.ico", $out.ToArray())
"wrote $PSScriptRoot\corvane.ico ($($out.Length) bytes)"
