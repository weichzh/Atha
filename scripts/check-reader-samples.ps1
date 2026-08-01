# Description: Validate every configured local reader sample in Windows host and light/dark browser modes.

[CmdletBinding()]
param(
    [string]$Manifest = 'reader/samples.json',
    [ValidateRange(1024, 65530)]
    [int]$BasePort = 18766
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$manifestPath = (Resolve-Path (Join-Path $repoRoot $Manifest)).Path
$samples = @(Get-Content -LiteralPath $manifestPath -Raw -Encoding utf8 | ConvertFrom-Json)
if ($samples.Count -eq 0) { throw 'Reader sample manifest is empty.' }

. (Join-Path $PSScriptRoot 'Import-AthaEnvironment.ps1') -RepoRoot $repoRoot
$cargoPath = $env:ATHA_CARGO
$hostPath = Join-Path $repoRoot 'target/debug/atha-reader-host.exe'
$screenshots = Join-Path $repoRoot 'artifacts/local/screenshots'
$serverScript = Join-Path $PSScriptRoot 'Serve-ReaderValidation.ps1'

function Invoke-Checked {
    param(
        [string]$FilePath,
        [string[]]$Arguments
    )

    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$FilePath failed with exit code $LASTEXITCODE."
    }
}

function Invoke-AgentBrowser {
    param([string[]]$Arguments)

    & agent-browser @Arguments
    if ($LASTEXITCODE -ne 0) { throw "agent-browser failed with exit code $LASTEXITCODE." }
}

function Invoke-ReaderHost {
    param(
        [string]$BookRoot,
        [string]$Entry
    )

    $startInfo = [Diagnostics.ProcessStartInfo]::new($hostPath)
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    foreach ($argument in @('--book-root', $BookRoot, '--entry', $Entry, '--verify-sample')) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::Start($startInfo)
    if (-not $process.WaitForExit(60000)) {
        $process.Kill($true)
        $process.WaitForExit()
        throw 'Reader host timed out.'
    }
    if ($process.ExitCode -ne 0) {
        throw "Reader host failed with exit code $($process.ExitCode)."
    }
}

function Start-ValidationServer {
    param(
        [string]$BookRoot,
        [int]$Port
    )

    $startInfo = [Diagnostics.ProcessStartInfo]::new('pwsh')
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    foreach ($argument in @('-NoProfile', '-File', $serverScript, '-BookRoot', $BookRoot, '-Port', [string]$Port)) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::Start($startInfo)
    for ($attempt = 0; $attempt -lt 50; $attempt++) {
        if ($process.HasExited) {
            throw "Reader validation server exited with code $($process.ExitCode)."
        }
        try {
            $response = Invoke-WebRequest -UseBasicParsing "http://127.0.0.1:$Port/health" -TimeoutSec 1
            if ($response.StatusCode -eq 200) { return $process }
        }
        catch {
            Start-Sleep -Milliseconds 100
        }
    }
    $process.Kill($true)
    $process.WaitForExit()
    throw 'Reader validation server did not become ready.'
}

$themeProbe = @'
(() => {
  const expectedDark = __EXPECTED_DARK__;
  const requireFormulas = __REQUIRE_FORMULAS__;
  const expectedOrdinaryImages = __EXPECTED_ORDINARY_IMAGES__;
  const requireCodeBlock = __REQUIRE_CODE_BLOCK__;
  const rgb = (value) => (value.match(/[\d.]+/g) || []).slice(0, 3).map(Number);
  const luminance = (value) => {
    const channels = rgb(value).map((channel) => {
      const normalized = channel / 255;
      return normalized <= 0.04045 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
    });
    return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
  };
  const contrast = (foreground, background) => {
    const values = [luminance(foreground), luminance(background)].sort((a, b) => b - a);
    return (values[0] + 0.05) / (values[1] + 0.05);
  };
  const reader = document.querySelector('.reader');
  const formulas = [...book.querySelectorAll('img.math-inline, img.math-display')];
  const ordinary = [...book.querySelectorAll('img:not(.math-inline):not(.math-display)')];
  const ordinaryPngCount = ordinary.filter((image) => new URL(image.src).pathname.toLowerCase().endsWith('.png')).length;
  const foreground = getComputedStyle(book).color;
  const background = getComputedStyle(reader).backgroundColor;
  const result = {
    status: document.documentElement.dataset.status || null,
    error: document.documentElement.dataset.error || null,
    dark: matchMedia('(prefers-color-scheme: dark)').matches,
    pages: state.pages,
    formulaCount: formulas.length,
    ordinaryCount: ordinary.length,
    ordinaryPngCount,
    codeBlockCount: book.querySelectorAll('pre code').length,
    contrast: contrast(foreground, background),
    formulaFilters: [...new Set(formulas.map((image) => getComputedStyle(image).filter))],
    ordinaryFilters: [...new Set(ordinary.map((image) => getComputedStyle(image).filter))],
  };
  if (result.status !== 'pass' || result.error) throw new Error('reader-status');
  if (requireFormulas && result.formulaCount < 1) throw new Error('no-formulas');
  if (!requireFormulas && result.formulaCount !== 0) throw new Error('unexpected-formulas');
  if (result.ordinaryCount !== expectedOrdinaryImages || result.ordinaryPngCount !== expectedOrdinaryImages) throw new Error('ordinary-image-count');
  if (requireCodeBlock && result.codeBlockCount < 1) throw new Error('missing-code-block');
  if (result.contrast < 4.5) throw new Error('contrast');
  if (result.dark !== expectedDark) throw new Error('theme-media');
  if (expectedDark && result.formulaFilters.includes('none')) throw new Error('formula-dark-filter');
  if (!expectedDark && result.formulaFilters.some((filter) => filter !== 'none')) throw new Error('formula-light-filter');
  if (result.ordinaryFilters.some((filter) => filter !== 'none')) throw new Error('ordinary-image-filter');
  return result;
})()
'@

