# Description: Generate the deterministic CBZ fixture and verify its prepared book root in the real WebView2 host.

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$fixturePath = Join-Path $repoRoot '.tmp/cbz-gate.cbz'
$importsRoot = Join-Path $repoRoot '.tmp/cbz-gate-imports'
$expectedFixtureSha256 = '5957e1a0daed2ed0a3a8b1439585cb7651d5478fe5cd51cde0401c7878eb30ed'
$validationLocalAppData = Join-Path $repoRoot '.tmp/cbz-gate-localappdata'
$hostPath = Join-Path $repoRoot 'target/debug/atha-reader-host.exe'

. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot
$cargoPath = $env:ATHA_CARGO

function Invoke-CheckedCargo {
    param([string[]] $Arguments, [string] $Failure)

    & $cargoPath @Arguments
    if ($LASTEXITCODE -ne 0) { throw $Failure }
}

function Invoke-ReaderHost {
    param([Parameter(Mandatory)][string]$BookRoot)

    $startInfo = [Diagnostics.ProcessStartInfo]::new($hostPath)
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.Environment['LOCALAPPDATA'] = $validationLocalAppData
    foreach ($argument in @(
        '--book-root',
        $BookRoot,
        '--manifest',
        '.atha-reader.json',
        '--verify-import'
    )) {
        [void] $startInfo.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::Start($startInfo)
    if (-not $process.WaitForExit(180000)) {
        $process.Kill($true)
        $process.WaitForExit()
        throw 'CBZ reader host timed out.'
    }
    if ($process.ExitCode -ne 0) {
        throw "CBZ reader host failed with exit code $($process.ExitCode)."
    }
}

$temporaryRoot = [IO.Path]::GetFullPath((Join-Path $repoRoot '.tmp')).TrimEnd('\') + '\'
foreach ($path in @($fixturePath, $importsRoot, $validationLocalAppData)) {
    $fullPath = [IO.Path]::GetFullPath($path)
    if (-not $fullPath.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Refusing to use a CBZ gate path outside the repository .tmp directory.'
    }
}

Push-Location $repoRoot
try {
    foreach ($path in @($fixturePath, $importsRoot, $validationLocalAppData)) {
        if (Test-Path -LiteralPath $path) {
            Remove-Item -LiteralPath $path -Recurse -Force
        }
    }
    [void] (New-Item -ItemType Directory -Path $validationLocalAppData)

    Invoke-CheckedCargo @('fmt', '--all', '--check') 'CBZ formatting check failed.'
    Invoke-CheckedCargo @('clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings') 'CBZ clippy check failed.'
    Invoke-CheckedCargo @('test', '--workspace', '--all-targets', '--locked') 'CBZ Rust tests failed.'
    Invoke-CheckedCargo @(
        'test',
        '--locked',
        '-p',
        'atha-backend',
        '--test',
        'cbz_import',
        'writes_cbz_gate_fixture',
        '--',
        '--ignored',
        '--exact'
    ) 'CBZ fixture generation failed.'

    if (-not (Test-Path -LiteralPath $fixturePath -PathType Leaf)) {
        throw 'The ignored CBZ fixture writer did not create the gate source.'
    }
    $fixtureSha256 = (Get-FileHash -LiteralPath $fixturePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($fixtureSha256 -ne $expectedFixtureSha256) {
        throw "Unexpected CBZ fixture SHA-256: $fixtureSha256"
    }

    $bookRoot = Join-Path $importsRoot $fixtureSha256
    $manifestPath = Join-Path $bookRoot '.atha-reader.json'
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw 'The ignored CBZ fixture writer did not create the prepared book root.'
    }
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.schema -ne 1 -or $manifest.contentVersion -ne $fixtureSha256) {
        throw 'Prepared CBZ reader manifest identity does not match the fixture.'
    }
    if ($manifest.sections.Count -ne 4 -or $manifest.resources.Count -ne 4 -or $manifest.toc.Count -ne 4) {
        throw "Unexpected CBZ import shape: sections=$($manifest.sections.Count) resources=$($manifest.resources.Count) toc=$($manifest.toc.Count)."
    }
    if ((@($manifest.toc.label) -join ',') -ne 'pages/1.png,pages/2.png,pages/3.png,pages/10.png') {
        throw 'Prepared CBZ reader manifest is not naturally ordered.'
    }

    Invoke-CheckedCargo @('build', '--package', 'atha-reader-host', '--locked') 'CBZ host build failed.'
    Invoke-ReaderHost -BookRoot $bookRoot

    [pscustomobject]@{
        fixture_sha256 = $fixtureSha256
        gate_sha256 = (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant()
        sections = $manifest.sections.Count
        resources = $manifest.resources.Count
        toc_items = $manifest.toc.Count
        evidence = 'real Windows WebView2 host'
    } | Format-List
}
finally {
    Pop-Location
    if (Test-Path -LiteralPath $validationLocalAppData) {
        Remove-Item -LiteralPath $validationLocalAppData -Recurse -Force
    }
}
