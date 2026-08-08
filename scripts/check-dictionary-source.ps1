# Description: Verify bounded offline dictionaries with private samples and optional real GUI gates.

[CmdletBinding()]
param(
    [string]$PrivateFixtures,
    [switch]$VerifyLinuxGui,
    [switch]$VerifyAndroid,
    [string]$Device
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot
$cargoPath = $env:ATHA_CARGO

function Invoke-Checked {
    param([string]$FilePath, [string[]]$Arguments, [string]$Failure)

    $output = & $FilePath @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) { throw $Failure }
    $output
}

if (($VerifyLinuxGui -or $VerifyAndroid) -and [string]::IsNullOrWhiteSpace($PrivateFixtures)) {
    throw 'GUI dictionary verification requires PrivateFixtures.'
}
if ($VerifyAndroid -and [string]::IsNullOrWhiteSpace($Device)) {
    throw 'VerifyAndroid requires an explicit physical Device serial.'
}

$resolvedFixtures = $null
if (-not [string]::IsNullOrWhiteSpace($PrivateFixtures)) {
    $resolvedFixtures = (Resolve-Path -LiteralPath $PrivateFixtures).Path
    if (-not (Test-Path -LiteralPath $resolvedFixtures -PathType Container)) {
        throw 'PrivateFixtures must name a local directory.'
    }
}

