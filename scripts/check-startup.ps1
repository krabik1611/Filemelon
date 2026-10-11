param([Parameter(Mandatory = $true)][string]$Executable)
$ErrorActionPreference = 'Stop'
$directory = Join-Path ([System.IO.Path]::GetTempPath()) "filemelon-smoke-$([guid]::NewGuid())"
New-Item -ItemType Directory -Path $directory | Out-Null
$env:FILEMELON_SMOKE_DIR = $directory
try {
    $process = Start-Process -FilePath $Executable -PassThru
    $deadline = (Get-Date).AddSeconds(45)
    do {
        if (Test-Path (Join-Path $directory 'ready')) { break }
        if ($process.HasExited) { throw "Filemelon exited before readiness (code $($process.ExitCode)). See $(Join-Path $directory 'startup.log')" }
        Start-Sleep -Milliseconds 250
    } while ((Get-Date) -lt $deadline)
    if (-not (Test-Path (Join-Path $directory 'ready'))) { throw "Filemelon did not report readiness within 45 seconds. See $(Join-Path $directory 'startup.log')" }
    if (-not $process.HasExited) { $process.WaitForExit(5000) | Out-Null }
    if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force; throw 'Filemelon did not exit after the smoke check.' }
    Write-Host "Startup smoke check passed: $Executable"
} finally {
    Remove-Item Env:FILEMELON_SMOKE_DIR -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $directory -Recurse -Force -ErrorAction SilentlyContinue
}
