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

function Get-TauriPermissionCommands {
    param([string]$Toml, [string]$Identifier)

    $blocks = @(
        [regex]::Matches($Toml, '(?ms)^\[\[permission\]\]\s*(.*?)(?=^\[\[permission\]\]|\z)') |
            ForEach-Object { $_.Groups[1].Value }
    )
    $identifierPattern = '(?m)^\s*identifier\s*=\s*"{0}"\s*$' -f [regex]::Escape($Identifier)
    $selectedBlocks = @($blocks | Where-Object { $_ -match $identifierPattern })
    if ($selectedBlocks.Count -ne 1) {
        throw "Expected exactly one Tauri permission block named $Identifier."
    }

    $allowList = [regex]::Match($selectedBlocks[0], '(?ms)^\s*commands\.allow\s*=\s*\[(.*?)\]')
    if (-not $allowList.Success) { throw "Tauri permission $Identifier has no commands.allow list." }
    @(
        [regex]::Matches($allowList.Groups[1].Value, '"([^"]+)"') |
            ForEach-Object { $_.Groups[1].Value }
    )
}

function Assert-MessageCommandsEnabled {
    $capability = Get-Content -LiteralPath 'reader/app/src-tauri/capabilities/main.json' -Raw | ConvertFrom-Json
    if ($capability.permissions -notcontains 'allow-message-commands') {
        throw 'Tauri main capability does not enable allow-message-commands.'
    }

    $parserProbe = @'
[[permission]]
identifier = "allow-message-commands"
commands.allow = ["message_roots"]

[[permission]]
identifier = "allow-other"
commands.allow = ["message_export"]
'@
    $parserProbeCommands = @(Get-TauriPermissionCommands -Toml $parserProbe -Identifier 'allow-message-commands')
    if ($parserProbeCommands.Count -ne 1 -or $parserProbeCommands[0] -ne 'message_roots') {
        throw 'Tauri permission parser crossed a permission block boundary.'
    }

    $permissions = Get-Content -LiteralPath 'reader/app/src-tauri/permissions/reader.toml' -Raw
    $tauriCommands = Get-Content -LiteralPath 'reader/app/src-tauri/src/lib.rs' -Raw
    $messageCommands = @(
        [regex]::Matches($tauriCommands, '(?m)^\s+(?:message_commands::)?(message_[a-z_]+),?$') |
            ForEach-Object { $_.Groups[1].Value }
    )
    $allowedCommands = @(Get-TauriPermissionCommands -Toml $permissions -Identifier 'allow-message-commands')
    if ($messageCommands.Count -eq 0) { throw 'No registered Tauri message commands found.' }
    if (($messageCommands | Sort-Object -Unique).Count -ne $messageCommands.Count) {
        throw 'Tauri message command registration contains duplicates.'
    }
    if (($allowedCommands | Sort-Object -Unique).Count -ne $allowedCommands.Count) {
        throw 'Tauri message command permission contains duplicates.'
    }
    $drift = @(Compare-Object ($messageCommands | Sort-Object) ($allowedCommands | Sort-Object))
    if ($drift.Count -gt 0) {
        $details = ($drift | ForEach-Object { "{0}{1}" -f $_.InputObject, $_.SideIndicator }) -join ', '
        throw "Tauri message command registration and permission differ: $details"
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
