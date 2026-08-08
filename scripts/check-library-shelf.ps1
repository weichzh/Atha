# Description: Verify the local EPUB library module, Tauri entry, and responsive shelf UI.

[CmdletBinding()]
param([string]$BookPath)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$hostPath = Join-Path $repoRoot 'target\debug\atha-reader-app.exe'
$appRoot = Join-Path $repoRoot 'reader\app'
$screenshots = Join-Path $repoRoot 'artifacts\local\screenshots'
$session = "atha-library-check-$PID"
$nativeSession = "$session-native"
$port = 1421
$isolationRoot = Join-Path $repoRoot ".tmp\library-shelf-gate-$PID"
$isolatedLocalAppData = Join-Path $isolationRoot 'LocalAppData'

. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot

function Invoke-Checked {
    param([string]$FilePath, [string[]]$Arguments)

    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$FilePath failed with exit code $LASTEXITCODE." }
}

function Wait-LocalPort {
    param([int]$Port)

    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        try {
            $client = [Net.Sockets.TcpClient]::new('127.0.0.1', $Port)
            $client.Dispose()
            return
        }
        catch {
            Start-Sleep -Milliseconds 100
        }
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Local UI did not listen on port $Port."
}

function Get-FreePort {
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start()
    try { return ([Net.IPEndPoint]$listener.LocalEndpoint).Port }
    finally { $listener.Stop() }
}

function Resolve-GateBookPath {
    param([string]$RequestedPath)

    if ([string]::IsNullOrWhiteSpace($RequestedPath)) {
        $candidate = Get-ChildItem -LiteralPath (Join-Path $repoRoot 'fixtures\local') -Recurse -File -Filter '*.epub' |
            Sort-Object Length |
            Select-Object -First 1
    }
    else {
        $path = if ([IO.Path]::IsPathRooted($RequestedPath)) {
            $RequestedPath
        }
        else {
            Join-Path $repoRoot $RequestedPath
        }
        $candidate = Get-Item -LiteralPath $path -ErrorAction SilentlyContinue
    }

    if (
        $null -eq $candidate -or
        $candidate.PSIsContainer -or
        $candidate.Extension -ine '.epub'
    ) {
        throw 'Library shelf gate requires one readable local EPUB.'
    }
    $candidate.FullName
}

