# Description: Build and verify the Tauri/Svelte reader with the real EPUB and shared performance gate.

[CmdletBinding()]
param(
    [string]$Epub = 'fixtures/local/数学及其历史 (2026).epub',
    [string]$BookRoot = 'fixtures/local/math-history-r8',
    [string]$Entry = 'EPUB/text/ch012.xhtml'
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$epubPath = (Resolve-Path (Join-Path $repoRoot $Epub)).Path
$bookRootPath = (Resolve-Path (Join-Path $repoRoot $BookRoot)).Path
$hostPath = Join-Path $repoRoot 'target/debug/atha-reader-app.exe'

. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot

function Invoke-Checked {
    param([string]$FilePath, [string[]]$Arguments)

    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$FilePath failed with exit code $LASTEXITCODE." }
}

function Invoke-TauriSmoke {
    $startInfo = [Diagnostics.ProcessStartInfo]::new($hostPath)
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    foreach ($argument in @('--epub', $epubPath, '--verify-import')) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::Start($startInfo)
    if (-not $process.WaitForExit(120000)) {
        $process.Kill($true)
        $process.WaitForExit()
        throw 'Tauri reader smoke test timed out.'
    }
    if ($process.ExitCode -ne 0) {
        throw "Tauri reader smoke test failed with exit code $($process.ExitCode)."
    }
}

Push-Location $repoRoot
try {
    Push-Location 'reader/app'
    try {
        Invoke-Checked $env:ATHA_PNPM @('install', '--frozen-lockfile')
        Invoke-Checked $env:ATHA_PNPM @('check')
        Invoke-Checked $env:ATHA_PNPM @('build')
    }
    finally {
        Pop-Location
    }

    Invoke-Checked 'pwsh' @(
        '-NoProfile', '-File', 'scripts/check-reader-slice.ps1',
        '-BookRoot', $bookRootPath,
        '-Entry', $Entry,
        '-HostPackage', 'atha-reader-app',
        '-HostPath', 'target/debug/atha-reader-app.exe'
    )
    Invoke-TauriSmoke
}
finally {
    Pop-Location
}
