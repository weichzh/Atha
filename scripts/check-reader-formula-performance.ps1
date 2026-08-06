# Description: Benchmark a fixed formula-heavy EPUB chapter in the real Tauri WebView2 reader.

[CmdletBinding()]
param(
    [string]$Epub = 'fixtures/local/数理逻辑导引 (2017).epub',
    [switch]$FullChecks
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$epubPath = (Resolve-Path (Join-Path $repoRoot $Epub)).Path
$expectedHash = 'c316559b6428d05b7ba81228879606e05f9adf6f3e67df917f6c90ce77ff6708'
$actualHash = (Get-FileHash -LiteralPath $epubPath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualHash -ne $expectedHash) {
    throw "Unexpected formula benchmark EPUB SHA-256: $actualHash"
}

. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot

Push-Location $repoRoot
try {
    & $env:ATHA_PNPM --dir reader/app build
    if ($LASTEXITCODE -ne 0) { throw 'Tauri reader frontend build failed.' }

    & python scripts/export_reader_sample.py `
        --epub $epubPath `
        --entry EPUB/text/ch095.xhtml `
        --output logic-heavy-ch095 |
        Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Formula benchmark fixture export failed.' }

    $arguments = @(
        '-NoProfile', '-File', 'scripts/check-reader-slice.ps1',
        '-BookRoot', 'fixtures/local/logic-heavy-ch095',
        '-Entry', 'EPUB/text/ch095.xhtml',
        '-HostPackage', 'atha-reader-app',
        '-HostPath', 'target/debug/atha-reader-app.exe',
        '-BenchmarkProfile', 'formula-heavy'
    )
    if (-not $FullChecks) { $arguments += '-BenchmarkOnly' }
    & pwsh @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Formula benchmark failed.' }
}
finally {
    Pop-Location
}