function Submit-NativeBookPicker {
    param(
        [Parameter(Mandatory)][int]$ProcessId,
        [Parameter(Mandatory)][string]$SelectedBookPath
    )

    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

namespace Atha {
    public static class NativePicker {
        [DllImport("user32.dll", EntryPoint = "SendMessageW", CharSet = CharSet.Unicode)]
        public static extern IntPtr SetText(IntPtr window, uint message, IntPtr parameter, string value);

        [DllImport("user32.dll", EntryPoint = "SendMessageW")]
        public static extern IntPtr Click(IntPtr window, uint message, IntPtr parameter, IntPtr value);
    }
}
'@
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    $processCondition = [System.Windows.Automation.PropertyCondition]::new(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
        $ProcessId
    )
    do {
        $windows = [System.Windows.Automation.AutomationElement]::RootElement.FindAll(
            [System.Windows.Automation.TreeScope]::Children,
            $processCondition
        )
        foreach ($window in $windows) {
            $fileName = $window.FindFirst(
                [System.Windows.Automation.TreeScope]::Descendants,
                [System.Windows.Automation.AndCondition]::new(
                    [System.Windows.Automation.PropertyCondition]::new(
                        [System.Windows.Automation.AutomationElement]::AutomationIdProperty,
                        '1148'
                    ),
                    [System.Windows.Automation.PropertyCondition]::new(
                        [System.Windows.Automation.AutomationElement]::ClassNameProperty,
                        'Edit'
                    )
                )
            )
            $openButton = $window.FindFirst(
                [System.Windows.Automation.TreeScope]::Descendants,
                [System.Windows.Automation.AndCondition]::new(
                    [System.Windows.Automation.PropertyCondition]::new(
                        [System.Windows.Automation.AutomationElement]::AutomationIdProperty,
                        '1'
                    ),
                    [System.Windows.Automation.PropertyCondition]::new(
                        [System.Windows.Automation.AutomationElement]::ClassNameProperty,
                        'Button'
                    )
                )
            )
            if ($null -eq $fileName -or $null -eq $openButton) { continue }

            try {
                $window.SetFocus()
                $textSet = [Atha.NativePicker]::SetText(
                    [IntPtr]$fileName.Current.NativeWindowHandle,
                    0x000C,
                    [IntPtr]::Zero,
                    $SelectedBookPath
                )
                if ($textSet -eq [IntPtr]::Zero) { continue }
                [void][Atha.NativePicker]::Click(
                    [IntPtr]$openButton.Current.NativeWindowHandle,
                    0x00F5,
                    [IntPtr]::Zero,
                    [IntPtr]::Zero
                )
                return
            }
            catch {
                continue
            }
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)

    throw 'Windows book picker could not be completed.'
}

function Get-AgentBoolean {
    param(
        [Parameter(Mandatory)][int]$Port,
        [Parameter(Mandatory)][string]$JavaScript
    )

    $raw = @(& agent-browser --session $nativeSession --cdp $Port eval $JavaScript) -join "`n"
    if ($LASTEXITCODE -ne 0) { throw 'agent-browser evaluation failed.' }
    [bool]($raw | ConvertFrom-Json)
}

function Assert-NativeBrowserClean {
    param(
        [Parameter(Mandatory)][int]$Port,
        [Parameter(Mandatory)][string]$Stage
    )

    $pageErrors = (@(& agent-browser --session $nativeSession --cdp $Port errors --json) -join "`n") |
        ConvertFrom-Json
    $console = (@(& agent-browser --session $nativeSession --cdp $Port console --json) -join "`n") |
        ConvertFrom-Json
    if (
        @($pageErrors.data.errors).Count -ne 0 -or
        @($console.data.messages | Where-Object type -eq 'error').Count -ne 0
    ) {
        throw "Native library $Stage check observed a page or console error."
    }
    Invoke-Checked 'agent-browser' @(
        '--session', $nativeSession, '--cdp', $Port, 'errors', '--clear'
    ) | Out-Null
    Invoke-Checked 'agent-browser' @(
        '--session', $nativeSession, '--cdp', $Port, 'console', '--clear'
    ) | Out-Null
}

function Invoke-LibraryWindowCheck {
    param([Parameter(Mandatory)][string]$SelectedBookPath)

    New-Item -ItemType Directory -Force -Path $isolatedLocalAppData | Out-Null
    $isolatedWebViewData = Join-Path $isolatedLocalAppData 'WebView2'
    $isolatedWebViewProfile = Join-Path $isolatedWebViewData 'EBWebView'
    $nativePort = Get-FreePort
    $startInfo = [Diagnostics.ProcessStartInfo]::new($hostPath)
    $startInfo.UseShellExecute = $false
    $startInfo.Environment['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS'] = "--remote-debugging-port=$nativePort"
    $startInfo.Environment['WEBVIEW2_USER_DATA_FOLDER'] = $isolatedWebViewData
    $startInfo.Environment['LOCALAPPDATA'] = $isolatedLocalAppData
    $process = [Diagnostics.Process]::Start($startInfo)
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        do {
            if ($process.HasExited) { throw "Atha library exited with code $($process.ExitCode)." }
            $process.Refresh()
            Start-Sleep -Milliseconds 100
        } while ($process.MainWindowHandle -eq [IntPtr]::Zero -and [DateTime]::UtcNow -lt $deadline)
        if ($process.MainWindowHandle -eq [IntPtr]::Zero) { throw 'Atha library window did not appear.' }
        if ($process.MainWindowTitle -ne 'Atha') {
            throw "Unexpected Atha library window title: $($process.MainWindowTitle)"
        }
        Wait-LocalPort $nativePort
        $webViewDeadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            $webViewIsolated = @(
                Get-CimInstance Win32_Process -Filter "ParentProcessId = $($process.Id)" |
                    Where-Object {
                        $_.Name -eq 'msedgewebview2.exe' -and
                        $_.CommandLine -like "*--user-data-dir=`"$isolatedWebViewProfile`"*"
                    }
            ).Count -gt 0
            if (-not $webViewIsolated) { Start-Sleep -Milliseconds 100 }
        } while (-not $webViewIsolated -and [DateTime]::UtcNow -lt $webViewDeadline)
        if (-not $webViewIsolated) { throw 'Atha WebView2 data was not isolated for the native shelf check.' }
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'wait', '--fn', "Boolean(document.querySelector('.library-shell'))"
        )
        $nativeUrl = @(& agent-browser --session $nativeSession --cdp $nativePort get url) -join "`n"
        if ($LASTEXITCODE -ne 0 -or $nativeUrl -notmatch '^https://tauri\.localhost/?$') {
            throw "Unexpected Atha library URL: $nativeUrl"
        }
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort, 'errors', '--clear'
        ) | Out-Null
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort, 'console', '--clear'
        ) | Out-Null

        $pickerJob = Start-ThreadJob -ScriptBlock ${function:Submit-NativeBookPicker} -ArgumentList @(
            $process.Id,
            $SelectedBookPath
        )
        try {
            Invoke-Checked 'agent-browser' @(
                '--session', $nativeSession, '--cdp', $nativePort,
                'find', 'role', 'button', 'click', '--name', '导入'
            ) | Out-Null
            if ($null -eq (Wait-Job -Job $pickerJob -Timeout 25)) {
                throw 'Windows book picker did not finish.'
            }
            Receive-Job -Job $pickerJob -ErrorAction Stop | Out-Null
            if ($pickerJob.State -ne 'Completed') { throw 'Windows book picker failed.' }
        }
        finally {
            Stop-Job -Job $pickerJob -ErrorAction SilentlyContinue
            Remove-Job -Job $pickerJob -Force -ErrorAction SilentlyContinue
        }
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'wait', '--fn', 'document.querySelectorAll(".library-book").length===1'
        ) | Out-Null
        Assert-NativeBrowserClean -Port $nativePort -Stage 'import'

        $metadataReady = Get-AgentBoolean -Port $nativePort -JavaScript '(()=>{const title=document.querySelector(".library-book-title")?.textContent?.trim();const author=document.querySelector(".library-book-author")?.textContent?.trim();if(!title||!author||author==="未知作者")return false;globalThis.__athaLibraryGate={title,author};return true})()'
        if (-not $metadataReady) { throw 'The selected EPUB must expose a title and author for shelf search verification.' }

        $miss = ([Guid]::NewGuid().ToString('N')).Substring(0, 12)
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'find', 'placeholder', '搜索书名或作者', 'fill', $miss
        ) | Out-Null
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'wait', '--fn', 'Boolean(document.querySelector(".library-no-results"))'
        ) | Out-Null

        $titleQuerySet = Get-AgentBoolean -Port $nativePort -JavaScript '(()=>{const input=document.querySelector("input[type=search]");const value=globalThis.__athaLibraryGate?.title;if(!input||!value)return false;input.value=value;input.dispatchEvent(new Event("input",{bubbles:true}));return input.value===value})()'
        if (-not $titleQuerySet) { throw 'Title search could not be exercised in the native shelf.' }
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'wait', '--fn', 'document.querySelectorAll(".library-book").length===1'
        ) | Out-Null

        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'find', 'placeholder', '搜索书名或作者', 'fill', $miss
        ) | Out-Null
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'wait', '--fn', 'Boolean(document.querySelector(".library-no-results"))'
        ) | Out-Null
        $authorQuerySet = Get-AgentBoolean -Port $nativePort -JavaScript '(()=>{const input=document.querySelector("input[type=search]");const value=globalThis.__athaLibraryGate?.author;if(!input||!value)return false;input.value=value;input.dispatchEvent(new Event("input",{bubbles:true}));return input.value===value})()'
        if (-not $authorQuerySet) { throw 'Author search could not be exercised in the native shelf.' }
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'wait', '--fn', 'document.querySelectorAll(".library-book").length===1'
        ) | Out-Null
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'find', 'placeholder', '搜索书名或作者', 'fill', ''
        ) | Out-Null
        Assert-NativeBrowserClean -Port $nativePort -Stage 'search'

        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'find', 'role', 'button', 'click', '--name', '进度'
        ) | Out-Null
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'wait', '--fn', 'Boolean(document.querySelector("#library-reading-heading")&&document.querySelector("#library-unread-heading"))'
        ) | Out-Null
        $progressValid = Get-AgentBoolean -Port $nativePort -JavaScript '(()=>{const reading=document.querySelector("#library-reading-heading")?.closest("section");const unread=document.querySelector("#library-unread-heading")?.closest("section");return Boolean(reading&&unread&&reading.querySelectorAll(".library-book").length===0&&unread.querySelectorAll(".library-book").length===1)})()'
        if (-not $progressValid) { throw 'Native shelf progress grouping was not isolated and deterministic.' }
        Assert-NativeBrowserClean -Port $nativePort -Stage 'progress'

        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'find', 'role', 'button', 'click', '--name', '默认'
        ) | Out-Null
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'find', 'role', 'button', 'click', '--name', '选择'
        ) | Out-Null
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'click', '.library-book-open'
        ) | Out-Null
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'wait', '--fn', 'Boolean(document.querySelector(".library-book-open[aria-pressed=true]")&&document.querySelector(".library-selection-header span")?.textContent==="已选择 1 本")'
        ) | Out-Null
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'click', '.library-selection-header>button:last-child'
        ) | Out-Null
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'wait', '--fn', 'Boolean(!document.querySelector(".library-selection-header")&&document.querySelectorAll(".library-book").length===1)'
        ) | Out-Null
        Assert-NativeBrowserClean -Port $nativePort -Stage 'selection'

        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'find', 'role', 'button', 'click', '--name', '选择'
        ) | Out-Null
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'click', '.library-selection-header>button:first-child'
        ) | Out-Null
        $confirmReady = Get-AgentBoolean -Port $nativePort -JavaScript @'
