# Description: Verify Markdown and opt-in private TXT imports without exposing source details.

[CmdletBinding()]
param(
    [switch]$IncludePrivateTxt,
    [string[]]$PreparedMarkdownBookRoot = @(),
    [string]$PreparedPrivateTxtBookRoot,
    [switch]$BackendOnly
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$readerSlice = Join-Path $PSScriptRoot 'check-reader-slice.ps1'
$validationLocalAppData = Join-Path $repoRoot ".tmp/text-source-host-$PID"
$privateTxtEnvironmentName = 'ATHA_LOCAL_TXT_SAMPLE'
$privateTxtTest = 'imports_private_local_txt_sample'
$markdownSources = @(
    (Join-Path $repoRoot 'README.md'),
    (Join-Path $repoRoot 'docs/architecture/READER-CORE.md')
)

if ($BackendOnly -and ($PreparedMarkdownBookRoot.Count -gt 0 -or -not [string]::IsNullOrWhiteSpace($PreparedPrivateTxtBookRoot))) {
    throw '-BackendOnly cannot be combined with prepared BookRoot inputs.'
}
if (-not $IncludePrivateTxt -and -not [string]::IsNullOrWhiteSpace($PreparedPrivateTxtBookRoot)) {
    throw '-PreparedPrivateTxtBookRoot requires -IncludePrivateTxt.'
}
if (-not $BackendOnly -and $PreparedMarkdownBookRoot.Count -eq 0) {
    throw 'Windows host verification requires at least one prepared Markdown BookRoot; use -BackendOnly only for the importer seam.'
}
if ($PreparedMarkdownBookRoot.Count -gt 2) {
    throw 'At most two prepared Markdown BookRoot inputs are accepted by this gate.'
}
if ($IncludePrivateTxt -and -not $BackendOnly -and [string]::IsNullOrWhiteSpace($PreparedPrivateTxtBookRoot)) {
    throw 'Private TXT host verification requires a prepared private TXT BookRoot.'
}
foreach ($source in $markdownSources) {
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw 'A required repository Markdown source is missing.'
    }
}
if (-not (Test-Path -LiteralPath $readerSlice -PathType Leaf)) {
    throw 'The Windows reader slice gate is missing.'
}
$temporaryRoot = [IO.Path]::GetFullPath((Join-Path $repoRoot '.tmp')).TrimEnd('\') + '\'
$validationLocalAppData = [IO.Path]::GetFullPath($validationLocalAppData)
if (-not $validationLocalAppData.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Refusing to use a Windows host profile outside the repository .tmp directory.'
}

$privateTxtPath = [Environment]::GetEnvironmentVariable($privateTxtEnvironmentName, 'Process')
if ($IncludePrivateTxt -and (
        [string]::IsNullOrWhiteSpace($privateTxtPath) -or
        -not (Test-Path -LiteralPath $privateTxtPath -PathType Leaf)
    )) {
    throw 'The opt-in private TXT environment variable must name an existing local file.'
}

. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot
$cargoPath = $env:ATHA_CARGO

function Invoke-CheckedCargo {
    param(
        [Parameter(Mandatory)][string[]]$Arguments,
        [Parameter(Mandatory)][string]$Failure
    )

    & $cargoPath @Arguments
    if ($LASTEXITCODE -ne 0) { throw $Failure }
}

function Invoke-PrivateCheckedCargo {
    param(
        [Parameter(Mandatory)][string[]]$Arguments,
        [Parameter(Mandatory)][string]$Failure
    )

    $startInfo = [Diagnostics.ProcessStartInfo]::new($cargoPath)
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in $Arguments) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::Start($startInfo)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(600000)) {
        $process.Kill($true)
        $process.WaitForExit()
        [void]$stdout.GetAwaiter().GetResult()
        [void]$stderr.GetAwaiter().GetResult()
        throw $Failure
    }
    [void]$stdout.GetAwaiter().GetResult()
    [void]$stderr.GetAwaiter().GetResult()
    if ($process.ExitCode -ne 0) { throw $Failure }
}

