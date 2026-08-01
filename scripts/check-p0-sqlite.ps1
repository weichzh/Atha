# Description: Verify the P0 SQLite, FTS5, and transactional Outbox baseline.

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$sqlDir = Join-Path $repoRoot 'p0\sqlite'
$buildDir = [System.IO.Path]::GetFullPath((Join-Path $repoRoot 'build\p0-sqlite'))
$databasePath = [System.IO.Path]::GetFullPath((Join-Path $buildDir 'atha-p0.sqlite'))
$buildPrefix = $buildDir.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot

if (-not $databasePath.StartsWith($buildPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'Generated database path escaped the build directory.'
}

$sqlitePath = $env:ATHA_SQLITE3

New-Item -ItemType Directory -Path $buildDir -Force | Out-Null
foreach ($generatedPath in @($databasePath, "$databasePath-wal", "$databasePath-shm")) {
    $fullGeneratedPath = [System.IO.Path]::GetFullPath($generatedPath)
    if (-not $fullGeneratedPath.StartsWith($buildPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw 'Generated SQLite path escaped the build directory.'
    }
    Remove-Item -LiteralPath $fullGeneratedPath -Force -ErrorAction SilentlyContinue
}

function Invoke-AthaSqlFile {
    param(
        [Parameter(Mandatory)]
        [string]$Path
    )

    $readCommand = '.read "' + $Path.Replace('\', '/') + '"'
    & $sqlitePath -bail $databasePath $readCommand
    if ($LASTEXITCODE -ne 0) {
        throw "SQLite check failed: $Path"
    }
}

Write-Output ('sqlite=' + (& $sqlitePath --version))
Invoke-AthaSqlFile (Join-Path $sqlDir 'schema.sql')
Invoke-AthaSqlFile (Join-Path $sqlDir 'check.sql')

$rollbackPath = (Join-Path $sqlDir 'rollback.sql').Replace('\', '/')
$rollbackCommand = '.read "' + $rollbackPath + '"'
$rollbackOutput = & $sqlitePath -bail $databasePath $rollbackCommand 2>&1
$rollbackExitCode = $LASTEXITCODE
if ($rollbackExitCode -eq 0) {
    throw 'The forced Outbox failure unexpectedly committed.'
}
$rollbackText = $rollbackOutput -join [Environment]::NewLine
if ($rollbackText -notmatch 'UNIQUE constraint failed: outbox_event\.id') {
    throw "The forced Outbox failure stopped for an unexpected reason: $rollbackText"
}
Write-Output 'forced_outbox_failure=rolled_back'

Invoke-AthaSqlFile (Join-Path $sqlDir 'verify_rollback.sql')

Invoke-AthaSqlFile (Join-Path $sqlDir 'benchmark_setup.sql')
$timer = [System.Diagnostics.Stopwatch]::StartNew()
Invoke-AthaSqlFile (Join-Path $sqlDir 'benchmark.sql')
$timer.Stop()
Invoke-AthaSqlFile (Join-Path $sqlDir 'benchmark_verify.sql')

Write-Output ("B-DB-001 local_smoke messages=10000 elapsed_ms={0}" -f $timer.ElapsedMilliseconds)
