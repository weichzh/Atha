# Description: Build and verify the Tauri/Svelte reader with the real EPUB and shared performance gate.

[CmdletBinding()]
param(
    [string]$Epub = 'fixtures/local/数学及其历史 (2026).epub',
    [string]$BookRoot = 'fixtures/local/math-history-r8',
    [string]$Entry = 'EPUB/text/ch012.xhtml',
    [string]$ExpectedTitle = '数学及其历史 (2026)'
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$epubPath = (Resolve-Path (Join-Path $repoRoot $Epub)).Path
$bookRootPath = (Resolve-Path (Join-Path $repoRoot $BookRoot)).Path
$hostPath = Join-Path $repoRoot 'target/debug/atha-reader-app.exe'

. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot

function Invoke-Checked {
    param([string]$FilePath, [string[]]$Arguments)

    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$FilePath failed with exit code $LASTEXITCODE." }
}

function Invoke-TauriSmoke {
    $startInfo = [Diagnostics.ProcessStartInfo]::new($hostPath)
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    foreach ($argument in @('--epub', $epubPath, '--verify-import')) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::Start($startInfo)
    if (-not $process.WaitForExit(120000)) {
        $process.Kill($true)
        $process.WaitForExit()
        throw 'Tauri reader smoke test timed out.'
    }
    if ($process.ExitCode -ne 0) {
        throw "Tauri reader smoke test failed with exit code $($process.ExitCode)."
    }
}

function Get-FreePort {
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start()
    try { return ([Net.IPEndPoint]$listener.LocalEndpoint).Port }
    finally { $listener.Stop() }
}

function Invoke-TauriInteractiveOpen {
    $port = Get-FreePort
    $session = "atha-reader-open-$PID"
    $startInfo = [Diagnostics.ProcessStartInfo]::new($hostPath)
    $startInfo.UseShellExecute = $false
    $startInfo.Environment['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS'] = "--remote-debugging-port=$port"
    foreach ($argument in @('--epub', $epubPath)) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::Start($startInfo)
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        do {
            if ($process.HasExited) { throw "Tauri reader exited with code $($process.ExitCode)." }
            try {
                $client = [Net.Sockets.TcpClient]::new('127.0.0.1', $port)
                $client.Dispose()
                break
            }
            catch { Start-Sleep -Milliseconds 100 }
        } while ([DateTime]::UtcNow -lt $deadline)
        if ([DateTime]::UtcNow -ge $deadline) { throw 'Tauri reader debug endpoint did not appear.' }

        Invoke-Checked 'agent-browser' @(
            '--session', $session, '--cdp', $port,
            'wait', '--fn', "['pass','fail'].includes(document.documentElement.dataset.status)"
        )
        $result = @(& agent-browser --session $session --cdp $port eval "(async()=>{const manifest=await fetch('https://atha-book.localhost/.atha-reader.json').then(response=>response.json());const edition=await window.__TAURI_INTERNALS__.invoke('message_edition_context',{contentVersion:manifest.contentVersion});return {status:document.documentElement.dataset.status,error:document.documentElement.dataset.error||null,edition}})()") -join "`n"
        if ($LASTEXITCODE -ne 0) { throw 'Could not read the Tauri reader result.' }
        $result = $result | ConvertFrom-Json
        if ($result.status -ne 'pass') { throw "Tauri reader failed with $($result.error)." }
        if ($result.edition.title -ne $ExpectedTitle) {
            throw "Unexpected direct EPUB edition title: $($result.edition.title)"
        }
    }
    finally {
        & agent-browser --session $session close 2>$null | Out-Null
        if (-not $process.HasExited) {
            $process.Kill($true)
            $process.WaitForExit()
        }
    }
}

