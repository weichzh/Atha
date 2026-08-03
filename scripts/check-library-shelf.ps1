# Description: Verify the local EPUB library module, Tauri entry, and responsive shelf UI.

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$hostPath = Join-Path $repoRoot 'target\debug\atha-reader-app.exe'
$appRoot = Join-Path $repoRoot 'reader\app'
$screenshots = Join-Path $repoRoot 'artifacts\local\screenshots'
$session = "atha-library-check-$PID"
$port = 1421

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

function Invoke-LibraryWindowSmoke {
    $startInfo = [Diagnostics.ProcessStartInfo]::new($hostPath)
    $startInfo.UseShellExecute = $false
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
    }
    finally {
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
        Invoke-Checked 'agent-browser' @('--session', $session, 'set', 'viewport', '390', '840')
        Invoke-Checked 'agent-browser' @('--session', $session, 'wait', '--text', '开始你的书架')
        $geometry = @(& agent-browser --session $session eval '({width:document.documentElement.scrollWidth,viewport:innerWidth,header:document.querySelector(".library-header")?.getBoundingClientRect().height,buttons:[...document.querySelectorAll("button")].every(button=>button.getBoundingClientRect().height>=42)})') -join "`n"
        if ($LASTEXITCODE -ne 0) { throw 'agent-browser geometry check failed.' }
        $result = $geometry | ConvertFrom-Json
        if ($result.width -ne $result.viewport -or $result.header -lt 80 -or -not $result.buttons) {
            throw "Invalid library geometry: $geometry"
        }
        Invoke-Checked 'agent-browser' @(
            '--session', $session, 'screenshot',
            (Join-Path $screenshots 'library-empty-mobile.png')
        )
        Invoke-Checked 'agent-browser' @(
            '--session', $session, 'find', 'role', 'button', 'click', '--name', '选择 EPUB'
        )
        Invoke-Checked 'agent-browser' @(
            '--session', $session, 'wait', '--text', '请在 Atha 桌面应用中选择 EPUB。'
        )
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
    Invoke-Checked $env:ATHA_CARGO @('build', '--locked', '-p', 'atha-reader-app')
    Invoke-LibraryWindowSmoke
    Invoke-LibraryBrowserCheck
}
finally {
    Pop-Location
}
