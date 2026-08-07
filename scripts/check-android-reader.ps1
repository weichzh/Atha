# Description: Build and verify the Android reader on the project 16 KiB x86_64 emulator.

[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [string]$EpubPath,
    [switch]$CleanAppData
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$appRoot = Join-Path $repoRoot 'reader\app'
$apkPath = Join-Path $appRoot 'src-tauri\gen\android\app\build\outputs\apk\universal\debug\app-universal-debug.apk'
$serial = 'emulator-5554'
$avdName = 'Atha_API_35_16K'
$package = 'com.atha.reader'
$documentsPackage = 'com.google.android.documentsui'
$resolvedEpub = $null

if ($CleanAppData -and [string]::IsNullOrWhiteSpace($EpubPath)) {
    throw '-CleanAppData requires -EpubPath so a clean reader slice is verified immediately.'
}
if (-not [string]::IsNullOrWhiteSpace($EpubPath)) {
    $resolvedEpub = (Resolve-Path -LiteralPath $EpubPath).Path
    if (-not (Test-Path -LiteralPath $resolvedEpub -PathType Leaf)) {
        throw 'EpubPath must name a local file.'
    }
    if ([IO.Path]::GetExtension($resolvedEpub) -ine '.epub') {
        throw 'EpubPath must have an .epub extension.'
    }
    if ((Get-Item -LiteralPath $resolvedEpub).Length -eq 0) {
        throw 'EpubPath must not be empty.'
    }
}

. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot

function Invoke-Checked {
    param(
        [Parameter(Mandatory)]
        [string]$FilePath,
        [string[]]$Arguments = @()
    )

    $output = & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$FilePath failed with exit code $LASTEXITCODE."
    }
    $output
}

foreach ($name in @('ATHA_JAVA_HOME', 'ATHA_ANDROID_HOME', 'ATHA_NDK_HOME')) {
    $path = [Environment]::GetEnvironmentVariable($name, 'Process')
    if ([string]::IsNullOrWhiteSpace($path) -or -not (Test-Path -LiteralPath $path -PathType Container)) {
        throw "Environment variable $name must name an existing directory in env/local.ps1."
    }
}

$javaHome = $env:ATHA_JAVA_HOME
$androidHome = $env:ATHA_ANDROID_HOME
$ndkHome = $env:ATHA_NDK_HOME
$javaRelease = Join-Path $javaHome 'release'
$adb = Join-Path $androidHome 'platform-tools\adb.exe'
$buildTools = Join-Path $androidHome 'build-tools\35.0.0'
$aapt2 = Join-Path $buildTools 'aapt2.exe'
$zipalign = Join-Path $buildTools 'zipalign.exe'
$androidPlatform = Join-Path $androidHome 'platforms\android-36\android.jar'
$readelf = Join-Path $ndkHome 'toolchains\llvm\prebuilt\windows-x86_64\bin\llvm-readelf.exe'

foreach ($tool in @($javaRelease, $adb, $aapt2, $zipalign, $androidPlatform, $readelf)) {
    if (-not (Test-Path -LiteralPath $tool -PathType Leaf)) {
        throw "Missing required Android tool: $tool"
    }
}
if ((Split-Path $ndkHome -Leaf) -ne '28.2.13676358') {
    throw 'Android gate requires NDK 28.2.13676358.'
}
if ((Get-Content -Raw -LiteralPath $javaRelease) -notmatch '(?m)^JAVA_VERSION="21(?:\.|")') {
    throw 'Android gate requires JDK 21.'
}
$nodeVersion = (Invoke-Checked $env:ATHA_NODE @('--version') | Out-String).Trim()
if ($nodeVersion -ne 'v24.1.0') {
    throw "Android gate requires Node v24.1.0, found $nodeVersion."
}

function Invoke-Adb {
    param([Parameter(ValueFromRemainingArguments)][string[]]$Arguments)
    Invoke-Checked $adb (@('-s', $serial) + $Arguments)
}

function Get-AppProcessId {
    $output = & $adb -s $serial shell pidof $package
    if ($LASTEXITCODE -ne 0) { return $null }
    $value = ($output | Out-String).Trim()
    if ($value -notmatch '^\d+$') { return $null }
    $value
}