Push-Location $repoRoot
try {
    Invoke-Checked $env:ATHA_NODE @(
        '--test', 'reader/web/annotations.test.mjs'
    ) 'Dictionary selection event test failed.' | Out-Host
    Invoke-Checked $cargoPath @(
        'test', '--locked', '-p', 'atha-backend', '--test', 'dictionary_lookup'
    ) 'Dictionary integration tests failed.' | Out-Host
    Invoke-Checked $cargoPath @(
        'test', '--locked', '-p', 'atha-backend', 'reader::dictionary', '--lib'
    ) 'Dictionary unit tests failed.' | Out-Host

    if ($resolvedFixtures) {
        $env:ATHA_PRIVATE_DICTIONARY_ROOT = $resolvedFixtures
        try {
            Invoke-Checked $cargoPath @(
                'test', '--locked', '--release', '-p', 'atha-backend', '--test', 'dictionary_lookup',
                'private_mdict_sample_imports_and_looks_up_without_content_artifacts', '--', '--exact'
            ) 'Private MDict compatibility test failed.' | Out-Host
            Invoke-Checked $cargoPath @(
                'test', '--locked', '--release', '-p', 'atha-backend',
                'reader::dictionary::tests::private_kindle_sample_supports_sparse_exact_lookup',
                '--lib', '--', '--exact'
            ) 'Private Kindle compatibility test failed.' | Out-Host
            $benchmarkOutput = Invoke-Checked $cargoPath @(
                'test', '--locked', '--release', '-p', 'atha-backend',
                'reader::dictionary::tests::private_dictionary_benchmark',
                '--lib', '--', '--ignored', '--exact', '--nocapture'
            ) 'Private dictionary benchmark failed.'
        }
        finally {
            Remove-Item Env:ATHA_PRIVATE_DICTIONARY_ROOT -ErrorAction SilentlyContinue
        }
        $benchmarkLine = $benchmarkOutput | Where-Object { $_ -match '^dictionary_benchmark=' } | Select-Object -Last 1
        if (-not $benchmarkLine) { throw 'Dictionary benchmark did not emit aggregate evidence.' }
        $benchmark = $benchmarkLine.Substring('dictionary_benchmark='.Length) | ConvertFrom-Json
        if ($benchmark.peak_rss_kib -le 0 -or
            $benchmark.kindle_cold_lookup_p95_us -gt 500000 -or
            $benchmark.mdict_cold_lookup_p95_us -gt 500000 -or
            $benchmark.mdd_cold_lookup_p95_us -gt 500000 -or
            $benchmark.kindle_hot_lookup_p95_us -gt 100000 -or
            $benchmark.mdict_hot_lookup_p95_us -gt 100000 -or
            $benchmark.mdd_hot_lookup_p95_us -gt 100000 -or
            $benchmark.peak_rss_kib -gt 65536) {
            throw 'Dictionary benchmark exceeded the accepted latency or memory budget.'
        }
        $benchmark | Format-List
    }

    if ($VerifyLinuxGui) {
        & (Join-Path $PSScriptRoot 'check-fb2-source.ps1') `
            -VerifyLinuxGui `
            -DictionaryFixtureRoot $resolvedFixtures
        if ($LASTEXITCODE -ne 0) { throw 'Dictionary Linux GUI gate failed.' }
    }

    if ($VerifyAndroid) {
        $adb = Join-Path $env:ATHA_ANDROID_HOME 'platform-tools/adb'
        $ndkBin = Join-Path $env:ATHA_NDK_HOME 'toolchains/llvm/prebuilt/linux-x86_64/bin'
        $linker = Join-Path $ndkBin 'aarch64-linux-android26-clang'
        $archiver = Join-Path $ndkBin 'llvm-ar'
        foreach ($tool in @($adb, $linker, $archiver)) {
            if (-not (Test-Path -LiteralPath $tool -PathType Leaf)) { throw 'Android benchmark toolchain is incomplete.' }
        }
        $devices = @(& $adb devices) | Where-Object { $_ -match "^$([regex]::Escape($Device))\s+device$" }
        if ($devices.Count -ne 1) { throw 'The requested physical Android device is not ready.' }
        $model = (& $adb -s $Device shell getprop ro.product.model | Out-String).Trim()
        $abi = (& $adb -s $Device shell getprop ro.product.cpu.abi | Out-String).Trim()
        $emulated = (& $adb -s $Device shell getprop ro.kernel.qemu | Out-String).Trim()
        if ($model -ne 'PCT-AL10' -or $abi -ne 'arm64-v8a' -or $emulated -eq '1') {
            throw 'VerifyAndroid requires the physical PCT-AL10 arm64 device.'
        }

        $stageRoot = Join-Path $repoRoot '.tmp/dictionary-android-fixtures'
        $remoteRoot = "/data/local/tmp/atha-dictionary-$PID"
        $env:ATHA_PRIVATE_DICTIONARY_ROOT = $resolvedFixtures
        $env:ATHA_DICTIONARY_ANDROID_STAGE_ROOT = $stageRoot
        try {
            Invoke-Checked $cargoPath @(
                'test', '--locked', '-p', 'atha-backend',
                'reader::dictionary::tests::stage_private_dictionary_android_fixtures',
                '--lib', '--', '--ignored', '--exact'
            ) 'Could not stage anonymous Android dictionary fixtures.' | Out-Host
        }
        finally {
            Remove-Item Env:ATHA_PRIVATE_DICTIONARY_ROOT -ErrorAction SilentlyContinue
            Remove-Item Env:ATHA_DICTIONARY_ANDROID_STAGE_ROOT -ErrorAction SilentlyContinue
        }
        $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = $linker
        $env:CC_aarch64_linux_android = $linker
        $env:AR_aarch64_linux_android = $archiver
        try {
            Invoke-Checked $cargoPath @(
                'test', '--locked', '--release', '--target', 'aarch64-linux-android',
                '-p', 'atha-backend', 'reader::dictionary::tests::private_dictionary_benchmark',
                '--lib', '--no-run'
            ) 'Could not build the Android dictionary benchmark.' | Out-Host
        }
        finally {
            Remove-Item Env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER -ErrorAction SilentlyContinue
            Remove-Item Env:CC_aarch64_linux_android -ErrorAction SilentlyContinue
            Remove-Item Env:AR_aarch64_linux_android -ErrorAction SilentlyContinue
        }
        $testBinary = Get-ChildItem (Join-Path $repoRoot 'target/aarch64-linux-android/release/deps') -File |
            Where-Object { $_.Name -match '^atha_backend-[0-9a-f]+$' } |
            Sort-Object LastWriteTimeUtc -Descending |
            Select-Object -First 1
        if (-not $testBinary) { throw 'Android dictionary benchmark binary is missing.' }
        try {
            & $adb -s $Device shell rm '-rf' $remoteRoot | Out-Null
            & $adb -s $Device shell mkdir '-p' "$remoteRoot/fixtures" "$remoteRoot/work" | Out-Null
            & $adb -s $Device push "$stageRoot/." "$remoteRoot/fixtures" | Out-Null
            if ($LASTEXITCODE -ne 0) { throw 'Could not copy anonymous dictionary fixtures to PCT-AL10.' }
            & $adb -s $Device push $testBinary.FullName "$remoteRoot/benchmark" | Out-Null
            if ($LASTEXITCODE -ne 0) { throw 'Could not copy the dictionary benchmark to PCT-AL10.' }
            & $adb -s $Device shell chmod 700 "$remoteRoot/benchmark" | Out-Null
            $androidOutput = & $adb -s $Device shell env `
                "ATHA_PRIVATE_DICTIONARY_ROOT=$remoteRoot/fixtures" `
                "ATHA_DICTIONARY_BENCHMARK_ROOT=$remoteRoot/work" `
                "$remoteRoot/benchmark" `
                'reader::dictionary::tests::private_dictionary_benchmark' `
                '--ignored' '--exact' '--nocapture' 2>&1
            if ($LASTEXITCODE -ne 0) { throw 'PCT-AL10 dictionary benchmark failed.' }
            $androidLine = $androidOutput | Where-Object { $_ -match '^dictionary_benchmark=' } | Select-Object -Last 1
            if (-not $androidLine) { throw 'PCT-AL10 dictionary benchmark emitted no aggregate evidence.' }
            $android = $androidLine.Substring('dictionary_benchmark='.Length) | ConvertFrom-Json
            if ($android.peak_rss_kib -le 0 -or
                $android.kindle_cold_lookup_p95_us -gt 500000 -or
                $android.mdict_cold_lookup_p95_us -gt 500000 -or
                $android.mdd_cold_lookup_p95_us -gt 500000 -or
                $android.kindle_hot_lookup_p95_us -gt 100000 -or
                $android.mdict_hot_lookup_p95_us -gt 100000 -or
                $android.mdd_hot_lookup_p95_us -gt 100000 -or
                $android.peak_rss_kib -gt 65536) {
                throw 'PCT-AL10 dictionary benchmark exceeded the accepted latency or memory budget.'
            }
            [pscustomobject]@{
                device = $model
                abi = $abi
                kindle_cold_lookup_p95_us = $android.kindle_cold_lookup_p95_us
                kindle_hot_lookup_p95_us = $android.kindle_hot_lookup_p95_us
                mdict_cold_lookup_p95_us = $android.mdict_cold_lookup_p95_us
                mdict_hot_lookup_p95_us = $android.mdict_hot_lookup_p95_us
                mdd_cold_lookup_p95_us = $android.mdd_cold_lookup_p95_us
                mdd_hot_lookup_p95_us = $android.mdd_hot_lookup_p95_us
                peak_rss_kib = $android.peak_rss_kib
                evidence = 'physical Android release native benchmark'
            } | Format-List
        }
        finally {
            & $adb -s $Device shell rm '-rf' $remoteRoot 2>$null | Out-Null
            if (Test-Path -LiteralPath $stageRoot) { Remove-Item -LiteralPath $stageRoot -Recurse -Force }
        }
    }
}
finally {
    $androidStageRoot = Join-Path $repoRoot '.tmp/dictionary-android-fixtures'
    if (Test-Path -LiteralPath $androidStageRoot) {
        Remove-Item -LiteralPath $androidStageRoot -Recurse -Force
    }
    Pop-Location
}
