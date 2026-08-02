# Description: Verify the Windows XHTML reader slice and record local benchmarks.

[CmdletBinding()]
param(
    [string]$BookRoot,
    [string]$Entry = 'EPUB/text/ch012.xhtml'
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if ([string]::IsNullOrWhiteSpace($BookRoot)) {
    $BookRoot = Join-Path $repoRoot 'fixtures/local/logic-1-2'
}
$BookRoot = (Resolve-Path -LiteralPath $BookRoot).Path
$entryPath = Join-Path $BookRoot ($Entry -replace '/', [IO.Path]::DirectorySeparatorChar)
if (-not (Test-Path -LiteralPath $entryPath -PathType Leaf)) {
    throw "Reader entry does not exist: $Entry"
}

. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot
$cargoPath = $env:ATHA_CARGO
$manifestPath = Join-Path $repoRoot 'Cargo.toml'
$hostPath = Join-Path $repoRoot 'target/debug/atha-reader-host.exe'
$runId = '{0}-{1}' -f [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds(), $PID
$benchmarkRoot = Join-Path $repoRoot 'artifacts/local/benchmarks'
$p95Limits = @{
    cold_start = 2000
    first_stable = 750
    hot_open = 120
    page_turn = 50
    font_reflow = 150
}

function Invoke-ReaderHost {
    param([string[]]$Arguments)

    $startInfo = [Diagnostics.ProcessStartInfo]::new($hostPath)
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    foreach ($argument in $Arguments) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::Start($startInfo)
    if (-not $process.WaitForExit(60000)) {
        $process.Kill($true)
        $process.WaitForExit()
        throw 'Reader host timed out.'
    }
    if ($process.ExitCode -ne 0) {
        throw "Reader host failed with exit code $($process.ExitCode)."
    }
}

function Get-PercentileSummary {
    param(
        [string]$Stage,
        [object[]]$Rows
    )

    $values = @(
        $Rows |
            Where-Object stage -EQ $Stage |
            ForEach-Object { [double]::Parse($_.duration_ms, [Globalization.CultureInfo]::InvariantCulture) } |
            Sort-Object
    )
    if ($values.Count -ne 10) {
        throw "Expected 10 $Stage samples, found $($values.Count)."
    }
    $p95 = $values[[Math]::Ceiling(0.95 * $values.Count) - 1]
    [pscustomobject]@{
        run_id = $runId
        stage = $Stage
        samples = $values.Count
        median_ms = (($values[4] + $values[5]) / 2).ToString('F3', [Globalization.CultureInfo]::InvariantCulture)
        p95_ms = $p95.ToString('F3', [Globalization.CultureInfo]::InvariantCulture)
        p95_limit_ms = $p95Limits[$Stage]
        within_limit = $p95 -le $p95Limits[$Stage]
    }
}

Push-Location $repoRoot
try {
    & $cargoPath fmt --manifest-path $manifestPath --all --check
    if ($LASTEXITCODE -ne 0) { throw 'Reader formatting check failed.' }

    & $cargoPath clippy --manifest-path $manifestPath --workspace --all-targets --locked -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Reader clippy check failed.' }

    & $cargoPath test --manifest-path $manifestPath --workspace --all-targets --locked
    if ($LASTEXITCODE -ne 0) { throw 'Reader Rust tests failed.' }

    & $cargoPath build --manifest-path $manifestPath --package atha-reader-host --locked
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $hostPath -PathType Leaf)) {
        throw 'Reader host build failed.'
    }

    for ($sample = 1; $sample -le 10; $sample++) {
        Invoke-ReaderHost -Arguments @(
            '--book-root', $BookRoot,
            '--entry', $Entry,
            '--verify-sample',
            '--benchmark-run', $runId,
            '--sample', [string]$sample,
            '--benchmark', 'cold'
        )
    }
    Invoke-ReaderHost -Arguments @(
        '--book-root', $BookRoot,
        '--entry', $Entry,
        '--verify-sample',
        '--benchmark-run', $runId,
        '--sample', '1',
        '--benchmark', 'hot'
    )

    $rawFiles = @(Get-ChildItem -LiteralPath $benchmarkRoot -Filter "$runId-*.csv" -File)
    if ($rawFiles.Count -ne 11) {
        throw "Expected 11 raw benchmark files, found $($rawFiles.Count)."
    }
    $rows = @($rawFiles | ForEach-Object { Import-Csv -LiteralPath $_.FullName })
    $summaries = @(
        'cold_start',
        'first_stable',
        'hot_open',
        'page_turn',
        'font_reflow'
    ) | ForEach-Object { Get-PercentileSummary -Stage $_ -Rows $rows }
    $summaryPath = Join-Path $benchmarkRoot "$runId-summary.csv"
    $summaries | Export-Csv -LiteralPath $summaryPath -NoTypeInformation -Encoding utf8
    $summaries | Format-Table -AutoSize
    Write-Host "Reader benchmark summary: $summaryPath"
    $violations = @($summaries | Where-Object within_limit -EQ $false)
    if ($violations.Count) {
        throw "Reader benchmark P95 exceeded its limit: $($violations.stage -join ', ')"
    }
}
finally {
    Pop-Location
}