function Assert-AppHealthy {
    param([Parameter(Mandatory)][string]$ExpectedProcessId)

    $currentProcessId = Get-AppProcessId
    if ($currentProcessId -ne $ExpectedProcessId) {
        throw 'Android reader process died or restarted unexpectedly.'
    }
    $allLogs = (Invoke-Adb logcat '-d' '-v' 'brief' | Out-String)
    $escapedProcessId = [regex]::Escape($ExpectedProcessId)
    if (
        $allLogs -match "(?im)Fatal signal \d+ .*\bpid $escapedProcessId\b" -or
        $allLogs -match '(?is)FATAL EXCEPTION:.*?Process:\s*com\.atha\.reader\b' -or
        $allLogs -match '(?im)ANR in com\.atha\.reader\b'
    ) {
        throw 'Android reader emitted a native fatal signal, uncaught exception, or ANR.'
    }
}

function Wait-AppLog {
    param(
        [Parameter(Mandatory)][string]$ProcessId,
        [Parameter(Mandatory)][string]$Pattern,
        [ValidateRange(1, 120)][int]$TimeoutSeconds = 30
    )

    $watch = [Diagnostics.Stopwatch]::StartNew()
    do {
        if ((Get-AppProcessId) -ne $ProcessId) {
            throw 'Android reader process died while waiting for an application event.'
        }
        $logs = (Invoke-Adb logcat '-d' "--pid=$ProcessId" '-v' 'brief' | Out-String)
        if ($logs -match $Pattern) { return $watch.ElapsedMilliseconds }
        Start-Sleep -Milliseconds 250
    } while ($watch.Elapsed.TotalSeconds -lt $TimeoutSeconds)

    throw 'Timed out waiting for an Android reader application event.'
}

function Get-UiHierarchy {
    for ($attempt = 0; $attempt -lt 12; $attempt++) {
        try {
            $raw = (Invoke-Adb exec-out uiautomator dump '--compressed' '/dev/tty' | Out-String)
        }
        catch {
            Start-Sleep -Milliseconds 250
            continue
        }
        $end = $raw.IndexOf('</hierarchy>', [StringComparison]::Ordinal)
        if ($end -ge 0) {
            return [xml]$raw.Substring(0, $end + '</hierarchy>'.Length)
        }
        Start-Sleep -Milliseconds 250
    }
    throw 'UIAutomator did not return an Android hierarchy.'
}

function Wait-UiNode {
    param(
        [Parameter(Mandatory)][string]$XPath,
        [ValidateRange(1, 120)][int]$TimeoutSeconds = 20
    )

    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $node = (Get-UiHierarchy).SelectSingleNode($XPath)
        if ($null -ne $node) { return $node }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Timed out waiting for an Android UI node.'
}

function Invoke-UiNode {
    param(
        [Parameter(Mandatory)][string]$XPath,
        [ValidateRange(1, 120)][int]$TimeoutSeconds = 20
    )

    $node = Wait-UiNode -XPath $XPath -TimeoutSeconds $TimeoutSeconds
    $bounds = [regex]::Match($node.bounds, '^\[(\d+),(\d+)\]\[(\d+),(\d+)\]$')
    if (-not $bounds.Success) { throw 'Android UI node returned invalid bounds.' }
    $x = ([int]$bounds.Groups[1].Value + [int]$bounds.Groups[3].Value) / 2
    $y = ([int]$bounds.Groups[2].Value + [int]$bounds.Groups[4].Value) / 2
    Invoke-Adb shell input tap ([int]$x) ([int]$y) | Out-Null
}

function Start-AthaReader {
    Invoke-Adb shell am force-stop $package | Out-Null
    Invoke-Adb logcat '-c' | Out-Null
    $launch = (Invoke-Adb shell am start '-W' '-n' "$package/.MainActivity" | Out-String)
    if ($launch -notmatch '(?m)^Status: ok\s*$') {
        throw 'Android cold start did not report Status: ok.'
    }
    $processId = Get-AppProcessId
    if ($processId -notmatch '^\d+$') {
        throw 'Android reader process is not running after cold start.'
    }
    $startupMs = Wait-AppLog -ProcessId $processId -Pattern 'atha::startup.*event=application_start stage=ready' -TimeoutSeconds 20
    $logs = (Invoke-Adb logcat '-d' "--pid=$processId" '-v' 'brief' | Out-String)
    if ($logs -notmatch 'atha::startup.*event=application_start stage=setup') {
        throw 'Missing atha::startup setup log.'
    }
    if ($logs -match 'atha::startup.*outcome=failed') {
        throw 'Android reader emitted a startup failure.'
    }
    Assert-AppHealthy -ExpectedProcessId $processId
    [pscustomobject]@{ ProcessId = $processId; StartupMs = $startupMs }
}

