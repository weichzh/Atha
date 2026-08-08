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
    $mobileScreenshot = Join-Path $guiRoot 'reader-mobile.png'
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
    document.querySelector('#progress-chapter').textContent === '\u6ce8\u91ca' &&
    document.querySelector('.reader').style.getPropertyValue('--page-left-margin') === '48px',
  modules: (() => {
    const record = Object.keys(localStorage).find((key) => key.startsWith('atha.reader.book.'));
    return record ? JSON.parse(localStorage.getItem(record)).preferences.styleModules.length : 0;
  })()
};
'@ -Accepted { param($value) $value.status -eq 'pass' -and $value.restored -and $value.modules -eq 2 }

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
            css_modules = $styles.modules
            css_editor = 'CodeMirror 6'
            css_modules_p95_ms = $styleBenchmark.p95
            css_modules_bytes = $styleBenchmark.bytes
            restored_section = 4
            screenshot_colors = $colors
            mobile_screenshot_colors = $mobileColors
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
