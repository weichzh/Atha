# Description: Run the complete M2 reader gate with difficult samples, a large book, crash recovery, memory, and performance evidence.

[CmdletBinding()]
param(
    [string]$LargeEpub = 'fixtures/local/数学及其历史 (2026).epub',
    [ValidateRange(1024, 65530)]
    [int]$BasePort = 21200
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$largeEpubPath = (Resolve-Path (Join-Path $repoRoot $LargeEpub)).Path
$largeOutputName = 'math-history-r8'
$largeRoot = Join-Path $repoRoot "fixtures/local/$largeOutputName"
$hostPath = Join-Path $repoRoot 'target/debug/atha-reader-host.exe'
$manifestName = '.atha-reader.json'
$memoryLimitMiB = 1024
$env:AGENT_BROWSER_DEFAULT_TIMEOUT = '120000'

function Invoke-Checked {
    param([string]$FilePath, [string[]]$Arguments, [switch]$Quiet)

    if ($Quiet) { & $FilePath @Arguments | Out-Null } else { & $FilePath @Arguments }
    if ($LASTEXITCODE -ne 0) { throw "$FilePath failed with exit code $LASTEXITCODE." }
}

function Invoke-AgentBrowser {
    param([string[]]$Arguments)

    & agent-browser @Arguments
    if ($LASTEXITCODE -ne 0) { throw "agent-browser failed with exit code $LASTEXITCODE." }
}

function Get-AgentBrowserScriptValue {
    param([string]$Session, [string]$Script)

    $output = @($Script | & agent-browser --session $Session eval --stdin)
    if ($LASTEXITCODE -ne 0) { throw "agent-browser eval failed with exit code $LASTEXITCODE." }
    return [string]::Join("`n", $output).Trim()
}

function Start-ReaderHost {
    param([string[]]$AdditionalArguments)

    $startInfo = [Diagnostics.ProcessStartInfo]::new($hostPath)
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    foreach ($argument in @('--book-root', $largeRoot, '--manifest', $manifestName) + $AdditionalArguments) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    return [Diagnostics.Process]::Start($startInfo)
}

function Wait-ReaderHost {
    param([Diagnostics.Process]$Process, [int]$TimeoutMilliseconds = 180000)

    if (-not $Process.WaitForExit($TimeoutMilliseconds)) {
        $Process.Kill($true)
        $Process.WaitForExit()
        throw 'Reader host timed out.'
    }
    if ($Process.ExitCode -ne 0) {
        throw "Reader host failed with exit code $($Process.ExitCode)."
    }
}

function Get-ProcessTreeSnapshot {
    param([int]$RootId)

    $processes = @(Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, WorkingSetSize)
    $ids = [Collections.Generic.HashSet[uint32]]::new()
    [void]$ids.Add([uint32]$RootId)
    do {
        $added = $false
        foreach ($process in $processes) {
            if ($ids.Contains([uint32]$process.ParentProcessId) -and $ids.Add([uint32]$process.ProcessId)) {
                $added = $true
            }
        }
    } while ($added)
    $members = @($processes | Where-Object { $ids.Contains([uint32]$_.ProcessId) })
    [pscustomobject]@{
        count = $members.Count
        process_ids = @($members | ForEach-Object { [int]$_.ProcessId })
        root_present = @($members | Where-Object { $_.ProcessId -eq $RootId }).Count -eq 1
        working_set_bytes = [uint64](($members | Measure-Object WorkingSetSize -Sum).Sum)
    }
}

function Measure-LargeBookHost {
    $process = Start-ReaderHost @('--verify-sample', '--hold-after-verify')
    $timer = [Diagnostics.Stopwatch]::StartNew()
    [uint64]$peakBytes = 0
    $peakProcesses = 0
    $samples = 0
    try {
        while ($true) {
            if ($process.WaitForExit(100)) {
                throw "Large-book reader exited before memory sampling completed: $($process.ExitCode)."
            }
            $snapshot = Get-ProcessTreeSnapshot $process.Id
            if ($snapshot.root_present -and $snapshot.working_set_bytes -gt 0) {
                $peakBytes = [Math]::Max($peakBytes, $snapshot.working_set_bytes)
                $peakProcesses = [Math]::Max($peakProcesses, $snapshot.count)
                $samples += 1
            }
            $process.Refresh()
            if ($samples -ge 5 -and $process.MainWindowTitle -eq 'Atha Reader Verification Complete') {
                break
            }
            if ($timer.ElapsedMilliseconds -gt 180000) { throw 'Large-book reader timed out.' }
        }
    }
    finally {
        if (-not $process.HasExited) {
            $process.Kill($true)
            $process.WaitForExit()
        }
    }
    $peakMiB = [Math]::Round($peakBytes / 1MB, 1)
    if ($samples -lt 5 -or $peakProcesses -lt 2 -or $peakMiB -le 0) {
        throw "WebView2 process-tree memory was not measurable: samples=$samples processes=$peakProcesses peakMiB=$peakMiB."
    }
    if ($peakMiB -gt $memoryLimitMiB) {
        throw "Large-book process-tree peak working set exceeded ${memoryLimitMiB}MiB: ${peakMiB}MiB."
    }
    return [pscustomobject]@{
        peak_mib = $peakMiB
        peak_processes = $peakProcesses
        samples = $samples
    }
}

function Test-CrashRecovery {
    $process = Start-ReaderHost @('--verify-sample', '--state-probe', 'write', '--hold-after-verify')
    $timer = [Diagnostics.Stopwatch]::StartNew()
    try {
        while (-not $process.HasExited -and $timer.ElapsedMilliseconds -lt 15000) {
            $process.Refresh()
            if ($process.MainWindowTitle -eq 'Atha Reader Verification Complete') { break }
            Start-Sleep -Milliseconds 100
        }
        if ($process.HasExited -or $process.MainWindowTitle -ne 'Atha Reader Verification Complete') {
            throw 'The write-probe host did not confirm durable probe state.'
        }
        $tree = Get-ProcessTreeSnapshot $process.Id
        if (-not $tree.root_present -or $tree.count -lt 2) {
            throw 'The crash probe did not observe the host and WebView2 process tree.'
        }
        $process.Kill($true)
        $process.WaitForExit()
        $timer.Restart()
        do {
            $survivors = @($tree.process_ids | ForEach-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
            if (-not $survivors) { break }
            Start-Sleep -Milliseconds 50
        } while ($timer.ElapsedMilliseconds -lt 10000)
        if ($survivors) {
            throw "The crash probe left process-tree survivors: $($survivors.Id -join ', ')."
        }
    }
    finally {
        if (-not $process.HasExited) {
            $process.Kill($true)
            $process.WaitForExit()
        }
    }
    Wait-ReaderHost (Start-ReaderHost @('--verify-sample', '--state-probe', 'read'))
}

function Test-LargeBookSearch {
    param([int]$Port)

    $session = "atha-r8-large-$PID"
    $startInfo = [Diagnostics.ProcessStartInfo]::new('pwsh')
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    foreach ($argument in @(
        '-NoProfile',
        '-File', (Join-Path $PSScriptRoot 'Serve-ReaderValidation.ps1'),
        '-BookRoot', $largeRoot,
        '-Port', [string]$Port
    )) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    $server = [Diagnostics.Process]::Start($startInfo)
    try {
        $ready = $false
        for ($attempt = 0; $attempt -lt 50; $attempt += 1) {
            try {
                $response = Invoke-WebRequest -UseBasicParsing "http://127.0.0.1:$Port/reader/atha-reader.html"
                if ($response.StatusCode -eq 200) {
                    $ready = $true
                    break
                }
            }
            catch { Start-Sleep -Milliseconds 100 }
        }
        if (-not $ready) { throw 'Large-book validation server did not become ready.' }
        $url = "http://127.0.0.1:$Port/reader/atha-reader.html?manifest=%2Fbook%2F.atha-reader.json&search-probe=1&state=math-history-r8"
        Invoke-AgentBrowser @('--session', $session, '--allowed-domains', '127.0.0.1', 'open', $url)
        Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "document.documentElement.dataset.status === 'pass' || Boolean(document.documentElement.dataset.error)")
        $pageState = Get-AgentBrowserScriptValue -Session $session -Script "({ status: document.documentElement.dataset.status, error: document.documentElement.dataset.error })" | ConvertFrom-Json
        if ($pageState.status -ne 'pass' -or $pageState.error) {
            throw "Large-book page failed: $($pageState | ConvertTo-Json -Compress)"
        }
        Invoke-AgentBrowser @('--session', $session, 'click', '.search > summary')
        Invoke-AgentBrowser @('--session', $session, 'fill', '#search-query', '数学')
        Invoke-AgentBrowser @('--session', $session, 'click', '#search-form button[type=submit]')
        Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "globalThis.__athaReaderDiagnostics.snapshot().search.status === 'complete'")
        $search = Get-AgentBrowserScriptValue -Session $session -Script 'globalThis.__athaReaderDiagnostics.snapshot().search' | ConvertFrom-Json
        if ($search.count -ne 288 -or @($search.sections).Count -ne 104 -or $search.truncated -or $search.lastError) {
            throw "Large-book search oracle changed: $($search | ConvertTo-Json -Compress -Depth 4)"
        }
    }
    finally {
        try { Invoke-AgentBrowser @('--session', $session, 'close') } catch { Write-Warning $_ }
        if ($server -and -not $server.HasExited) {
            $server.Kill($true)
            $server.WaitForExit()
        }
    }
}