$env:JAVA_HOME = $javaHome
$env:ANDROID_HOME = $androidHome
$env:ANDROID_SDK_ROOT = $androidHome
$env:NDK_HOME = $ndkHome
$env:ANDROID_NDK_HOME = $ndkHome
$env:PATH = @(
    (Split-Path $env:ATHA_NODE),
    (Split-Path $env:ATHA_CARGO),
    (Join-Path $javaHome 'bin'),
    (Join-Path $androidHome 'platform-tools'),
    $env:PATH
) -join [IO.Path]::PathSeparator

$deviceState = (Invoke-Checked $adb @('-s', $serial, 'get-state') | Out-String).Trim()
if ($deviceState -ne 'device') { throw "Android device $serial is not ready." }

$actualAvd = (Invoke-Checked $adb @('-s', $serial, 'shell', 'getprop', 'ro.boot.qemu.avd_name') | Out-String).Trim()
$actualApi = (Invoke-Checked $adb @('-s', $serial, 'shell', 'getprop', 'ro.build.version.sdk') | Out-String).Trim()
$actualAbi = (Invoke-Checked $adb @('-s', $serial, 'shell', 'getprop', 'ro.product.cpu.abi') | Out-String).Trim()
$pageSize = (Invoke-Checked $adb @('-s', $serial, 'shell', 'getconf', 'PAGE_SIZE') | Out-String).Trim()
if ($actualAvd -ne $avdName) { throw "Expected AVD $avdName, found $actualAvd." }
if ($actualApi -ne '35') { throw "Expected Android API 35, found $actualApi." }
if ($actualAbi -ne 'x86_64') { throw "Expected x86_64 ABI, found $actualAbi." }
if ($pageSize -ne '16384') { throw "Expected 16384-byte pages, found $pageSize." }

$webviewState = (Invoke-Adb shell dumpsys webviewupdate | Out-String)
$webviewMatch = [regex]::Match(
    $webviewState,
    'Current WebView package \(name, version\): \(([^,]+), ([^)]+)\)'
)
if (-not $webviewMatch.Success) { throw 'Unable to identify the active Android WebView provider.' }
$webviewPackage = $webviewMatch.Groups[1].Value
$webviewVersion = $webviewMatch.Groups[2].Value

if (-not $SkipBuild) {
    Push-Location $appRoot
    try {
        Invoke-Checked $env:ATHA_PNPM @('tauri', 'android', 'build', '--debug', '--target', 'x86_64', '--apk', '--ci')
    }
    finally {
        Pop-Location
    }
}
if (-not (Test-Path -LiteralPath $apkPath -PathType Leaf)) {
    throw "Android APK not found: $apkPath"
}

$badging = (Invoke-Checked $aapt2 @('dump', 'badging', $apkPath) | Out-String)
if ($badging -notmatch "(?m)^package: name='com\.atha\.reader'") { throw 'Unexpected Android package id.' }
if ($badging -notmatch "(?m)^minSdkVersion:'26'\s*$") { throw 'Unexpected Android minSdkVersion.' }
if ($badging -notmatch "(?m)^targetSdkVersion:'36'\s*$") { throw 'Unexpected Android targetSdkVersion.' }
if ($badging -notmatch "(?m)^native-code: 'x86_64'\s*$") { throw 'APK is not x86_64-only.' }

foreach ($permission in @(
    'READ_EXTERNAL_STORAGE',
    'WRITE_EXTERNAL_STORAGE',
    'MANAGE_EXTERNAL_STORAGE',
    'MANAGE_MEDIA',
    'ACCESS_MEDIA_LOCATION',
    'READ_MEDIA_AUDIO',
    'READ_MEDIA_IMAGES',
    'READ_MEDIA_VIDEO'
)) {
    if ($badging -match [regex]::Escape("android.permission.$permission")) {
        throw "APK requests broad storage permission android.permission.$permission."
    }
}

Invoke-Checked $zipalign @('-c', '-P', '16', '-v', '4', $apkPath) | Out-Null

