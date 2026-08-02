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

function Invoke-AgentBrowserScript {
    param(
        [string]$Session,
        [string]$Script
    )

    $Script | & agent-browser --session $Session eval --stdin
    if ($LASTEXITCODE -ne 0) { throw "agent-browser eval failed with exit code $LASTEXITCODE." }
}

function Get-AgentBrowserOutput {
    param([string[]]$Arguments)

    $output = @(& agent-browser @Arguments)
    if ($LASTEXITCODE -ne 0) { throw "agent-browser failed with exit code $LASTEXITCODE." }
    return [string]::Join("`n", $output)
}

function Get-AgentBrowserScriptValue {
    param(
        [string]$Session,
        [string]$Script
    )

    $output = @($Script | & agent-browser --session $Session eval --stdin)
    if ($LASTEXITCODE -ne 0) { throw "agent-browser eval failed with exit code $LASTEXITCODE." }
    return [string]::Join("`n", $output).Trim()
}

function Invoke-ReaderHost {
    param(
        [string]$BookRoot,
        [object]$Sample
    )

    $startInfo = [Diagnostics.ProcessStartInfo]::new($hostPath)
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $source = if ($Sample.manifest) {
        @('--manifest', [string]$Sample.manifest)
    }
    else {
        @('--entry', [string]$Sample.entry)
    }
    foreach ($argument in @('--book-root', $BookRoot) + $source + @('--verify-sample')) {
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
(async () => {
  const expectedDark = __EXPECTED_DARK__;
  const requireFormulas = __REQUIRE_FORMULAS__;
  const expectedOrdinaryImages = __EXPECTED_ORDINARY_IMAGES__;
  const requireCodeBlock = __REQUIRE_CODE_BLOCK__;
  const expectedSections = __EXPECTED_SECTIONS__;
  const expectedSequence = __EXPECTED_SEQUENCE__;
  const expectedHeadings = __EXPECTED_HEADINGS__;
  if (expectedSections > 1) {
    const toc = document.querySelector('#toc');
    if (toc.options.length !== expectedSections) throw new Error('toc-control');
    const waitForSection = async (section) => {
      for (let frame = 0; frame < 60; frame += 1) {
        const state = globalThis.__athaReaderDiagnostics?.snapshot().session;
        if (state.currentSection === section && state.state === 'layout-stable') return;
        await new Promise(requestAnimationFrame);
      }
      throw new Error('toc-control');
    };
    toc.value = '1';
    toc.dispatchEvent(new Event('change', { bubbles: true }));
    await waitForSection(expectedSequence[1]);
    toc.value = '0';
    toc.dispatchEvent(new Event('change', { bubbles: true }));
    await waitForSection(expectedSequence[0]);
  }
  const preferences = document.querySelector('.preferences');
  const status = document.querySelector('#preferences-status');
  const readerBefore = document.querySelector('.reader').getBoundingClientRect();
  preferences.open = true;
  if (!preferences.open || !document.querySelector('#user-stylesheet')) throw new Error('preferences-control');
  const waitForPreference = async (message) => {
    for (let frame = 0; frame < 120; frame += 1) {
      if (status.textContent === message && status.dataset.error !== 'true') return;
      await new Promise(requestAnimationFrame);
    }
    throw new Error('preferences-control');
  };
  status.textContent = '';
  const density = document.querySelector('#density');
  density.value = 'compact';
  density.dispatchEvent(new Event('change', { bubbles: true }));
  await waitForPreference('已应用');
  status.textContent = '';
  document.querySelector('#reset-application-preferences').click();
  await waitForPreference('已恢复应用默认');
  preferences.open = false;
  const readerAfter = document.querySelector('.reader').getBoundingClientRect();
  if (readerBefore.width !== readerAfter.width || readerBefore.height !== readerAfter.height) throw new Error('preferences-control');
  const result = globalThis.__athaReaderDiagnostics?.snapshot();
  if (!result) throw new Error('missing-reader-diagnostics');
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
  result.contrast = contrast(result.foreground, result.background);
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
  if (result.standaloneOrdinaryCount > 0 && !result.contentActions.ordinaryPreview) throw new Error('ordinary-image-preview');
  if (result.standaloneFormulaCount > 0 && !result.contentActions.formulaPreview) throw new Error('formula-preview');
  if (!result.contentActions.mediaPagePreserved || !result.contentActions.linkImageProtected) throw new Error('media-preview-policy');
  if (result.tableCount > 0 && !result.contentActions.tablePreview) throw new Error('table-preview');
  if (result.codeBlockCount > 0 && !result.contentActions.codePreview) throw new Error('code-preview');
  if (!result.contentActions.structuredPagePreserved || !result.contentActions.structuredProjectionSafe || !result.contentActions.structuredSelectionProtected) throw new Error('structured-preview-policy');
  if (result.structuredLinkCount > 0 && !result.contentActions.structuredLinkProtected) throw new Error('structured-link-policy');
  if (result.session.sections !== expectedSections || result.session.state !== 'layout-stable' || result.session.currentIndex !== 0) throw new Error('session-state');
  if (JSON.stringify(result.session.verifiedSections) !== JSON.stringify(expectedSequence)) throw new Error('session-sequence');
  if (expectedHeadings.length && JSON.stringify(result.session.verifiedHeadings) !== JSON.stringify(expectedHeadings)) throw new Error('session-content');
  if (result.session.releasedSections < Math.max(0, expectedSections - 1)) throw new Error('session-release');
  if (result.session.contentLoads < expectedSections + 1 || result.session.stableLayouts < expectedSections + 1 || result.session.closes < expectedSections) throw new Error('session-lifecycle');
  if (!result.navigation.locatorRoundTrip || !result.navigation.rangeCompared || !result.navigation.reflowRestored || result.navigation.fallback !== 'locator-version') throw new Error('locator-navigation');
  if (expectedSections > 1 && (result.navigation.tocSection !== expectedSequence[1] || result.navigation.previousSection !== expectedSequence[0] || result.navigation.nextSection !== expectedSequence[1])) throw new Error('section-navigation');
  if (!result.interaction.keyboardVerified || !result.interaction.wheelVerified || !result.interaction.mouseVerified || !result.interaction.touchVerified || !result.interaction.selectionVerified || !result.interaction.controlsVerified || !result.interaction.linksVerified || !result.interaction.multiTouchVerified) throw new Error('page-input');
  if (!result.contentActions.selectionCopied || !result.contentActions.sameSection || !result.contentActions.tailFragmentRecovered || !result.contentActions.missingTargetRecovered || !result.contentActions.unknownSectionRecovered || !result.contentActions.auxiliaryActivation || !result.contentActions.externalBlocked || !result.contentActions.footnoteDialog || !result.contentActions.dialogInputProtected || !result.contentActions.focusRestored) throw new Error('content-actions');
  if (expectedSections > 1 && !result.contentActions.crossSection) throw new Error('content-link-section');
  return result;
})()
'@

Push-Location $repoRoot
try {
    if (-not (Get-Command agent-browser -ErrorAction SilentlyContinue)) {
        throw 'agent-browser is not available.'
    }
    Invoke-Checked 'python3' @('scripts/export_reader_sample.py', '--self-check')
    foreach ($module in @(
        'reader/web/app.mjs',
        'reader/web/content.mjs',
        'reader/web/content-actions.mjs',
        'reader/web/structured-actions.mjs',
        'reader/web/diagnostics.mjs',
        'reader/web/interaction.mjs',
        'reader/web/locator.mjs',
        'reader/web/navigation.mjs',
        'reader/web/pagination.mjs',
        'reader/web/preferences.mjs',
        'reader/web/session.mjs'
    )) {
        Invoke-Checked 'node' @('--check', $module)
    }
    Invoke-Checked $cargoPath @('fmt', '--manifest-path', 'Cargo.toml', '--all', '--check')
    Invoke-Checked $cargoPath @('clippy', '--manifest-path', 'Cargo.toml', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings')
    Invoke-Checked $cargoPath @('test', '--manifest-path', 'Cargo.toml', '--workspace', '--all-targets', '--locked')
    Invoke-Checked $cargoPath @('build', '--manifest-path', 'Cargo.toml', '--package', 'atha-reader-host', '--locked')
    New-Item -ItemType Directory -Path $screenshots -Force | Out-Null

    for ($index = 0; $index -lt $samples.Count; $index++) {
        $sample = $samples[$index]
        Write-Host "Validating $($sample.id)..."
        if ($sample.id -notmatch '^[a-z0-9-]+$') { throw "Invalid sample id: $($sample.id)" }
        if ($sample.source) {
            $sourcePath = (Resolve-Path (Join-Path $repoRoot $sample.source)).Path
            $outputName = Split-Path ([string]$sample.root) -Leaf
            $exportArguments = @('scripts/export_reader_sample.py', '--epub', $sourcePath, '--output', $outputName)
            foreach ($entry in @($sample.exportEntries)) {
                $exportArguments += @('--entry', [string]$entry)
            }
            Invoke-Checked 'python3' $exportArguments
        }
        $bookRoot = (Resolve-Path (Join-Path $repoRoot $sample.root)).Path
        $expectedSections = if ($sample.expectedSections) { [int]$sample.expectedSections } else { 1 }
        $expectedSequence = if ($sample.expectedSequence) { @($sample.expectedSequence) } else { @('entry') }
        $expectedHeadings = if ($sample.expectedHeadings) { @($sample.expectedHeadings) } else { @() }
        if ($sample.manifest) {
            $bookManifest = Get-Content -LiteralPath (Join-Path $bookRoot $sample.manifest) -Raw -Encoding utf8 | ConvertFrom-Json
            if ($bookManifest.contentVersion -ne $sample.contentVersion -or @($bookManifest.sections).Count -ne $expectedSections) {
                throw "$($sample.id) manifest does not match the configured source."
            }
            $manifestEntries = @($bookManifest.sections | ForEach-Object { [string]$_.href })
            if (($manifestEntries -join [char]0) -ne (@($sample.exportEntries) -join [char]0)) {
                throw "$($sample.id) manifest section order changed."
            }
            for ($sectionIndex = 0; $sectionIndex -lt $manifestEntries.Count; $sectionIndex++) {
                $sectionSource = Get-Content -LiteralPath (Join-Path $bookRoot ($manifestEntries[$sectionIndex] -replace '/', [IO.Path]::DirectorySeparatorChar)) -Raw -Encoding utf8
                if (-not $sectionSource.Contains([string]$sample.sectionContains[$sectionIndex])) {
                    throw "$($sample.id) section $sectionIndex has unexpected content."
                }
            }
        }
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

        Invoke-ReaderHost $bookRoot $sample
        $port = $BasePort + $index
        $server = Start-ValidationServer $bookRoot $port
        $session = "atha-reader-$($sample.id)"
        try {
            $sourceQuery = if ($sample.manifest) {
                'manifest=' + [Uri]::EscapeDataString("/book/$($sample.manifest)")
            }
            else {
                'book=' + [Uri]::EscapeDataString("/book/$($sample.entry)")
            }
            $probeUrl = [Uri]::EscapeDataString('https://example.com/blocked.png')
            $url = "http://127.0.0.1:$port/reader/atha-reader.html?$sourceQuery&verify=1&probe=$probeUrl"
            Invoke-AgentBrowser @('--session', $session, '--allowed-domains', '127.0.0.1', 'open', $url)
            Invoke-AgentBrowser @('--session', $session, 'set', 'viewport', '1264', '1680')
            foreach ($theme in @('light', 'dark')) {
                Write-Host "  Rendering $theme mode..."
                Invoke-AgentBrowser @('--session', $session, 'set', 'media', $theme)
                Invoke-AgentBrowser @('--session', $session, 'reload')
                Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "document.documentElement.dataset.status === 'pass'")
                $selectionProbe = Get-AgentBrowserScriptValue -Session $session -Script @'
(() => {
  const point = globalThis.__athaReaderDiagnostics.selectionProbe();
  return point && [point.startX, point.startY, point.endX, point.endY].map(Math.round);
})()
'@ | ConvertFrom-Json
                if (@($selectionProbe).Count -ne 4) { throw 'No visible text was available for selection.' }
                $copyCount = [int](Get-AgentBrowserScriptValue -Session $session -Script 'globalThis.__athaReaderDiagnostics.snapshot().contentActions.trustedCopies')
                Invoke-AgentBrowser @('--session', $session, 'mouse', 'move', [string]$selectionProbe[0], [string]$selectionProbe[1])
                Invoke-AgentBrowser @('--session', $session, 'mouse', 'down', 'left')
                Invoke-AgentBrowser @('--session', $session, 'mouse', 'move', [string]$selectionProbe[2], [string]$selectionProbe[3])
                Invoke-AgentBrowser @('--session', $session, 'mouse', 'up', 'left')
                $selectionLength = [int](Get-AgentBrowserScriptValue -Session $session -Script 'globalThis.__athaReaderDiagnostics.snapshot().contentActions.selectionLength')
                if ($selectionLength -le 0) { throw 'Real mouse text selection failed.' }
                [void](Get-AgentBrowserScriptValue -Session $session -Script 'globalThis.__athaReaderDiagnostics.armCopyProbe(); true')
                Invoke-AgentBrowser @('--session', $session, 'press', 'Control+c')
                $trustedCopies = [int](Get-AgentBrowserScriptValue -Session $session -Script 'globalThis.__athaReaderDiagnostics.snapshot().contentActions.trustedCopies')
                if ($trustedCopies -ne $copyCount + 1) { throw 'Trusted Ctrl+C copy event was not observed.' }
                [void](Get-AgentBrowserScriptValue -Session $session -Script 'globalThis.__athaReaderDiagnostics.clearSelection()')
                $expectedDark = if ($theme -eq 'dark') { 'true' } else { 'false' }
                $requireFormulas = if ($sample.requireFormulas) { 'true' } else { 'false' }
                $requireCodeBlock = if ($sample.requireCodeBlock) { 'true' } else { 'false' }
                $sequenceJson = ConvertTo-Json @($expectedSequence) -Compress
                $headingsJson = ConvertTo-Json @($expectedHeadings) -Compress
                $probe = $themeProbe.Replace('__EXPECTED_DARK__', $expectedDark).
                    Replace('__REQUIRE_FORMULAS__', $requireFormulas).
                    Replace('__EXPECTED_ORDINARY_IMAGES__', [string]$sample.ordinaryImages).
                    Replace('__REQUIRE_CODE_BLOCK__', $requireCodeBlock).
                    Replace('__EXPECTED_SECTIONS__', [string]$expectedSections).
                    Replace('__EXPECTED_SEQUENCE__', $sequenceJson).
                    Replace('__EXPECTED_HEADINGS__', $headingsJson)
                Invoke-AgentBrowserScript -Session $session -Script $probe
                if ([int]$sample.ordinaryImages -gt 0) {
                    $mediaPoint = Get-AgentBrowserScriptValue -Session $session -Script @'
(async () => {
  const point = await globalThis.__athaReaderDiagnostics.mediaPoint('ordinary');
  return point && [point.x, point.y].map(Math.round);
})()
'@ | ConvertFrom-Json
                    if (@($mediaPoint).Count -ne 2) { throw 'No ordinary image was available for real mouse preview.' }
                    Invoke-AgentBrowser @('--session', $session, 'mouse', 'move', [string]$mediaPoint[0], [string]$mediaPoint[1])
                    Invoke-AgentBrowser @('--session', $session, 'mouse', 'down', 'left')
                    Invoke-AgentBrowser @('--session', $session, 'mouse', 'up', 'left')
                    $previewState = Get-AgentBrowserScriptValue -Session $session -Script 'globalThis.__athaReaderDiagnostics.previewState()' | ConvertFrom-Json
                    if (-not $previewState.open) { throw 'Real mouse image preview failed.' }
                    Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "document.querySelector('#content-dialog-image').complete && document.querySelector('#content-dialog-image').naturalWidth > 0")
                    $previewScreenshot = Join-Path $screenshots "$($sample.id)-$theme-preview.png"
                    Invoke-AgentBrowser @('--session', $session, 'screenshot', $previewScreenshot)
                    Invoke-AgentBrowser @('--session', $session, 'press', 'Escape')
                    $previewState = Get-AgentBrowserScriptValue -Session $session -Script 'globalThis.__athaReaderDiagnostics.previewState()' | ConvertFrom-Json
                    if ($previewState.open -or -not $previewState.focusRestored) { throw 'Escape did not close the image preview and restore focus.' }
                    if ((Get-AgentBrowserScriptValue -Session $session -Script "globalThis.__athaReaderDiagnostics.focusMedia('ordinary')") -ne 'true') { throw 'Ordinary image focus failed.' }
                    Invoke-AgentBrowser @('--session', $session, 'press', 'Space')
                    $previewState = Get-AgentBrowserScriptValue -Session $session -Script 'globalThis.__athaReaderDiagnostics.previewState()' | ConvertFrom-Json
                    if (-not $previewState.open) { throw 'Trusted Space image preview failed.' }
                    Invoke-AgentBrowser @('--session', $session, 'press', 'Escape')
                }
                if ($sample.requireFormulas) {
                    if ((Get-AgentBrowserScriptValue -Session $session -Script "globalThis.__athaReaderDiagnostics.focusMedia('formula')") -ne 'true') { throw 'Formula focus failed.' }
                    Invoke-AgentBrowser @('--session', $session, 'press', 'Enter')
                    $previewState = Get-AgentBrowserScriptValue -Session $session -Script 'globalThis.__athaReaderDiagnostics.previewState()' | ConvertFrom-Json
                    if (-not $previewState.open) { throw 'Trusted Enter formula preview failed.' }
                    Invoke-AgentBrowser @('--session', $session, 'press', 'Escape')
                }
                $structuredCounts = Get-AgentBrowserScriptValue -Session $session -Script '(() => { const value = globalThis.__athaReaderDiagnostics.snapshot(); return [value.tableCount, value.codeBlockCount]; })()' | ConvertFrom-Json
                $structuredTargets = @()
                if ([int]$structuredCounts[0] -gt 0) { $structuredTargets += @{ kind = 'table'; key = 'Enter' } }
                if ([int]$structuredCounts[1] -gt 0) { $structuredTargets += @{ kind = 'code'; key = 'Space' } }
                foreach ($target in $structuredTargets) {
                    $kind = [string]$target.kind
                    if ((Get-AgentBrowserScriptValue -Session $session -Script "globalThis.__athaReaderDiagnostics.focusMedia('$kind')") -ne 'true') { throw "$kind focus failed." }
                    Invoke-AgentBrowser @('--session', $session, 'press', [string]$target.key)
                    $previewState = Get-AgentBrowserScriptValue -Session $session -Script 'globalThis.__athaReaderDiagnostics.previewState()' | ConvertFrom-Json
                    if (-not $previewState.open) { throw "Trusted $($target.key) $kind preview failed." }
                    $structuredScreenshot = Join-Path $screenshots "$($sample.id)-$theme-$kind-preview.png"
                    Invoke-AgentBrowser @('--session', $session, 'screenshot', $structuredScreenshot)
                    Invoke-AgentBrowser @('--session', $session, 'press', 'Escape')
                    $previewState = Get-AgentBrowserScriptValue -Session $session -Script 'globalThis.__athaReaderDiagnostics.previewState()' | ConvertFrom-Json
                    if ($previewState.open -or -not $previewState.focusRestored) { throw "Escape did not close the $kind preview and restore focus." }
                }
                $linkRequests = Get-AgentBrowserOutput @('--session', $session, 'network', 'requests', '--filter', 'atha-link-probe.invalid', '--json') | ConvertFrom-Json
                if (-not $linkRequests.success -or $null -eq $linkRequests.data.requests) { throw 'External link network evidence is unavailable.' }
                if (@($linkRequests.data.requests).Count -ne 0) { throw 'Blocked external link issued a network request.' }
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