(()=>{globalThis.__athaLibraryGateOriginalConfirm=globalThis.confirm;globalThis.__athaLibraryGateConfirmCalls=0;globalThis.confirm=()=>{globalThis.__athaLibraryGateConfirmCalls+=1;return true};return true})()
'@
        if (-not $confirmReady) { throw 'Browser confirmation probe was not installed.' }
        try {
            Invoke-Checked 'agent-browser' @(
                '--session', $nativeSession, '--cdp', $nativePort,
                'click', '.library-selection-bar button'
            ) | Out-Null
            $confirmValid = Get-AgentBoolean -Port $nativePort -JavaScript @'
(()=>globalThis.__athaLibraryGateConfirmCalls===1)()
'@
            if (-not $confirmValid) { throw 'Browser confirmation was not requested exactly once.' }
        }
        finally {
            & agent-browser --session $nativeSession --cdp $nativePort eval @'
(()=>{if(globalThis.__athaLibraryGateOriginalConfirm){globalThis.confirm=globalThis.__athaLibraryGateOriginalConfirm}delete globalThis.__athaLibraryGateOriginalConfirm;delete globalThis.__athaLibraryGateConfirmCalls;return true})()
'@ 2>$null | Out-Null
        }
        Invoke-Checked 'agent-browser' @(
            '--session', $nativeSession, '--cdp', $nativePort,
            'wait', '--text', '开始你的书架'
        ) | Out-Null
        Assert-NativeBrowserClean -Port $nativePort -Stage 'remove'
    }
    finally {
        & agent-browser --session $nativeSession close 2>$null | Out-Null
        if (-not $process.HasExited) {
            $process.Kill($true)
            $process.WaitForExit()
        }
    }
}

