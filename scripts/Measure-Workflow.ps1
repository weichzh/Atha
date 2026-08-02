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
    [ValidateSet('check', 'station')]
    [string]$WorkflowCommand,
    [string]$Target,
    [ValidateSet('none', 'research', 'specification', 'planning', 'implementation', 'validation', 'documentation', 'review', 'waiting')]
    [string]$Activity,
    [string]$Scope,
    [Nullable[int]]$ExitCode,
    [string]$ErrorType,
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

function Assert-OptionalToken {
    param([string]$Value, [string]$Name, [int]$Maximum = 96)

    if (-not [string]::IsNullOrWhiteSpace($Value)) {
        Assert-Token $Value $Name $Maximum
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
        [Nullable[long]]$DurationMs,
        [Collections.IDictionary]$Metadata
    )

    $record = [ordered]@{
        schema = 2
        timestamp_utc = [DateTimeOffset]::UtcNow.ToString('o')
        event = $Event
        run_id = $EventRunId
        task = $EventTask
        phase = if ([string]::IsNullOrWhiteSpace($EventPhase)) { $null } else { $EventPhase }
        status = if ([string]::IsNullOrWhiteSpace($EventStatus)) { $null } else { $EventStatus }
        duration_ms = $DurationMs
    }
    if ($null -ne $Metadata) {
        foreach ($key in $Metadata.Keys) { $record[$key] = $Metadata[$key] }
    }
    return $record
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
    param(
        [string]$EventTask,
        [string]$EventCommand,
        [string]$EventTarget,
        [string]$EventActivity,
        [string]$EventScope
    )

    Assert-Token $EventTask 'Task' 64
    Assert-OptionalToken $EventTarget 'Target' 64
    Assert-OptionalToken $EventScope 'Scope' 80
    $id = '{0}-{1}-{2}' -f
        [DateTimeOffset]::UtcNow.ToString('yyyyMMddTHHmmssfffZ'),
        $EventTask.ToLowerInvariant(),
        [Guid]::NewGuid().ToString('N').Substring(0, 8)
    $previous = @(
        Read-Events |
            Where-Object event -EQ 'run_end' |
            Sort-Object { [DateTimeOffset]$_.timestamp_utc } |
            Select-Object -Last 1
    )
    $interval = if ($previous.Count) {
        [Math]::Max(
            0,
            [long]([DateTimeOffset]::UtcNow - [DateTimeOffset]$previous[0].timestamp_utc).TotalMilliseconds
        )
    } else { $null }
    $metadata = [ordered]@{
        command = if ($EventCommand) { $EventCommand } else { $null }
        target = if ($EventTarget) { $EventTarget } else { $null }
        activity = if ($EventActivity) { $EventActivity } else { $null }
        scope = if ($EventScope) { $EventScope } else { $null }
        exit_code = $null
        error_type = $null
        previous_run_id = if ($previous.Count) { $previous[0].run_id } else { $null }
        interval_ms = $interval
    }
    Write-Event (New-Event 'run_start' $id $EventTask $null $null $null $metadata)
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
    param(
        [string]$EventRunId,
        [string]$EventStatus,
        [Nullable[int]]$EventExitCode,
        [string]$EventErrorType
    )

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
    $metadata = [ordered]@{
        command = $start.command
        target = $start.target
        activity = $start.activity
        scope = $start.scope
        exit_code = $EventExitCode
        error_type = if ($EventErrorType) { $EventErrorType } else { $null }
        previous_run_id = $start.previous_run_id
        interval_ms = $start.interval_ms
    }
    Write-Event (New-Event 'run_end' $EventRunId $start.task $null $EventStatus $duration $metadata)
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
                    command = $_.command
                    target = $_.target
                    activity = $_.activity
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
                timestamp_utc = $_.timestamp_utc
            }
        }
    )
    $rows = @(
        $measurements |
            Group-Object task, phase |
            ForEach-Object {
                $values = @($_.Group | ForEach-Object { [double]$_.duration_ms })
                $median = Get-Median $values
                $failures = @($_.Group | Where-Object status -NE 'success')
                [pscustomobject]@{
                    task = $_.Group[0].task
                    phase = $_.Group[0].phase
                    samples = $values.Count
                    failures = $failures.Count
                    median_ms = [Math]::Round($median, 1)
                    p95_ms = if ($values.Count -ge 5) { [Math]::Round((Get-Percentile $values 0.95), 1) } else { $null }
                    slow_runs = if ($values.Count -ge 5) { @($values | Where-Object { $_ -gt 2 * $median }).Count } else { $null }
                    recent_failure_utc = if ($failures.Count) {
                        @($failures | Sort-Object { [DateTimeOffset]$_.timestamp_utc } | Select-Object -Last 1)[0].timestamp_utc
                    } else { $null }
                }
            } |
            Sort-Object task, phase
    )
    $repeatedFailures = @(
        $completed |
            Group-Object task |
            ForEach-Object {
                $previousFailed = $false
                $occurrences = 0
                foreach ($event in @($_.Group | Sort-Object { [DateTimeOffset]$_.timestamp_utc })) {
                    $failed = $event.status -eq 'failure'
                    if ($failed -and $previousFailed) { $occurrences++ }
                    $previousFailed = $failed
                }
                if ($occurrences) {
                    [pscustomobject]@{ task = $_.Name; occurrences = $occurrences }
                }
            }
    )
    return [ordered]@{
        completed_runs = $completed.Count
        unfinished_runs = $unfinished
        metrics = $rows
        friction = [ordered]@{
            unfinished_runs = $unfinished.Count
            blocked_runs = @($completed | Where-Object status -EQ 'blocked').Count
            repeated_failures = $repeatedFailures
        }
    }
}

