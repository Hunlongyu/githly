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
        $manifest = @{
            version = $version
            notes_url = "https://github.com/Hunlongyu/githly/releases/tag/v$version"
            installer = @{
                url = "https://github.com/Hunlongyu/githly/releases/download/v$version/$installerName"
                sha256 = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
                size = (Get-Item -LiteralPath $installer).Length
            }
        }
        $manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $output 'update.json') -Encoding utf8NoBOM
        $files += Join-Path $output 'update.json'
    }
    $lines = $files | ForEach-Object { '{0}  {1}' -f (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant(), [IO.Path]::GetFileName($_) }
    $lines | Set-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt') -Encoding utf8NoBOM
    Write-Output "Packaged Githly $version in $output"
} finally { Pop-Location }
