# Description: Generate and verify the deterministic FB2 fixture, with an optional Linux GUI gate.

[CmdletBinding()]
param(
    [switch]$VerifyLinuxGui,
    [string]$DictionaryFixtureRoot,
    [string]$FormulaBenchmarkEpub,
    [string]$FormulaBenchmarkEntry,
    [ValidateRange(1, 100000)]
    [int]$FormulaBenchmarkMinimumFormulas = 1000,
    [ValidateRange(1, 10000)]
    [int]$FormulaBenchmarkMinimumPages = 10,
    [ValidateRange(0, 20)]
    [int]$GestureWarmupSamples = 5,
    [ValidateRange(1, 100)]
    [int]$GestureMeasureSamples = 20
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$fixturePath = Join-Path $repoRoot '.tmp/fb2-gate.fb2'
$importsRoot = Join-Path $repoRoot '.tmp/fb2-gate-imports'
$guiRoot = Join-Path $repoRoot '.tmp/fb2-linux-gui'
$expectedFixtureSha256 = '155225e7aa977574c5f75559f58ad121004bf714b91e10caeacd774da5550186'

. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot
$cargoPath = $env:ATHA_CARGO
$formulaSource = $null
$formulaPrivateTokens = @()
if ([string]::IsNullOrWhiteSpace($FormulaBenchmarkEpub) -ne [string]::IsNullOrWhiteSpace($FormulaBenchmarkEntry)) {
    throw 'FormulaBenchmarkEpub and FormulaBenchmarkEntry must be provided together.'
}
if (-not [string]::IsNullOrWhiteSpace($FormulaBenchmarkEpub)) {
    if (-not $VerifyLinuxGui) { throw 'FormulaBenchmarkEpub requires VerifyLinuxGui.' }
    if (-not (Test-Path -LiteralPath $FormulaBenchmarkEpub -PathType Leaf)) {
        throw 'Formula benchmark EPUB does not exist.'
    }
    $formulaSource = (Resolve-Path -LiteralPath $FormulaBenchmarkEpub).Path
}

function Invoke-CheckedCargo {
    param([string[]] $Arguments, [string] $Failure)

    & $cargoPath @Arguments
    if ($LASTEXITCODE -ne 0) { throw $Failure }
}

function Invoke-Checked {
    param([string] $FilePath, [string[]] $Arguments, [string] $Failure)

    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) { throw $Failure }
}

function Get-FreePort {
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start()
    try { return ([Net.IPEndPoint]$listener.LocalEndpoint).Port }
    finally { $listener.Stop() }
}

function Invoke-WebDriver {
    param(
        [string] $BaseUrl,
        [string] $Method,
        [string] $Path,
        [object] $Body
    )

    $arguments = @{
        Uri = "$BaseUrl$Path"
        Method = $Method
        ContentType = 'application/json'
        TimeoutSec = 30
    }
    if ($null -ne $Body) {
        $arguments.Body = $Body | ConvertTo-Json -Depth 8 -Compress
    }
    Invoke-RestMethod @arguments
}

function Invoke-WebDriverScript {
    param([string] $BaseUrl, [string] $Session, [string] $Script)

    (Invoke-WebDriver -BaseUrl $BaseUrl -Method Post -Path "/session/$Session/execute/sync" -Body @{
        script = $Script
        args = @()
    }).value
}

function Invoke-WebDriverAsyncScript {
    param(
        [string] $BaseUrl,
        [string] $Session,
        [string] $Script,
        [object[]] $ScriptArguments = @()
    )

    (Invoke-WebDriver -BaseUrl $BaseUrl -Method Post -Path "/session/$Session/execute/async" -Body @{
        script = $Script
        args = $ScriptArguments
    }).value
}

function Enable-ReaderGestureDiagnostics {
    param([string] $BaseUrl, [string] $Session)

    if (Invoke-WebDriverScript -BaseUrl $BaseUrl -Session $Session -Script 'return Boolean(globalThis.__athaReaderDiagnostics?.beginGestureProbe);') {
        return
    }
    [void](Invoke-WebDriverScript -BaseUrl $BaseUrl -Session $Session -Script @'
const url = new URL(location.href);
url.searchParams.set('gesture-probe', '1');
location.replace(url.href);
return true;
'@)
    [void](Wait-WebDriverScript -BaseUrl $BaseUrl -Session $Session -Failure 'Reader gesture diagnostics did not become ready.' -Script @'
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  available: Boolean(globalThis.__athaReaderDiagnostics?.beginGestureProbe)
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.available })
}

function Invoke-WebDriverRequestedTouchGesture {
    param(
        [string] $BaseUrl,
        [string] $Session,
        [object] $Point,
        [ValidateSet('tap', 'drag')]
        [string] $Action,
        [ValidateRange(8, 12)]
        [int] $Steps = 10
    )

    $actions = [Collections.Generic.List[object]]::new()
    $actions.Add(@{
        type = 'pointerMove'
        duration = 0
        origin = 'viewport'
        x = [int]$Point.x
        y = [int]$Point.y
    })
    $actions.Add(@{ type = 'pointerDown'; button = 0 })
    if ($Action -eq 'tap') {
        $actions.Add(@{ type = 'pause'; duration = 40 })
    }
    else {
        foreach ($step in 1..$Steps) {
            $ratio = $step / $Steps
            $actions.Add(@{
                type = 'pointerMove'
                duration = 16
                origin = 'viewport'
                x = [int][Math]::Round([double]$Point.x + ([double]$Point.endX - [double]$Point.x) * $ratio)
                y = [int][Math]::Round([double]$Point.y + ([double]$Point.endY - [double]$Point.y) * $ratio)
            })
        }
    }
    $actions.Add(@{ type = 'pointerUp'; button = 0 })

    try {
        [void](Invoke-WebDriver -BaseUrl $BaseUrl -Method Post -Path "/session/$Session/actions" -Body @{
            actions = @(@{
                type = 'pointer'
                id = "atha-touch-$([Guid]::NewGuid().ToString('N'))"
                parameters = @{ pointerType = 'touch' }
                actions = @($actions)
            })
        })
    }
    finally {
        try { [void](Invoke-WebDriver -BaseUrl $BaseUrl -Method Delete -Path "/session/$Session/actions" -Body $null) }
        catch { }
    }
}

function Get-NearestRank {
    param([double[]] $Values, [double] $Ratio = 0.95)

    if ($Values.Count -eq 0) { return $null }
    $ordered = @($Values | Sort-Object)
    $ordered[[Math]::Max(0, [Math]::Ceiling($ordered.Count * $Ratio) - 1)]
}

function Test-FiniteNumber {
    param([object] $Value)

    if ($null -eq $Value) { return $false }
    try { $number = [double]$Value }
    catch { return $false }
    -not [double]::IsNaN($number) -and -not [double]::IsInfinity($number)
}

