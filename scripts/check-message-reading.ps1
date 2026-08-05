# Description: Verify message-reading contracts and the compiled Tauri frontend.

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot

function Invoke-Checked {
    param([string]$FilePath, [string[]]$Arguments)

    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$FilePath failed with exit code $LASTEXITCODE." }
}

function Assert-MessageCommandsEnabled {
    $capability = Get-Content -LiteralPath 'reader/app/src-tauri/capabilities/main.json' -Raw | ConvertFrom-Json
    if ($capability.permissions -notcontains 'allow-message-commands') {
        throw 'Tauri main capability does not enable allow-message-commands.'
    }

    $permissions = Get-Content -LiteralPath 'reader/app/src-tauri/permissions/reader.toml' -Raw
    $tauriCommands = Get-Content -LiteralPath 'reader/app/src-tauri/src/lib.rs' -Raw
    $messageCommands = [regex]::Matches($tauriCommands, '(?m)^\s+(message_[a-z_]+),$') |
        ForEach-Object { $_.Groups[1].Value }
    if ($messageCommands.Count -eq 0) { throw 'No registered Tauri message commands found.' }
    foreach ($command in $messageCommands) {
        if (-not $permissions.Contains(('"{0}"' -f $command))) {
            throw "Tauri permission does not allow $command."
        }
    }
}

Push-Location $repoRoot
try {
    Assert-MessageCommandsEnabled
    Invoke-Checked $env:ATHA_CARGO @('fmt', '--all', '--check')
    Invoke-Checked $env:ATHA_CARGO @('test', '-p', 'atha-backend', '--test', 'message_reading')
    Invoke-Checked $env:ATHA_NODE @('reader/web/conversations.test.mjs')
    Push-Location 'reader/app'
    try {
        Invoke-Checked $env:ATHA_PNPM @('test:markdown')
        Invoke-Checked $env:ATHA_PNPM @('check')
        Invoke-Checked $env:ATHA_PNPM @('build')
    }
    finally {
        Pop-Location
    }
    Invoke-Checked $env:ATHA_CARGO @('test', '-p', 'atha-reader-host', '-p', 'atha-reader-app')
}
finally {
    Pop-Location
}
