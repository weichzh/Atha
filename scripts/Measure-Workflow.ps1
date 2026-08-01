# Description: Record and summarize local workflow timings.

[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateSet('Start', 'Begin', 'End', 'Finish', 'Report', 'SelfCheck')]
    [string]$Action,
    [string]$Task,
    [string]$RunId,
    [ValidateSet('context', 'specification', 'planning', 'review', 'implementation', 'validation', 'documentation')]
    [string]$Phase,
    [ValidateSet('success', 'failure', 'blocked', 'cancelled')]
    [string]$Status = 'success',
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$logRoot = Join-Path $repoRoot 'artifacts\local\workflow'
$script:logPath = Join-Path $logRoot 'events.jsonl'

function Assert-Token {
    param([string]$Value, [string]$Name, [int]$Maximum = 96)

    if ([string]::IsNullOrWhiteSpace($Value) -or
        $Value.Length -gt $Maximum -or
        $Value -notmatch '^[A-Za-z0-9][A-Za-z0-9._-]*$') {
        throw "$Name must be a stable ASCII identifier of at most $Maximum characters."
    }
}

function Read-Events {
    if (-not (Test-Path -LiteralPath $script:logPath -PathType Leaf)) { return @() }
    $events = @()
    foreach ($line in [IO.File]::ReadLines($script:logPath)) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        try { $events += $line | ConvertFrom-Json }
        catch { throw "Invalid workflow log line: $($_.Exception.Message)" }
    }
    return $events
}

function Write-Event {
    param([Collections.IDictionary]$Event)

    $directory = Split-Path -Parent $script:logPath
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
    $line = ($Event | ConvertTo-Json -Compress) + [Environment]::NewLine
    $bytes = [Text.UTF8Encoding]::new($false).GetBytes($line)
    # ponytail: one append-only log; shard by run only if writer contention becomes measurable.
    for ($attempt = 0; $attempt -lt 10; $attempt++) {
        try {
            $stream = [IO.File]::Open(
                $script:logPath,
                [IO.FileMode]::Append,
                [IO.FileAccess]::Write,
                [IO.FileShare]::Read
            )
            try {
                $stream.Write($bytes, 0, $bytes.Length)
                $stream.Flush($true)
            }
            finally { $stream.Dispose() }
            return
        }
        catch [IO.IOException] {
            if ($attempt -eq 9) { throw }
            Start-Sleep -Milliseconds 20
        }
    }
}

function New-Event {
    param(
        [string]$Event,
        [string]$EventRunId,
        [string]$EventTask,
        [string]$EventPhase,
        [string]$EventStatus,
        [Nullable[long]]$DurationMs
    )

    return [ordered]@{
        schema = 1
        timestamp_utc = [DateTimeOffset]::UtcNow.ToString('o')
        event = $Event
        run_id = $EventRunId
        task = $EventTask
        phase = $EventPhase
        status = $EventStatus
        duration_ms = $DurationMs
    }
}

function Get-RunEvents {
    param([string]$EventRunId)

    Assert-Token $EventRunId 'RunId'
    $events = @(Read-Events | Where-Object run_id -EQ $EventRunId)
    if (@($events | Where-Object event -EQ 'run_start').Count -ne 1) {
        throw "Unknown or invalid workflow run: $EventRunId"
    }
    return $events
}

function Start-Run {
    param([string]$EventTask)

    Assert-Token $EventTask 'Task' 64
    $id = '{0}-{1}-{2}' -f
        [DateTimeOffset]::UtcNow.ToString('yyyyMMddTHHmmssfffZ'),
        $EventTask.ToLowerInvariant(),
        [Guid]::NewGuid().ToString('N').Substring(0, 8)
    Write-Event (New-Event 'run_start' $id $EventTask $null $null $null)
    return $id
}