function Invoke-ReaderGestureGate {
    param(
        [string] $BaseUrl,
        [string] $Session,
        [int] $WarmupSamples,
        [int] $MeasureSamples
    )

    $beginScript = @'
const done = arguments[arguments.length - 1];
globalThis.__athaReaderDiagnostics
  .beginGestureProbe(arguments[0], arguments[1], arguments[2], arguments[3])
  .then((value) => done({ ok: true, value }))
  .catch((error) => done({ ok: false, error: error instanceof Error ? error.message : String(error) }));
'@
    $finishScript = @'
const done = arguments[arguments.length - 1];
globalThis.__athaReaderDiagnostics
  .finishGestureProbe(arguments[0])
  .then((value) => done({ ok: true, value }))
  .catch((error) => done({ ok: false, error: error instanceof Error ? error.message : String(error) }));
'@
    $cleanupScript = @'
const done = arguments[arguments.length - 1];
globalThis.__athaReaderDiagnostics
  .cleanupGestureProbe()
  .then((value) => done({ ok: true, value }))
  .catch((error) => done({ ok: false, error: error instanceof Error ? error.message : String(error) }));
'@
    $scenarios = @(
        [pscustomobject]@{ name = 'ordinary-tap'; target = 'ordinary'; action = 'tap'; mode = 'edge'; direction = 1; expectation = 'page' },
        [pscustomobject]@{ name = 'ordinary-drag'; target = 'ordinary'; action = 'drag'; mode = 'edge'; direction = 1; expectation = 'page' },
        [pscustomobject]@{ name = 'formula-tap'; target = 'formula'; action = 'tap'; mode = 'edge'; direction = 1; expectation = 'page' },
        [pscustomobject]@{ name = 'formula-drag'; target = 'formula'; action = 'drag'; mode = 'edge'; direction = 1; expectation = 'page' },
        [pscustomobject]@{ name = 'formula-vertical'; target = 'formula'; action = 'drag'; mode = 'vertical'; direction = 1; expectation = 'protected' },
        [pscustomobject]@{ name = 'table-tap'; target = 'table'; action = 'tap'; mode = 'edge'; direction = 1; expectation = 'page' },
        [pscustomobject]@{ name = 'table-drag'; target = 'table'; action = 'drag'; mode = 'edge'; direction = 1; expectation = 'page' },
        [pscustomobject]@{ name = 'table-vertical'; target = 'table'; action = 'drag'; mode = 'vertical'; direction = 1; expectation = 'protected' },
        [pscustomobject]@{ name = 'overflow-table-tap'; target = 'overflow-table'; action = 'tap'; mode = 'edge'; direction = 1; expectation = 'page' },
        [pscustomobject]@{ name = 'overflow-table-pan-next'; target = 'overflow-table'; action = 'drag'; mode = 'pan'; direction = 1; expectation = 'pan' },
        [pscustomobject]@{ name = 'overflow-table-pan-previous'; target = 'overflow-table'; action = 'drag'; mode = 'pan'; direction = -1; expectation = 'pan' },
        [pscustomobject]@{ name = 'overflow-table-edge-next'; target = 'overflow-table'; action = 'drag'; mode = 'edge'; direction = 1; expectation = 'page' },
        [pscustomobject]@{ name = 'overflow-table-edge-previous'; target = 'overflow-table'; action = 'drag'; mode = 'edge'; direction = -1; expectation = 'page' }
    )
    $failures = [Collections.Generic.List[string]]::new()
    $measurements = [Collections.Generic.List[object]]::new()
    $smokeChecked = $false
    $trustedPointer = $false
    $nativeTouchObserved = $false
    $pointerTypes = @()

    Enable-ReaderGestureDiagnostics -BaseUrl $BaseUrl -Session $Session
    try {
        foreach ($scenario in $scenarios) {
            $samples = $WarmupSamples + $MeasureSamples
            for ($sample = 0; $sample -lt $samples; $sample += 1) {
                $begin = Invoke-WebDriverAsyncScript -BaseUrl $BaseUrl -Session $Session -Script $beginScript -ScriptArguments @(
                    $scenario.target, $scenario.action, $scenario.mode, $scenario.direction
                )
                if (-not $begin.ok) {
                    throw "Gesture probe setup failed for $($scenario.name): $($begin.error)"
                }
                Invoke-WebDriverRequestedTouchGesture -BaseUrl $BaseUrl -Session $Session -Point $begin.value -Action $scenario.action -Steps 10
                $finished = Invoke-WebDriverAsyncScript -BaseUrl $BaseUrl -Session $Session -Script $finishScript -ScriptArguments @($begin.value.id)
                if (-not $finished.ok) {
                    throw "Gesture probe result failed for $($scenario.name): $($finished.error)"
                }
                $result = $finished.value
                if (-not $smokeChecked) {
                    if (-not $result.targetHit -or -not $result.trusted) {
                        $smoke = [ordered]@{
                            targetHit = [bool]$result.targetHit
                            trusted = [bool]$result.trusted
                            touch = [bool]$result.touch
                            pointerTypes = @($result.pointerTypes)
                            pointerMoves = [int]$result.pointerMoves
                        } | ConvertTo-Json -Compress
                        throw "W3C requested-touch pointer smoke failed: $smoke"
                    }
                    $trustedPointer = [bool]$result.trusted
                    $nativeTouchObserved = [bool]$result.touch
                    $pointerTypes = @($result.pointerTypes)
                    $smokeChecked = $true
                }

                $sampleLabel = if ($sample -lt $WarmupSamples) { "warmup-$($sample + 1)" } else { "measure-$($sample - $WarmupSamples + 1)" }
                $reasons = [Collections.Generic.List[string]]::new()
                if (-not $result.targetHit) { $reasons.Add('target-miss') }
                if (-not $result.trusted) { $reasons.Add('untrusted-pointer') }
                if (-not $result.settled) { $reasons.Add('not-settled') }
                if ($result.preview) { $reasons.Add('preview-opened') }
                if ($result.compatibilityEvents -ne 0) { $reasons.Add("compatibility-events=$($result.compatibilityEvents)") }
                if ($scenario.action -eq 'drag' -and $result.pointerMoves -lt 10) { $reasons.Add("pointer-moves=$($result.pointerMoves)") }
                if (-not (Test-FiniteNumber $result.timing.releaseToStableMs)) { $reasons.Add('invalid-settle-timing') }
                if ($scenario.expectation -ne 'protected') {
                    if (-not (Test-FiniteNumber $result.timing.inputToFirstVisualMs)) { $reasons.Add('invalid-input-timing') }
                    if ($scenario.action -eq 'tap' -and -not (Test-FiniteNumber $result.timing.releaseToFirstVisualMs)) { $reasons.Add('invalid-tap-timing') }
                }
                if ($scenario.expectation -eq 'page') {
                    if (-not $result.singlePage) { $reasons.Add('not-single-page') }
                    if ($scenario.action -eq 'drag') {
                        if ($result.rafTransformSamples -lt 6) { $reasons.Add("raf-transforms=$($result.rafTransformSamples)") }
                    }
                }
                elseif ($scenario.expectation -eq 'pan') {
                    if (-not $result.samePage) { $reasons.Add('pan-changed-page') }
                    if ([Math]::Abs([double]$result.scrollDelta) -lt 24) { $reasons.Add("scroll-delta=$($result.scrollDelta)") }
                }
                else {
                    if (-not $result.samePage) { $reasons.Add('vertical-changed-page') }
                    if ([Math]::Abs([double]$result.scrollDelta) -ge 1) { $reasons.Add("vertical-scroll-delta=$($result.scrollDelta)") }
                    if ($result.visualUpdateSamples -ne 0) { $reasons.Add("vertical-visual-updates=$($result.visualUpdateSamples)") }
                }
                if ($scenario.action -eq 'drag' -and $scenario.expectation -ne 'protected') {
                    $minimumVisualUpdates = [Math]::Max(3, [Math]::Ceiling([double]$result.pointerMoves / 2))
                    if ($result.visualUpdateSamples -lt $minimumVisualUpdates) { $reasons.Add("visual-updates=$($result.visualUpdateSamples)") }
                    if (-not (Test-FiniteNumber $result.timing.frameP95Ms)) { $reasons.Add('invalid-frame-timing') }
                    if (-not (Test-FiniteNumber $result.timing.maxFrameMs)) { $reasons.Add('invalid-maximum-timing') }
                }
                if ($reasons.Count -gt 0) {
                    $failures.Add("$($scenario.name)/$sampleLabel[$([string]::Join(',', $reasons))]")
                }
                if ($sample -ge $WarmupSamples -and $scenario.expectation -ne 'protected') {
                    $measurements.Add([pscustomobject]@{
                        name = $scenario.name
                        action = $scenario.action
                        input = $result.timing.inputToFirstVisualMs
                        tap = $result.timing.releaseToFirstVisualMs
                        frame = $result.timing.frameP95Ms
                        maximum = $result.timing.maxFrameMs
                        settle = $result.timing.releaseToStableMs
                    })
                }
            }
        }
    }
    finally {
        $cleanup = Invoke-WebDriverAsyncScript -BaseUrl $BaseUrl -Session $Session -Script $cleanupScript
        if (-not $cleanup.ok) { throw "Gesture probe cleanup failed: $($cleanup.error)" }
    }

    if ($failures.Count -gt 0) {
        throw "Trusted pointer gesture semantics failed: $([string]::Join('; ', $failures))"
    }
    $scenarioMetrics = @($measurements | Group-Object name | ForEach-Object {
        $samples = @($_.Group)
        $drag = @($samples | Where-Object action -eq 'drag')
        $tap = @($samples | Where-Object action -eq 'tap')
        [pscustomobject]@{
            name = $_.Name
            input = if ($drag.Count) { Get-NearestRank -Values @($drag | ForEach-Object { [double]$_.input }) } else { $null }
            tap = if ($tap.Count) { Get-NearestRank -Values @($tap | ForEach-Object { [double]$_.tap }) } else { $null }
            frame = if ($drag.Count) { Get-NearestRank -Values @($drag | ForEach-Object { [double]$_.frame }) } else { $null }
            settle = Get-NearestRank -Values @($samples | ForEach-Object { [double]$_.settle })
        }
    })
    if ($MeasureSamples -ge 20) {
        foreach ($metric in $scenarioMetrics) {
            if ($null -ne $metric.input -and $metric.input -gt 33.4) { throw "Gesture input-to-first-visual P95 exceeded 33.4 ms for $($metric.name): $($metric.input) ms." }
            if ($null -ne $metric.tap -and $metric.tap -gt 50) { throw "Gesture tap-to-first-visual P95 exceeded 50 ms for $($metric.name): $($metric.tap) ms." }
            if ($null -ne $metric.frame -and $metric.frame -gt 25) { throw "Gesture drag frame P95 exceeded 25 ms for $($metric.name): $($metric.frame) ms." }
            if ($metric.settle -gt 400) { throw "Gesture release-to-stable P95 exceeded 400 ms for $($metric.name): $($metric.settle) ms." }
        }
    }
    $maxFrameMeasurement = $measurements | Where-Object action -eq 'drag' | Sort-Object maximum -Descending | Select-Object -First 1
    $maxFrame = $maxFrameMeasurement.maximum
    if ($maxFrame -gt 50) { throw "Gesture maximum frame interval exceeded 50 ms for $($maxFrameMeasurement.name): $maxFrame ms." }
    $inputP95 = ($scenarioMetrics | Measure-Object -Property input -Maximum).Maximum
    $tapP95 = ($scenarioMetrics | Measure-Object -Property tap -Maximum).Maximum
    $dragFrameP95 = ($scenarioMetrics | Measure-Object -Property frame -Maximum).Maximum
    $settleP95 = ($scenarioMetrics | Measure-Object -Property settle -Maximum).Maximum

    [pscustomobject]@{
        requested_pointer_type = 'touch'
        trusted_pointer_events = $trustedPointer
        native_touch_observed = $nativeTouchObserved
        observed_pointer_types = [string]::Join(',', $pointerTypes)
        warmups_per_scenario = $WarmupSamples
        measurements_per_scenario = $MeasureSamples
        scenarios = $scenarios.Count
        input_to_first_visual_p95_ms = $inputP95
        tap_to_first_visual_p95_ms = $tapP95
        drag_frame_p95_ms = $dragFrameP95
        maximum_frame_ms = $maxFrame
        release_to_stable_p95_ms = $settleP95
    }
}

