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

Push-Location $repoRoot
try {
    Invoke-Checked $env:ATHA_CARGO @('fmt', '--all', '--check')
    Invoke-Checked $env:ATHA_CARGO @('test', '-p', 'atha-backend', '--test', 'message_reading')
    Push-Location 'reader/app'
    try {
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
