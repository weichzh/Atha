# Description: Build and verify the Tauri/Svelte reader with the real EPUB and shared performance gate.

[CmdletBinding()]
param(
    [string]$Epub = 'fixtures/local/数学及其历史 (2026).epub',
    [string]$BookRoot = 'fixtures/local/math-history-r8',
    [string]$Entry = 'EPUB/text/ch012.xhtml'
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

function Invoke-TauriWindowBehavior {
    if (-not ('AthaWindowProbe' -as [type])) {
        Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class AthaWindowProbe {
    [StructLayout(LayoutKind.Sequential)]
    public struct Point { public int X, Y; }

    [StructLayout(LayoutKind.Sequential)]
    public struct MinMaxInfo {
        public Point Reserved, MaxSize, MaxPosition, MinTrackSize, MaxTrackSize;
    }

    [DllImport("user32.dll")]
    public static extern bool IsZoomed(IntPtr handle);

    [DllImport("user32.dll")]
    public static extern IntPtr SendMessage(IntPtr handle, uint message, IntPtr wParam, ref MinMaxInfo info);

    [DllImport("user32.dll")]
    public static extern bool ShowWindowAsync(IntPtr handle, int command);
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
        if ($minimum.MinTrackSize.X -lt 360 -or $minimum.MinTrackSize.Y -lt 640) {
            throw "Tauri reader minimum tracking size failed: $($minimum.MinTrackSize.X)x$($minimum.MinTrackSize.Y)."
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
    Invoke-TauriSmoke
    Invoke-TauriWindowBehavior
}
finally {
    Pop-Location
}