function Send-WebDriverText {
    param([string] $BaseUrl, [string] $Session, [string] $Selector, [string] $Text)

    $element = (Invoke-WebDriver -BaseUrl $BaseUrl -Method Post -Path "/session/$Session/element" -Body @{
        using = 'css selector'
        value = $Selector
    }).value
    $elementId = $element.'element-6066-11e4-a52e-4f735466cecf'
    if (-not $elementId) { throw "WebDriver could not find $Selector." }
    [void](Invoke-WebDriver -BaseUrl $BaseUrl -Method Post -Path "/session/$Session/element/$elementId/clear" -Body @{})
    [void](Invoke-WebDriver -BaseUrl $BaseUrl -Method Post -Path "/session/$Session/element/$elementId/value" -Body @{
        text = $Text
        value = @($Text.ToCharArray() | ForEach-Object { [string]$_ })
    })
}

function Wait-WebDriverScript {
    param(
        [string] $BaseUrl,
        [string] $Session,
        [string] $Script,
        [scriptblock] $Accepted,
        [string] $Failure
    )

    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $value = Invoke-WebDriverScript -BaseUrl $BaseUrl -Session $Session -Script $Script
        if (& $Accepted $value) { return $value }
        if ($value.status -eq 'fail') { throw "$Failure Error: $($value.error)" }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "$Failure State: $($value | ConvertTo-Json -Depth 4 -Compress)"
}

function New-TauriSession {
    param([string] $BaseUrl, [string] $Application)

    $response = Invoke-WebDriver -BaseUrl $BaseUrl -Method Post -Path '/session' -Body @{
        capabilities = @{
            alwaysMatch = @{
                'tauri:options' = @{ application = $Application }
            }
        }
    }
    if ([string]::IsNullOrWhiteSpace($response.value.sessionId)) {
        throw 'Tauri WebDriver did not create a session.'
    }
    $response.value
}

function Invoke-LinuxGuiGate {
    param(
        [string] $Application,
        [string] $DataRoot,
        [string] $DictionaryQuery,
        [string[]] $DictionaryPrivateTokens,
        [string[]] $BookPrivateTokens,
        [string] $FormulaEntry,
        [int] $FormulaMinimumFormulas,
        [int] $FormulaMinimumPages,
        [int] $GestureWarmups,
        [int] $GestureMeasurements
    )

    if (-not $IsLinux) { throw 'The FB2 Linux GUI gate requires Linux.' }
    foreach ($command in @('systemctl', 'systemd-run', 'WebKitWebDriver', 'identify')) {
        if (-not (Get-Command $command -ErrorAction SilentlyContinue)) {
            throw "$command is required for the FB2 Linux GUI gate."
        }
    }
    $driver = if ($env:ATHA_TAURI_DRIVER) {
        (Resolve-Path -LiteralPath $env:ATHA_TAURI_DRIVER).Path
    }
    elseif (Get-Command 'tauri-driver' -ErrorAction SilentlyContinue) {
        (Get-Command 'tauri-driver').Source
    }
    else {
        $temporaryDriver = Join-Path $repoRoot '.tmp/tauri-driver/bin/tauri-driver'
        if (-not (Test-Path -LiteralPath $temporaryDriver -PathType Leaf)) {
            throw 'tauri-driver is required; install it with cargo install tauri-driver --locked.'
        }
        $temporaryDriver
    }
    $userEnvironment = @(& systemctl --user show-environment)
    foreach ($name in @('DISPLAY', 'WAYLAND_DISPLAY', 'XDG_RUNTIME_DIR', 'DBUS_SESSION_BUS_ADDRESS')) {
        if (-not ($userEnvironment | Where-Object { $_.StartsWith("$name=", [StringComparison]::Ordinal) })) {
            throw "The systemd user manager is missing $name."
        }
    }

    $driverPort = Get-FreePort
    $nativePort = Get-FreePort
    while ($nativePort -eq $driverPort) { $nativePort = Get-FreePort }
    $baseUrl = "http://127.0.0.1:$driverPort"
    $unit = "atha-fb2-gui-$PID"
    $session = $null
    $unitStarted = $false
    $screenshot = Join-Path $guiRoot 'reader.png'
    $mobileScreenshot = Join-Path $guiRoot 'reader-mobile.png'
    $statisticsScreenshot = Join-Path $guiRoot 'reading-statistics.png'
    $statisticsMobileScreenshot = Join-Path $guiRoot 'reading-statistics-mobile.png'
    $settingsMobileScreenshot = Join-Path $guiRoot 'reader-settings-mobile.png'
    $expectedBooks = if ([string]::IsNullOrWhiteSpace($FormulaEntry)) { 1 } else { 2 }
    $formulaBenchmark = $null
    $formulaShape = $null
    try {
        Invoke-Checked 'systemd-run' @(
            '--user', '--collect', "--unit=$unit", "--working-directory=$repoRoot",
            "--setenv=XDG_DATA_HOME=$DataRoot", '--setenv=GDK_BACKEND=wayland',
            $driver, '--port', [string]$driverPort, '--native-port', [string]$nativePort
        ) 'Could not start the Tauri Linux WebDriver.'
        $unitStarted = $true
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        do {
            try {
                $client = [Net.Sockets.TcpClient]::new('127.0.0.1', $driverPort)
                $client.Dispose()
                break
            }
            catch { Start-Sleep -Milliseconds 100 }
        } while ([DateTime]::UtcNow -lt $deadline)
        if ([DateTime]::UtcNow -ge $deadline) { throw 'Tauri WebDriver did not become ready.' }

        $created = New-TauriSession -BaseUrl $baseUrl -Application $Application
        $session = $created.sessionId
        $library = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'FB2 library did not become ready.' -Script @'
return {
  ready: document.readyState,
  href: location.href,
  cards: document.querySelectorAll('.library-book-open').length,
  fb2: [...document.querySelectorAll('.library-book')].filter((card) =>
    card.querySelector('.library-book-title')?.textContent === 'Atha FB2 Gate' &&
    card.querySelector('.library-book-author')?.textContent === 'Ada Lin'
  ).length
};
'@ -Accepted { param($value) $value.ready -eq 'complete' -and $value.cards -eq $expectedBooks -and $value.fb2 -eq 1 }
        if ($library.href -ne 'tauri://localhost') {
            throw 'The Linux library did not expose the prepared FB2 book.'
        }

        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
const card = [...document.querySelectorAll('.library-book')].find((item) =>
  item.querySelector('.library-book-title')?.textContent === 'Atha FB2 Gate'
);
card.querySelector('.library-book-open').click();
return true;
'@)
        $reader = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'FB2 reader did not become ready.' -Script @'
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  href: location.href,
  toc: document.querySelectorAll('#toc option').length
};
'@ -Accepted { param($value) $value.status -eq 'pass' }
        if (-not $reader.href.StartsWith('tauri://localhost/index.html?', [StringComparison]::Ordinal) -or $reader.toc -ne 3) {
            throw 'The Linux reader did not load the expected FB2 manifest.'
        }

        [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Post -Path "/session/$session/window/rect" -Body @{ width = 600; height = 760 })
        $gestureBenchmark = Invoke-ReaderGestureGate `
            -BaseUrl $baseUrl `
            -Session $session `
            -WarmupSamples $GestureWarmups `
            -MeasureSamples $GestureMeasurements

        [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Post -Path "/session/$session/window/rect" -Body @{ width = 600; height = 760 })
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
document.documentElement.setAttribute('data-reader-tools', '');
const preferences = document.querySelector('.reader-tool.preferences');
preferences.open = true;
return true;
'@)
        Start-Sleep -Milliseconds 100
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
document.querySelector('[data-settings-target="font"]').click();
const slider = document.querySelector('#font-size');
const started = performance.now();
for (let index = 0; index < 48; index += 1) {
  slider.value = String(16 + (index % 25));
  slider.dispatchEvent(new Event('input', { bubbles: true }));
}
slider.value = '36';
slider.dispatchEvent(new Event('input', { bubbles: true }));
window.__athaFontInputBurstMs = performance.now() - started;
return true;
'@)
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Typography slider did not provide live feedback.' -Script @'
const panel = document.querySelector('.preferences-panel').getBoundingClientRect();
const slider = document.querySelector('#font-size');
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  mobile: matchMedia('(max-width: 640px)').matches,
  contained: panel.left >= 0 && panel.right <= innerWidth && panel.top >= 0 && panel.bottom <= innerHeight,
  backdrop: getComputedStyle(document.querySelector('.preferences-backdrop')).display !== 'none',
  type: slider.type,
  min: slider.min,
  max: slider.max,
  output: document.querySelector('#font-size-value').textContent,
  pixels: document.querySelector('.reader').dataset.fontPixels,
  burstMs: window.__athaFontInputBurstMs
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.mobile -and $value.contained -and $value.backdrop -and $value.type -eq 'range' -and $value.min -eq '16' -and $value.max -eq '40' -and $value.output -eq '36' -and $value.pixels -eq '36' -and $value.burstMs -lt 20 })
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
const slider = document.querySelector('#font-size');
slider.dispatchEvent(new Event('change', { bubbles: true }));
return true;
'@)
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Typography slider did not persist its committed value.' -Script @'
const key = Object.keys(localStorage).find((value) => value.startsWith('atha.reader.application.'));
const saved = key ? JSON.parse(localStorage.getItem(key)) : null;
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  fontSize: saved?.preferences?.fontSize
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.fontSize -eq 36 })
        $settingsShot = Invoke-WebDriver -BaseUrl $baseUrl -Method Get -Path "/session/$session/screenshot" -Body $null
        [IO.File]::WriteAllBytes($settingsMobileScreenshot, [Convert]::FromBase64String($settingsShot.value))
        $settingsMobileColors = [int](& identify -format '%k' $settingsMobileScreenshot)
        if ($LASTEXITCODE -ne 0 -or $settingsMobileColors -lt 10) { throw 'The mobile settings screenshot is blank.' }

        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