function Begin-Phase {
    param([string]$EventRunId, [string]$EventPhase)

    if ([string]::IsNullOrWhiteSpace($EventPhase)) { throw 'Phase is required.' }
    $events = @(Get-RunEvents $EventRunId)
    if (@($events | Where-Object event -EQ 'run_end').Count -gt 0) { throw 'Workflow run is finished.' }
    $starts = @($events | Where-Object { $_.event -eq 'phase_start' -and $_.phase -eq $EventPhase })
    $ends = @($events | Where-Object { $_.event -eq 'phase_end' -and $_.phase -eq $EventPhase })
    if ($starts.Count -ne $ends.Count) { throw "Phase is already active: $EventPhase" }
    $taskName = ($events | Where-Object event -EQ 'run_start')[0].task
    Write-Event (New-Event 'phase_start' $EventRunId $taskName $EventPhase $null $null)
}

function End-Phase {
    param([string]$EventRunId, [string]$EventPhase, [string]$EventStatus)

    if ([string]::IsNullOrWhiteSpace($EventPhase)) { throw 'Phase is required.' }
    $events = @(Get-RunEvents $EventRunId)
    $starts = @($events | Where-Object { $_.event -eq 'phase_start' -and $_.phase -eq $EventPhase })
    $ends = @($events | Where-Object { $_.event -eq 'phase_end' -and $_.phase -eq $EventPhase })
    if ($starts.Count -ne $ends.Count + 1) { throw "Phase is not active: $EventPhase" }
    $started = [DateTimeOffset]::Parse($starts[-1].timestamp_utc)
    $duration = [Math]::Max(0, [long]([DateTimeOffset]::UtcNow - $started).TotalMilliseconds)
    $taskName = ($events | Where-Object event -EQ 'run_start')[0].task
    Write-Event (New-Event 'phase_end' $EventRunId $taskName $EventPhase $EventStatus $duration)
    return $duration
}

function Finish-Run {
    param([string]$EventRunId, [string]$EventStatus)

    $events = @(Get-RunEvents $EventRunId)
    if (@($events | Where-Object event -EQ 'run_end').Count -gt 0) { throw 'Workflow run is already finished.' }
    foreach ($name in @($events | Where-Object event -EQ 'phase_start' | Select-Object -ExpandProperty phase -Unique)) {
        $starts = @($events | Where-Object { $_.event -eq 'phase_start' -and $_.phase -eq $name }).Count
        $ends = @($events | Where-Object { $_.event -eq 'phase_end' -and $_.phase -eq $name }).Count
        if ($starts -ne $ends) { throw "Workflow run has an active phase: $name" }
    }
    $start = ($events | Where-Object event -EQ 'run_start')[0]
    $duration = [Math]::Max(
        0,
        [long]([DateTimeOffset]::UtcNow - [DateTimeOffset]::Parse($start.timestamp_utc)).TotalMilliseconds
    )
    Write-Event (New-Event 'run_end' $EventRunId $start.task $null $EventStatus $duration)
    return $duration
}

function Get-Percentile {
    param([double[]]$Values, [double]$Percentile)

    $sorted = @($Values | Sort-Object)
    if ($sorted.Count -eq 0) { return 0 }
    return $sorted[[Math]::Ceiling($Percentile * $sorted.Count) - 1]
}

function Get-Median {
    param([double[]]$Values)

    $sorted = @($Values | Sort-Object)
    if ($sorted.Count -eq 0) { return 0 }
    $middle = [Math]::Floor($sorted.Count / 2)
    if ($sorted.Count % 2) { return $sorted[$middle] }
    return ($sorted[$middle - 1] + $sorted[$middle]) / 2
}