$tempRoot = [IO.Path]::GetFullPath((Join-Path $repoRoot ".tmp\android-gate-$PID"))
$allowedTempRoot = [IO.Path]::GetFullPath((Join-Path $repoRoot '.tmp')).TrimEnd('\') + '\'
if (-not $tempRoot.StartsWith($allowedTempRoot, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Refusing to use a temporary directory outside the repository .tmp directory.'
}
New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null
$archive = $null
try {
    $archive = [IO.Compression.ZipFile]::OpenRead($apkPath)
    $nativeEntries = @($archive.Entries | Where-Object { $_.FullName -match '^lib/x86_64/[^/]+\.so$' })
    if ($nativeEntries.Count -eq 0) { throw 'APK contains no x86_64 shared libraries.' }

    for ($index = 0; $index -lt $nativeEntries.Count; $index++) {
        $entry = $nativeEntries[$index]
        $libraryPath = Join-Path $tempRoot ("{0:D2}-{1}" -f $index, [IO.Path]::GetFileName($entry.FullName))
        [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $libraryPath, $true)
        $headers = (Invoke-Checked $readelf @('-lW', $libraryPath) | Out-String)
        $loads = @($headers -split "`r?`n" | Where-Object { $_ -match '^\s*LOAD\s' })
        if ($loads.Count -eq 0) { throw "No ELF LOAD headers found for $($entry.FullName)." }
        if ($loads.Where({ $_ -notmatch '\s0x4000\s*$' }).Count -ne 0) {
            throw "ELF LOAD alignment is not 0x4000 for $($entry.FullName)."
        }
    }
}
finally {
    if ($null -ne $archive) { $archive.Dispose() }
    if (Test-Path -LiteralPath $tempRoot) { Remove-Item -LiteralPath $tempRoot -Recurse -Force }
}

Invoke-Adb install '-r' $apkPath | Out-Null
if ($CleanAppData) {
    if ($serial -notmatch '^emulator-' -or $actualAvd -ne $avdName) {
        throw 'Refusing to clear data outside the dedicated Android gate AVD.'
    }
    $clearResult = (Invoke-Adb shell pm clear $package | Out-String).Trim()
    if ($clearResult -ne 'Success') { throw 'Unable to clear Android reader data.' }
}

$firstLaunch = Start-AthaReader

if ($null -ne $resolvedEpub) {
    $remoteFixture = '/sdcard/Download/Atha-validation.epub'
    $null = & $adb -s $serial push $resolvedEpub $remoteFixture 2>&1
    if ($LASTEXITCODE -ne 0) { throw 'Unable to copy the local EPUB fixture to the Android gate AVD.' }
    Invoke-Adb shell touch $remoteFixture | Out-Null
    Invoke-Adb shell am broadcast '-a' 'android.intent.action.MEDIA_SCANNER_SCAN_FILE' '-d' "file://$remoteFixture" | Out-Null

    $importXPath = "//node[@package='$package' and @class='android.widget.Button' and @text='导入']"
    $fileXPath = "//node[@package='$documentsPackage' and @resource-id='android:id/title' and @text='Atha-validation.epub']/.."
    $bookXPath = "//node[@package='$package' and @text='书籍']//node[@class='android.widget.Button' and @clickable='true' and not(starts-with(@text,'从书架移除'))][1]"

    Invoke-UiNode -XPath $importXPath
    Wait-UiNode -XPath "//node[@package='$documentsPackage']" | Out-Null
    $picker = Get-UiHierarchy
    if ($null -eq $picker.SelectSingleNode($fileXPath)) {
        $inDownloads = $null -ne $picker.SelectSingleNode(
            "//node[@package='$documentsPackage' and @resource-id='$documentsPackage`:id/breadcrumb_text' and @text='Downloads']"
        )
        if (-not $inDownloads) {
            Invoke-UiNode -XPath "//node[@package='$documentsPackage' and @content-desc='Show roots']"
            Invoke-UiNode -XPath "//node[@package='$documentsPackage' and @resource-id='android:id/title' and @text='Downloads']/.."
        }
    }
    Invoke-UiNode -XPath $fileXPath

    $importMs = Wait-AppLog -ProcessId $firstLaunch.ProcessId -Pattern 'atha::library.*operation=import outcome=ok count=1 failure_count=0'
    Wait-UiNode -XPath $bookXPath | Out-Null
    Assert-AppHealthy -ExpectedProcessId $firstLaunch.ProcessId
    Invoke-UiNode -XPath $bookXPath
    $openMs = Wait-AppLog -ProcessId $firstLaunch.ProcessId -Pattern 'atha::library.*operation=open outcome=ok'
    $firstStableMs = Wait-AppLog -ProcessId $firstLaunch.ProcessId -Pattern 'atha::reader.*event=reader_metric stage=first_stable'
    $readyMs = Wait-AppLog -ProcessId $firstLaunch.ProcessId -Pattern 'atha::reader.*event=reader_ready'
    Assert-AppHealthy -ExpectedProcessId $firstLaunch.ProcessId

    $pickerCache = (Invoke-Adb shell run-as $package ls '-A' 'cache/Picker' | Out-String).Trim()
    if ($pickerCache.Length -ne 0) { throw 'Android picker cache was not cleaned after EPUB import.' }
    $athaLogs = (Invoke-Adb logcat '-d' "--pid=$($firstLaunch.ProcessId)" '-v' 'brief' | Out-String)
    if ($athaLogs -match '(?i)content://|/sdcard/|Atha-validation\.epub') {
        throw 'Android application logs exposed picker URI or file details.'
    }

    $secondLaunch = Start-AthaReader
    Wait-UiNode -XPath $bookXPath | Out-Null
    Assert-AppHealthy -ExpectedProcessId $secondLaunch.ProcessId
    Invoke-UiNode -XPath $bookXPath
    $reopenMs = Wait-AppLog -ProcessId $secondLaunch.ProcessId -Pattern 'atha::library.*operation=open outcome=ok'
    $secondStableMs = Wait-AppLog -ProcessId $secondLaunch.ProcessId -Pattern 'atha::reader.*event=reader_metric stage=first_stable'
    $secondReadyMs = Wait-AppLog -ProcessId $secondLaunch.ProcessId -Pattern 'atha::reader.*event=reader_ready'
    Assert-AppHealthy -ExpectedProcessId $secondLaunch.ProcessId

    $evidenceDirectory = Join-Path $repoRoot 'artifacts\local\android'
    New-Item -ItemType Directory -Path $evidenceDirectory -Force | Out-Null
    $evidence = [ordered]@{
        schema = 'atha.android-reader-gate.v1'
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
        evidence_level = 'android-emulator'
        target = [ordered]@{
            avd = $actualAvd
            api = [int]$actualApi
            abi = $actualAbi
            page_size = [int]$pageSize
        }
        webview = [ordered]@{
            package = $webviewPackage
            version = $webviewVersion
        }
        apk = [ordered]@{
            sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $apkPath).Hash.ToLowerInvariant()
            x86_64_library_count = $nativeEntries.Count
            zipaligned_16k = $true
        }
        installation = [ordered]@{
            clean_app_data = [bool]$CleanAppData
            cold_start_ready = $true
            startup_wait_ms = [long]$firstLaunch.StartupMs
        }
        epub = [ordered]@{
            system_picker = 'passed'
            import = 'passed'
            import_wait_ms = [long]$importMs
            open = 'passed'
            open_wait_ms = [long]$openMs
            first_stable = 'passed'
            first_stable_wait_ms = [long]$firstStableMs
            reader_ready = 'passed'
            reader_ready_wait_ms = [long]$readyMs
            restart_shelf_persistence = 'passed'
            reopen = 'passed'
            reopen_wait_ms = [long]$reopenMs
            restart_first_stable_wait_ms = [long]$secondStableMs
            restart_reader_ready_wait_ms = [long]$secondReadyMs
            picker_cache_clean = $true
        }
        # Message SAF stays a separate opt-in round-trip: restore replaces the store and export needs an opened edition.
        message_saf = [ordered]@{
            backup = 'separate-opt-in'
            restore = 'separate-opt-in'
            export = 'separate-opt-in'
        }
    }
    $evidence | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $evidenceDirectory 'reader-gate-epub.json') -Encoding utf8NoBOM
    Write-Host 'Android EPUB picker, open, reader-ready, restart, and persistence slice passed; structured evidence recorded.'
}

Write-Host "Android reader gate passed: $avdName, API $actualApi, $actualAbi, ${pageSize}-byte pages, WebView $webviewPackage $webviewVersion."
