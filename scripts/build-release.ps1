param([ValidateSet('all', 'installer', 'portable')][string]$Mode = 'all')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$config = Get-Content -LiteralPath (Join-Path $projectRoot 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
$version = $config.version
$target = 'x86_64-pc-windows-msvc'
$targetDir = Join-Path $projectRoot 'src-tauri\target'
$binaryDir = Join-Path $targetDir "$target\release"
$outputDir = Join-Path $projectRoot "release\$version-windows-x64"
$previousTargetDir = $env:CARGO_TARGET_DIR
Push-Location $projectRoot
try {
    & node (Join-Path $PSScriptRoot 'check-release.cjs')
    if ($LASTEXITCODE -ne 0) { throw 'Release configuration validation failed.' }
    $env:CARGO_TARGET_DIR = $targetDir
    $buildArgs = @('run', 'tauri', '--', 'build', '--target', $target)
    if ($Mode -eq 'portable') { $buildArgs += '--no-bundle' }
    else { $buildArgs += @('--bundles', 'nsis') }
    $buildArgs += @('--', '--locked')
    & npm.cmd @buildArgs
    if ($LASTEXITCODE -ne 0) { throw "Production build failed (exit code $LASTEXITCODE). No release files were published." }

    $portableSource = Join-Path $binaryDir 'filemelon.exe'
    & node (Join-Path $PSScriptRoot 'check-exe-icon.cjs') $portableSource
    if ($LASTEXITCODE -ne 0) { throw 'Executable icon verification failed. No release files were published.' }
    $installerSources = @()
    if ($Mode -ne 'portable') {
        $installerSources = @(Get-ChildItem -LiteralPath (Join-Path $binaryDir 'bundle\nsis') -File | Where-Object { $_.Name -like "filemelon_${version}_*-setup.exe" })
        if ($installerSources.Count -ne 1) { throw 'Expected exactly one matching NSIS installer.' }
    }
    if ($Mode -ne 'installer' -and -not (Test-Path -LiteralPath $portableSource -PathType Leaf)) { throw 'Portable executable is missing.' }

    New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
    if ($Mode -ne 'portable') {
        $installerOutput = Join-Path $outputDir "Filemelon-$version-windows-x64-setup.exe"
        Copy-Item -LiteralPath $installerSources[0].FullName -Destination $installerOutput -Force
        Write-Host "Installer: $installerOutput"
    }
    if ($Mode -ne 'installer') {
        $portableOutput = Join-Path $outputDir "Filemelon-$version-windows-x64-portable.exe"
        Copy-Item -LiteralPath $portableSource -Destination $portableOutput -Force
        # Notify Explorer about this file only; leave the user's icon cache intact.
        try {
            if (-not ('FilemelonShellNotify' -as [type])) {
                Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class FilemelonShellNotify {
    [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
    public static extern void SHChangeNotify(uint eventId, uint flags, string item1, IntPtr item2);
}
"@
            }
            [FilemelonShellNotify]::SHChangeNotify(0x2000, 0x1005, $portableOutput, [IntPtr]::Zero)
        } catch { Write-Warning "Explorer icon refresh was unavailable: $_" }
        Write-Host "Portable: $portableOutput"
        Write-Host 'Portable requires installed WebView2. Rules and logs remain in AppData.'
    }
    & node (Join-Path $PSScriptRoot 'write-checksums.cjs') $outputDir
    if ($LASTEXITCODE -ne 0) { throw 'Release checksum generation failed.' }
} finally {
    $env:CARGO_TARGET_DIR = $previousTargetDir
    Pop-Location
}