function Get-Report {
    $events = @(Read-Events)
    $completed = @($events | Where-Object event -EQ 'run_end')
    $finishedIds = @($completed | Select-Object -ExpandProperty run_id -Unique)
    $unfinished = @(
        $events |
            Where-Object event -EQ 'run_start' |
            Where-Object run_id -NotIn $finishedIds |
            ForEach-Object {
                [pscustomobject]@{
                    run_id = $_.run_id
                    task = $_.task
                    timestamp_utc = ([DateTimeOffset]$_.timestamp_utc).ToUniversalTime().ToString('o')
                }
            }
    )
    $measurements = @(
        $events | Where-Object event -EQ 'phase_end'
        $completed | ForEach-Object {
            [pscustomobject]@{
                task = $_.task
                phase = 'total'
                status = $_.status
                duration_ms = $_.duration_ms
            }
        }
    )
    $rows = @(
        $measurements |
            Group-Object task, phase |
            ForEach-Object {
                $values = @($_.Group | ForEach-Object { [double]$_.duration_ms })
                $median = Get-Median $values
                [pscustomobject]@{
                    task = $_.Group[0].task
                    phase = $_.Group[0].phase
                    samples = $values.Count
                    failures = @($_.Group | Where-Object status -NE 'success').Count
                    median_ms = [Math]::Round($median, 1)
                    p95_ms = [Math]::Round((Get-Percentile $values 0.95), 1)
                    slow_runs = if ($values.Count -ge 5) { @($values | Where-Object { $_ -gt 2 * $median }).Count } else { 0 }
                }
            } |
            Sort-Object task, phase
    )
    return [ordered]@{
        completed_runs = $completed.Count
        unfinished_runs = $unfinished
        metrics = $rows
    }
}

function Invoke-SelfCheck {
    $testRoot = Join-Path $repoRoot ".tmp\workflow-log-selfcheck-$PID"
    $testLog = Join-Path $testRoot 'events.jsonl'
    New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
    $originalLog = $script:logPath
    $script:logPath = $testLog
    try {
        $testRun = Start-Run 'self-check'
        Begin-Phase $testRun 'validation'
        $phaseDuration = End-Phase $testRun 'validation' 'success'
        $runDuration = Finish-Run $testRun 'success'
        $unfinishedRun = Start-Run 'self-check'
        Begin-Phase $unfinishedRun 'validation'
        End-Phase $unfinishedRun 'validation' 'failure' | Out-Null
        $events = @(Read-Events)
        $report = Get-Report
        if ($events.Count -ne 7 -or $phaseDuration -lt 0 -or $runDuration -lt 0) {
            throw 'Workflow event self-check failed.'
        }
        $validation = @($report.metrics | Where-Object phase -EQ 'validation')
        if ($report.completed_runs -ne 1 -or
            $report.metrics.Count -ne 2 -or
            $report.unfinished_runs.Count -ne 1 -or
            $validation.Count -ne 1 -or
            $validation[0].failures -ne 1) {
            throw 'Workflow report self-check failed.'
        }
        Write-Output 'workflow_log: ok'
    }
    finally {
        $script:logPath = $originalLog
        if (Test-Path -LiteralPath $testLog) { Remove-Item -LiteralPath $testLog -Force }
        if (Test-Path -LiteralPath $testRoot) { Remove-Item -LiteralPath $testRoot -Force }
    }
}

switch ($Action) {
    'Start' {
        if ([string]::IsNullOrWhiteSpace($Task)) { throw 'Task is required.' }
        Start-Run $Task
    }
    'Begin' {
        Begin-Phase $RunId $Phase
    }
    'End' {
        End-Phase $RunId $Phase $Status
    }
    'Finish' {
        Finish-Run $RunId $Status
    }
    'Report' {
        $report = Get-Report
        if ($Json) {
            $report | ConvertTo-Json -Depth 5
        }
        else {
            Write-Host "Completed runs: $($report.completed_runs)"
            Write-Host "Unfinished runs: $($report.unfinished_runs.Count)"
            if ($report.metrics.Count) { $report.metrics | Format-Table -AutoSize }
            if ($report.unfinished_runs.Count) {
                Write-Host 'Unfinished:'
                $report.unfinished_runs | Format-Table -AutoSize
            }
        }
    }
    'SelfCheck' {
        Invoke-SelfCheck
    }
}