const slider = document.querySelector('#font-size');
slider.value = '40';
slider.dispatchEvent(new Event('input', { bubbles: true }));
slider.dispatchEvent(new Event('change', { bubbles: true }));
document.querySelector('[data-settings-back]').click();
document.querySelector('[data-settings-target="behavior"]').click();
document.querySelector('[data-preference-for="reading-mode"][data-preference-value="scroll"]').click();
return true;
'@)
        [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Post -Path "/session/$session/window/rect" -Body @{ width = 600; height = 420 })
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Scrolled reading mode did not settle.' -Script @'
const key = Object.keys(localStorage).find((value) => value.startsWith('atha.reader.book.'));
const saved = key ? JSON.parse(localStorage.getItem(key)) : null;
const reader = document.querySelector('.reader');
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  mode: reader.dataset.readingMode,
  saved: saved?.preferences?.readingMode,
  columns: reader.dataset.pageColumns,
  scrollable: reader.dataset.scrollable
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.mode -eq 'scroll' -and $value.saved -eq 'scroll' -and $value.columns -eq '1' })
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
document.querySelector('[data-preference-for="reading-mode"][data-preference-value="paged"]').click();
const slider = document.querySelector('#font-size');
slider.value = '19';
slider.dispatchEvent(new Event('input', { bubbles: true }));
slider.dispatchEvent(new Event('change', { bubbles: true }));
return true;
'@)
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Paged reading mode did not restore.' -Script @'
const reader = document.querySelector('.reader');
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  mode: reader.dataset.readingMode,
  columns: reader.dataset.pageColumns
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.mode -eq 'paged' -and $value.columns -eq 'paged' })
        $settingsKeyboard = Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
const owner = document.querySelector('.reader-tool.preferences');
const panel = owner.querySelector('.preferences-panel');
const summary = owner.querySelector(':scope > summary');
panel.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
return {
  label: panel.getAttribute('aria-label'),
  open: owner.open,
  summaryFocused: document.activeElement === summary
};
'@
        if ($settingsKeyboard.label -ne '阅读设置' -or $settingsKeyboard.open -or -not $settingsKeyboard.summaryFocused) {
            throw 'The settings dialog did not expose a stable name and Escape focus return.'
        }
        [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Post -Path "/session/$session/window/rect" -Body @{ width = 600; height = 760 })
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
document.querySelector('.reader-tool.preferences').open = false;
const reader = document.querySelector('.reader');
const rect = reader.getBoundingClientRect();
reader.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, pointerId: 91, pointerType: 'touch', isPrimary: true, button: 0, clientX: rect.right - 80, clientY: rect.top + 120 }));
window.dispatchEvent(new PointerEvent('pointermove', { bubbles: true, pointerId: 91, pointerType: 'touch', isPrimary: true, button: 0, clientX: rect.right - 180, clientY: rect.top + 124 }));
return true;
'@)
        $dragging = Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script "return document.querySelector('.reader').dataset.swipeDragging === 'true';"
        if (-not $dragging) { throw 'Paged reading did not follow the horizontal pointer move.' }
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script "window.dispatchEvent(new PointerEvent('pointercancel', { bubbles: true, pointerId: 91, pointerType: 'touch', isPrimary: true })); return true;")
        [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Post -Path "/session/$session/window/rect" -Body @{ width = 1200; height = 900 })

        if (-not [string]::IsNullOrWhiteSpace($DictionaryQuery)) {
            $queryLiteral = ConvertTo-Json $DictionaryQuery -Compress
            [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Offline dictionary list did not become ready.' -Script @'
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  dictionaries: document.querySelectorAll('.dictionary-source option').length,
  message: document.querySelector('.dictionary-status')?.textContent || ''
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.dictionaries -eq 1 })
            [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @"
globalThis.dispatchEvent(new CustomEvent('atha:dictionary-lookup', { detail: { query: $queryLiteral } }));
return true;
"@)
            $dictionary = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Offline dictionary lookup did not finish.' -Script @'
const panel = document.querySelector('.dictionary-panel');
const rect = panel?.getBoundingClientRect();
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  dictionaries: document.querySelectorAll('.dictionary-source option').length,
  title: document.querySelector('.dictionary-source option:checked')?.textContent || '',
  headword: document.querySelector('.dictionary-result h3')?.textContent || '',
  definition: document.querySelector('.dictionary-result p')?.textContent || '',
  message: document.querySelector('.dictionary-status')?.textContent || '',
  tools: document.documentElement.hasAttribute('data-reader-tools'),
  open: document.querySelector('.reader-tool.dictionary').open,
  contained: Boolean(rect && rect.left >= 0 && rect.right <= innerWidth && rect.top >= 0 && rect.bottom <= innerHeight)
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.dictionaries -eq 1 -and $value.headword -and $value.definition -and $value.tools -and $value.open }
            if (-not $dictionary.contained -or $dictionary.message) {
                throw 'Offline dictionary panel overflowed or reported an error.'
            }
            [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script "document.querySelector('.reader-tool.dictionary > summary').click(); return true;")
        }

        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
const url = new URL(location.href);
url.searchParams.set('statistics-probe', '1');
location.href = url.href;
return true;
'@)
        $statisticsBaseline = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Reading statistics did not become ready.' -Script @'
const diagnostics = globalThis.__athaReaderDiagnostics;
const snapshot = diagnostics?.snapshot().readingStatistics;
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  ready: Boolean(diagnostics && snapshot),
  bookMs: snapshot?.bookMs || 0,
  benchmark: diagnostics?.readingStatisticsBenchmark() || null
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.ready -and $value.benchmark.samples -eq 20 }
        if ($statisticsBaseline.benchmark.p95Ms -ge 5) {
            throw "Reading statistics heartbeat benchmark exceeded 5 ms: $($statisticsBaseline.benchmark.p95Ms)."
        }
        Start-Sleep -Seconds 16
        $statisticsActive = Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script 'return globalThis.__athaReaderDiagnostics.snapshot().readingStatistics;'
        $activeDelta = [double]$statisticsActive.bookMs - [double]$statisticsBaseline.bookMs
        if ($activeDelta -lt 10000 -or $activeDelta -gt 25000) {
            throw "Foreground reading statistics advanced by an unexpected $activeDelta ms."
        }

        [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Post -Path "/session/$session/window/minimize" -Body @{})
        Start-Sleep -Seconds 16
        [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Post -Path "/session/$session/window/maximize" -Body @{})
        Start-Sleep -Milliseconds 500
        $statisticsFocused = Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script 'return globalThis.__athaReaderDiagnostics.snapshot().readingStatistics;'
        $backgroundDelta = [double]$statisticsFocused.bookMs - [double]$statisticsActive.bookMs
        if ($backgroundDelta -lt 0 -or $backgroundDelta -gt 3000) {
            throw "Background reading statistics advanced by $backgroundDelta ms."
        }

        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
