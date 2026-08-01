# Description: Build and verify the P0 C ABI comparison on Windows.

[CmdletBinding()]
param(
    [ValidateRange(5, 101)]
    [int]$Samples = 31
)

$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$sourceDir = Join-Path $repoRoot 'p0\ffi'
$buildDir = Join-Path $repoRoot 'build\p0-ffi'
$rustManifest = Join-Path $sourceDir 'rust\Cargo.toml'
. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot

& $env:ATHA_CMAKE -S $sourceDir -B $buildDir -G 'Visual Studio 18 2026' -A x64
if ($LASTEXITCODE -ne 0) { throw 'CMake configure failed.' }

& $env:ATHA_CMAKE --build $buildDir --config Release
if ($LASTEXITCODE -ne 0) { throw 'C++ build failed.' }

& $env:ATHA_CTEST --test-dir $buildDir -C Release --output-on-failure
if ($LASTEXITCODE -ne 0) { throw 'C++ ABI check failed.' }

$cargoPath = $env:ATHA_CARGO

& $cargoPath fmt --manifest-path $rustManifest --check
if ($LASTEXITCODE -ne 0) { throw 'Rust formatting check failed.' }

& $cargoPath test --manifest-path $rustManifest
if ($LASTEXITCODE -ne 0) { throw 'Rust unit tests failed.' }

& $cargoPath build --manifest-path $rustManifest --release
if ($LASTEXITCODE -ne 0) { throw 'Rust build failed.' }

$runner = Join-Path $buildDir 'Release\atha_p0_ffi_runner.exe'
$cppLibrary = Join-Path $buildDir 'Release\atha_p0_cpp.dll'
$rustLibrary = Join-Path $sourceDir 'rust\target\release\atha_p0_ffi_rust.dll'

& $runner $cppLibrary $Samples
if ($LASTEXITCODE -ne 0) { throw 'C++ benchmark failed.' }

& $runner $rustLibrary $Samples
if ($LASTEXITCODE -ne 0) { throw 'Rust benchmark failed.' }