Push-Location $repoRoot
try {
    if (-not (Get-Command agent-browser -ErrorAction SilentlyContinue)) {
        throw 'agent-browser is not available.'
    }
    Invoke-Checked 'python3' @('scripts/export_reader_sample.py', '--self-check')
    Invoke-Checked 'node' @('--check', 'reader/atha-reader.js')
    Invoke-Checked $cargoPath @('fmt', '--manifest-path', 'Cargo.toml', '--all', '--check')
    Invoke-Checked $cargoPath @('clippy', '--manifest-path', 'Cargo.toml', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings')
    Invoke-Checked $cargoPath @('test', '--manifest-path', 'Cargo.toml', '--workspace', '--all-targets', '--locked')
    Invoke-Checked $cargoPath @('build', '--manifest-path', 'Cargo.toml', '--package', 'atha-reader-host', '--locked')
    New-Item -ItemType Directory -Path $screenshots -Force | Out-Null

    for ($index = 0; $index -lt $samples.Count; $index++) {
        $sample = $samples[$index]
        Write-Host "Validating $($sample.id)..."
        if ($sample.id -notmatch '^[a-z0-9-]+$') { throw "Invalid sample id: $($sample.id)" }
        $bookRoot = (Resolve-Path (Join-Path $repoRoot $sample.root)).Path
        $entryPath = Join-Path $bookRoot ($sample.entry -replace '/', [IO.Path]::DirectorySeparatorChar)
        $source = Get-Content -LiteralPath $entryPath -Raw -Encoding utf8
        if (-not $source.Contains([string]$sample.contains)) {
            throw "$($sample.id) is missing its expected boundary text."
        }
        foreach ($excluded in @($sample.excludes)) {
            if ($source.Contains([string]$excluded)) {
                throw "$($sample.id) contains excluded boundary text: $excluded"
            }
        }
        $formulaSelectorCount = [regex]::Matches($source, 'math-(?:inline|display)').Count
        if ($sample.requireFormulas -and $formulaSelectorCount -eq 0) {
            throw "$($sample.id) contains no formula selectors."
        }
        if (-not $sample.requireFormulas -and $formulaSelectorCount -ne 0) {
            throw "$($sample.id) unexpectedly contains formula selectors."
        }
        if ($sample.requireCodeBlock -and $source -notmatch '(?s)<pre\b[^>]*>.*?<code\b') {
            throw "$($sample.id) contains no code block."
        }

        Invoke-ReaderHost $bookRoot $sample.entry
        $port = $BasePort + $index
        $server = Start-ValidationServer $bookRoot $port
        $session = "atha-reader-$($sample.id)"
        try {
            $bookUrl = [Uri]::EscapeDataString("/book/$($sample.entry)")
            $probeUrl = [Uri]::EscapeDataString('https://example.com/blocked.png')
            $url = "http://127.0.0.1:$port/reader/atha-reader.html?book=$bookUrl&verify=1&probe=$probeUrl"
            Invoke-AgentBrowser @('--session', $session, '--allowed-domains', '127.0.0.1', 'open', $url)
            Invoke-AgentBrowser @('--session', $session, 'set', 'viewport', '1264', '1680')
            foreach ($theme in @('light', 'dark')) {
                Write-Host "  Rendering $theme mode..."
                Invoke-AgentBrowser @('--session', $session, 'set', 'media', $theme)
                Invoke-AgentBrowser @('--session', $session, 'reload')
                Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "document.documentElement.dataset.status === 'pass'")
                $expectedDark = if ($theme -eq 'dark') { 'true' } else { 'false' }
                $requireFormulas = if ($sample.requireFormulas) { 'true' } else { 'false' }
                $requireCodeBlock = if ($sample.requireCodeBlock) { 'true' } else { 'false' }
                $probe = $themeProbe.Replace('__EXPECTED_DARK__', $expectedDark).
                    Replace('__REQUIRE_FORMULAS__', $requireFormulas).
                    Replace('__EXPECTED_ORDINARY_IMAGES__', [string]$sample.ordinaryImages).
                    Replace('__REQUIRE_CODE_BLOCK__', $requireCodeBlock)
                $encodedProbe = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($probe))
                Invoke-AgentBrowser @('--session', $session, 'eval', '-b', $encodedProbe)
                $screenshot = Join-Path $screenshots "$($sample.id)-$theme.png"
                Invoke-AgentBrowser @('--session', $session, 'screenshot', $screenshot)
                Invoke-AgentBrowser @('--session', $session, 'errors')
                Invoke-AgentBrowser @('--session', $session, 'network', 'requests', '--filter', 'example.com')
            }
        }
        finally {
            try { Invoke-AgentBrowser @('--session', $session, 'close') } catch { Write-Warning $_ }
            if ($server -and -not $server.HasExited) {
                $server.Kill($true)
                $server.WaitForExit()
            }
        }
        [pscustomobject]@{ sample = $sample.id; host = 'pass'; light = 'pass'; dark = 'pass' } | Format-Table -AutoSize
    }
}
finally {
    Pop-Location
}