document.querySelector('.reader-tool.directory > summary').click();
document.querySelector('#directory-list [data-value="2"]').click();
return true;
'@)
        $navigation = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'FB2 directory navigation did not reach the final notes section.' -Script @'
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  toc: document.querySelector('#toc').value,
  lastSection: document.querySelector('#progress-position').textContent.includes('4/4'),
  lastChapter: document.querySelector('#progress-chapter').textContent === '\u6ce8\u91ca'
};
'@ -Accepted { param($value) $value.toc -eq '2' -and $value.lastSection -and $value.lastChapter }

        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
document.querySelector('.reader-tool.search > summary').click();
const query = document.querySelector('#search-query');
query.value = '\u6ce8\u91ca\u6b63\u6587';
query.dispatchEvent(new Event('input', { bubbles: true }));
document.querySelector('#search-form').requestSubmit();
return true;
'@)
        $search = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'FB2 full-book search did not finish.' -Script @'
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  results: document.querySelector('#search-results').options.length
};
'@ -Accepted { param($value) $value.results -eq 1 }

        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
document.documentElement.setAttribute('data-reader-tools', '');
document.querySelector('.reader-tool.preferences > summary').click();
document.querySelector('[data-settings-target="layout"]').click();
for (const [selector, value] of [
  ['#paragraph-indent', 'two'],
  ['#paragraph-spacing', 'comfortable'],
  ['#page-margin', 'wide']
]) {
  const control = document.querySelector(selector);
  control.value = value;
  control.dispatchEvent(new Event('change', { bubbles: true }));
}
return true;
'@)
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Visual CSS controls did not reflow.' -Script @'
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  margin: document.querySelector('.reader').style.getPropertyValue('--page-left-margin'),
  saved: [...Object.keys(localStorage)].some((key) => key.startsWith('atha.reader.book.') && localStorage.getItem(key).includes('"pageMargin":"wide"'))
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.margin -eq '48px' -and $value.saved })

        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
document.querySelector('[data-settings-back]').click();
document.querySelector('[data-settings-target="modules"]').click();
return true;
'@)
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Empty CSS module editor was not read-only.' -Script @'
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  editor: Boolean(document.querySelector('.cm-editor')),
  editable: document.querySelector('.cm-content')?.contentEditable || null
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.editor -and $value.editable -eq 'false' })
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script "document.querySelector('#add-style-module').click(); return true;")
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'CSS module editor did not create a module.' -Script @'
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  modules: document.querySelector('#style-module-list').options.length,
  editor: Boolean(document.querySelector('.cm-editor')),
  editable: document.querySelector('.cm-content')?.contentEditable || null
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.modules -eq 1 -and $value.editor -and $value.editable -eq 'true' })

        Send-WebDriverText -BaseUrl $baseUrl -Session $session -Selector '.cm-content' -Text '@#$'
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'CodeMirror lint gutter did not report invalid CSS.' -Script @'
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  css: document.querySelector('#user-stylesheet').value,
  lint: document.querySelectorAll('.cm-lint-marker-error, .cm-lintRange-error').length,
  gutter: Boolean(document.querySelector('.cm-gutter-lint'))
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.css -eq '@#$' -and $value.lint -gt 0 -and $value.gutter })
        Send-WebDriverText -BaseUrl $baseUrl -Session $session -Selector '.cm-content' -Text '.book { --atha-css-gate: applied; }'
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
document.querySelector('#style-module-name').value = 'Gate emphasis';
document.querySelector('#style-module-group').value = 'Layout';
document.querySelector('#save-style-module').click();
return true;
'@)
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Valid CSS module did not persist.' -Script @'
const record = Object.keys(localStorage).find((key) => key.startsWith('atha.reader.book.'));
const saved = record ? JSON.parse(localStorage.getItem(record)) : null;
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  message: document.querySelector('#preferences-status').textContent,
  css: saved?.preferences?.styleModules?.[0]?.css || '',
  name: saved?.preferences?.styleModules?.[0]?.name || ''
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.css -eq '.book { --atha-css-gate: applied; }' -and $value.name -eq 'Gate emphasis' })

        Send-WebDriverText -BaseUrl $baseUrl -Session $session -Selector '.cm-content' -Text '.book { --atha-css-gate: draft; }'
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
const enabled = document.querySelector('#style-module-enabled');
enabled.checked = false;
enabled.dispatchEvent(new Event('change', { bubbles: true }));
return true;
'@)
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Debounced CSS draft overwrote newer module metadata.' -Script @'
const record = Object.keys(localStorage).find((key) => key.startsWith('atha.reader.book.'));
const module = record ? JSON.parse(localStorage.getItem(record))?.preferences?.styleModules?.[0] : null;
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  css: module?.css || '',
  enabled: module?.enabled
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.css -eq '.book { --atha-css-gate: draft; }' -and $value.enabled -eq $false })
        Send-WebDriverText -BaseUrl $baseUrl -Session $session -Selector '.cm-content' -Text '.book { --atha-css-gate: applied; }'
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
const enabled = document.querySelector('#style-module-enabled');
enabled.checked = true;
enabled.dispatchEvent(new Event('change', { bubbles: true }));
return true;
'@)
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'CSS module did not restore after the debounce probe.' -Script @'
const record = Object.keys(localStorage).find((key) => key.startsWith('atha.reader.book.'));
const module = record ? JSON.parse(localStorage.getItem(record))?.preferences?.styleModules?.[0] : null;
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  css: module?.css || '',
  enabled: module?.enabled
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.css -eq '.book { --atha-css-gate: applied; }' -and $value.enabled })

        Send-WebDriverText -BaseUrl $baseUrl -Session $session -Selector '.cm-content' -Text '.book { --atha-css-gate: applied; font-size: 18px; }'
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script "document.querySelector('#save-style-module').click(); return true;")
        $renderedStyle = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Rendered CSS rollback probe did not persist.' -Script @'
const record = Object.keys(localStorage).find((key) => key.startsWith('atha.reader.book.'));
const module = record ? JSON.parse(localStorage.getItem(record))?.preferences?.styleModules?.[0] : null;
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  css: module?.css || '',
  position: document.querySelector('#position').textContent
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.css -eq '.book { --atha-css-gate: applied; font-size: 18px; }' }

        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
const css = document.querySelector('#user-stylesheet');
css.value = '.book { background: url(https://example.invalid/x); }';
document.querySelector('#save-style-module').click();
return true;
'@)
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Unsafe CSS module did not roll back.' -Script @'
const record = Object.keys(localStorage).find((key) => key.startsWith('atha.reader.book.'));
const saved = record ? JSON.parse(localStorage.getItem(record)) : null;
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  rejected: document.querySelector('#preferences-status').dataset.error === 'true',
  message: document.querySelector('#preferences-status').textContent,
  css: saved?.preferences?.styleModules?.[0]?.css || ''
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.rejected -and $value.message -like '*Gate emphasis*' -and $value.css -eq '.book { --atha-css-gate: applied; font-size: 18px; }' })

        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
globalThis.__athaSetItem = Object.getOwnPropertyDescriptor(Storage.prototype, 'setItem');
Object.defineProperty(Storage.prototype, 'setItem', {
  configurable: true,
  value() { throw new DOMException('Gate quota', 'QuotaExceededError'); }
});
const css = document.querySelector('#user-stylesheet');
document.querySelector('#preferences-status').textContent = '';
document.querySelector('#preferences-status').dataset.error = 'false';
css.value = '.book { --atha-css-gate: unsaved; font-size: 56px; }';
document.querySelector('#save-style-module').click();
return true;
'@)
        $rollback = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Failed persistence did not restore the previous CSS.' -Script @'