function Read-PreparedManifest {
    param(
        [Parameter(Mandatory)][string]$BookRoot,
        [switch]$PrivateTxt
    )

    try {
        $resolvedRoot = (Resolve-Path -LiteralPath $BookRoot -ErrorAction Stop).Path
    }
    catch {
        throw 'A prepared BookRoot is unavailable.'
    }
    $manifestPath = Join-Path $resolvedRoot '.atha-reader.json'
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw 'A prepared BookRoot does not contain .atha-reader.json.'
    }
    try {
        $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    }
    catch {
        throw 'A prepared reader manifest is not valid JSON.'
    }
    $sectionCount = @($manifest.sections).Count
    $tocCount = @($manifest.toc).Count
    if ($manifest.schema -ne 1 -or $sectionCount -lt 1 -or $sectionCount -gt 1000 -or $tocCount -gt 2000) {
        throw 'A prepared reader manifest is outside the schema 1 text boundary.'
    }
    if ($PrivateTxt -and (($sectionCount -lt 2 -or $sectionCount -gt 16) -or $tocCount -ne 1134)) {
        throw 'The private TXT manifest does not match the accepted aggregate section and TOC counts.'
    }
    [pscustomobject]@{
        Root = $resolvedRoot
        Sections = $sectionCount
        TocItems = $tocCount
    }
}

function Invoke-PreparedReaderHost {
    param([Parameter(Mandatory)][string]$BookRoot)

    if (Test-Path -LiteralPath $validationLocalAppData) {
        Remove-Item -LiteralPath $validationLocalAppData -Recurse -Force
    }
    [void](New-Item -ItemType Directory -Path $validationLocalAppData)
    $originalLocalAppData = [Environment]::GetEnvironmentVariable('LOCALAPPDATA', 'Process')
    try {
        [Environment]::SetEnvironmentVariable('LOCALAPPDATA', $validationLocalAppData, 'Process')
        & $readerSlice -BookRoot $BookRoot -Manifest '.atha-reader.json' -BenchmarkOnly -VerifyImportOnly
        if ($LASTEXITCODE -ne 0) {
            throw 'The Windows WebView2 reader host gate failed.'
        }
    }
    finally {
        [Environment]::SetEnvironmentVariable('LOCALAPPDATA', $originalLocalAppData, 'Process')
    }
}

$originalPrivateTxtPath = $privateTxtPath
$privateTxtResult = 'not-requested'
$hostResults = @()
Push-Location $repoRoot
try {
    [Environment]::SetEnvironmentVariable($privateTxtEnvironmentName, $null, 'Process')
    Invoke-CheckedCargo @(
        'test',
        '--locked',
        '-p',
        'atha-backend',
        '--test',
        'text_import'
    ) 'Markdown and TXT backend integration tests failed.'

    if ($IncludePrivateTxt) {
        [Environment]::SetEnvironmentVariable($privateTxtEnvironmentName, $originalPrivateTxtPath, 'Process')
        Invoke-PrivateCheckedCargo @(
            'test',
            '--locked',
            '-p',
            'atha-backend',
            '--test',
            'text_import',
            $privateTxtTest,
            '--',
            '--ignored',
            '--exact'
        ) 'The opt-in private TXT backend verification failed; private test output was suppressed.'
        $privateTxtResult = 'passed'
    }

    if (-not $BackendOnly) {
        foreach ($bookRoot in $PreparedMarkdownBookRoot) {
            $prepared = Read-PreparedManifest -BookRoot $bookRoot
            Invoke-PreparedReaderHost -BookRoot $prepared.Root
            $hostResults += [pscustomobject]@{
                format = 'markdown'
                sections = $prepared.Sections
                toc_items = $prepared.TocItems
                result = 'passed'
            }
        }
        if ($IncludePrivateTxt) {
            $prepared = Read-PreparedManifest -BookRoot $PreparedPrivateTxtBookRoot -PrivateTxt
            Invoke-PreparedReaderHost -BookRoot $prepared.Root
            $hostResults += [pscustomobject]@{
                format = 'txt'
                sections = $prepared.Sections
                toc_items = $prepared.TocItems
                result = 'passed'
            }
        }
    }
}
finally {
    [Environment]::SetEnvironmentVariable($privateTxtEnvironmentName, $originalPrivateTxtPath, 'Process')
    Pop-Location
    if (Test-Path -LiteralPath $validationLocalAppData) {
        Remove-Item -LiteralPath $validationLocalAppData -Recurse -Force
    }
}

[pscustomobject]@{
    repository_markdown_sources = $markdownSources.Count
    backend_text_import = 'passed'
    private_txt = $privateTxtResult
    windows_webview2_host = if ($BackendOnly) { 'not-run-explicit-backend-only' } else { 'passed' }
    host_roots = $hostResults.Count
} | Format-List
$hostResults | Format-Table -AutoSize
