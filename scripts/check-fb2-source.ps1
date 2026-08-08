# Description: Generate and verify the deterministic FB2 fixture, with an optional Linux GUI gate.

[CmdletBinding()]
param(
    [switch]$VerifyLinuxGui
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$fixturePath = Join-Path $repoRoot '.tmp/fb2-gate.fb2'
$importsRoot = Join-Path $repoRoot '.tmp/fb2-gate-imports'
$guiRoot = Join-Path $repoRoot '.tmp/fb2-linux-gui'
$expectedFixtureSha256 = '155225e7aa977574c5f75559f58ad121004bf714b91e10caeacd774da5550186'

. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot
$cargoPath = $env:ATHA_CARGO

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
    throw $Failure
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
    param([string] $Application, [string] $DataRoot)

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
  title: document.querySelector('.library-book-title')?.textContent || '',
  author: document.querySelector('.library-book-author')?.textContent || ''
};
'@ -Accepted { param($value) $value.ready -eq 'complete' -and $value.cards -eq 1 }
        if ($library.href -ne 'tauri://localhost' -or $library.title -ne 'Atha FB2 Gate' -or $library.author -ne 'Ada Lin') {
            throw 'The Linux library did not expose the prepared FB2 book.'
        }

        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script "document.querySelector('.library-book-open').click(); return true;")
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

        $shot = Invoke-WebDriver -BaseUrl $baseUrl -Method Get -Path "/session/$session/screenshot" -Body $null
        [IO.File]::WriteAllBytes($screenshot, [Convert]::FromBase64String($shot.value))
        $colors = [int](& identify -format '%k' $screenshot)
        if ($LASTEXITCODE -ne 0 -or $colors -lt 10) { throw 'The Linux reader screenshot is blank.' }

        [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Delete -Path "/session/$session" -Body $null)
        $session = $null
        $created = New-TauriSession -BaseUrl $baseUrl -Application $Application
        $session = $created.sessionId
        [void](Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'Restarted FB2 library did not become ready.' -Script "return { ready: document.readyState, cards: document.querySelectorAll('.library-book-open').length };" -Accepted { param($value) $value.ready -eq 'complete' -and $value.cards -eq 1 })
        [void](Invoke-WebDriverScript -BaseUrl $baseUrl -Session $session -Script "document.querySelector('.library-book-open').click(); return true;")
        $restored = Wait-WebDriverScript -BaseUrl $baseUrl -Session $session -Failure 'FB2 reading position was not restored.' -Script @'
if (document.documentElement.dataset.status !== 'pass') {
  return { status: document.documentElement.dataset.status || null, error: document.documentElement.dataset.error || null };
}
return {
  status: 'pass',
  restored: document.querySelector('#progress-position').textContent.includes('4/4') &&
    document.querySelector('#progress-chapter').textContent === '\u6ce8\u91ca'
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.restored }

        [void](Invoke-WebDriver -BaseUrl $baseUrl -Method Delete -Path "/session/$session" -Body $null)
        $session = $null
        Invoke-Checked 'systemctl' @('--user', 'stop', "$unit.service") 'Could not stop the FB2 Linux GUI gate.'
        $unitStarted = $false

        $privatePattern = 'Atha FB2 Gate|Ada Lin|第一章|正文|重点|注释正文|fb2-gate\.fb2|5cec82bcb5514780'
        $logs = Join-Path $DataRoot 'com.atha.reader/logs'
        foreach ($log in @(Get-ChildItem -LiteralPath $logs -File -ErrorAction SilentlyContinue)) {
            if ((Get-Content -LiteralPath $log.FullName -Raw) -match $privatePattern) {
                throw 'The Linux AppLog contains private FB2 fixture data.'
            }
        }

        [pscustomobject]@{
            webview = "WebKitGTK $($created.capabilities.browserVersion)"
            sections = 4
            toc_items = $reader.toc
            search_results = $search.results
            restored_section = 4
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
        Invoke-LinuxGuiGate -Application (Join-Path $repoRoot 'target/debug/atha-reader-app') -DataRoot (Join-Path $guiRoot 'data') | Format-List
    }
}
finally {
    Pop-Location
    foreach ($path in @($fixturePath, $importsRoot, $guiRoot)) {
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Recurse -Force }
    }
}