const record = Object.keys(localStorage).find((key) => key.startsWith('atha.reader.book.'));
const saved = record ? JSON.parse(localStorage.getItem(record)) : null;
const result = {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  rejected: document.querySelector('#preferences-status').dataset.error === 'true',
  message: document.querySelector('#preferences-status').textContent,
  savedCss: saved?.preferences?.styleModules?.[0]?.css || '',
  editorCss: document.querySelector('#user-stylesheet').value,
  position: document.querySelector('#position').textContent
};
if (result.rejected && globalThis.__athaSetItem) {
  Object.defineProperty(Storage.prototype, 'setItem', globalThis.__athaSetItem);
  delete globalThis.__athaSetItem;
}
return result;
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.rejected -and $value.message -like '*无法保存*' }
        if (
            $rollback.savedCss -ne '.book { --atha-css-gate: applied; font-size: 18px; }' -or
            $rollback.editorCss -ne '.book { --atha-css-gate: applied; font-size: 18px; }' -or
            $rollback.position -ne $renderedStyle.position
        ) {
            throw "Failed persistence restored stored='$($rollback.savedCss)' editor='$($rollback.editorCss)' position='$($rollback.position)'."
        }

        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
document.querySelector('#open-style-module-transfer').click();
const transfer = document.querySelector('#style-module-transfer');
transfer.value = JSON.stringify({ schema: 1, modules: [
  { id: 'gate-one', name: 'Gate one', group: 'Layout', enabled: true, css: '.book { --atha-gate-one: 1; }' },
  { id: 'gate-two', name: 'Gate two', group: 'Typography', enabled: true, css: '.book p { letter-spacing: .02em; }' }
] });
document.querySelector('#import-style-modules').click();
return true;
'@)
        $styles = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'CSS module import did not settle.' -Script @'
const record = Object.keys(localStorage).find((key) => key.startsWith('atha.reader.book.'));
const saved = record ? JSON.parse(localStorage.getItem(record)) : null;
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  modules: saved?.preferences?.styleModules?.length || 0,
  margin: saved?.preferences?.pageMargin || '',
  editorLoaded: Boolean(document.querySelector('.cm-editor'))
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.modules -eq 2 -and $value.margin -eq 'wide' -and $value.editorLoaded }
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
document.querySelector('#style-module-transfer-dialog').close();
const preferences = document.querySelector('.reader-tool.preferences');
preferences.open = true;
document.querySelector('[data-settings-target="modules"]').click();
const search = document.querySelector('#style-module-search');
search.value = 'two';
search.dispatchEvent(new Event('input', { bubbles: true }));
return true;
'@)
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'CSS module search did not filter.' -Script "return { status: document.documentElement.dataset.status || null, error: document.documentElement.dataset.error || null, modules: document.querySelector('#style-module-list').options.length };" -Accepted { param($value) $value.status -eq 'pass' -and $value.modules -eq 1 })
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'CSS module panel was not visible for screenshot evidence.' -Script @'
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  open: document.querySelector('.reader-tool.preferences').open,
  visible: !document.querySelector('[data-settings-page="modules"]').hidden,
  width: document.querySelector('.preferences-panel').getBoundingClientRect().width,
  searchWidth: document.querySelector('#style-module-search').getBoundingClientRect().width,
  filterWidth: document.querySelector('#style-module-filter').getBoundingClientRect().width,
  moduleRows: document.querySelectorAll('#style-module-list-view [data-module-id]').length,
  toggleWidth: document.querySelector('.module-enabled-row .switch-track').getBoundingClientRect().width,
  themeChoices: document.querySelectorAll('[data-preference-for="theme"]').length,
  layoutChoices: document.querySelectorAll('[data-preference-for="page-margin"]').length
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.open -and $value.visible -and $value.width -gt 300 -and $value.searchWidth -gt 200 -and $value.filterWidth -gt 200 -and $value.moduleRows -eq 1 -and $value.toggleWidth -ge 40 -and $value.themeChoices -eq 4 -and $value.layoutChoices -eq 3 })

        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script "document.activeElement?.blur(); return true;")
        Start-Sleep -Milliseconds 800
        $shot = Invoke-WebDriver -BaseUrl $baseUrl -Method Get -Path "/session/$session/screenshot" -Body $null
        [IO.File]::WriteAllBytes($screenshot, [Convert]::FromBase64String($shot.value))
        $colors = [int](& identify -format '%k' $screenshot)
        if ($LASTEXITCODE -ne 0 -or $colors -lt 10) { throw 'The Linux reader screenshot is blank.' }

        [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Post -Path "/session/$session/window/rect" -Body @{ width = 600; height = 760 })
        $mobileLayout = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'CSS module panel did not enter the mobile viewport.' -Script @'
const panel = document.querySelector('.preferences-panel');
const rect = panel.getBoundingClientRect();
const settings = document.querySelector('.module-settings');
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  mobile: matchMedia('(max-width: 640px)').matches,
  left: rect.left,
  right: innerWidth - rect.right,
  overflow: settings.scrollWidth - settings.clientWidth,
  columns: getComputedStyle(settings).gridTemplateColumns.split(' ').length
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.mobile }
        if ([Math]::Abs($mobileLayout.left) -ge 1 -or [Math]::Abs($mobileLayout.right) -ge 1 -or $mobileLayout.overflow -gt 1 -or $mobileLayout.columns -ne 1) {
            throw "CSS module mobile layout failed: $($mobileLayout | ConvertTo-Json -Compress)."
        }
        $mobileShot = Invoke-WebDriver -BaseUrl $baseUrl -Method Get -Path "/session/$session/screenshot" -Body $null
        [IO.File]::WriteAllBytes($mobileScreenshot, [Convert]::FromBase64String($mobileShot.value))
        $mobileColors = [int](& identify -format '%k' $mobileScreenshot)
        if ($LASTEXITCODE -ne 0 -or $mobileColors -lt 10) { throw 'The mobile Linux reader screenshot is blank.' }

        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
const url = new URL(location.href);
url.searchParams.set('style-module-probe', '1');
location.href = url.href;
return true;
'@)
        $styleBenchmark = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'CSS module benchmark did not pass.' -Script @'
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  p95: Number(document.documentElement.dataset.styleModuleP95 || 0),
  bytes: Number(document.documentElement.dataset.styleModuleBytes || 0)
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.p95 -gt 0 -and $value.p95 -lt 50 -and $value.bytes -le 65536 }

        [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Post -Path "/session/$session/window/rect" -Body @{ width = 1200; height = 900 })
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
const progress = document.querySelector('#progress-range');
globalThis.__athaStatisticsScreenshotProgress = progress.value;
progress.value = '0';
progress.dispatchEvent(new Event('change', { bubbles: true }));
return true;
'@)
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Reading statistics desktop context did not settle.' -Script @'
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  desktop: matchMedia('(min-width: 601px)').matches,
  first: document.querySelector('#progress-position').textContent.includes('1/4')
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.desktop -and $value.first })
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
document.documentElement.setAttribute('data-reader-tools', '');
document.querySelector('.reader-tool.progress > summary').click();
return true;
'@)
        $statisticsLayout = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Reading statistics panel did not become visible.' -Script @'
const panel = document.querySelector('.progress-panel');
const metrics = document.querySelector('.reading-statistics');
const rect = panel.getBoundingClientRect();
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  open: document.querySelector('.reader-tool.progress').open,
  columns: getComputedStyle(metrics).gridTemplateColumns.split(' ').length,
  overflow: metrics.scrollWidth - metrics.clientWidth,
  inside: rect.left >= 0 && rect.right <= innerWidth && rect.top >= 0 && rect.bottom <= innerHeight,
  values: [...metrics.querySelectorAll('dd')].map((value) => value.textContent)
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.open -and $value.columns -eq 4 -and $value.overflow -le 1 -and $value.inside -and $value.values.Count -eq 4 }
        Start-Sleep -Milliseconds 200
        $statisticsShot = Invoke-WebDriver -BaseUrl $baseUrl -Method Get -Path "/session/$session/screenshot" -Body $null
        [IO.File]::WriteAllBytes($statisticsScreenshot, [Convert]::FromBase64String($statisticsShot.value))
        $statisticsColors = [int](& identify -format '%k' $statisticsScreenshot)
        if ($LASTEXITCODE -ne 0 -or $statisticsColors -lt 10) { throw 'The reading statistics screenshot is blank.' }

        [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Post -Path "/session/$session/window/rect" -Body @{ width = 600; height = 760 })
        $statisticsMobileLayout = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Reading statistics did not enter the mobile layout.' -Script @'
