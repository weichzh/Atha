# Description: Verify the formal Atha backend workspace on Windows.

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$manifestPath = Join-Path $repoRoot 'Cargo.toml'
. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot
$cargoPath = $env:ATHA_CARGO

& $cargoPath fmt --manifest-path $manifestPath --all --check
if ($LASTEXITCODE -ne 0) { throw 'Backend formatting check failed.' }

& $cargoPath clippy --manifest-path $manifestPath --workspace --all-targets --locked -- -D warnings
if ($LASTEXITCODE -ne 0) { throw 'Backend clippy check failed.' }

& $cargoPath test --manifest-path $manifestPath --workspace --all-targets --locked
if ($LASTEXITCODE -ne 0) { throw 'Backend test check failed.' }

$previousRustdocFlags = $env:RUSTDOCFLAGS
try {
    $env:RUSTDOCFLAGS = '-D warnings'
    & $cargoPath doc --manifest-path $manifestPath --workspace --no-deps --locked
    $docExitCode = $LASTEXITCODE
} finally {
    $env:RUSTDOCFLAGS = $previousRustdocFlags
}
if ($docExitCode -ne 0) { throw 'Backend documentation check failed.' }