function Invoke-SelfCheck {
    $testRoot = Join-Path $repoRoot ".tmp\workflow-log-selfcheck-$PID"
    $testLog = Join-Path $testRoot 'events.jsonl'
    New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
    $originalLog = $script:logPath
    $script:logPath = $testLog
    try {
        $testRun = Start-Run 'self-check' 'check' 'docs' 'validation' 'self-check'
        Begin-Phase $testRun 'validation'
        $phaseDuration = End-Phase $testRun 'validation' 'success'
        $runDuration = Finish-Run $testRun 'success' 0 $null
        $unfinishedRun = Start-Run 'self-check-pending' 'check' 'docs' 'validation' $null
        Begin-Phase $unfinishedRun 'validation'
        End-Phase $unfinishedRun 'validation' 'failure' | Out-Null
        foreach ($unused in 1..2) {
            $failedRun = Start-Run 'self-check-failure' 'check' 'docs' 'validation' $null
            Finish-Run $failedRun 'failure' 7 'nonzero_exit' | Out-Null
        }
        $blockedRun = Start-Run 'self-check-station' 'station' $null 'waiting' $null
        Finish-Run $blockedRun 'blocked' 0 $null | Out-Null
        $events = @(Read-Events)
        $report = Get-Report
        if ($events.Count -ne 13 -or $phaseDuration -lt 0 -or $runDuration -lt 0) {
            throw 'Workflow event self-check failed.'
        }
        $validation = @($report.metrics | Where-Object phase -EQ 'validation')
        $failedMetric = @($report.metrics | Where-Object task -EQ 'self-check-failure')
        if ($report.completed_runs -ne 4 -or
            $report.metrics.Count -ne 5 -or
            $report.unfinished_runs.Count -ne 1 -or
            $validation.Count -ne 2 -or
            $failedMetric.Count -ne 1 -or
            $failedMetric[0].failures -ne 2 -or
            $null -ne $failedMetric[0].p95_ms -or
            $report.friction.blocked_runs -ne 1 -or
            $report.friction.repeated_failures.Count -ne 1) {
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
        Start-Run $Task $WorkflowCommand $Target $Activity $Scope
    }
    'Begin' {
        Begin-Phase $RunId $Phase
    }
    'End' {
        End-Phase $RunId $Phase $Status
    }
    'Finish' {
        Assert-OptionalToken $ErrorType 'ErrorType' 80
        Finish-Run $RunId $Status $ExitCode $ErrorType
    }
    'Report' {
        $report = Get-Report
        if ($Json) {
            $report | ConvertTo-Json -Depth 5
        }
        else {
            Write-Host "Completed runs: $($report.completed_runs)"
            Write-Host "Unfinished runs: $($report.unfinished_runs.Count)"
            Write-Host "Blocked runs: $($report.friction.blocked_runs)"
            if ($report.metrics.Count) { $report.metrics | Format-Table -AutoSize }
            if ($report.friction.repeated_failures.Count) {
                Write-Host 'Repeated failures:'
                $report.friction.repeated_failures | Format-Table -AutoSize
            }
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