Push-Location $repoRoot
try {
    Invoke-Checked 'pwsh' @('-NoProfile', '-File', 'scripts/check-reader-samples.ps1', '-BasePort', [string]$BasePort)
    Invoke-Checked 'python' @(
        'scripts/export_reader_sample.py',
        '--epub', $largeEpubPath,
        '--all-xhtml',
        '--entry', 'EPUB/text/ch012.xhtml',
        '--entry', 'EPUB/text/ch013.xhtml',
        '--entry', 'EPUB/text/ch014.xhtml',
        '--output', $largeOutputName
    ) -Quiet
    $manifest = Get-Content -LiteralPath (Join-Path $largeRoot $manifestName) -Raw -Encoding utf8 | ConvertFrom-Json
    $sourceHash = (Get-FileHash -LiteralPath $largeEpubPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($sourceHash -ne '0af5dff0c0d1eb369a096b18d05eb77a4cd9c03808748db8274d5e77bbfe7368' -or @($manifest.sections).Count -ne 173) {
        throw 'Large-book fixture identity changed.'
    }
    Test-LargeBookSearch ($BasePort + 10)
    $memoryRuns = @(1..3 | ForEach-Object { Measure-LargeBookHost })
    $memory = $memoryRuns | Sort-Object peak_mib -Descending | Select-Object -First 1
    Test-CrashRecovery
    Invoke-Checked 'pwsh' @(
        '-NoProfile',
        '-File', 'scripts/check-reader-slice.ps1',
        '-BookRoot', $largeRoot,
        '-Entry', 'EPUB/text/ch012.xhtml'
    )
    [pscustomobject]@{
        source_mib = [Math]::Round((Get-Item -LiteralPath $largeEpubPath).Length / 1MB, 2)
        sections = @($manifest.sections).Count
        resources = @($manifest.resources).Count
        peak_processes = $memory.peak_processes
        peak_working_set_mib = $memory.peak_mib
        memory_run_peaks_mib = [string]::Join(', ', @($memoryRuns.peak_mib))
        memory_samples = [string]::Join(', ', @($memoryRuns.samples))
        search = '288 results / 104 sections'
        crash_recovery = 'pass'
        performance = 'pass'
    } | Format-List
}
finally {
    Pop-Location
}