function Invoke-TauriWindowBehavior {
    if (-not ('AthaWindowProbe' -as [type])) {
        Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class AthaWindowProbe {
    [StructLayout(LayoutKind.Sequential)]
    public struct Point { public int X, Y; }

    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left, Top, Right, Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    public struct MinMaxInfo {
        public Point Reserved, MaxSize, MaxPosition, MinTrackSize, MaxTrackSize;
    }

    [DllImport("user32.dll")]
    public static extern bool IsZoomed(IntPtr handle);

    [DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(IntPtr handle);

    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
    private static extern IntPtr GetWindowLongPtr(IntPtr handle, int index);

    [DllImport("user32.dll")]
    private static extern bool AdjustWindowRectExForDpi(ref Rect rect, uint style, bool menu, uint extendedStyle, uint dpi);

    [DllImport("user32.dll")]
    public static extern IntPtr SendMessage(IntPtr handle, uint message, IntPtr wParam, ref MinMaxInfo info);

    [DllImport("user32.dll")]
    public static extern bool ShowWindowAsync(IntPtr handle, int command);

    public static Point RequiredTrackSize(IntPtr handle, int logicalWidth, int logicalHeight) {
        uint dpi = GetDpiForWindow(handle);
        Rect rect = new Rect {
            Right = (int)Math.Ceiling(logicalWidth * dpi / 96.0),
            Bottom = (int)Math.Ceiling(logicalHeight * dpi / 96.0)
        };
        uint style = unchecked((uint)GetWindowLongPtr(handle, -16).ToInt64());
        uint extendedStyle = unchecked((uint)GetWindowLongPtr(handle, -20).ToInt64());
        if (!AdjustWindowRectExForDpi(ref rect, style, false, extendedStyle, dpi)) {
            throw new InvalidOperationException("Could not calculate the native minimum window size.");
        }
        return new Point { X = rect.Right - rect.Left, Y = rect.Bottom - rect.Top };
    }
}
'@
    }

    $startInfo = [Diagnostics.ProcessStartInfo]::new($hostPath)
    $startInfo.UseShellExecute = $false
    foreach ($argument in @('--book-root', $bookRootPath, '--manifest', '.atha-reader.json')) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::Start($startInfo)
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        do {
            if ($process.HasExited) { throw "Tauri reader exited with code $($process.ExitCode)." }
            $process.Refresh()
            Start-Sleep -Milliseconds 100
        } while ($process.MainWindowHandle -eq [IntPtr]::Zero -and [DateTime]::UtcNow -lt $deadline)
        $handle = $process.MainWindowHandle
        if ($handle -eq [IntPtr]::Zero) { throw 'Tauri reader window did not appear.' }

        [void][AthaWindowProbe]::ShowWindowAsync($handle, 3)
        $deadline = [DateTime]::UtcNow.AddSeconds(5)
        while (-not [AthaWindowProbe]::IsZoomed($handle) -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 100
        }
        if (-not [AthaWindowProbe]::IsZoomed($handle)) { throw 'Tauri reader window could not maximize.' }

        [void][AthaWindowProbe]::ShowWindowAsync($handle, 9)
        $minimum = [AthaWindowProbe+MinMaxInfo]::new()
        [void][AthaWindowProbe]::SendMessage($handle, 0x24, [IntPtr]::Zero, [ref]$minimum)
        $required = [AthaWindowProbe]::RequiredTrackSize($handle, 360, 640)
        if ($minimum.MinTrackSize.X -lt $required.X -or $minimum.MinTrackSize.Y -lt $required.Y) {
            throw "Tauri reader minimum tracking size failed: actual=$($minimum.MinTrackSize.X)x$($minimum.MinTrackSize.Y) required=$($required.X)x$($required.Y)."
        }
    }
    finally {
        if (-not $process.HasExited) {
            $process.Kill($true)
            $process.WaitForExit()
        }
    }
}

Push-Location $repoRoot
try {
    Push-Location 'reader/app'
    try {
        Invoke-Checked $env:ATHA_PNPM @('install', '--frozen-lockfile')
        Invoke-Checked $env:ATHA_PNPM @('check')
        Invoke-Checked $env:ATHA_PNPM @('build')
    }
    finally {
        Pop-Location
    }

    Invoke-Checked 'pwsh' @(
        '-NoProfile', '-File', 'scripts/check-reader-slice.ps1',
        '-BookRoot', $bookRootPath,
        '-Entry', $Entry,
        '-HostPackage', 'atha-reader-app',
        '-HostPath', 'target/debug/atha-reader-app.exe'
    )
    Invoke-TauriInteractiveOpen
    Invoke-TauriSmoke
    Invoke-TauriWindowBehavior
}
finally {
    Pop-Location
}