function Invoke-LibraryBrowserCheck {
    New-Item -ItemType Directory -Force -Path $screenshots | Out-Null
    $stdout = Join-Path $repoRoot ".tmp\library-check-$PID.stdout.log"
    $stderr = Join-Path $repoRoot ".tmp\library-check-$PID.stderr.log"
    $server = Start-Process -FilePath $env:ATHA_PNPM -ArgumentList @(
        '--dir', $appRoot, 'dev', '--host', '127.0.0.1', '--port', $port
    ) -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
    try {
        Wait-LocalPort $port
        Invoke-Checked 'agent-browser' @('--session', $session, 'open', "http://127.0.0.1:$port/")
        Invoke-Checked 'agent-browser' @('--session', $session, 'set', 'viewport', '360', '800')
        Invoke-Checked 'agent-browser' @('--session', $session, 'wait', '--text', '开始你的书架')

        $browserQuery = ([Guid]::NewGuid().ToString('N')).Substring(0, 12)
        Invoke-Checked 'agent-browser' @(
            '--session', $session, 'find', 'placeholder', '搜索书名或作者', 'fill', $browserQuery
        )
        $queryValue = @(& agent-browser --session $session get value 'input[type=search]') -join "`n"
        if ($LASTEXITCODE -ne 0 -or $queryValue.Trim() -ne $browserQuery) {
            throw 'Library search input did not retain its local value.'
        }
        Invoke-Checked 'agent-browser' @(
            '--session', $session, 'find', 'placeholder', '搜索书名或作者', 'fill', ''
        )

        foreach ($viewName in @('默认', '进度', '书名', '作者')) {
            Invoke-Checked 'agent-browser' @(
                '--session', $session, 'find', 'role', 'button', 'click', '--name', $viewName
            )
            $activeView = @(& agent-browser --session $session eval 'document.querySelector(".library-views button[aria-current=page]")?.textContent.trim()') -join "`n"
            if ($LASTEXITCODE -ne 0 -or ($activeView | ConvertFrom-Json) -ne $viewName) {
                throw "Library view did not activate: $viewName"
            }
        }

        Invoke-Checked 'agent-browser' @('--session', $session, 'click', '.library-management summary')
        Invoke-Checked 'agent-browser' @('--session', $session, 'wait', '--fn', 'document.querySelector(".library-management")?.open===true')
        $menu = @(& agent-browser --session $session eval '(()=>{const menu=document.querySelector(".library-management-menu");const rect=menu?.getBoundingClientRect();return {visible:Boolean(rect&&rect.width&&rect.height),inside:Boolean(rect&&rect.left>=0&&rect.right<=innerWidth),buttons:[...menu.querySelectorAll("button")].every(button=>{const box=button.getBoundingClientRect();return box.width>=44&&box.height>=44})}})()') -join "`n"
        if ($LASTEXITCODE -ne 0) { throw 'agent-browser management-menu check failed.' }
        $menuResult = $menu | ConvertFrom-Json
        if (-not $menuResult.visible -or -not $menuResult.inside -or -not $menuResult.buttons) {
            throw "Invalid library management menu geometry: $menu"
        }
        Invoke-Checked 'agent-browser' @('--session', $session, 'click', '.library-management summary')

        foreach ($viewport in @(
            @{ Width = 360; Height = 800 },
            @{ Width = 412; Height = 915 },
            @{ Width = 768; Height = 1024 },
            @{ Width = 1280; Height = 900 }
        )) {
            Invoke-Checked 'agent-browser' @(
                '--session', $session, 'set', 'viewport', [string]$viewport.Width, [string]$viewport.Height
            )
            $geometry = @(& agent-browser --session $session eval '(()=>{const visible=element=>{const style=getComputedStyle(element);const rect=element.getBoundingClientRect();return style.visibility!=="hidden"&&style.display!=="none"&&rect.width>0&&rect.height>0};const controls=[...document.querySelectorAll("button,input,summary")].filter(visible);const controlsValid=controls.every(control=>{const rect=control.getBoundingClientRect();return rect.width>=44&&rect.height>=44});const empty=document.querySelector(".library-empty");const shell=document.querySelector(".library-shell");const fixture=document.createElement("section");fixture.className="library-grid";fixture.dataset.layoutFixture="true";for(let index=0;index<12;index++){const article=document.createElement("article");article.className="library-book";const button=document.createElement("button");button.className="library-book-open";const cover=document.createElement("span");cover.className="library-cover";const title=document.createElement("span");title.className="library-book-title";title.textContent="Layout";const author=document.createElement("span");author.className="library-book-author";author.textContent="Author";button.append(cover,title,author);article.append(button);fixture.append(article)}const bar=document.createElement("div");bar.className="library-selection-bar";const remove=document.createElement("button");remove.append(document.createElement("span"));remove.firstElementChild.textContent="Remove";bar.append(remove);empty.style.display="none";shell.classList.add("library-selecting");shell.append(fixture,bar);scrollTo(0,document.documentElement.scrollHeight);const last=fixture.lastElementChild.getBoundingClientRect();const barRect=bar.getBoundingClientRect();const columns=getComputedStyle(fixture).gridTemplateColumns.split(" ").length;const selectionButton=remove.getBoundingClientRect();const result={documentWidth:document.documentElement.scrollWidth,viewport:innerWidth,empty:Boolean(empty),controls:controlsValid,columns,lastRowClear:last.bottom<=barRect.top,selectionButton:selectionButton.width>=44&&selectionButton.height>=44};fixture.remove();bar.remove();shell.classList.remove("library-selecting");empty.style.removeProperty("display");scrollTo(0,0);return result})()') -join "`n"
            if ($LASTEXITCODE -ne 0) { throw 'agent-browser responsive geometry check failed.' }
            $result = $geometry | ConvertFrom-Json
            $expectedColumns = if ($viewport.Width -lt 620) { 3 } else { 4 }
            if (
                $result.documentWidth -gt $result.viewport -or
                -not $result.empty -or
                -not $result.controls -or
                $result.columns -lt $expectedColumns -or
                -not $result.lastRowClear -or
                -not $result.selectionButton
            ) {
                throw "Invalid library geometry at $($viewport.Width)x$($viewport.Height): $geometry"
            }
            if ($viewport.Width -in @(360, 1280)) {
                Invoke-Checked 'agent-browser' @(
                    '--session', $session, 'screenshot',
                    (Join-Path $screenshots "library-empty-$($viewport.Width)x$($viewport.Height).png")
                )
            }
        }

        Invoke-Checked 'agent-browser' @(
            '--session', $session, 'find', 'role', 'button', 'click', '--name', '选择书籍'
        )
        Invoke-Checked 'agent-browser' @(
            '--session', $session, 'wait', '--text', '请在 Atha 应用中选择 EPUB、CBZ、Markdown 或 TXT。'
        )

        $pageErrors = (@(& agent-browser --session $session errors --json) -join "`n") | ConvertFrom-Json
        $console = (@(& agent-browser --session $session console --json) -join "`n") | ConvertFrom-Json
        $clientFailures = (@(& agent-browser --session $session network requests --status 400-499 --json) -join "`n") | ConvertFrom-Json
        $serverFailures = (@(& agent-browser --session $session network requests --status 500-599 --json) -join "`n") | ConvertFrom-Json
        $unexpectedClientFailures = @($clientFailures.data.requests | Where-Object url -notmatch '/favicon\.ico$')
        if (
            @($pageErrors.data.errors).Count -ne 0 -or
            @($console.data.messages | Where-Object type -eq 'error').Count -ne 0 -or
            $unexpectedClientFailures.Count -ne 0 -or
            @($serverFailures.data.requests).Count -ne 0
        ) {
            throw 'Library browser check observed a page, console, or HTTP error.'
        }
    }
    finally {
        & agent-browser --session $session close 2>$null | Out-Null
        if (-not $server.HasExited) {
            $server.Kill($true)
            $server.WaitForExit()
        }
        Remove-Item -LiteralPath $stdout, $stderr -Force -ErrorAction SilentlyContinue
    }
}

