param(
    [string]$Tag = '',
    [string]$Executable = '',
    [switch]$SkipInstaller
)
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location -LiteralPath $projectRoot
try {
    $metadata = cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed.' }
    $version = ($metadata.packages | Where-Object name -eq 'githly').version
    if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'Expected a stable X.Y.Z Cargo version.' }
    if ($Tag -and $Tag -ne "v$version") { throw "Tag $Tag does not match Cargo version $version." }
    if (-not $Executable) { $Executable = Join-Path $projectRoot 'target\release\githly.exe' }
    $Executable = (Resolve-Path -LiteralPath $Executable).Path
    $output = Join-Path $projectRoot 'dist\release'
    $stage = Join-Path $projectRoot "target\package-$version"
    New-Item -ItemType Directory -Path $output, $stage -Force | Out-Null
    Copy-Item -LiteralPath $Executable -Destination (Join-Path $stage 'Githly.exe') -Force
    Copy-Item -LiteralPath 'README.md', 'README.zh-CN.md' -Destination $stage -Force
    Copy-Item -LiteralPath 'vendor\gpui-component\LICENSE-APACHE' -Destination (Join-Path $stage 'GPUI-Component-LICENSE.txt') -Force
    $zipName = "Githly-$version-windows-x64.zip"
    Compress-Archive -LiteralPath (Join-Path $stage 'Githly.exe'), (Join-Path $stage 'README.md'), (Join-Path $stage 'README.zh-CN.md'), (Join-Path $stage 'GPUI-Component-LICENSE.txt') -DestinationPath (Join-Path $output $zipName) -Force
    $files = @((Join-Path $output $zipName))
    $sections = (Get-Content -LiteralPath 'CHANGELOG.md' -Encoding utf8 -Raw) -split '(?m)^## '
    $section = $sections | Where-Object { $_.StartsWith("v$version · ") } | Select-Object -First 1
    $notes = if ($section) { ($section -split "\n", 2)[1].Trim() } else { '' }
    function Write-UpdateManifest([string]$Asset, [string]$ManifestName) {
        $package = Get-Item -LiteralPath (Join-Path $output $Asset)
        $manifest = @{
            version = $version
            notes_url = "https://github.com/Hunlongyu/githly/releases/tag/v$version"
            notes = $notes
            installer = @{
                url = "https://github.com/Hunlongyu/githly/releases/download/v$version/$Asset"
                sha256 = (Get-FileHash -LiteralPath $package.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
                size = $package.Length
            }
        }
        $path = Join-Path $output $ManifestName
        $manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $path -Encoding utf8NoBOM
        return $path
    }
    $files += Write-UpdateManifest $zipName 'update-windows-x64-portable.json'
    if (-not $SkipInstaller) {
        $compiler = Get-Command ISCC.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -First 1
        if (-not $compiler -and (Test-Path -LiteralPath (Join-Path $projectRoot 'target\tools\inno-6.7.4\ISCC.exe'))) {
            $compiler = Join-Path $projectRoot 'target\tools\inno-6.7.4\ISCC.exe'
        }
        if (-not $compiler) { $compiler = Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe' }
        if (-not (Test-Path -LiteralPath $compiler)) { throw 'Run scripts/install-inno.ps1 or add Inno Setup 6 ISCC.exe to PATH.' }
        & $compiler "/DAppVersion=$version" "/DSourceExe=$Executable" "/DOutputPath=$output" 'installer\githly.iss'
        if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed.' }
        $installerName = "Githly-$version-windows-x64-setup.exe"
        $installer = Join-Path $output $installerName
        $files += $installer
        $files += Write-UpdateManifest $installerName 'update.json'
    }
    $lines = $files | ForEach-Object { '{0}  {1}' -f (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant(), [IO.Path]::GetFileName($_) }
    $lines | Set-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt') -Encoding utf8NoBOM
    Write-Output "Packaged Githly $version in $output"
} finally { Pop-Location }
