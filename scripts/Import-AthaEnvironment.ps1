# Description: Load and validate local Atha tool locations.
param(
    [Parameter(Mandatory)]
    [string]$RepoRoot
)

$environmentPath = Join-Path $RepoRoot 'env/local.ps1'
if (-not (Test-Path -LiteralPath $environmentPath -PathType Leaf)) {
    throw 'Missing env/local.ps1. Copy env/example.ps1 and set local tool paths.'
}

. $environmentPath

foreach ($name in @('ATHA_CARGO', 'ATHA_CMAKE', 'ATHA_CTEST', 'ATHA_NODE', 'ATHA_PNPM', 'ATHA_SQLITE3')) {
    $path = [Environment]::GetEnvironmentVariable($name, 'Process')
    if ([string]::IsNullOrWhiteSpace($path) -or -not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Environment variable $name must name an existing executable in env/local.ps1."
    }
}

$env:RUSTUP_DIST_SERVER = 'https://rsproxy.cn'
$env:RUSTUP_UPDATE_ROOT = 'https://rsproxy.cn/rustup'