Push-Location $repoRoot
try {
    Invoke-Checked $env:ATHA_CARGO @('test', '--locked', '-p', 'atha-backend', '--test', 'epub_import')
    Push-Location $appRoot
    try {
        Invoke-Checked $env:ATHA_PNPM @('install', '--frozen-lockfile')
        Invoke-Checked $env:ATHA_PNPM @('check')
        Invoke-Checked $env:ATHA_PNPM @('build')
    }
    finally {
        Pop-Location
    }
    Invoke-Checked $env:ATHA_NODE @('--test', 'reader/app/tests/library.test.ts')
    Invoke-Checked $env:ATHA_CARGO @('build', '--locked', '-p', 'atha-reader-app')
    $selectedBookPath = Resolve-GateBookPath -RequestedPath $BookPath
    Invoke-LibraryWindowCheck -SelectedBookPath $selectedBookPath
    Invoke-LibraryBrowserCheck
}
finally {
    Pop-Location
    $resolvedIsolationRoot = [IO.Path]::GetFullPath($isolationRoot)
    $allowedIsolationRoot = [IO.Path]::GetFullPath((Join-Path $repoRoot '.tmp')) + [IO.Path]::DirectorySeparatorChar
    if (-not $resolvedIsolationRoot.StartsWith($allowedIsolationRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Refusing to clean an unexpected shelf gate directory.'
    }
    for ($attempt = 0; $attempt -lt 10 -and (Test-Path -LiteralPath $resolvedIsolationRoot); $attempt++) {
        try {
            Remove-Item -LiteralPath $resolvedIsolationRoot -Recurse -Force -ErrorAction Stop
        }
        catch {
            if ($attempt -eq 9) { throw }
            Start-Sleep -Milliseconds 100
        }
    }
}
