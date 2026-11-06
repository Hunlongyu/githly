# Download the pinned upstream compiler into the ignored build directory.
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$toolsDirectory = Join-Path $projectRoot 'target\tools'
$compilerDirectory = Join-Path $toolsDirectory 'inno-6.7.4'
$installer = Join-Path $toolsDirectory 'innosetup-6.7.4.exe'
New-Item -ItemType Directory -Path $toolsDirectory -Force | Out-Null
if (-not (Test-Path -LiteralPath (Join-Path $compilerDirectory 'ISCC.exe'))) {
    if (-not (Test-Path -LiteralPath $installer)) {
        Invoke-WebRequest -Uri 'https://github.com/jrsoftware/issrc/releases/download/is-6_7_4/innosetup-6.7.4.exe' -OutFile $installer -TimeoutSec 300
    }
    $expectedHash = 'A549929319F9120716677C685535CA4671E4B937B5FDC87E2585F32D8EB30E8C'
    if ((Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash -ne $expectedHash) {
        throw 'Inno Setup download is incomplete or does not match the pinned release.'
    }
    $signature = Get-AuthenticodeSignature -LiteralPath $installer
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Pyrsys B\.V\.') {
        throw 'Inno Setup signature verification failed. Remove the incomplete download and retry.'
    }
    $arguments = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CURRENTUSER /PORTABLE=1 /NOICONS /TASKS="" /DIR="{0}"' -f $compilerDirectory
    $process = Start-Process -FilePath $installer -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "Inno Setup extraction failed: $($process.ExitCode)" }
}
if (-not (Test-Path -LiteralPath (Join-Path $compilerDirectory 'ISCC.exe'))) { throw 'Compiler was not extracted.' }
Write-Output (Join-Path $compilerDirectory 'ISCC.exe')