const panel = document.querySelector('.progress-panel');
const metrics = document.querySelector('.reading-statistics');
const rect = panel.getBoundingClientRect();
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  mobile: matchMedia('(max-width: 600px)').matches,
  columns: getComputedStyle(metrics).gridTemplateColumns.split(' ').length,
  overflow: Math.max(panel.scrollWidth - panel.clientWidth, metrics.scrollWidth - metrics.clientWidth),
  inside: rect.left >= 0 && rect.right <= innerWidth && rect.top >= 0 && rect.bottom <= innerHeight
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.mobile -and $value.columns -eq 2 -and $value.overflow -le 1 -and $value.inside }
        $statisticsMobileShot = Invoke-WebDriver -BaseUrl $baseUrl -Method Get -Path "/session/$session/screenshot" -Body $null
        [IO.File]::WriteAllBytes($statisticsMobileScreenshot, [Convert]::FromBase64String($statisticsMobileShot.value))
        $statisticsMobileColors = [int](& identify -format '%k' $statisticsMobileScreenshot)
        if ($LASTEXITCODE -ne 0 -or $statisticsMobileColors -lt 10) { throw 'The mobile reading statistics screenshot is blank.' }
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
const progress = document.querySelector('#progress-range');
progress.value = globalThis.__athaStatisticsScreenshotProgress;
progress.dispatchEvent(new Event('change', { bubbles: true }));
delete globalThis.__athaStatisticsScreenshotProgress;
return true;
'@)
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'FB2 reading position did not return after statistics screenshots.' -Script @'
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  restored: document.querySelector('#progress-position').textContent.includes('4/4')
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.restored })
        $artifactRoot = Join-Path $repoRoot 'artifacts/local/screenshots'
        [void](New-Item -ItemType Directory -Path $artifactRoot -Force)
        Copy-Item -LiteralPath $statisticsScreenshot -Destination (Join-Path $artifactRoot 'atha-reading-statistics-linux.png') -Force
        Copy-Item -LiteralPath $statisticsMobileScreenshot -Destination (Join-Path $artifactRoot 'atha-reading-statistics-linux-mobile.png') -Force
        Copy-Item -LiteralPath $settingsMobileScreenshot -Destination (Join-Path $artifactRoot 'atha-reader-settings-linux-mobile.png') -Force

        [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Delete -Path "/session/$session" -Body $null)
        $session = $null
        $created = New-TauriSession -BaseUrl $baseUrl -Application $Application
        $session = $created.sessionId
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Restarted FB2 library did not become ready.' -Script "return { ready: document.readyState, cards: document.querySelectorAll('.library-book-open').length };" -Accepted { param($value) $value.ready -eq 'complete' -and $value.cards -eq $expectedBooks })
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
const card = [...document.querySelectorAll('.library-book')].find((item) =>
  item.querySelector('.library-book-title')?.textContent === 'Atha FB2 Gate'
);
card.querySelector('.library-book-open').click();
return true;
'@)
        $restored = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'FB2 reading position was not restored.' -Script @'
if (document.documentElement.dataset.status !== 'pass') {
  return { status: document.documentElement.dataset.status || null, error: document.documentElement.dataset.error || null };
}
return {
  status: 'pass',
  restored: document.querySelector('#progress-position').textContent.includes('4/4') &&
    document.querySelector('#progress-chapter').textContent === '\u6ce8\u91ca' &&
    document.querySelector('.reader').style.getPropertyValue('--page-left-margin') === '48px',
  modules: (() => {
    const record = Object.keys(localStorage).find((key) => key.startsWith('atha.reader.book.'));
    return record ? JSON.parse(localStorage.getItem(record)).preferences.styleModules.length : 0;
  })(),
  statistics: (() => {
    const value = JSON.parse(localStorage.getItem('atha.reader.statistics.v1') || 'null');
    return {
      stored: value?.books?.some((book) => book.durationMs > 0) || false,
      visible: document.querySelector('#statistics-book').textContent !== '0 \u5206\u949f'
    };
  })()
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.restored -and $value.modules -eq 2 -and $value.statistics.stored -and $value.statistics.visible }

        if (-not [string]::IsNullOrWhiteSpace($FormulaEntry)) {
            [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script "location.assign('tauri://localhost'); return true;")
            [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Formula benchmark library did not become ready.' -Script "return { ready: document.readyState, cards: document.querySelectorAll('.library-book-open').length };" -Accepted { param($value) $value.ready -eq 'complete' -and $value.cards -eq $expectedBooks })
            [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
const card = [...document.querySelectorAll('.library-book')].find((item) =>
  item.querySelector('.library-book-title')?.textContent !== 'Atha FB2 Gate'
);
card.querySelector('.library-book-open').click();
return true;
'@)
            [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Formula benchmark reader did not become ready.' -Script @'
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null
};
'@ -Accepted { param($value) $value.status -eq 'pass' })
            Enable-ReaderGestureDiagnostics -BaseUrl $baseUrl -Session $session
            $formulaSectionScript = @'
const done = arguments[arguments.length - 1];
globalThis.__athaReaderDiagnostics
  .openGestureSection(arguments[0])
  .then((value) => done({ ok: true, value }))
  .catch((error) => done({ ok: false, error: error instanceof Error ? error.message : String(error) }));
'@
            $openedFormula = Invoke-WebDriverAsyncScript `
                -BaseUrl $baseUrl `
                -Session $session `
                -Script $formulaSectionScript `
                -ScriptArguments @($FormulaEntry)
            if (-not $openedFormula.ok -or
                $openedFormula.value.formulas -lt $FormulaMinimumFormulas -or
                $openedFormula.value.pages -lt $FormulaMinimumPages) {
                throw 'Formula benchmark section did not expose formulas.'
            }
            $formulaShape = $openedFormula.value
            $formulaBenchmark = Invoke-ReaderGestureGate `
                -BaseUrl $baseUrl `
                -Session $session `
                -WarmupSamples $GestureWarmups `
                -MeasureSamples $GestureMeasurements
        }

        [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Delete -Path "/session/$session" -Body $null)
        $session = $null
        Invoke-Checked 'systemctl' @('--user', 'stop', "$unit.service") 'Could not stop the FB2 Linux GUI gate.'
        $unitStarted = $false

        $privatePattern = 'Atha FB2 Gate|Ada Lin|第一章|正文|重点|注释正文|fb2-gate\.fb2|5cec82bcb5514780'
        $privateTokens = @($DictionaryPrivateTokens) + @($BookPrivateTokens) + @(
            $DictionaryQuery,
            $dictionary.title,
            $dictionary.headword,
            $dictionary.definition
        ) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
        $logs = Join-Path $DataRoot 'com.atha.reader/logs'
        foreach ($log in @(Get-ChildItem -LiteralPath $logs -File -ErrorAction SilentlyContinue)) {
            $logText = Get-Content -LiteralPath $log.FullName -Raw
            if ($logText -match $privatePattern -or
                ($privateTokens | Where-Object { $logText.Contains($_, [StringComparison]::Ordinal) })) {
                throw 'The Linux AppLog contains private fixture data.'
            }
        }

        [pscustomobject]@{
            webview = "WebKitGTK $($created.capabilities.browserVersion)"
            sections = 4
            toc_items = $reader.toc
            search_results = $search.results
            css_modules = $styles.modules
            css_editor = 'CodeMirror 6'
            css_modules_p95_ms = $styleBenchmark.p95
            css_modules_bytes = $styleBenchmark.bytes
            reading_statistics_p95_ms = $statisticsBaseline.benchmark.p95Ms
            gesture_requested_pointer_type = $gestureBenchmark.requested_pointer_type
            gesture_trusted_pointer_events = $gestureBenchmark.trusted_pointer_events
            gesture_native_touch_observed = $gestureBenchmark.native_touch_observed
            gesture_observed_pointer_types = $gestureBenchmark.observed_pointer_types
            gesture_scenarios = $gestureBenchmark.scenarios
            gesture_input_p95_ms = $gestureBenchmark.input_to_first_visual_p95_ms
            gesture_tap_p95_ms = $gestureBenchmark.tap_to_first_visual_p95_ms
            gesture_frame_p95_ms = $gestureBenchmark.drag_frame_p95_ms
            gesture_maximum_frame_ms = $gestureBenchmark.maximum_frame_ms
            gesture_settle_p95_ms = $gestureBenchmark.release_to_stable_p95_ms
            formula_section = $formulaShape.section
            formula_count = $formulaShape.formulas
            formula_pages = $formulaShape.pages
            formula_gesture_input_p95_ms = $formulaBenchmark.input_to_first_visual_p95_ms
            formula_gesture_tap_p95_ms = $formulaBenchmark.tap_to_first_visual_p95_ms
            formula_gesture_frame_p95_ms = $formulaBenchmark.drag_frame_p95_ms
            formula_gesture_maximum_frame_ms = $formulaBenchmark.maximum_frame_ms
            formula_gesture_settle_p95_ms = $formulaBenchmark.release_to_stable_p95_ms
            foreground_duration_ms = $activeDelta
            background_duration_ms = $backgroundDelta
            restored_section = 4
            screenshot_colors = $colors
            mobile_screenshot_colors = $mobileColors
            statistics_screenshot_colors = $statisticsColors
            statistics_mobile_screenshot_colors = $statisticsMobileColors
            settings_mobile_screenshot_colors = $settingsMobileColors
            evidence = 'Linux Tauri GUI'
        }
    }
    finally {
        if ($session) {
            try { [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Delete -Path "/session/$session" -Body $null) }
            catch { }
        }
        if ($unitStarted) { & systemctl --user stop "$unit.service" 2>$null | Out-Null }
    }
}

