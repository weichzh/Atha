# Description: Run tracked Atha engineering commands.

[CmdletBinding()]
param(
    [Parameter(Mandatory, Position = 0)]
    [ValidateSet('check', 'station', 'report')]
    [string]$Command,
    [Parameter(Position = 1)]
    [string]$Target,
    [ValidateSet('none', 'research', 'specification', 'planning', 'implementation', 'validation', 'documentation', 'review', 'waiting')]
    [string]$Activity,
    [string]$Scope,
    [ValidateSet('success', 'failure', 'blocked', 'cancelled')]
    [string]$Outcome = 'success',
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$measure = Join-Path $PSScriptRoot 'Measure-Workflow.ps1'

function Assert-Token {
    param([string]$Value, [string]$Name, [int]$Maximum = 80)

    if ([string]::IsNullOrWhiteSpace($Value) -or
        $Value.Length -gt $Maximum -or
        $Value -notmatch '^[A-Za-z0-9][A-Za-z0-9._-]*$') {
        throw "$Name must be a stable ASCII identifier of at most $Maximum characters."
    }
}

function Start-TrackedRun {
    param([string]$Task, [string]$TrackedCommand, [string]$TrackedTarget)

    $arguments = @{
        Action = 'Start'
        Task = $Task
        WorkflowCommand = $TrackedCommand
        Activity = $Activity
    }
    if ($TrackedTarget) { $arguments.Target = $TrackedTarget }
    if ($Scope) { $arguments.Scope = $Scope }
    return & $measure @arguments
}

if ($Scope) { Assert-Token $Scope 'Scope' }

switch ($Command) {
    'report' {
        if ($Target -or $Activity -or $Scope -or $Outcome -ne 'success') {
            throw 'report only accepts -Json.'
        }
        & $measure -Action Report -Json:$Json
        exit 0
    }
    'station' {
        if ($Target) { throw 'station does not accept a target.' }
        if ($Json) { throw 'station does not accept -Json.' }
        if (-not $Activity) { throw 'station requires -Activity.' }
        $runId = Start-TrackedRun 'station' 'station' $null
        & $measure -Action Finish -RunId $runId -Status $Outcome -ExitCode 0 | Out-Null
        Write-Output "station: $Outcome"
        exit 0
    }
    'check' {
        if (-not $Activity) { throw 'check requires -Activity.' }
        if (-not $Target) { throw 'check requires a target.' }
        if ($Json) { throw 'check does not accept -Json.' }
        if ($Outcome -ne 'success') { throw 'check derives its outcome and does not accept -Outcome.' }
        if ($Target -ne 'docs') { throw "Unsupported check target: $Target" }
    }
}

$runId = Start-TrackedRun "check.$Target" 'check' $Target
& $measure -Action Begin -RunId $runId -Phase validation
$exitCode = 0
$errorType = $null
$recordError = $null
try {
    foreach ($script in @('scripts/doc_guard.py', 'scripts/doc_length_check.py')) {
        & python3 (Join-Path $repoRoot $script)
        if ($LASTEXITCODE -ne 0) {
            $exitCode = $LASTEXITCODE
            $errorType = 'nonzero_exit'
            break
        }
    }
}
catch {
    $exitCode = 1
    $errorType = 'command_error'
    Write-Error $_ -ErrorAction Continue
}
finally {
    $status = if ($exitCode -eq 0) { 'success' } else { 'failure' }
    try {
        & $measure -Action End -RunId $runId -Phase validation -Status $status | Out-Null
        & $measure -Action Finish -RunId $runId -Status $status -ExitCode $exitCode -ErrorType $errorType | Out-Null
    }
    catch { $recordError = $_ }
}

if ($recordError) {
    Write-Error "Unable to finish workflow log: $recordError"
    exit 1
}
exit $exitCode
