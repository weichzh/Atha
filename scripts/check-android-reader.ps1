# Description: Build and verify the Android reader on the project 16 KiB x86_64 emulator.

[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [Alias('EpubPath')]
    [string]$BookPath,
    [switch]$CleanAppData,
    [switch]$VerifyEpub2NcxFixture,
    [switch]$VerifyCbzFixture,
    [switch]$VerifyLibraryShelfUi,
    [switch]$VerifyMarkdownText,
    [ValidateRange(0, 10)]
    [int]$TextBenchmarkSample = 0,
    [string]$ExpectedAvd = 'Atha_API_35_16K',
    [ValidateSet(35, 36)]
    [int]$ExpectedApi = 35
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$appRoot = Join-Path $repoRoot 'reader\app'
$apkPath = Join-Path $appRoot 'src-tauri\gen\android\app\build\outputs\apk\universal\debug\app-universal-debug.apk'
$serial = 'emulator-5554'
$avdName = $ExpectedAvd
$package = 'com.atha.reader'
$documentsPackage = 'com.google.android.documentsui'
$epub2NcxFixtureSha256 = '6991bfb8edd895a44cb5b0e9066805ee6cea030f47856f3607e8ee2cf4be5887'
$cbzFixtureSha256 = '5957e1a0daed2ed0a3a8b1439585cb7651d5478fe5cd51cde0401c7878eb30ed'
$cbzFixtureTitle = 'Atha CBZ Gate 71c9'
$cbzFixtureWriter = 'Gate Writer 71c9'
$cbzFixtureImageToken = 'Atha CBZ Image 71c9'
$privateTxtExpectedEncoding = 'GBK'
$resolvedBook = $null
$bookSha256 = $null
$bookExtension = $null
$bookFormat = $null
$bookInputBytes = $null

if ($CleanAppData -and [string]::IsNullOrWhiteSpace($BookPath)) {
    throw '-CleanAppData requires -BookPath so a clean reader slice is verified immediately.'
}
if (($VerifyEpub2NcxFixture -or $VerifyCbzFixture -or $VerifyMarkdownText) -and [string]::IsNullOrWhiteSpace($BookPath)) {
    throw 'Fixture verification requires -BookPath.'
}
if (@($VerifyEpub2NcxFixture, $VerifyCbzFixture, $VerifyMarkdownText).Where({ $_ }).Count -gt 1) {
    throw 'EPUB2, CBZ, and Markdown/TXT verification are mutually exclusive.'
}
if ($VerifyCbzFixture -and -not $CleanAppData) {
    throw 'VerifyCbzFixture requires -CleanAppData on the dedicated gate AVD.'
}
if ($VerifyMarkdownText -and -not $CleanAppData) {
    throw 'VerifyMarkdownText requires -CleanAppData on the dedicated gate AVD.'
}
if ($TextBenchmarkSample -gt 0 -and -not $VerifyMarkdownText) {
    throw '-TextBenchmarkSample requires -VerifyMarkdownText.'
}
if ($VerifyLibraryShelfUi -and ([string]::IsNullOrWhiteSpace($BookPath) -or -not $CleanAppData)) {
    throw 'VerifyLibraryShelfUi requires -BookPath and -CleanAppData on the dedicated gate AVD.'
}
if (-not [string]::IsNullOrWhiteSpace($BookPath)) {
    $resolvedBook = (Resolve-Path -LiteralPath $BookPath).Path
    if (-not (Test-Path -LiteralPath $resolvedBook -PathType Leaf)) {
        throw 'BookPath must name a local file.'
    }
    $bookExtension = [IO.Path]::GetExtension($resolvedBook).ToLowerInvariant()
    if ($bookExtension -notin @('.epub', '.cbz', '.md', '.markdown', '.txt')) {
        throw 'BookPath must have an .epub, .cbz, .md, .markdown, or .txt extension.'
    }
    $bookInputBytes = [long](Get-Item -LiteralPath $resolvedBook).Length
    if ($bookInputBytes -eq 0) {
        throw 'BookPath must not be empty.'
    }
    $bookFormat = switch ($bookExtension) {
        '.epub' { 'epub' }
        '.cbz' { 'cbz' }
        { $_ -in @('.md', '.markdown') } { 'markdown' }
        '.txt' { 'txt' }
    }
    if ($bookFormat -in @('epub', 'cbz')) {
        $bookSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $resolvedBook).Hash.ToLowerInvariant()
    }
    if ($VerifyEpub2NcxFixture -and $bookExtension -ne '.epub') {
        throw 'VerifyEpub2NcxFixture requires an EPUB file.'
    }
    if ($VerifyCbzFixture -and $bookExtension -ne '.cbz') {
        throw 'VerifyCbzFixture requires a CBZ file.'
    }
    if ($VerifyMarkdownText -and $bookFormat -notin @('markdown', 'txt')) {
        throw 'VerifyMarkdownText requires a Markdown or TXT file.'
    }
    if (-not $VerifyMarkdownText -and $bookFormat -in @('markdown', 'txt')) {
        throw 'Markdown and TXT inputs require -VerifyMarkdownText.'
    }
    if ($TextBenchmarkSample -gt 0 -and $bookFormat -ne 'txt') {
        throw '-TextBenchmarkSample is reserved for the accepted private TXT performance gate.'
    }
    if ($VerifyLibraryShelfUi -and $bookFormat -eq 'txt') {
        throw 'Private TXT verification cannot capture shelf screenshots.'
    }
    if ($VerifyEpub2NcxFixture -and $bookSha256 -ne $epub2NcxFixtureSha256) {
        throw 'VerifyEpub2NcxFixture requires the generated and EPUBCheck-verified fixture.'
    }
    if ($VerifyCbzFixture -and $bookSha256 -ne $cbzFixtureSha256) {
        throw 'VerifyCbzFixture requires the generated deterministic fixture.'
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
$androidWebViewEval = Join-Path $PSScriptRoot 'android-webview-eval.mjs'

foreach ($tool in @($javaRelease, $adb, $aapt2, $zipalign, $androidPlatform, $readelf, $androidWebViewEval)) {
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

function Open-AndroidWebViewSession {
    param([Parameter(Mandatory)][string]$ProcessId)

    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    do {
        $socketLine = @(& $adb -s $serial shell cat /proc/net/unix) |
            Where-Object { $_ -match "@webview_devtools_remote_$ProcessId(?:\s|$)" } |
            Select-Object -First 1
        if ($socketLine) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $socketLine) { throw 'Android reader WebView debugging socket is unavailable.' }

    $socket = ([regex]::Match($socketLine, '@([^\s]+)$')).Groups[1].Value
    $portText = (Invoke-Adb forward 'tcp:0' "localabstract:$socket" | Out-String).Trim()
    if ($portText -notmatch '^\d+$') { throw 'ADB did not allocate a WebView debugging port.' }

    $connection = [pscustomobject]@{ Port = [int]$portText }
    try {
        $url = [string](Get-AndroidWebViewValue -Connection $connection -JavaScript 'location.href')
        if ($url -notmatch '^https://tauri\.localhost/') {
            throw 'Android reader WebView exposed an unexpected origin.'
        }
    }
    catch {
        Invoke-Adb forward '--remove' "tcp:$portText" | Out-Null
        throw
    }
    $connection
}

function Close-AndroidWebViewSession {
    param($Connection)

    if ($null -eq $Connection) { return }
    & $adb -s $serial forward '--remove' "tcp:$($Connection.Port)" 2>$null | Out-Null
}

function Get-AndroidWebViewValue {
    param(
        [Parameter(Mandatory)]$Connection,
        [Parameter(Mandatory)][string]$JavaScript
    )

    $raw = (Invoke-Checked `
        -FilePath $env:ATHA_NODE `
        -Arguments @($androidWebViewEval, [string]$Connection.Port, $JavaScript) |
            Out-String).Trim()
    try { $raw | ConvertFrom-Json }
    catch { throw 'Android reader WebView returned malformed diagnostic data.' }
}

function Get-CdpReaderViewportState {
    param([Parameter(Mandatory)]$Connection)

    $title = [string](Get-AndroidWebViewValue -Connection $Connection -JavaScript 'document.title')
    $position = [regex]::Match(
        $title,
        '^Atha Reader — section (\d+) / (\d+) — page (\d+) / (\d+)$'
    )
    if (-not $position.Success) { throw 'Android reader CDP position is not observable.' }
    [pscustomobject]@{
        Section = [int]$position.Groups[1].Value
        Sections = [int]$position.Groups[2].Value
        Page = [int]$position.Groups[3].Value
        Pages = [int]$position.Groups[4].Value
    }
}

function Wait-CdpReaderViewportState {
    param(
        [Parameter(Mandatory)]$Connection,
        [Parameter(Mandatory)][int]$Section,
        [int]$Page = 0,
        [ValidateRange(1, 120)][int]$TimeoutSeconds = 20
    )

    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $state = Get-CdpReaderViewportState -Connection $Connection
        if ($state.Section -eq $Section -and ($Page -eq 0 -or $state.Page -eq $Page)) {
            return $state
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Timed out waiting for the requested Android reader CDP location.'
}

function Wait-CdpReaderViewportChange {
    param(
        [Parameter(Mandatory)]$Connection,
        [Parameter(Mandatory)]$Before,
        [ValidateRange(1, 120)][int]$TimeoutSeconds = 20
    )

    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $state = Get-CdpReaderViewportState -Connection $Connection
        if ($state.Section -ne $Before.Section -or $state.Page -ne $Before.Page) { return $state }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Timed out waiting for the Android reader CDP location to change.'
}

function Invoke-CdpClick {
    param(
        [Parameter(Mandatory)]$Connection,
        [Parameter(Mandatory)][string]$Selector
    )

    $escapedSelector = $Selector | ConvertTo-Json -Compress
    $clicked = Get-AndroidWebViewValue `
        -Connection $Connection `
        -JavaScript "(()=>{const element=document.querySelector($escapedSelector);if(!element)return false;element.click();return true})()"
    if (-not [bool]$clicked) { throw 'Android reader WebView control is unavailable.' }
}

function Show-CdpReaderTools {
    param([Parameter(Mandatory)]$Connection)

    $shown = Get-AndroidWebViewValue -Connection $Connection -JavaScript @'
(()=>{
  if(document.documentElement.hasAttribute("data-reader-tools"))return true;
  const reader=document.querySelector(".reader");
  if(!reader)return false;
  const rect=reader.getBoundingClientRect();
  const init={
    bubbles:true,
    pointerId:91,
    pointerType:"touch",
    isPrimary:true,
    clientX:rect.left+rect.width/2,
    clientY:rect.top+rect.height/2,
    button:0
  };
  reader.dispatchEvent(new PointerEvent("pointerdown",init));
  window.dispatchEvent(new PointerEvent("pointerup",init));
  return document.documentElement.hasAttribute("data-reader-tools");
})()
'@
    if (-not [bool]$shown) { throw 'Android reader tools did not open.' }
}

function Focus-CdpElement {
    param(
        [Parameter(Mandatory)]$Connection,
        [Parameter(Mandatory)][string]$Selector
    )

    $escapedSelector = $Selector | ConvertTo-Json -Compress
    $focused = Get-AndroidWebViewValue `
        -Connection $Connection `
        -JavaScript "(()=>{const element=document.querySelector($escapedSelector);if(!element)return false;element.focus();return document.activeElement===element})()"
    if (-not [bool]$focused) { throw 'Android reader WebView input did not receive focus.' }
}

function Wait-CdpCondition {
    param(
        [Parameter(Mandatory)]$Connection,
        [Parameter(Mandatory)][string]$JavaScript,
        [ValidateRange(1, 120)][int]$TimeoutSeconds = 20
    )

    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        if ([bool](Get-AndroidWebViewValue -Connection $Connection -JavaScript $JavaScript)) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Timed out waiting for the Android reader WebView condition.'
}

function Save-UiScreenshot {
    param([Parameter(Mandatory)][string]$Name)

    $directory = Join-Path $repoRoot 'artifacts\local\screenshots'
    $remote = "/data/local/tmp/$Name.png"
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
    try {
        Invoke-Adb shell screencap '-p' $remote | Out-Null
        Invoke-Adb pull $remote (Join-Path $directory "$Name.png") | Out-Null
    }
    finally {
        Invoke-Adb shell rm '-f' $remote | Out-Null
    }
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
        $allLogs -match '(?im)ANR in com\.atha\.reader\b' -or
        $allLogs -match '(?im)OutOfMemoryError.*com\.atha\.reader\b' -or
        $allLogs -match '(?im)lmkd.*com\.atha\.reader\b' -or
        $allLogs -match '(?im)(?:com\.atha\.reader.*(?:render process gone|renderer.*crash)|(?:render process gone|renderer.*crash).*com\.atha\.reader)' -or
        $allLogs -match '(?im)atha::reader.*event=reader_failure'
    ) {
        throw 'Android reader emitted a fatal signal, uncaught exception, ANR, OOM, LMK, or terminal reader failure.'
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

function Get-AppLogDurationMs {
    param(
        [Parameter(Mandatory)][string]$ProcessId,
        [Parameter(Mandatory)][string]$Pattern,
        [switch]$Required
    )

    $logs = (Invoke-Adb logcat '-d' "--pid=$ProcessId" '-v' 'brief' | Out-String)
    $matches = [regex]::Matches($logs, "(?m)$Pattern[^`r`n]*\bduration_ms=([0-9]+(?:\.[0-9]+)?)\b")
    if ($matches.Count -eq 0) {
        if ($Required) {
            throw 'Android application telemetry did not expose the requested duration.'
        }
        return $null
    }
    [double]::Parse(
        $matches[$matches.Count - 1].Groups[1].Value,
        [Globalization.CultureInfo]::InvariantCulture
    )
}

function Get-TextImportTelemetry {
    param(
        [Parameter(Mandatory)][string]$ProcessId,
        [ValidateSet('markdown', 'txt')][string]$Format
    )

    $logs = (Invoke-Adb logcat '-d' "--pid=$ProcessId" '-v' 'brief' | Out-String)
    $escapedFormat = [regex]::Escape($Format)
    $pattern = "(?m)^(?=[^\r\n]*atha::reader)(?=[^\r\n]*\boperation=import\b)(?=[^\r\n]*\bformat=$escapedFormat\b)(?=[^\r\n]*\boutcome=success\b)[^\r\n]*\r?$"
    $matches = [regex]::Matches($logs, $pattern)
    if ($matches.Count -eq 0) {
        throw 'Android text importer telemetry is missing or malformed.'
    }

    $line = $matches[$matches.Count - 1].Value
    $readUnsigned = {
        param([Parameter(Mandatory)][string]$Name)

        $fieldName = [regex]::Escape($Name)
        $fieldMatches = [regex]::Matches($line, "(?:^|\s)$fieldName=(\d+)(?=\s|$)")
        if ($fieldMatches.Count -ne 1) {
            throw 'Android text importer telemetry is missing or malformed.'
        }
        [long]::Parse(
            $fieldMatches[0].Groups[1].Value,
            [Globalization.CultureInfo]::InvariantCulture
        )
    }
    $stageNames = if ($Format -eq 'markdown') {
        @('fingerprint', 'decode', 'markdown_parse', 'render_write', 'publish')
    }
    else {
        @('detect', 'chapter_scan', 'render_write', 'publish')
    }
    $stageDurationsMs = [ordered]@{}
    foreach ($stageName in $stageNames) {
        $stageDurationsMs[$stageName] = & $readUnsigned "${stageName}_ms"
    }
    $encoding = $null
    if ($Format -eq 'txt') {
        $encodingMatches = [regex]::Matches($line, '(?:^|\s)encoding=([A-Za-z0-9._-]+)(?=\s|$)')
        if ($encodingMatches.Count -ne 1) {
            throw 'Android text importer telemetry is missing or malformed.'
        }
        $encoding = $encodingMatches[0].Groups[1].Value
    }

    [pscustomobject]@{
        InputBytes = & $readUnsigned 'input_bytes'
        Encoding = $encoding
        Sections = [int](& $readUnsigned 'sections')
        TocItems = [int](& $readUnsigned 'toc_items')
        DurationMs = [double](& $readUnsigned 'total_ms')
        StageDurationsMs = $stageDurationsMs
    }
}

function Get-SearchTelemetry {
    param([Parameter(Mandatory)][string]$ProcessId)

    $logs = (Invoke-Adb logcat '-d' "--pid=$ProcessId" '-v' 'brief' | Out-String)
    $matches = [regex]::Matches($logs, '(?m)^[^\r\n]*atha::reader[^\r\n]*\bevent=reader_search\b[^\r\n]*\r?$')
    if ($matches.Count -eq 0) {
        throw 'Android reader search telemetry is missing or malformed.'
    }
    $line = $matches[$matches.Count - 1].Value
    $readUnsigned = {
        param([Parameter(Mandatory)][string]$Name)

        $fieldMatches = [regex]::Matches($line, "(?:^|\s)$([regex]::Escape($Name))=(\d+)(?=\s|$)")
        if ($fieldMatches.Count -ne 1) {
            throw 'Android reader search telemetry is missing or malformed.'
        }
        [int]::Parse($fieldMatches[0].Groups[1].Value, [Globalization.CultureInfo]::InvariantCulture)
    }
    $truncatedMatches = [regex]::Matches($line, '(?:^|\s)search_truncated=(true|false)(?=\s|$)')
    $durationMatches = [regex]::Matches($line, '(?:^|\s)duration_ms=([0-9]+(?:\.[0-9]+)?)(?=\s|$)')
    if ($truncatedMatches.Count -ne 1 -or $durationMatches.Count -ne 1) {
        throw 'Android reader search telemetry is missing or malformed.'
    }

    [pscustomobject]@{
        Results = & $readUnsigned 'search_results'
        Truncated = $truncatedMatches[0].Groups[1].Value -eq 'true'
        SectionsScanned = & $readUnsigned 'sections_scanned'
        DurationMs = [double]::Parse(
            $durationMatches[0].Groups[1].Value,
            [Globalization.CultureInfo]::InvariantCulture
        )
    }
}

function Get-AthaPrivacyLogs {
    param([Parameter(Mandatory)][string]$ProcessId)

    $processLogcat = (Invoke-Adb logcat '-d' "--pid=$ProcessId" '-v' 'brief' | Out-String)
    $persistentAppLog = @(
        ((Invoke-Adb exec-out run-as $package find logs '-maxdepth' 1 '-type' f '-name' 'Atha.log*' | Out-String) -split "`r?`n") |
            ForEach-Object { $_.Trim() } |
            Where-Object { $_ } |
            ForEach-Object { Invoke-Adb exec-out run-as $package cat $_ }
    ) -join "`n"
    "$processLogcat`n$persistentAppLog"
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
        [string]$Stage = 'unspecified',
        [ValidateRange(1, 120)][int]$TimeoutSeconds = 20
    )

    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $node = (Get-UiHierarchy).SelectSingleNode($XPath)
        if ($null -ne $node) { return $node }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for an Android UI node at stage=$Stage."
}

function Invoke-UiNode {
    param(
        [Parameter(Mandatory)][string]$XPath,
        [string]$Stage = 'unspecified',
        [ValidateRange(1, 120)][int]$TimeoutSeconds = 20
    )

    $node = Wait-UiNode -XPath $XPath -Stage $Stage -TimeoutSeconds $TimeoutSeconds
    $bounds = [regex]::Match($node.bounds, '^\[(\d+),(\d+)\]\[(\d+),(\d+)\]$')
    if (-not $bounds.Success) { throw 'Android UI node returned invalid bounds.' }
    $x = ([int]$bounds.Groups[1].Value + [int]$bounds.Groups[3].Value) / 2
    $y = ([int]$bounds.Groups[2].Value + [int]$bounds.Groups[4].Value) / 2
    Invoke-Adb shell input tap ([int]$x) ([int]$y) | Out-Null
}

function Assert-UiNodeTouchTarget {
    param([Parameter(Mandatory)][string]$XPath)

    $node = Wait-UiNode -XPath $XPath
    $bounds = [regex]::Match($node.bounds, '^\[(\d+),(\d+)\]\[(\d+),(\d+)\]$')
    if (-not $bounds.Success) { throw 'Android UI node returned invalid bounds.' }
    $left = [int]$bounds.Groups[1].Value
    $top = [int]$bounds.Groups[2].Value
    $right = [int]$bounds.Groups[3].Value
    $bottom = [int]$bounds.Groups[4].Value
    $minimumPixels = [Math]::Ceiling(44 * $displayDensityDpi / 160)
    if (
        $left -lt 0 -or $top -lt 0 -or
        $right -gt $displayWidth -or $bottom -gt $displayHeight -or
        ($right - $left) -lt $minimumPixels -or ($bottom - $top) -lt $minimumPixels
    ) {
        throw 'Android shelf control is clipped or smaller than 44 CSS px.'
    }
}

function Get-ReaderViewportState {
    $hierarchy = Get-UiHierarchy
    $webviews = @(
        $hierarchy.SelectNodes("//node[@package='$package' and @class='android.webkit.WebView']") |
            Where-Object { [string]$_.text -match '^Atha Reader — section \d+ / \d+ — page \d+ / \d+$' }
    )
    if ($webviews.Count -ne 1) { throw 'Android reader WebView is not uniquely observable.' }
    $position = [regex]::Match(
        [string]$webviews[0].text,
        '^Atha Reader — section (\d+) / (\d+) — page (\d+) / (\d+)$'
    )
    if (-not $position.Success) { throw 'Android reader position is not observable.' }
    [pscustomobject]@{
        Section = [int]$position.Groups[1].Value
        Sections = [int]$position.Groups[2].Value
        Page = [int]$position.Groups[3].Value
        Pages = [int]$position.Groups[4].Value
    }
}

function Wait-ReaderViewportState {
    param(
        [Parameter(Mandatory)][int]$Section,
        [int]$Page = 0,
        [ValidateRange(1, 120)][int]$TimeoutSeconds = 20
    )

    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        try {
            $state = Get-ReaderViewportState
            if (
                $state.Section -eq $Section -and
                ($Page -eq 0 -or $state.Page -eq $Page)
            ) {
                return $state
            }
        }
        catch {
            # UIAutomator can return a transient partial WebView tree during section layout.
        }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Timed out waiting for the requested Android reader location.'
}

function Wait-ReaderViewportChange {
    param(
        [Parameter(Mandatory)]$Before,
        [ValidateRange(1, 120)][int]$TimeoutSeconds = 20
    )

    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        try {
            $state = Get-ReaderViewportState
            if ($state.Section -ne $Before.Section -or $state.Page -ne $Before.Page) {
                return $state
            }
        }
        catch {
            # UIAutomator can return a transient partial WebView tree during pagination.
        }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Timed out waiting for an Android reader page turn.'
}

function Invoke-ReaderNextPage {
    Invoke-Adb shell input keyevent 93 | Out-Null
}

function Get-AppPssKiB {
    param([Parameter(Mandatory)][string]$ProcessId)

    $memory = (Invoke-Adb shell dumpsys meminfo $ProcessId | Out-String)
    $summary = [regex]::Match($memory, '(?m)^\s*TOTAL PSS:\s*(\d+)\b')
    if (-not $summary.Success) {
        $summary = [regex]::Match($memory, '(?m)^\s*TOTAL\s+(\d+)\b')
    }
    if (-not $summary.Success) {
        throw 'Android meminfo did not expose app process TOTAL PSS.'
    }
    [long]$summary.Groups[1].Value
}

function Get-TenSampleSummary {
    param([Parameter(Mandatory)][object[]]$Values)

    if ($Values.Count -ne 10 -or $Values.Where({ $null -eq $_ }).Count -ne 0) {
        throw 'Android TXT benchmark summaries require exactly 10 numeric samples.'
    }
    $sorted = @($Values | ForEach-Object { [double]$_ } | Sort-Object)
    if ($sorted.Where({ -not [double]::IsFinite($_) -or $_ -lt 0 }).Count -ne 0) {
        throw 'Android TXT benchmark samples must be finite and non-negative.'
    }
    [ordered]@{
        samples = 10
        median = [Math]::Round(($sorted[4] + $sorted[5]) / 2.0, 3)
        p95 = [Math]::Round($sorted[9], 3)
    }
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
    $startupDurationMs = Get-AppLogDurationMs -ProcessId $processId -Pattern 'atha::startup.*event=application_start stage=ready' -Required:$VerifyMarkdownText
    $logs = (Invoke-Adb logcat '-d' "--pid=$processId" '-v' 'brief' | Out-String)
    if ($logs -notmatch 'atha::startup.*event=application_start stage=setup') {
        throw 'Missing atha::startup setup log.'
    }
    if ($logs -match 'atha::startup.*outcome=failed') {
        throw 'Android reader emitted a startup failure.'
    }
    Assert-AppHealthy -ExpectedProcessId $processId
    [pscustomobject]@{
        ProcessId = $processId
        StartupMs = $startupMs
        StartupDurationMs = $startupDurationMs
    }
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
if ($actualApi -ne [string]$ExpectedApi) { throw "Expected Android API $ExpectedApi, found $actualApi." }
if ($actualAbi -ne 'x86_64') { throw "Expected x86_64 ABI, found $actualAbi." }
if ($pageSize -ne '16384') { throw "Expected 16384-byte pages, found $pageSize." }

$densityMatches = [regex]::Matches((Invoke-Adb shell wm density | Out-String), '(?m)^(?:Physical|Override) density:\s*(\d+)\s*$')
$sizeMatches = [regex]::Matches((Invoke-Adb shell wm size | Out-String), '(?m)^(?:Physical|Override) size:\s*(\d+)x(\d+)\s*$')
if ($densityMatches.Count -eq 0 -or $sizeMatches.Count -eq 0) {
    throw 'Android display density or bounds are not observable.'
}
$displayDensityDpi = [int]$densityMatches[$densityMatches.Count - 1].Groups[1].Value
$displayWidth = [int]$sizeMatches[$sizeMatches.Count - 1].Groups[1].Value
$displayHeight = [int]$sizeMatches[$sizeMatches.Count - 1].Groups[2].Value

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

if ($null -ne $resolvedBook) {
    $remoteName = "Atha-validation$bookExtension"
    $remoteFixture = "/sdcard/Download/$remoteName"
    try {
    $null = & $adb -s $serial push $resolvedBook $remoteFixture 2>&1
    if ($LASTEXITCODE -ne 0) { throw 'Unable to copy the local book fixture to the Android gate AVD.' }
    Invoke-Adb shell touch $remoteFixture | Out-Null
    Invoke-Adb shell am broadcast '-a' 'android.intent.action.MEDIA_SCANNER_SCAN_FILE' '-d' "file://$remoteFixture" | Out-Null

    $importXPath = "//node[@package='$package' and @class='android.widget.Button' and @text='导入']"
    $fileXPath = "//node[@package='$documentsPackage' and @resource-id='android:id/title' and @text='$remoteName']/.."
    $bookXPath = "//node[@package='$package' and @text='书籍']//node[@class='android.widget.Button' and @clickable='true' and not(starts-with(@text,'从书架移除'))][1]"
    $shelfPssKiB = if ($bookFormat -eq 'cbz' -or $VerifyMarkdownText) {
        Get-AppPssKiB -ProcessId $firstLaunch.ProcessId
    }
    else {
        $null
    }

    Invoke-UiNode -XPath $importXPath -Stage 'import-trigger'
    Wait-UiNode -XPath "//node[@package='$documentsPackage']" -Stage 'picker-visible' | Out-Null
    $picker = Get-UiHierarchy
    if ($null -eq $picker.SelectSingleNode($fileXPath)) {
        $inDownloads = $null -ne $picker.SelectSingleNode(
            "//node[@package='$documentsPackage' and @resource-id='$documentsPackage`:id/breadcrumb_text' and @text='Downloads']"
        )
        if (-not $inDownloads) {
            Invoke-UiNode -XPath "//node[@package='$documentsPackage' and @content-desc='Show roots']" -Stage 'picker-roots'
            Invoke-UiNode -XPath "//node[@package='$documentsPackage' and @resource-id='android:id/title' and @text='Downloads']/.." -Stage 'picker-downloads'
        }
    }
    Invoke-UiNode -XPath $fileXPath -Stage 'picker-book'

    $importMs = Wait-AppLog -ProcessId $firstLaunch.ProcessId -Pattern 'atha::library.*operation=import outcome=ok count=1 failure_count=0'
    $importDurationMs = Get-AppLogDurationMs -ProcessId $firstLaunch.ProcessId -Pattern 'atha::library.*operation=import outcome=ok count=1 failure_count=0' -Required:$VerifyMarkdownText
    $textImportTelemetry = if ($VerifyMarkdownText) {
        Get-TextImportTelemetry -ProcessId $firstLaunch.ProcessId -Format $bookFormat
    }
    else {
        $null
    }
    if ($VerifyMarkdownText -and $textImportTelemetry.InputBytes -ne $bookInputBytes) {
        throw 'Android text importer telemetry reported an unexpected input byte count.'
    }
    if ($bookFormat -eq 'txt' -and $textImportTelemetry.Encoding -ne $privateTxtExpectedEncoding) {
        throw 'Android private TXT encoding detection changed from the accepted aggregate baseline.'
    }
    Wait-UiNode -XPath $bookXPath -Stage 'shelf-book' | Out-Null
    Assert-AppHealthy -ExpectedProcessId $firstLaunch.ProcessId
    $importedShelfPssKiB = if ($VerifyMarkdownText) {
        Get-AppPssKiB -ProcessId $firstLaunch.ProcessId
    }
    else {
        $null
    }
    $shelfCoverResult = 'not-requested'
    if ($VerifyCbzFixture) {
        $shelfCoverXPath = "$bookXPath[starts-with(@text,'书籍封面 ')]"
        Wait-UiNode -XPath $shelfCoverXPath | Out-Null
        Start-Sleep -Milliseconds 500
        Wait-UiNode -XPath $shelfCoverXPath | Out-Null
        $shelfCoverResult = 'passed'
    }
    Invoke-UiNode -XPath $bookXPath
    $openMs = Wait-AppLog -ProcessId $firstLaunch.ProcessId -Pattern 'atha::library.*operation=open outcome=ok'
    $openDurationMs = Get-AppLogDurationMs -ProcessId $firstLaunch.ProcessId -Pattern 'atha::library.*operation=open outcome=ok' -Required:$VerifyMarkdownText
    $firstStableMs = Wait-AppLog -ProcessId $firstLaunch.ProcessId -Pattern 'atha::reader.*event=reader_metric stage=first_stable'
    $firstStableDurationMs = Get-AppLogDurationMs -ProcessId $firstLaunch.ProcessId -Pattern 'atha::reader.*event=reader_metric stage=first_stable' -Required:$VerifyMarkdownText
    $readyMs = Wait-AppLog -ProcessId $firstLaunch.ProcessId -Pattern 'atha::reader.*event=reader_ready'
    Assert-AppHealthy -ExpectedProcessId $firstLaunch.ProcessId

    $afterDirectoryJump = $null
    $directoryJumpResult = 'not-requested'
    $restartLocatorResult = 'not-requested'
    $pageTurnWaitMs = @()
    $firstPagePssKiB = $null
    $middlePagePssKiB = $null
    $lastPagePssKiB = $null
    $tocBoundPssKiB = $null
    $lastLocation = $null
    $corruptPagePlaceholderResult = 'not-requested'
    $directoryTargetResult = 'not-requested'
    $directoryTargetWaitMs = [ordered]@{}
    $searchResult = 'not-requested'
    $searchWaitMs = $null
    $searchToken = $null
    $pageTurnBenchmarkMs = $null
    $observedSectionCount = $null
    $tocItemCount = $null
    if ($VerifyEpub2NcxFixture) {
        $beforeDirectoryJump = Get-ReaderViewportState
        if ($beforeDirectoryJump.Section -ne 1 -or $beforeDirectoryJump.Sections -ne 2) {
            throw 'The Android EPUB2 fixture must open at section 1 of 2.'
        }
        $readerPageXPath = "//node[@package='$package' and @text='阅读页']"
        $directoryXPath = "//node[@package='$package' and @text='目录' and @clickable='true']"
        $firstDirectoryItemXPath = "(//node[@package='$package' and @class='android.widget.Button' and @clickable='true' and ancestor::node[@text='章节和书签']])[1]"
        $secondDirectoryItemXPath = "(//node[@package='$package' and @class='android.widget.Button' and @clickable='true' and ancestor::node[@text='章节和书签']])[2]"
        Invoke-UiNode -XPath $readerPageXPath
        Invoke-UiNode -XPath $directoryXPath
        $firstDirectoryItem = Wait-UiNode -XPath $firstDirectoryItemXPath
        $secondDirectoryItem = Wait-UiNode -XPath $secondDirectoryItemXPath
        $targetChapter = ([string]$secondDirectoryItem.text).Trim()
        if (
            [string]::IsNullOrWhiteSpace($targetChapter) -or
            $targetChapter -eq ([string]$firstDirectoryItem.text).Trim()
        ) {
            throw 'The Android EPUB2 fixture must expose a distinct second directory item.'
        }
        Invoke-UiNode -XPath $secondDirectoryItemXPath
        $afterDirectoryJump = Wait-ReaderViewportState -Section 2
        $directoryJumpResult = 'passed'
        Assert-AppHealthy -ExpectedProcessId $firstLaunch.ProcessId
    }
    elseif ($VerifyCbzFixture) {
        $firstLocation = Get-ReaderViewportState
        if (
            $firstLocation.Section -ne 1 -or
            $firstLocation.Sections -ne 4 -or
            $firstLocation.Page -ne 1 -or
            $firstLocation.Pages -ne 1
        ) {
            throw 'The Android CBZ fixture must open at section 1 of 4 on its only page.'
        }
        $firstPagePssKiB = Get-AppPssKiB -ProcessId $firstLaunch.ProcessId
        for ($section = 2; $section -le $firstLocation.Sections; $section++) {
            $turn = [Diagnostics.Stopwatch]::StartNew()
            Invoke-ReaderNextPage
            $lastLocation = Wait-ReaderViewportState -Section $section -Page 1
            if ($section -eq 3) {
                Wait-UiNode -XPath "//node[@package='$package' and (@text='图片无法显示' or @content-desc='图片无法显示')]" | Out-Null
                $corruptPagePlaceholderResult = 'passed'
            }
            $pageTurnWaitMs += [long]$turn.ElapsedMilliseconds
            Assert-AppHealthy -ExpectedProcessId $firstLaunch.ProcessId
        }
        $lastPagePssKiB = Get-AppPssKiB -ProcessId $firstLaunch.ProcessId
        $restartLocatorResult = 'pending'
    }
    elseif ($VerifyMarkdownText) {
        Write-Host '[android:text] stage=reader-open'
        $textWebView = Open-AndroidWebViewSession -ProcessId $firstLaunch.ProcessId
        try {
            $initialTextLocation = Get-CdpReaderViewportState -Connection $textWebView
            $observedSectionCount = $initialTextLocation.Sections
            if (
                $initialTextLocation.Section -ne 1 -or
                $initialTextLocation.Sections -lt 1 -or
                $initialTextLocation.Sections -gt 1000
            ) {
                throw 'The Android text reader must open at section 1 within the manifest boundary.'
            }
            if (
                $bookFormat -eq 'txt' -and
                ($initialTextLocation.Sections -lt 2 -or $initialTextLocation.Sections -gt 16)
            ) {
                throw 'The private TXT reader must expose the accepted aggregate section count.'
            }

            Show-CdpReaderTools -Connection $textWebView
            Invoke-CdpClick -Connection $textWebView -Selector '.reader-tool.directory > summary'
            $tocItemCount = [int](Get-AndroidWebViewValue -Connection $textWebView -JavaScript "document.querySelectorAll('#directory-list button').length")
            if ($tocItemCount -lt 1 -or $tocItemCount -gt 2000) {
                throw 'The Android text directory item count is outside the manifest boundary.'
            }
            if ($bookFormat -eq 'txt' -and $tocItemCount -ne 1134) {
                throw 'The private TXT directory must expose the accepted aggregate TOC count.'
            }
            if (
                $textImportTelemetry.Sections -ne $observedSectionCount -or
                $textImportTelemetry.TocItems -ne $tocItemCount
            ) {
                throw 'Android text importer telemetry and the rendered manifest counts disagree.'
            }
            $tocBoundPssKiB = Get-AppPssKiB -ProcessId $firstLaunch.ProcessId
            $firstDirectoryLocation = $null
            $previousDirectoryLocation = $null
            $directoryTargets = @(
                [pscustomobject]@{ Name = 'first'; Index = 1 },
                [pscustomobject]@{ Name = 'middle'; Index = [int][Math]::Ceiling($tocItemCount / 2.0) },
                [pscustomobject]@{ Name = 'last'; Index = $tocItemCount }
            )
            foreach ($target in $directoryTargets) {
                Write-Host "[android:text] stage=directory-$($target.Name)"
                $directoryWatch = [Diagnostics.Stopwatch]::StartNew()
                Invoke-CdpClick -Connection $textWebView -Selector "#directory-list button:nth-child($($target.Index))"
                Wait-CdpCondition -Connection $textWebView -JavaScript "!document.documentElement.hasAttribute('data-reader-tools')" -TimeoutSeconds 120
                $targetLocation = Get-CdpReaderViewportState -Connection $textWebView
                if (
                    $targetLocation.Section -lt 1 -or
                    $targetLocation.Section -gt $observedSectionCount -or
                    ($null -ne $previousDirectoryLocation -and (
                        $targetLocation.Section -lt $previousDirectoryLocation.Section -or
                        ($targetLocation.Section -eq $previousDirectoryLocation.Section -and
                            $targetLocation.Page -lt $previousDirectoryLocation.Page)
                    ))
                ) {
                    throw 'The Android text directory does not follow document order.'
                }
                if ($null -eq $firstDirectoryLocation) { $firstDirectoryLocation = $targetLocation }
                $previousDirectoryLocation = $targetLocation
                $directoryTargetWaitMs[$target.Name] = [long]$directoryWatch.ElapsedMilliseconds
                switch ($target.Name) {
                    'first' {
                        $firstPagePssKiB = Get-AppPssKiB -ProcessId $firstLaunch.ProcessId
                        $pageTurnWatch = [Diagnostics.Stopwatch]::StartNew()
                        Invoke-ReaderNextPage
                        $null = Wait-CdpReaderViewportChange -Connection $textWebView -Before $targetLocation -TimeoutSeconds 120
                        $pageTurnBenchmarkMs = [long]$pageTurnWatch.ElapsedMilliseconds
                        $pageTurnWaitMs += $pageTurnBenchmarkMs
                    }
                    'middle' {
                        $middlePagePssKiB = Get-AppPssKiB -ProcessId $firstLaunch.ProcessId
                    }
                    'last' {
                        $lastPagePssKiB = Get-AppPssKiB -ProcessId $firstLaunch.ProcessId
                        $lastLocation = $targetLocation
                    }
                }
                Assert-AppHealthy -ExpectedProcessId $firstLaunch.ProcessId
                if ($target.Name -ne 'last') {
                    Show-CdpReaderTools -Connection $textWebView
                    Invoke-CdpClick -Connection $textWebView -Selector '.reader-tool.directory > summary'
                }
            }
            if (
                $firstDirectoryLocation.Section -eq $lastLocation.Section -and
                $firstDirectoryLocation.Page -eq $lastLocation.Page
            ) {
                throw 'The Android text directory did not move between its first and last items.'
            }
            $directoryTargetResult = 'passed'
            $directoryJumpResult = 'passed-first-middle-last'

            Write-Host '[android:text] stage=full-search'
            Show-CdpReaderTools -Connection $textWebView
            Invoke-CdpClick -Connection $textWebView -Selector '.reader-tool.search > summary'
            Focus-CdpElement -Connection $textWebView -Selector '#search-query'
            $searchToken = 'atha' + [Guid]::NewGuid().ToString('N')
            Invoke-Adb shell input text $searchToken | Out-Null
            $searchWatch = [Diagnostics.Stopwatch]::StartNew()
            Invoke-CdpClick -Connection $textWebView -Selector '#search-form button[type="submit"]'
            Wait-CdpCondition -Connection $textWebView -JavaScript "document.querySelector('#search-status')?.textContent === '找到 0 条'" -TimeoutSeconds 120
            $searchWaitMs = [long]$searchWatch.ElapsedMilliseconds
            $null = Wait-AppLog -ProcessId $firstLaunch.ProcessId -Pattern 'atha::reader.*event=reader_search' -TimeoutSeconds 120
            $searchTelemetry = Get-SearchTelemetry -ProcessId $firstLaunch.ProcessId
            if (
                $searchTelemetry.Results -ne 0 -or
                $searchTelemetry.Truncated -or
                $searchTelemetry.SectionsScanned -ne $observedSectionCount
            ) {
                throw 'Android full-search telemetry did not match the completed zero-result scan.'
            }
            $searchResult = 'passed-full-scan-zero-results'
            $restartLocatorResult = 'pending'
            Assert-AppHealthy -ExpectedProcessId $firstLaunch.ProcessId
        }
        finally {
            Close-AndroidWebViewSession -Connection $textWebView
        }
    }

    $pickerCache = (Invoke-Adb shell run-as $package ls '-A' 'cache/Picker' | Out-String).Trim()
    if ($pickerCache.Length -ne 0) { throw 'Android picker cache was not cleaned after book import.' }
    $privateLogPattern = "(?i)content://|/sdcard/|$([regex]::Escape($remoteName))"
    if ($VerifyEpub2NcxFixture) {
        $privateLogPattern += '|Example EPUB2|Legacy Author|Part One|Nested Two|fixture-body-(?:one|two)-7cb4'
    }
    elseif ($VerifyCbzFixture) {
        $privateLogPattern += "|ComicInfo|pages[/\\](?:1|2|3|10)\.(?:png|jpe?g)|$([regex]::Escape($cbzFixtureTitle))|$([regex]::Escape($cbzFixtureWriter))|$([regex]::Escape($cbzFixtureImageToken))"
    }
    elseif ($VerifyMarkdownText) {
        $privateLogPattern += '|(?m)^.*atha::.*\b[a-f0-9]{64}\b'
        if (-not [string]::IsNullOrWhiteSpace($searchToken)) {
            $privateLogPattern += "|$([regex]::Escape($searchToken))"
        }
    }
    if ($VerifyLibraryShelfUi -or $VerifyMarkdownText) {
        $libraryRecordPaths = @(
            ((Invoke-Adb exec-out run-as $package find Library '-maxdepth' 1 '-type' f | Out-String) -split "`r?`n") |
                ForEach-Object { $_.Trim() } |
                Where-Object { $_ }
        )
        if ($libraryRecordPaths.Count -ne 1) {
            throw 'Clean Android verification requires exactly one local library record.'
        }
        try {
            $libraryRecord = (Invoke-Adb exec-out run-as $package cat $libraryRecordPaths[0] | Out-String) |
                ConvertFrom-Json
        }
        catch {
            throw 'Android local library record could not be parsed for privacy verification.'
        }
        $privateRecordValues = @([string]$libraryRecord.id) + @(
            $libraryRecord.authors | ForEach-Object { [string]$_ }
        )
        if ([string]::IsNullOrWhiteSpace([string]$libraryRecord.title)) {
            throw 'Android local library record has no title for privacy verification.'
        }
        if (-not [string]::Equals([string]$libraryRecord.title, 'Atha', [StringComparison]::OrdinalIgnoreCase)) {
            $privateRecordValues += [string]$libraryRecord.title
        }
        foreach ($privateRecordValue in $privateRecordValues.Where({ -not [string]::IsNullOrWhiteSpace($_) })) {
            $privateLogPattern += "|$([regex]::Escape($privateRecordValue))"
        }
    }
    $athaLogs = Get-AthaPrivacyLogs -ProcessId $firstLaunch.ProcessId
    if ($athaLogs -match $privateLogPattern) {
        throw 'Android application logs exposed picker or book content details.'
    }

    $restartWatch = [Diagnostics.Stopwatch]::StartNew()
    $secondLaunch = Start-AthaReader
    Wait-UiNode -XPath $bookXPath | Out-Null
    Assert-AppHealthy -ExpectedProcessId $secondLaunch.ProcessId
    Invoke-UiNode -XPath $bookXPath
    $reopenMs = Wait-AppLog -ProcessId $secondLaunch.ProcessId -Pattern 'atha::library.*operation=open outcome=ok'
    $reopenDurationMs = Get-AppLogDurationMs -ProcessId $secondLaunch.ProcessId -Pattern 'atha::library.*operation=open outcome=ok' -Required:$VerifyMarkdownText
    $secondStableMs = Wait-AppLog -ProcessId $secondLaunch.ProcessId -Pattern 'atha::reader.*event=reader_metric stage=first_stable'
    $secondStableDurationMs = Get-AppLogDurationMs -ProcessId $secondLaunch.ProcessId -Pattern 'atha::reader.*event=reader_metric stage=first_stable' -Required:$VerifyMarkdownText
    $secondReadyMs = Wait-AppLog -ProcessId $secondLaunch.ProcessId -Pattern 'atha::reader.*event=reader_ready'
    if ($VerifyEpub2NcxFixture) {
        $null = Wait-ReaderViewportState -Section 2 -Page $afterDirectoryJump.Page
        $restartLocatorResult = 'passed'
    }
    elseif ($VerifyCbzFixture) {
        $null = Wait-ReaderViewportState -Section $lastLocation.Section -Page $lastLocation.Page
        $restartLocatorResult = 'passed'
    }
    elseif ($VerifyMarkdownText) {
        Write-Host '[android:text] stage=restart-restore'
        $restartWebView = Open-AndroidWebViewSession -ProcessId $secondLaunch.ProcessId
        try {
            $null = Wait-CdpReaderViewportState `
                -Connection $restartWebView `
                -Section $lastLocation.Section `
                -Page $lastLocation.Page `
                -TimeoutSeconds 120
            $restartLocatorResult = 'passed'
        }
        finally {
            Close-AndroidWebViewSession -Connection $restartWebView
        }
    }
    $restartRestoreMs = [long]$restartWatch.ElapsedMilliseconds
    Assert-AppHealthy -ExpectedProcessId $secondLaunch.ProcessId
    $restoredPssKiB = if ($bookFormat -eq 'cbz' -or $VerifyMarkdownText) {
        Get-AppPssKiB -ProcessId $secondLaunch.ProcessId
    }
    else {
        $null
    }
    $secondAthaLogs = Get-AthaPrivacyLogs -ProcessId $secondLaunch.ProcessId
    if ($secondAthaLogs -match $privateLogPattern) {
        throw 'Restarted Android application logs exposed picker or book content details.'
    }

    $libraryShelfUiResult = 'not-requested'
    $libraryShelfOrderingResult = 'not-requested'
    if ($VerifyLibraryShelfUi) {
        $shelfLaunch = Start-AthaReader
        Wait-UiNode -XPath $bookXPath | Out-Null
        Assert-AppHealthy -ExpectedProcessId $shelfLaunch.ProcessId

        $searchXPath = "//node[@package='$package' and @class='android.widget.EditText' and @clickable='true']"
        Assert-UiNodeTouchTarget -XPath $searchXPath
        Invoke-UiNode -XPath $searchXPath
        $searchToken = ([Guid]::NewGuid().ToString('N')).Substring(0, 12)
        Invoke-Adb shell input text $searchToken | Out-Null
        Wait-UiNode -XPath "//node[@package='$package' and @text='没有找到书籍']" | Out-Null
        Invoke-Adb shell input keyevent 123 | Out-Null
        for ($index = 0; $index -lt $searchToken.Length; $index++) {
            Invoke-Adb shell input keyevent 67 | Out-Null
        }
        Invoke-Adb shell input keyevent 111 | Out-Null
        Wait-UiNode -XPath $bookXPath | Out-Null

        foreach ($viewName in @('默认', '进度', '书名', '作者')) {
            $viewXPath = "//node[@package='$package' and @class='android.widget.Button' and @text='$viewName' and @clickable='true']"
            Assert-UiNodeTouchTarget -XPath $viewXPath
            Invoke-UiNode -XPath $viewXPath
            if ($viewName -eq '进度') {
                Wait-UiNode -XPath "//node[@package='$package' and @text='在读']" | Out-Null
                Wait-UiNode -XPath "//node[@package='$package' and @text='未开始']" | Out-Null
                Start-Sleep -Seconds 2
                Save-UiScreenshot -Name 'library-android-progress'
            }
            Assert-AppHealthy -ExpectedProcessId $shelfLaunch.ProcessId
        }

        $defaultViewXPath = "//node[@package='$package' and @class='android.widget.Button' and @text='默认' and @clickable='true']"
        Invoke-UiNode -XPath $defaultViewXPath
        Wait-UiNode -XPath $bookXPath | Out-Null
        Start-Sleep -Seconds 2
        Save-UiScreenshot -Name 'library-android-default'

        $selectXPath = "//node[@package='$package' and @class='android.widget.Button' and @text='选择' and @clickable='true']"
        $selectAllXPath = "//node[@package='$package' and @class='android.widget.Button' and @text='全选' and @clickable='true']"
        $cancelXPath = "//node[@package='$package' and @class='android.widget.Button' and @text='取消' and @clickable='true']"
        Assert-UiNodeTouchTarget -XPath $selectXPath
        Invoke-UiNode -XPath $selectXPath
        Wait-UiNode -XPath "//node[@package='$package' and @text='选择书籍']" | Out-Null
        Assert-UiNodeTouchTarget -XPath $selectAllXPath
        Invoke-UiNode -XPath $selectAllXPath
        Wait-UiNode -XPath "//node[@package='$package' and @text='取消全选']" | Out-Null
        Wait-UiNode -XPath "//node[@package='$package' and @text='已选择 1 本']" | Out-Null
        Start-Sleep -Seconds 2
        Save-UiScreenshot -Name 'library-android-selection'
        Assert-UiNodeTouchTarget -XPath $cancelXPath
        Invoke-UiNode -XPath $cancelXPath
        Wait-UiNode -XPath $selectXPath | Out-Null

        Invoke-UiNode -XPath $selectXPath
        Invoke-UiNode -XPath $selectAllXPath
        $removeXPath = "//node[@package='$package' and @class='android.widget.Button' and @text='移出书架' and @clickable='true']"
        Assert-UiNodeTouchTarget -XPath $removeXPath
        Invoke-UiNode -XPath $removeXPath
        Invoke-UiNode -XPath "//node[@resource-id='android:id/button1' and @clickable='true']"
        Wait-UiNode -XPath "//node[@package='$package' and @text='开始你的书架']" | Out-Null
        Assert-AppHealthy -ExpectedProcessId $shelfLaunch.ProcessId

        $shelfLogs = Get-AthaPrivacyLogs -ProcessId $shelfLaunch.ProcessId
        $shelfPrivateLogPattern = "$privateLogPattern|$([regex]::Escape($searchToken))"
        if ($shelfLogs -match $shelfPrivateLogPattern) {
            throw 'Android shelf UI logs exposed picker, book, or search details.'
        }
        $libraryShelfUiResult = 'passed'
        $libraryShelfOrderingResult = 'not-asserted-single-book-emulator'
    }

    $evidenceDirectory = Join-Path $repoRoot 'artifacts\local\android'
    New-Item -ItemType Directory -Path $evidenceDirectory -Force | Out-Null
    $apkSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $apkPath).Hash.ToLowerInvariant()
    $gateSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $PSCommandPath).Hash.ToLowerInvariant()
    $fixtureEvidenceSha256 = if ($VerifyEpub2NcxFixture -or $VerifyCbzFixture) {
        $bookSha256
    }
    else {
        'not-requested'
    }
    $textBenchmarkMetricsMs = $null
    if ($VerifyMarkdownText) {
        $textBenchmarkMetricsMs = [ordered]@{
            cold_import = [double]$importDurationMs
            backend_total = [double]$textImportTelemetry.DurationMs
        }
        foreach ($stage in $textImportTelemetry.StageDurationsMs.GetEnumerator()) {
            $textBenchmarkMetricsMs["backend_$($stage.Key)"] = [double]$stage.Value
        }
        $textBenchmarkMetricsMs.cached_open = [double]$openDurationMs
        $textBenchmarkMetricsMs.first_stable = [double]$firstStableDurationMs
        $textBenchmarkMetricsMs.directory_first = [double]$directoryTargetWaitMs.first
        $textBenchmarkMetricsMs.directory_middle = [double]$directoryTargetWaitMs.middle
        $textBenchmarkMetricsMs.directory_last = [double]$directoryTargetWaitMs.last
        $textBenchmarkMetricsMs.page_turn = [double]$pageTurnBenchmarkMs
        $textBenchmarkMetricsMs.full_search = [double]$searchWaitMs
        $textBenchmarkMetricsMs.restart_restore = [double]$restartRestoreMs
    }
    $evidence = [ordered]@{
        schema = 'atha.android-reader-gate.v2'
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
            sha256 = $apkSha256
            x86_64_library_count = $nativeEntries.Count
            zipaligned_16k = $true
        }
        gate = [ordered]@{
            sha256 = $gateSha256
        }
        installation = [ordered]@{
            clean_app_data = [bool]$CleanAppData
            cold_start_ready = $true
            startup_wait_ms = [long]$firstLaunch.StartupMs
            startup_duration_ms = $firstLaunch.StartupDurationMs
        }
        privacy = [ordered]@{
            application_process_applog_logcat = 'passed'
            boundary = 'process-scoped logcat plus persistent logs/Atha.log*; system picker process logs excluded'
            private_text_screenshot = if ($bookFormat -eq 'txt') { 'not-captured' } else { 'not-applicable' }
            private_text_search_snapshot = if ($bookFormat -eq 'txt') { 'not-serialized' } else { 'not-applicable' }
            source_path_title_content_hash = if ($VerifyMarkdownText) { 'not-recorded' } else { 'not-applicable' }
        }
        health = 'passed'
        library_shelf_ui = [ordered]@{
            requested = [bool]$VerifyLibraryShelfUi
            result = $libraryShelfUiResult
            ordering = $libraryShelfOrderingResult
        }
        book = [ordered]@{
            format = $bookFormat
            fixture_sha256 = $fixtureEvidenceSha256
            input_bytes = if ($VerifyMarkdownText) { [long]$bookInputBytes } else { $null }
            encoding = if ($bookFormat -eq 'txt') { $textImportTelemetry.Encoding } else { $null }
            system_picker = 'passed'
            import = 'passed'
            import_wait_ms = [long]$importMs
            import_duration_ms = $importDurationMs
            backend_import_duration_ms = if ($VerifyMarkdownText) { [double]$textImportTelemetry.DurationMs } else { $null }
            backend_import_stage_duration_ms = if ($VerifyMarkdownText) { $textImportTelemetry.StageDurationsMs } else { $null }
            open = 'passed'
            open_wait_ms = [long]$openMs
            open_duration_ms = $openDurationMs
            first_stable = 'passed'
            first_stable_wait_ms = [long]$firstStableMs
            first_stable_duration_ms = $firstStableDurationMs
            reader_ready = 'passed'
            reader_ready_wait_ms = [long]$readyMs
            directory_second_item_jump = $directoryJumpResult
            directory_first_middle_last = $directoryTargetResult
            directory_wait_ms = $directoryTargetWaitMs
            sections = if ($VerifyMarkdownText) { [int]$observedSectionCount } else { $null }
            toc_items = if ($VerifyMarkdownText) { [int]$tocItemCount } else { $null }
            full_search = $searchResult
            full_search_results = if ($VerifyMarkdownText) { [int]$searchTelemetry.Results } else { $null }
            full_search_truncated = if ($VerifyMarkdownText) { [bool]$searchTelemetry.Truncated } else { $null }
            full_search_sections_scanned = if ($VerifyMarkdownText) { [int]$searchTelemetry.SectionsScanned } else { $null }
            full_search_wait_ms = $searchWaitMs
            page_turns = if ($VerifyCbzFixture -or $VerifyMarkdownText) { 'passed' } else { 'not-requested' }
            page_turn_wait_ms = @($pageTurnWaitMs)
            corrupt_page_placeholder = $corruptPagePlaceholderResult
            shelf_cover = $shelfCoverResult
            restart_shelf_persistence = 'passed'
            restart_locator_persistence = $restartLocatorResult
            reopen = 'passed'
            reopen_wait_ms = [long]$reopenMs
            reopen_duration_ms = $reopenDurationMs
            restart_first_stable_wait_ms = [long]$secondStableMs
            restart_first_stable_duration_ms = $secondStableDurationMs
            restart_reader_ready_wait_ms = [long]$secondReadyMs
            restart_restore_wait_ms = [long]$restartRestoreMs
            picker_cache_clean = $true
        }
        text_benchmark = [ordered]@{
            requested = $TextBenchmarkSample -gt 0
            sample = if ($TextBenchmarkSample -gt 0) { $TextBenchmarkSample } else { $null }
            required_samples = if ($VerifyMarkdownText -and $bookFormat -eq 'txt') { 10 } else { $null }
            metrics_ms = $textBenchmarkMetricsMs
        }
        memory = [ordered]@{
            unit = 'KiB'
            app_process = [ordered]@{
                shelf_pss = $shelfPssKiB
                imported_shelf_pss = $importedShelfPssKiB
                toc_bound_pss = $tocBoundPssKiB
                first_page_pss = $firstPagePssKiB
                middle_page_pss = $middlePagePssKiB
                last_page_pss = $lastPagePssKiB
                restored_pss = $restoredPssKiB
            }
            webview_renderer = [ordered]@{
                pss = $null
                reason = 'renderer-process-not-uniquely-attributable'
            }
        }
        # Message SAF stays a separate opt-in round-trip: restore replaces the store and export needs an opened edition.
        message_saf = [ordered]@{
            backup = 'separate-opt-in'
            restore = 'separate-opt-in'
            export = 'separate-opt-in'
        }
    }
    $evidenceName = if ($TextBenchmarkSample -gt 0) {
        'reader-gate-txt-sample-{0:D2}.json' -f $TextBenchmarkSample
    }
    else {
        "reader-gate-$bookFormat.json"
    }
    $evidencePath = Join-Path $evidenceDirectory $evidenceName
    $evidence | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $evidencePath -Encoding utf8NoBOM
    $textBenchmarkSummaryRecorded = $false
    if ($TextBenchmarkSample -gt 0) {
        $sampleRecords = @()
        for ($sample = 1; $sample -le 10; $sample++) {
            $samplePath = Join-Path $evidenceDirectory ('reader-gate-txt-sample-{0:D2}.json' -f $sample)
            if (-not (Test-Path -LiteralPath $samplePath -PathType Leaf)) { continue }
            try {
                $record = Get-Content -LiteralPath $samplePath -Raw | ConvertFrom-Json
            }
            catch {
                throw 'An Android TXT benchmark sample is not valid JSON.'
            }
            if (
                $record.gate.sha256 -eq $gateSha256 -and
                $record.apk.sha256 -eq $apkSha256 -and
                $record.webview.package -eq $webviewPackage -and
                $record.webview.version -eq $webviewVersion -and
                $record.target.avd -eq $actualAvd -and
                [int]$record.target.api -eq [int]$actualApi -and
                $record.target.abi -eq $actualAbi -and
                [int]$record.target.page_size -eq [int]$pageSize -and
                $record.book.format -eq 'txt' -and
                [long]$record.book.input_bytes -eq $bookInputBytes -and
                $record.book.encoding -eq $textImportTelemetry.Encoding -and
                [int]$record.text_benchmark.sample -eq $sample -and
                [int]$record.book.sections -ge 2 -and
                [int]$record.book.sections -le 16 -and
                [int]$record.book.toc_items -eq 1134 -and
                $record.book.directory_first_middle_last -eq 'passed' -and
                $record.book.full_search -eq 'passed-full-scan-zero-results' -and
                [int]$record.book.full_search_results -eq 0 -and
                -not [bool]$record.book.full_search_truncated -and
                [int]$record.book.full_search_sections_scanned -eq [int]$record.book.sections -and
                $record.book.restart_locator_persistence -eq 'passed' -and
                $record.privacy.application_process_applog_logcat -eq 'passed' -and
                $record.health -eq 'passed'
            ) {
                $sampleRecords += $record
            }
        }
        if ($sampleRecords.Count -eq 10) {
            $timingSummaries = [ordered]@{}
            foreach ($metric in @(
                'cold_import',
                'backend_total',
                'backend_detect',
                'backend_chapter_scan',
                'backend_render_write',
                'backend_publish',
                'cached_open',
                'first_stable',
                'directory_first',
                'directory_middle',
                'directory_last',
                'page_turn',
                'full_search',
                'restart_restore'
            )) {
                $values = @($sampleRecords | ForEach-Object { $_.text_benchmark.metrics_ms.$metric })
                $timingSummaries[$metric] = Get-TenSampleSummary -Values $values
            }
            $pssSummaries = [ordered]@{}
            foreach ($stage in @(
                'shelf_pss',
                'imported_shelf_pss',
                'toc_bound_pss',
                'first_page_pss',
                'middle_page_pss',
                'last_page_pss',
                'restored_pss'
            )) {
                $values = @($sampleRecords | ForEach-Object { $_.memory.app_process.$stage })
                $pssSummaries[$stage] = Get-TenSampleSummary -Values $values
            }
            $summary = [ordered]@{
                schema = 'atha.android-text-benchmark.v1'
                recorded_at_utc = [DateTime]::UtcNow.ToString('o')
                evidence_level = 'android-emulator-relative-baseline'
                target = $evidence.target
                webview = $evidence.webview
                apk = $evidence.apk
                gate = $evidence.gate
                format = 'txt'
                input_bytes = [long]$bookInputBytes
                encoding = $textImportTelemetry.Encoding
                sections = [int]$sampleRecords[0].book.sections
                toc_items = 1134
                samples = 10
                median_method = 'mean-of-middle-two'
                p95_method = 'nearest-rank'
                timing_ms = $timingSummaries
                app_pss_kib = $pssSummaries
                renderer_pss = $null
                renderer_pss_reason = 'renderer-process-not-uniquely-attributable'
                completion_scope = 'AVD functional and same-environment baseline; ARM64 device still required'
            }
            $summaryPath = Join-Path $evidenceDirectory 'reader-gate-txt-summary.json'
            $summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $summaryPath -Encoding utf8NoBOM
            $textBenchmarkSummaryRecorded = $true
        }
    }
    if ($VerifyEpub2NcxFixture) {
        Write-Host 'Android EPUB picker, open, directory jump, reader-ready, restart, and location persistence slice passed; structured evidence recorded.'
    }
    elseif ($VerifyCbzFixture) {
        Write-Host 'Android CBZ picker, first page, all page turns, restart, and final location persistence slice passed; structured evidence recorded.'
    }
    elseif ($VerifyMarkdownText) {
        if ($TextBenchmarkSample -gt 0) {
            Write-Host "Android private TXT functional and performance sample $TextBenchmarkSample of 10 passed; structured evidence recorded."
            if ($textBenchmarkSummaryRecorded) {
                Write-Host 'All 10 Android private TXT samples share the current APK and gate; median and nearest-rank P95 summary recorded.'
            }
        }
        else {
            Write-Host 'Android Markdown/TXT picker, section and TOC counts, directory targets, full search, page turn, restart, and location persistence slice passed; structured evidence recorded.'
        }
    }
    else {
        Write-Host 'Android book picker, open, reader-ready, restart, and shelf persistence slice passed; structured evidence recorded.'
    }
    }
    finally {
        $null = & $adb -s $serial shell rm '-f' $remoteFixture 2>&1
    }
}

Write-Host "Android reader gate passed: $avdName, API $actualApi, $actualAbi, ${pageSize}-byte pages, WebView $webviewPackage $webviewVersion."