$temporaryRoot = [IO.Path]::GetFullPath((Join-Path $repoRoot '.tmp')).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
foreach ($path in @($fixturePath, $importsRoot, $guiRoot)) {
    if (-not [IO.Path]::GetFullPath($path).StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Refusing to use an FB2 gate path outside the repository .tmp directory.'
    }
}

Push-Location $repoRoot
try {
    foreach ($path in @($fixturePath, $importsRoot, $guiRoot)) {
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Recurse -Force }
    }
    Invoke-Checked 'node' @('--test', 'reader/web/reader-state.test.mjs') 'Reading statistics tests failed.'
    Invoke-CheckedCargo @('fmt', '--all', '--check') 'FB2 formatting check failed.'
    Invoke-CheckedCargo @('clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings') 'FB2 clippy check failed.'
    Invoke-CheckedCargo @('test', '--workspace', '--all-targets', '--locked') 'FB2 Rust tests failed.'
    if ($VerifyLinuxGui) {
        Invoke-Checked 'pnpm' @('--dir', 'reader/app', 'check') 'Svelte check failed.'
        Invoke-Checked 'pnpm' @('--dir', 'reader/app', 'build') 'Svelte build failed.'
        Invoke-CheckedCargo @('build', '--locked', '-p', 'atha-reader-app') 'Linux Tauri build failed.'
        [void](New-Item -ItemType Directory -Path (Join-Path $guiRoot 'data') -Force)
        $env:ATHA_FB2_GATE_LIBRARY_ROOT = Join-Path $guiRoot 'data/com.atha.reader'
    }
    try {
        Invoke-CheckedCargo @(
            'test', '--locked', '-p', 'atha-backend', '--test', 'fb2_import',
            'writes_fb2_gate_fixture', '--', '--ignored', '--exact'
        ) 'FB2 fixture generation failed.'
    }
    finally {
        Remove-Item Env:ATHA_FB2_GATE_LIBRARY_ROOT -ErrorAction SilentlyContinue
    }
    if ($formulaSource) {
        $env:ATHA_EPUB_GATE_LIBRARY_ROOT = Join-Path $guiRoot 'data/com.atha.reader'
        $env:ATHA_EPUB_GATE_SOURCE = $formulaSource
        try {
            Invoke-CheckedCargo @(
                'test', '--locked', '-p', 'atha-backend', '--test', 'epub_import',
                'seeds_private_formula_gui_benchmark', '--', '--ignored', '--exact'
            ) 'Formula Linux GUI seed failed.'
        }
        finally {
            Remove-Item Env:ATHA_EPUB_GATE_LIBRARY_ROOT -ErrorAction SilentlyContinue
            Remove-Item Env:ATHA_EPUB_GATE_SOURCE -ErrorAction SilentlyContinue
        }
        $formulaRecords = @(Get-ChildItem -LiteralPath (Join-Path $guiRoot 'data/com.atha.reader/Library') -File |
            ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json } |
            Where-Object title -NE 'Atha FB2 Gate')
        if ($formulaRecords.Count -ne 1) { throw 'Formula GUI seed produced an invalid library record.' }
        $formulaPrivateTokens = @(
            $formulaSource,
            (Split-Path -Leaf $formulaSource),
            $FormulaBenchmarkEntry,
            $formulaRecords[0].id,
            $formulaRecords[0].title
        ) + @($formulaRecords[0].authors)
    }
    $dictionaryQuery = $null
    $dictionaryPrivateTokens = @()
    if (-not [string]::IsNullOrWhiteSpace($DictionaryFixtureRoot)) {
        if (-not $VerifyLinuxGui) { throw 'DictionaryFixtureRoot requires VerifyLinuxGui.' }
        $resolvedDictionaryFixtures = (Resolve-Path -LiteralPath $DictionaryFixtureRoot).Path
        $dictionaryQueryPath = Join-Path $guiRoot 'dictionary-query.json'
        $env:ATHA_PRIVATE_DICTIONARY_ROOT = $resolvedDictionaryFixtures
        $env:ATHA_DICTIONARY_GATE_DATA_ROOT = Join-Path $guiRoot 'data/com.atha.reader'
        $env:ATHA_DICTIONARY_GATE_QUERY_PATH = $dictionaryQueryPath
        try {
            Invoke-CheckedCargo @(
                'test', '--locked', '-p', 'atha-backend',
                'reader::dictionary::tests::seed_private_dictionary_gui_gate', '--lib', '--', '--ignored', '--exact'
            ) 'Dictionary Linux GUI seed failed.'
        }
        finally {
            Remove-Item Env:ATHA_PRIVATE_DICTIONARY_ROOT -ErrorAction SilentlyContinue
            Remove-Item Env:ATHA_DICTIONARY_GATE_DATA_ROOT -ErrorAction SilentlyContinue
            Remove-Item Env:ATHA_DICTIONARY_GATE_QUERY_PATH -ErrorAction SilentlyContinue
        }
        $dictionaryQuery = Get-Content -LiteralPath $dictionaryQueryPath -Raw | ConvertFrom-Json
        if ($dictionaryQuery -isnot [string] -or $dictionaryQuery.Length -eq 0 -or $dictionaryQuery.Length -gt 128) {
            throw 'Dictionary GUI seed produced an invalid private query.'
        }
        $dictionaryPrivateTokens += $resolvedDictionaryFixtures
        $dictionaryPrivateTokens += Get-ChildItem -LiteralPath $resolvedDictionaryFixtures -Recurse -File |
            Where-Object { $_.Extension -in @('.mdx', '.mdd') } |
            ForEach-Object { $_.FullName; $_.Name }
        $dictionaryRecord = Get-ChildItem -LiteralPath (Join-Path $guiRoot 'data/com.atha.reader/Dictionaries') -Directory |
            Select-Object -First 1 |
            ForEach-Object { Get-Content -LiteralPath (Join-Path $_.FullName 'dictionary.json') -Raw | ConvertFrom-Json }
        if (-not $dictionaryRecord) { throw 'Dictionary GUI seed produced no dictionary record.' }
        $dictionaryPrivateTokens += @($dictionaryRecord.id, $dictionaryRecord.title)
    }

    $fixtureSha256 = (Get-FileHash -LiteralPath $fixturePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($fixtureSha256 -ne $expectedFixtureSha256) {
        throw "Unexpected FB2 fixture SHA-256: $fixtureSha256"
    }
    $bookRoots = @(Get-ChildItem -LiteralPath $importsRoot -Directory)
    if ($bookRoots.Count -ne 1) { throw 'FB2 gate must prepare exactly one content root.' }
    $manifest = Get-Content -LiteralPath (Join-Path $bookRoots[0].FullName '.atha-reader.json') -Raw | ConvertFrom-Json
    if ($manifest.schema -ne 1 -or $manifest.contentVersion -ne $bookRoots[0].Name) {
        throw 'Prepared FB2 manifest identity does not match its content root.'
    }
    if ($manifest.sections.Count -ne 4 -or $manifest.resources.Count -ne 1 -or $manifest.toc.Count -ne 3) {
        throw "Unexpected FB2 import shape: sections=$($manifest.sections.Count) resources=$($manifest.resources.Count) toc=$($manifest.toc.Count)."
    }
    $chapter = Get-Content -LiteralPath (Join-Path $bookRoots[0].FullName '.atha-fb2/section-0002.xhtml') -Raw
    foreach ($token in @('正文', 'section-0004.xhtml#note-1', 'images/image-0001.png')) {
        if (-not $chapter.Contains($token, [StringComparison]::Ordinal)) {
            throw 'Prepared FB2 chapter is missing an expected projected token.'
        }
    }

    $sourceEvidence = [pscustomobject]@{
        fixture_sha256 = $fixtureSha256
        content_version = $manifest.contentVersion
        gate_sha256 = (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant()
        sections = $manifest.sections.Count
        resources = $manifest.resources.Count
        toc_items = $manifest.toc.Count
        evidence = 'local bounded importer'
    }
    $sourceEvidence | Format-List
    if ($VerifyLinuxGui) {
        Invoke-LinuxGuiGate `
            -Application (Join-Path $repoRoot 'target/debug/atha-reader-app') `
            -DataRoot (Join-Path $guiRoot 'data') `
            -DictionaryQuery $dictionaryQuery `
            -DictionaryPrivateTokens $dictionaryPrivateTokens `
            -BookPrivateTokens $formulaPrivateTokens `
            -FormulaEntry $FormulaBenchmarkEntry `
            -FormulaMinimumFormulas $FormulaBenchmarkMinimumFormulas `
            -FormulaMinimumPages $FormulaBenchmarkMinimumPages `
            -GestureWarmups $GestureWarmupSamples `
            -GestureMeasurements $GestureMeasureSamples | Format-List
    }
}
finally {
    Pop-Location
    foreach ($path in @($fixturePath, $importsRoot, $guiRoot)) {
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Recurse -Force }
    }
}
