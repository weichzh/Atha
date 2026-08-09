# Description: Run the formula-heavy EPUB benchmark through the Linux Tauri GUI gate.

[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Epub,
    [string]$Metadata = 'fixtures/local/logic-heavy-ch095/.atha-reader-sample.json',
    [ValidateRange(1, 100000)]
    [int]$MinimumFormulas = 1000,
    [ValidateRange(1, 10000)]
    [int]$MinimumPages = 10,
    [ValidateRange(0, 20)]
    [int]$GestureWarmupSamples = 5,
    [ValidateRange(1, 100)]
    [int]$GestureMeasureSamples = 20
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (-not (Test-Path -LiteralPath $Epub -PathType Leaf)) { throw 'Formula benchmark EPUB does not exist.' }
$epubPath = (Resolve-Path -LiteralPath $Epub).Path
$metadataPath = (Resolve-Path -LiteralPath (Join-Path $repoRoot $Metadata)).Path
$metadataRecord = Get-Content -LiteralPath $metadataPath -Raw | ConvertFrom-Json
if ($metadataRecord.source_sha256 -notmatch '^[0-9a-f]{64}$' -or [string]::IsNullOrWhiteSpace($metadataRecord.entry)) {
    throw 'Formula benchmark metadata is invalid.'
}
if ((Get-FileHash -LiteralPath $epubPath -Algorithm SHA256).Hash -ne $metadataRecord.source_sha256) {
    throw 'Formula benchmark EPUB does not match its private metadata.'
}

Push-Location $repoRoot
try {
    & pwsh -NoProfile -File scripts/check-fb2-source.ps1 `
        -VerifyLinuxGui `
        -FormulaBenchmarkEpub $epubPath `
        -FormulaBenchmarkEntry $metadataRecord.entry `
        -FormulaBenchmarkMinimumFormulas $MinimumFormulas `
        -FormulaBenchmarkMinimumPages $MinimumPages `
        -GestureWarmupSamples $GestureWarmupSamples `
        -GestureMeasureSamples $GestureMeasureSamples
    if ($LASTEXITCODE -ne 0) { throw 'Formula benchmark failed.' }
}
finally {
    Pop-Location
}
