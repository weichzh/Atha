# Description: Verify the Windows Tauri/WebView2 shelf and the production browser fixture.

[CmdletBinding()]
param([string]$BookPath)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$targetRoot = Join-Path $repoRoot 'target'
$hostPath = Join-Path $targetRoot 'debug/atha-reader-app.exe'
$appRoot = Join-Path $repoRoot 'reader/app'
$uiCheck = Join-Path $appRoot 'tests/windows-library-shelf.browser.mjs'
$screenshots = Join-Path $repoRoot 'artifacts/local/screenshots'
$runId = "$PID-$([Guid]::NewGuid().ToString('N'))"
$session = "atha-library-check-$runId"
$isolationRoot = Join-Path $repoRoot ".tmp/library-shelf-gate-$runId"
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

function Invoke-LibraryWindowCheck {
    param([Parameter(Mandatory)][string]$SelectedBookPath)

    New-Item -ItemType Directory -Path $isolatedLocalAppData | Out-Null
    $isolatedWebViewData = Join-Path $isolatedLocalAppData 'WebView2'
    $isolatedWebViewProfile = Join-Path $isolatedWebViewData 'EBWebView'
    $profileArgument = [regex]::Escape($isolatedWebViewProfile)
    $controlBook = Join-Path $isolationRoot "Atha Shelf Gate Control $runId.txt"
    Set-Content -LiteralPath $controlBook -Value 'Synthetic Windows shelf control book.' -Encoding utf8

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
            if ($process.MainWindowHandle -ne [IntPtr]::Zero) { break }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $deadline)
        if ($process.MainWindowHandle -eq [IntPtr]::Zero) { throw 'Atha library window did not appear.' }
        if ($process.MainWindowTitle -ne 'Atha') { throw 'Unexpected Atha library window title.' }

        Wait-LocalPort $nativePort
        $webViewDeadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            $webViewIsolated = @(
                Get-CimInstance Win32_Process -Filter "ParentProcessId = $($process.Id)" |
                    Where-Object {
                        $_.Name -eq 'msedgewebview2.exe' -and
                        $_.CommandLine -match "(?:^|\s)--user-data-dir=(?:`"$profileArgument`"|$profileArgument)(?:\s|$)" -and
                        $_.CommandLine -match "(?:^|\s)--remote-debugging-port=$nativePort(?:\s|$)"
                    }
            ).Count -gt 0
            if (-not $webViewIsolated) { Start-Sleep -Milliseconds 100 }
        } while (-not $webViewIsolated -and [DateTime]::UtcNow -lt $webViewDeadline)
        if (-not $webViewIsolated) { throw 'Could not verify the owned WebView2 profile and debug port.' }

        $sources = Join-Path $isolatedLocalAppData 'Atha/SourceBooks'
        if (!(Test-Path -LiteralPath $sources -PathType Container) -or
            @(Get-ChildItem -LiteralPath $sources -Force).Count -ne 0) {
            throw 'The native shelf gate requires an empty, isolated Atha source directory before importing.'
        }
        # Keep the user sample path out of both driver and picker command lines.
        $driverStart = [Diagnostics.ProcessStartInfo]::new($env:ATHA_NODE)
        $driverStart.UseShellExecute = $false
        $driverStart.Environment['ATHA_SHELF_GATE_EPUB'] = $SelectedBookPath
        $driverStart.Environment['DEBUG'] = ''
        $driverStart.Environment['PWDEBUG'] = ''
        foreach ($arg in @(
            $uiCheck, 'native', "http://127.0.0.1:$nativePort", [string]$process.Id,
            $controlBook, (Join-Path $PSHOME 'pwsh.exe')
        )) { $driverStart.ArgumentList.Add($arg) }
        $driver = [Diagnostics.Process]::Start($driverStart)
        try {
            $driver.WaitForExit()
            if ($driver.ExitCode -ne 0) { throw 'Native shelf UI check failed.' }
        }
        finally {
            if (-not $driver.HasExited) {
                $driver.Kill($true)
                $driver.WaitForExit()
            }
            $driver.Dispose()
        }

        if (@(Get-ChildItem -LiteralPath $sources -File -Recurse).Count -ne 2) {
            throw 'Expected both imported sources to remain in the isolated data root after shelf removal.'
        }
        Write-Output 'Windows Tauri/WebView2 shelf check passed.'
    }
    finally {
        if (-not $process.HasExited) {
            $process.Kill($true)
            $process.WaitForExit()
        }
        $process.Dispose()
    }
}

function Invoke-LibraryBrowserCheck {
    # Reuse the caller's existing agent-browser installation; use a separate,
    # owned session so its Chromium binary/profile is never the native lane.
    try {
        Invoke-Checked 'agent-browser' @('--session', $session, 'open', 'about:blank') | Out-Null
        $cdp = @(& agent-browser --session $session get cdp-url) -join "`n"
        if ($LASTEXITCODE -ne 0 -or $cdp.Trim() -notmatch '^ws://127\.0\.0\.1:\d+/devtools/browser/[a-f0-9-]+/?$') {
            throw 'Could not obtain the owned Chromium session endpoint.'
        }
        Invoke-Checked $env:ATHA_NODE @($uiCheck, 'browser', $cdp.Trim(), $screenshots)
    }
    finally {
        & agent-browser --session $session close 2>$null | Out-Null
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
    Invoke-Checked $env:ATHA_NODE @('--test', 'reader/app/tests/library.test.ts', 'reader/app/tests/shelf-browse.test.ts', 'reader/app/tests/shelf-reading.test.ts')
    Invoke-Checked $env:ATHA_CARGO @('build', '--locked', '-p', 'atha-reader-app', '--target-dir', $targetRoot)
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
