# Description: Verify the formal Atha backend workspace on Windows.

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$env:RUSTUP_DIST_SERVER = 'https://rsproxy.cn'
$env:RUSTUP_UPDATE_ROOT = 'https://rsproxy.cn/rustup'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$manifestPath = Join-Path $repoRoot 'Cargo.toml'

$cargoCommand = Get-Command cargo.exe -ErrorAction SilentlyContinue
if ($null -eq $cargoCommand) {
    $userProfile = [Environment]::GetFolderPath('UserProfile')
    $cargoPath = Join-Path $userProfile '.cargo\bin\cargo.exe'
    if (-not (Test-Path -LiteralPath $cargoPath -PathType Leaf)) {
        throw 'cargo.exe was not found.'
    }
} else {
    $cargoPath = $cargoCommand.Source
}

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
