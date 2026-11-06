param([string]$Source = "$PSScriptRoot\logo-transparent.png")
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

# Re-encode the selected artwork without redrawing or changing its background.
$sourceImage = [Drawing.Image]::FromFile((Resolve-Path -LiteralPath $Source).Path)
$sizes = @(16, 24, 32, 48, 64, 128, 256)
$frames = [Collections.Generic.List[byte[]]]::new()
try {
    foreach ($size in $sizes) {
        $bitmap = [Drawing.Bitmap]::new($size, $size, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        $buffer = [IO.MemoryStream]::new()
        $attributes = [Drawing.Imaging.ImageAttributes]::new()
        try {
            $graphics.CompositingMode = [Drawing.Drawing2D.CompositingMode]::SourceCopy
            $graphics.CompositingQuality = [Drawing.Drawing2D.CompositingQuality]::HighQuality
            $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $graphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $attributes.SetWrapMode([Drawing.Drawing2D.WrapMode]::TileFlipXY)
            $graphics.Clear([Drawing.Color]::Transparent)
            $scale = [Math]::Min($size / $sourceImage.Width, $size / $sourceImage.Height)
            $width = [int][Math]::Round($sourceImage.Width * $scale)
            $height = [int][Math]::Round($sourceImage.Height * $scale)
            $destination = [Drawing.Rectangle]::new(($size - $width) / 2, ($size - $height) / 2, $width, $height)
            $graphics.DrawImage($sourceImage, $destination, 0, 0, $sourceImage.Width, $sourceImage.Height, [Drawing.GraphicsUnit]::Pixel, $attributes)
            $bitmap.Save($buffer, [Drawing.Imaging.ImageFormat]::Png)
            $frames.Add($buffer.ToArray())
        } finally { $attributes.Dispose(); $buffer.Dispose(); $graphics.Dispose(); $bitmap.Dispose() }
    }
} finally { $sourceImage.Dispose() }

$iconPath = Join-Path $PSScriptRoot 'githly.ico'
$iconStream = [IO.File]::Create($iconPath)
$writer = [IO.BinaryWriter]::new($iconStream)
try {
    $writer.Write([uint16]0)
    $writer.Write([uint16]1)
    $writer.Write([uint16]$sizes.Count)
    $offset = 6 + 16 * $sizes.Count
    for ($index = 0; $index -lt $sizes.Count; $index++) {
        $encodedSize = if ($sizes[$index] -eq 256) { 0 } else { $sizes[$index] }
        $writer.Write([byte]$encodedSize)
        $writer.Write([byte]$encodedSize)
        $writer.Write([byte]0)
        $writer.Write([byte]0)
        $writer.Write([uint16]1)
        $writer.Write([uint16]32)
        $writer.Write([uint32]$frames[$index].Length)
        $writer.Write([uint32]$offset)
        $offset += $frames[$index].Length
    }
    foreach ($frame in $frames) { $writer.Write($frame) }
} finally { $writer.Dispose(); $iconStream.Dispose() }

# Decode the ICO itself, so the preview and size validation reflect the deliverable.
# GDI+ selects 128px for a requested 256px in a multi-frame PNG ICO; select the
# directory entry explicitly (ICO encodes 256 as zero) rather than its fallback.
function Read-IcoBitmap([string]$Path, [int]$Size) {
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ([BitConverter]::ToUInt16($bytes, 0) -ne 0 -or [BitConverter]::ToUInt16($bytes, 2) -ne 1) { throw 'Invalid ICO header' }
    $count = [BitConverter]::ToUInt16($bytes, 4)
    for ($index = 0; $index -lt $count; $index++) {
        $entry = 6 + 16 * $index
        $width = if ($bytes[$entry] -eq 0) { 256 } else { [int]$bytes[$entry] }
        if ($width -ne $Size) { continue }
        $length = [BitConverter]::ToUInt32($bytes, $entry + 8)
        $start = [BitConverter]::ToUInt32($bytes, $entry + 12)
        if ($start + $length -gt $bytes.Length) { throw 'Invalid ICO frame bounds' }
        $frame = [IO.MemoryStream]::new($bytes, $start, $length)
        $image = [Drawing.Image]::FromStream($frame)
        try {
            if ($image.Width -ne $Size -or $image.Height -ne $Size) { throw "ICO ${Size}px frame decoding failed" }
            return [Drawing.Bitmap]::new($image)
        } finally { $image.Dispose(); $frame.Dispose() }
    }
    throw "ICO ${Size}px frame missing"
}

$preview = [Drawing.Bitmap]::new(1000, 440)
$canvas = [Drawing.Graphics]::FromImage($preview)
$font = [Drawing.Font]::new('Segoe UI', 11, [Drawing.FontStyle]::Regular, [Drawing.GraphicsUnit]::Pixel)
$headingFont = [Drawing.Font]::new('Segoe UI', 17, [Drawing.FontStyle]::Regular, [Drawing.GraphicsUnit]::Pixel)
$darkBrush = [Drawing.SolidBrush]::new([Drawing.ColorTranslator]::FromHtml('#191A1B'))
$lightText = [Drawing.SolidBrush]::new([Drawing.ColorTranslator]::FromHtml('#BFBFBF'))
$darkText = [Drawing.SolidBrush]::new([Drawing.ColorTranslator]::FromHtml('#303030'))
try {
    $canvas.Clear([Drawing.ColorTranslator]::FromHtml('#FAFAFD'))
    $canvas.FillRectangle($darkBrush, 500, 0, 500, 440)
    $canvas.TextRenderingHint = [Drawing.Text.TextRenderingHint]::AntiAliasGridFit
    for ($panel = 0; $panel -lt 2; $panel++) {
        $baseX = $panel * 500
        $textBrush = if ($panel -eq 0) { $darkText } else { $lightText }
        $canvas.DrawString('Githly  /  ICO preview', $headingFont, $textBrush, $baseX + 28, 20)
        $largeBitmap = Read-IcoBitmap $iconPath 256
        try {
            if ($largeBitmap.Width -ne 256) { throw 'ICO 256px frame decoding failed' }
            $canvas.DrawImageUnscaled($largeBitmap, $baseX + 28, 65)
            if ($panel -eq 0) { $largeBitmap.Save((Join-Path $PSScriptRoot 'githly-256.png'), [Drawing.Imaging.ImageFormat]::Png) }
        } finally { $largeBitmap.Dispose() }
        $canvas.DrawString('256 px', $font, $textBrush, $baseX + 28, 331)
        $xPositions = @(28, 96, 170, 250, 350)
        for ($index = 0; $index -lt 5; $index++) {
            $size = $sizes[$index]
            $smallBitmap = Read-IcoBitmap $iconPath $size
            try {
                if ($smallBitmap.Width -ne $size) { throw "ICO ${size}px frame decoding failed" }
                $canvas.DrawImageUnscaled($smallBitmap, $baseX + $xPositions[$index], 353 + (64 - $size) / 2)
                $canvas.DrawString("$size px", $font, $textBrush, $baseX + $xPositions[$index], 420)
            } finally { $smallBitmap.Dispose() }
        }
        $mediumBitmap = Read-IcoBitmap $iconPath 128
        try {
            if ($mediumBitmap.Width -ne 128) { throw 'ICO 128px frame decoding failed' }
            $canvas.DrawImageUnscaled($mediumBitmap, $baseX + 330, 129)
            $canvas.DrawString('128 px', $font, $textBrush, $baseX + 330, 270)
        } finally { $mediumBitmap.Dispose() }
    }
    $preview.Save((Join-Path $PSScriptRoot 'logo-preview.png'), [Drawing.Imaging.ImageFormat]::Png)
} finally {
    $darkBrush.Dispose(); $lightText.Dispose(); $darkText.Dispose()
    $font.Dispose(); $headingFont.Dispose(); $canvas.Dispose(); $preview.Dispose()
}
Write-Output "Created and decoded all seven ICO sizes: $($sizes -join ', ')."
Get-Item -LiteralPath $iconPath | Select-Object FullName, Length
