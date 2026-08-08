# Description: Verify private Kindle samples, release performance, and the optional Linux GUI path.

[CmdletBinding()]
param(
    [ValidateRange(1, 100)]
    [int]$BenchmarkRuns = 10,
    [switch]$VerifyLinuxGui
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$temporaryRoot = Join-Path $repoRoot '.tmp'
$guiRoot = Join-Path $temporaryRoot 'kindle-linux-gui'
$benchmarkRoot = Join-Path $temporaryRoot 'kindle-benchmark'
$ordinaryP95LimitMs = 468
$ordinaryRssLimitKiB = 51216
$dictionaryP95LimitMs = 2000
$dictionaryRssLimitKiB = 131072

. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot
$cargoPath = $env:ATHA_CARGO

function Invoke-Checked {
    param([string]$FilePath, [string[]]$Arguments, [string]$Failure)

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
    param([string]$BaseUrl, [string]$Method, [string]$Path, [object]$Body)

    $arguments = @{
        Uri = "$BaseUrl$Path"
        Method = $Method
        ContentType = 'application/json'
        TimeoutSec = 30
    }
    if ($null -ne $Body) { $arguments.Body = $Body | ConvertTo-Json -Depth 8 -Compress }
    Invoke-RestMethod @arguments
}

function Invoke-WebDriverScript {
    param([string]$BaseUrl, [string]$Session, [string]$Script)

    (Invoke-WebDriver -BaseUrl $BaseUrl -Method Post -Path "/session/$Session/execute/sync" -Body @{
        script = $Script
        args = @()
    }).value
}

function Wait-WebDriverScript {
    param(
        [string]$BaseUrl,
        [string]$Session,
        [string]$Script,
        [scriptblock]$Accepted,
        [string]$Failure
    )

    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $value = Invoke-WebDriverScript -BaseUrl $BaseUrl -Session $Session -Script $Script
        if (& $Accepted $value) { return $value }
        if ($value.status -eq 'fail') { throw "$Failure Error: $($value.error)" }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "$Failure Last state: $($value | ConvertTo-Json -Compress -Depth 4)"
}

function New-TauriSession {
    param([string]$BaseUrl, [string]$Application)

    $response = Invoke-WebDriver -BaseUrl $BaseUrl -Method Post -Path '/session' -Body @{
        capabilities = @{ alwaysMatch = @{ 'tauri:options' = @{ application = $Application } } }
    }
    if ([string]::IsNullOrWhiteSpace($response.value.sessionId)) {
        throw 'Tauri WebDriver did not create a session.'
    }
    $response.value
}

function Get-Median {
    param([double[]]$Values)

    $sorted = @($Values | Sort-Object)
    $middle = [int][Math]::Floor($sorted.Count / 2)
    if ($sorted.Count % 2) { return $sorted[$middle] }
    ($sorted[$middle - 1] + $sorted[$middle]) / 2
}

function Invoke-ReleaseBenchmark {
    param(
        [string]$Binary,
        [string]$Test,
        [string]$Label,
        [int]$Runs,
        [double]$P95LimitMs,
        [long]$RssLimitKiB
    )

    if (-not $IsLinux) { throw 'Kindle RSS benchmark requires Linux.' }
    Invoke-Checked $Binary @($Test, '--ignored', '--exact') "$Label benchmark warm-up failed."
    $durations = [Collections.Generic.List[double]]::new()
    $rssValues = [Collections.Generic.List[long]]::new()
    for ($index = 1; $index -le $Runs; $index++) {
        $start = [Diagnostics.ProcessStartInfo]::new($Binary)
        $start.UseShellExecute = $false
        foreach ($argument in @($Test, '--ignored', '--exact')) { $start.ArgumentList.Add($argument) }
        $process = [Diagnostics.Process]::new()
        $process.StartInfo = $start
        $watch = [Diagnostics.Stopwatch]::StartNew()
        if (-not $process.Start()) { throw "$Label benchmark run $index did not start." }
        $peakRss = 0L
        do {
            try {
                $process.Refresh()
                $peakRss = [Math]::Max($peakRss, $process.WorkingSet64)
            }
            catch { }
        } while (-not $process.WaitForExit(1))
        $watch.Stop()
        if ($process.ExitCode -ne 0) { throw "$Label benchmark run $index failed." }
        $durations.Add($watch.Elapsed.TotalMilliseconds)
        $rssValues.Add([long][Math]::Ceiling($peakRss / 1KB))
        $process.Dispose()
    }
    $ordered = @($durations | Sort-Object)
    $p95 = $ordered[[Math]::Ceiling($Runs * 0.95) - 1]
    $rssP95 = (@($rssValues | Sort-Object))[[Math]::Ceiling($Runs * 0.95) - 1]
    if ($p95 -gt $P95LimitMs -or $rssP95 -gt $RssLimitKiB) {
        throw "$Label benchmark exceeded its accepted P95 budget: $([Math]::Round($p95, 1)) ms, $rssP95 KiB."
    }
    [pscustomobject]@{
        sample = $Label
        runs = $Runs
        median_ms = [Math]::Round((Get-Median $durations), 1)
        p95_ms = [Math]::Round($p95, 1)
        peak_rss_p95_kib = $rssP95
        failures = 0
    }
}

function Invoke-LinuxGuiGate {
    param([string]$Application, [string]$DataRoot)

    if (-not $IsLinux) { throw 'The Kindle Linux GUI gate requires Linux.' }
    foreach ($command in @('systemctl', 'systemd-run', 'identify')) {
        if (-not (Get-Command $command -ErrorAction SilentlyContinue)) {
            throw "$command is required for the Kindle Linux GUI gate."
        }
    }
    $driver = if ($env:ATHA_TAURI_DRIVER) {
        (Resolve-Path -LiteralPath $env:ATHA_TAURI_DRIVER).Path
    }
    elseif (Get-Command 'tauri-driver' -ErrorAction SilentlyContinue) {
        (Get-Command 'tauri-driver').Source
    }
    else {
        $temporaryDriver = Join-Path $temporaryRoot 'tauri-driver/bin/tauri-driver'
        if (-not (Test-Path -LiteralPath $temporaryDriver -PathType Leaf)) {
            throw 'tauri-driver is required for the Kindle Linux GUI gate.'
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
    $unit = "atha-kindle-gui-$PID"
    $session = $null
    $unitStarted = $false
    $screenshot = Join-Path $guiRoot 'reader.png'
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
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Kindle library did not become ready.' -Script @'
return { ready: document.readyState, cards: document.querySelectorAll('.library-book-open').length };
'@ -Accepted { param($value) $value.ready -eq 'complete' -and $value.cards -eq 1 })
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script "document.querySelector('.library-book-open').click(); return true;")
        $reader = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Kindle reader did not become ready.' -Script @'
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  toc: document.querySelectorAll('#toc option').length
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.toc -gt 1 }

        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
const items = [...document.querySelectorAll('#directory-list [data-value]')];
items[Math.min(19, items.length - 1)].click();
const sizes = document.querySelector('#font-size');
sizes.value = sizes.options[sizes.options.length - 1].value;
sizes.dispatchEvent(new Event('change', { bubbles: true }));
const themes = document.querySelector('#theme');
themes.value = themes.options[themes.options.length - 1].value;
themes.dispatchEvent(new Event('change', { bubbles: true }));
return true;
'@)
        $position = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Kindle navigation did not settle.' -Script @'
const text = document.querySelector('#progress-position')?.textContent || '';
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  position: text,
  section: Number(text.match(/\d+/u)?.[0] || 0)
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.section -gt 1 }

        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script @'
document.querySelector('.reader-tool.search > summary').click();
const labels = [...document.querySelectorAll('#toc option')].map((item) => item.textContent.trim());
const match = labels.map((label) => label.match(/[\p{L}\p{N}]{2,8}/u)?.[0]).find(Boolean);
if (!match) return false;
const query = document.querySelector('#search-query');
query.value = match;
query.dispatchEvent(new Event('input', { bubbles: true }));
document.querySelector('#search-form').requestSubmit();
return true;
'@)
        $search = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Kindle full-book search did not finish.' -Script @'
return {
  status: document.documentElement.dataset.status || null,
  error: document.documentElement.dataset.error || null,
  results: document.querySelector('#search-results').options.length
};
'@ -Accepted { param($value) $value.results -gt 0 }

        $shot = Invoke-WebDriver -BaseUrl $baseUrl -Method Get -Path "/session/$session/screenshot" -Body $null
        [IO.File]::WriteAllBytes($screenshot, [Convert]::FromBase64String($shot.value))
        $colors = [int](& identify -format '%k' $screenshot)
        if ($LASTEXITCODE -ne 0 -or $colors -lt 10) { throw 'The Kindle reader screenshot is blank.' }
        Start-Sleep -Seconds 1

        [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Delete -Path "/session/$session" -Body $null)
        $session = $null
        $created = New-TauriSession -BaseUrl $baseUrl -Application $Application
        $session = $created.sessionId
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Restarted Kindle library did not become ready.' -Script "return { ready: document.readyState, cards: document.querySelectorAll('.library-book-open').length };" -Accepted { param($value) $value.ready -eq 'complete' -and $value.cards -eq 1 })
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script "document.querySelector('.library-book-open').click(); return true;")
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Kindle reading position was not restored.' -Script "const text = document.querySelector('#progress-position')?.textContent || ''; const section = Number(text.match(/\d+/u)?.[0] || 0); return { status: document.documentElement.dataset.status || null, error: document.documentElement.dataset.error || null, section, restored: section === $($position.section) };" -Accepted { param($value) $value.status -eq 'pass' -and $value.restored })

        [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Delete -Path "/session/$session" -Body $null)
        $session = $null
        Invoke-Checked 'systemctl' @('--user', 'stop', "$unit.service") 'Could not stop the Kindle Linux GUI gate.'
        $unitStarted = $false

        $privateValues = [Collections.Generic.List[string]]::new()
        foreach ($record in @(Get-ChildItem -LiteralPath (Join-Path $DataRoot 'com.atha.reader/Library') -Filter '*.json' -File)) {
            $book = Get-Content -LiteralPath $record.FullName -Raw | ConvertFrom-Json
            foreach ($value in @($book.id, $book.title) + @($book.authors)) {
                if (-not [string]::IsNullOrWhiteSpace($value)) { $privateValues.Add([string]$value) }
            }
        }
        foreach ($sample in @(Get-ChildItem -LiteralPath (Join-Path $repoRoot 'fixtures/local') -File)) {
            $privateValues.Add($sample.FullName)
            $privateValues.Add($sample.Name)
        }
        foreach ($log in @(Get-ChildItem -LiteralPath (Join-Path $DataRoot 'com.atha.reader/logs') -File -ErrorAction SilentlyContinue)) {
            $content = Get-Content -LiteralPath $log.FullName -Raw
            if ($privateValues | Where-Object { $content.Contains($_, [StringComparison]::Ordinal) }) {
                throw 'The Linux AppLog contains private Kindle sample data.'
            }
        }

        [pscustomobject]@{
            webview = "WebKitGTK $($created.capabilities.browserVersion)"
            toc_items = $reader.toc
            search_results = $search.results
            screenshot_colors = $colors
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

foreach ($path in @($guiRoot, $benchmarkRoot)) {
    $resolved = [IO.Path]::GetFullPath($path)
    if (-not $resolved.StartsWith(([IO.Path]::GetFullPath($temporaryRoot).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar), [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Refusing to use a Kindle gate path outside the repository .tmp directory.'
    }
}

Push-Location $repoRoot
try {
    foreach ($path in @($guiRoot, $benchmarkRoot)) {
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Recurse -Force }
        [void](New-Item -ItemType Directory -Path $path -Force)
    }
    Invoke-Checked $cargoPath @('fmt', '--all', '--check') 'Kindle formatting check failed.'
    Invoke-Checked $cargoPath @('test', '--locked', '-p', 'atha-backend', '--test', 'kindle_import') 'Kindle importer tests failed.'
    Invoke-Checked $cargoPath @('test', '--release', '--locked', '-p', 'atha-backend', '--test', 'kindle_import', '--no-run') 'Kindle release test build failed.'
    $testBinary = Get-ChildItem -LiteralPath (Join-Path $repoRoot 'target/release/deps') -File |
        Where-Object { $_.Name -match '^kindle_import-[0-9a-f]+$' } |
        Sort-Object LastWriteTimeUtc |
        Select-Object -Last 1
    if (-not $testBinary) { throw 'Could not locate the Kindle release test binary.' }

    Invoke-ReleaseBenchmark -Binary $testBinary.FullName -Test 'imports_private_kf8_samples' -Label 'ordinary' -Runs $BenchmarkRuns -P95LimitMs $ordinaryP95LimitMs -RssLimitKiB $ordinaryRssLimitKiB | Format-List
    Invoke-ReleaseBenchmark -Binary $testBinary.FullName -Test 'rejects_private_dictionary_before_expansion' -Label 'dictionary' -Runs $BenchmarkRuns -P95LimitMs $dictionaryP95LimitMs -RssLimitKiB $dictionaryRssLimitKiB | Format-List

    if ($VerifyLinuxGui) {
        Invoke-Checked 'pnpm' @('--dir', 'reader/app', 'check') 'Svelte check failed.'
        Invoke-Checked 'pnpm' @('--dir', 'reader/app', 'build') 'Svelte build failed.'
        Invoke-Checked $cargoPath @('build', '--locked', '-p', 'atha-reader-app') 'Linux Tauri build failed.'
        $dataRoot = Join-Path $guiRoot 'data'
        [void](New-Item -ItemType Directory -Path $dataRoot -Force)
        $env:ATHA_KINDLE_GATE_LIBRARY_ROOT = Join-Path $dataRoot 'com.atha.reader'
        try {
            Invoke-Checked $testBinary.FullName @('imports_private_kf8_samples', '--ignored', '--exact') 'Kindle GUI library preparation failed.'
        }
        finally {
            Remove-Item Env:ATHA_KINDLE_GATE_LIBRARY_ROOT -ErrorAction SilentlyContinue
        }
        Invoke-LinuxGuiGate -Application (Join-Path $repoRoot 'target/debug/atha-reader-app') -DataRoot $dataRoot | Format-List
    }
}
finally {
    Pop-Location
    foreach ($path in @($guiRoot, $benchmarkRoot)) {
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Recurse -Force }
    }
}
