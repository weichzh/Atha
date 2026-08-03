# Description: Verify the EPUB3 importer and open the fixed large sample in the real WebView2 host.

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$epubPath = (Resolve-Path (Join-Path $repoRoot 'fixtures/local/数学及其历史 (2026).epub')).Path
$expectedSourceHash = '0af5dff0c0d1eb369a096b18d05eb77a4cd9c03808748db8274d5e77bbfe7368'
$sourceHash = (Get-FileHash -LiteralPath $epubPath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($sourceHash -ne $expectedSourceHash) {
    throw "Unexpected EPUB fixture SHA-256: $sourceHash"
}
$validationLocalAppData = Join-Path $repoRoot '.tmp/m3-epub-source-localappdata'
$manifestPath = Join-Path $validationLocalAppData "Atha/ImportedBooks/$sourceHash/.atha-reader.json"
$hostPath = Join-Path $repoRoot 'target/debug/atha-reader-host.exe'

. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot
$cargoPath = $env:ATHA_CARGO

function Invoke-CheckedCargo {
    param([string[]] $Arguments, [string] $Failure)

    & $cargoPath @Arguments
    if ($LASTEXITCODE -ne 0) { throw $Failure }
}

function Invoke-ReaderHost {
    $startInfo = [Diagnostics.ProcessStartInfo]::new($hostPath)
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.Environment['LOCALAPPDATA'] = $validationLocalAppData
    foreach ($argument in @('--epub', $epubPath, '--verify-import')) {
        [void] $startInfo.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::Start($startInfo)
    if (-not $process.WaitForExit(180000)) {
        $process.Kill($true)
        $process.WaitForExit()
        throw 'EPUB reader host timed out.'
    }
    if ($process.ExitCode -ne 0) {
        throw "EPUB reader host failed with exit code $($process.ExitCode)."
    }
}

Push-Location $repoRoot
try {
    if (Test-Path -LiteralPath $validationLocalAppData) {
        Remove-Item -LiteralPath $validationLocalAppData -Recurse -Force
    }
    [void] (New-Item -ItemType Directory -Path $validationLocalAppData)
    Invoke-CheckedCargo @('fmt', '--all', '--check') 'EPUB formatting check failed.'
    Invoke-CheckedCargo @('clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings') 'EPUB clippy check failed.'
    Invoke-CheckedCargo @('test', '--workspace', '--all-targets', '--locked') 'EPUB Rust tests failed.'
    Invoke-CheckedCargo @('build', '--package', 'atha-reader-host', '--locked') 'EPUB host build failed.'
    Invoke-ReaderHost

    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw "Imported reader manifest is missing: $manifestPath"
    }
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.schema -ne 1 -or $manifest.contentVersion -ne $sourceHash) {
        throw 'Imported reader manifest identity does not match the EPUB.'
    }
    if ($manifest.sections.Count -ne 173 -or $manifest.resources.Count -ne 2527 -or $manifest.toc.Count -ne 197) {
        throw "Unexpected import shape: sections=$($manifest.sections.Count) resources=$($manifest.resources.Count) toc=$($manifest.toc.Count)."
    }
    $manifestTimestamp = (Get-Item -LiteralPath $manifestPath).LastWriteTimeUtc
    Invoke-ReaderHost
    if ((Get-Item -LiteralPath $manifestPath).LastWriteTimeUtc -ne $manifestTimestamp) {
        throw 'Repeated EPUB open rewrote the completed import cache.'
    }
    [pscustomobject]@{
        source_sha256 = $sourceHash
        sections = $manifest.sections.Count
        resources = $manifest.resources.Count
        toc_items = $manifest.toc.Count
        cache_root = Split-Path -Parent $manifestPath
        evidence = 'real Windows WebView2 host'
    } | Format-List
}
finally {
    Pop-Location
    if (Test-Path -LiteralPath $validationLocalAppData) {
        Remove-Item -LiteralPath $validationLocalAppData -Recurse -Force
    }
}
