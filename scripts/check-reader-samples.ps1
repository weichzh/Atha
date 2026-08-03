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
        [object]$Sample,
        [ValidateSet('write', 'read')]
        [string]$StateProbe
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
    $probeArguments = if ($StateProbe) { @('--state-probe', $StateProbe) } else { @() }
    foreach ($argument in @('--book-root', $BookRoot) + $source + @('--verify-sample') + $probeArguments) {
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
    if (toc.querySelectorAll('option:not([data-bookmark-id])').length !== expectedSections) throw new Error('toc-control');
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
  const progress = document.querySelector('#progress-range');
  if (Number(progress.max) > 1) {
    progress.value = '2';
    progress.dispatchEvent(new Event('change', { bubbles: true }));
    for (let frame = 0; frame < 60 && (!document.querySelector('#position').textContent.startsWith('2 / ') || document.querySelector('#previous').disabled); frame += 1) {
      await new Promise(requestAnimationFrame);
    }
    if (document.querySelector('#previous').disabled) throw new Error('progress-control');
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
  const annotationStatus = document.querySelector('#annotations-status');
  annotationStatus.dataset.error = 'true';
  result.annotationErrorColor = getComputedStyle(annotationStatus).color;
  delete annotationStatus.dataset.error;
  if (result.status !== 'pass' || result.error) throw new Error('reader-status');
  if (requireFormulas && result.formulaCount < 1) throw new Error('no-formulas');
  if (!requireFormulas && result.formulaCount !== 0) throw new Error('unexpected-formulas');
  if (result.ordinaryCount !== expectedOrdinaryImages || result.ordinaryPngCount !== expectedOrdinaryImages) throw new Error('ordinary-image-count');
  if (requireCodeBlock && result.codeBlockCount < 1) throw new Error('missing-code-block');
  if (result.contrast < 4.5) throw new Error('contrast');
  if (result.dark !== expectedDark) throw new Error('theme-media');
  if (result.annotationErrorColor !== (expectedDark ? 'rgb(255, 179, 173)' : 'rgb(163, 41, 33)')) throw new Error('annotation-error-contrast');
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
  if (!result.interaction.keyboardVerified || !result.interaction.wheelVerified || !result.interaction.mouseVerified || !result.interaction.touchVerified || !result.interaction.touchCenterVerified || !result.interaction.selectionVerified || !result.interaction.controlsVerified || !result.interaction.linksVerified || !result.interaction.multiTouchVerified) throw new Error('page-input');
  if (!result.contentActions.selectionCopied || !result.contentActions.sameSection || !result.contentActions.tailFragmentRecovered || !result.contentActions.missingTargetRecovered || !result.contentActions.unknownSectionRecovered || !result.contentActions.auxiliaryActivation || !result.contentActions.externalBlocked || !result.contentActions.footnoteDialog || !result.contentActions.dialogInputProtected || !result.contentActions.focusRestored) throw new Error('content-actions');
  if (expectedSections > 1 && !result.contentActions.crossSection) throw new Error('content-link-section');
  if (!result.readerState.available || !result.readerState.durable || !result.readerState.restored || result.readerState.pending || !result.readerState.coalesced || !result.readerState.lifecycleFlushed || !result.readerState.versionRejected) throw new Error('reader-state');
  if (!result.bookmarks.created || !result.bookmarks.toggled || !result.bookmarks.jumped || !result.bookmarks.deleted || result.bookmarks.items.length !== 0) throw new Error('bookmarks');
  if (!result.search.replaced || !result.search.canceled || !result.search.errorIsolated || !result.search.activeContentRejected) throw new Error('search');
  if (!result.annotations.sourceAnchor || !result.annotations.noteUpdated || !result.annotations.writeFailureRejected || !result.annotations.softDeleted || !result.annotations.reanchored || !result.annotations.ambiguousRejected || !result.annotations.missingRejected || !result.annotations.missingSectionRejected || !result.annotations.corruptHashRejected) throw new Error('annotations');
  return result;
})()
'@

Push-Location $repoRoot
try {
    if (-not (Get-Command agent-browser -ErrorAction SilentlyContinue)) {
        throw 'agent-browser is not available.'
    }
    Invoke-Checked 'python3' @('scripts/export_reader_sample.py', '--self-check')
    $readerModules = @(
        'reader/web/app.mjs',
        'reader/web/content.mjs',
        'reader/web/content-actions.mjs',
        'reader/web/structured-actions.mjs',
        'reader/web/reader-state.mjs',
        'reader/web/bookmarks.mjs',
        'reader/web/search.mjs',
        'reader/web/annotation-store.mjs',
        'reader/web/annotations.mjs',
        'reader/web/diagnostics.mjs',
        'reader/web/interaction.mjs',
        'reader/web/locator.mjs',
        'reader/web/navigation.mjs',
        'reader/web/pagination.mjs',
        'reader/web/preferences.mjs',
        'reader/web/session.mjs'
    )
    foreach ($module in $readerModules) {
        Invoke-Checked 'node' @('--check', $module)
    }
    $bundleCheck = Join-Path $repoRoot '.tmp/atha-reader-bundle-check.mjs'
    [IO.File]::WriteAllText(
        $bundleCheck,
        [string]::Join("`n", @($readerModules | ForEach-Object { [IO.File]::ReadAllText((Join-Path $repoRoot $_)) })),
        [Text.UTF8Encoding]::new($false)
    )
    try {
        Invoke-Checked 'node' @('--check', $bundleCheck)
    }
    finally {
        Remove-Item -LiteralPath $bundleCheck -Force
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
        $session = "atha-reader-$($sample.id)-$PID"
        try {
            $sourceQuery = if ($sample.manifest) {
                'manifest=' + [Uri]::EscapeDataString("/book/$($sample.manifest)")
            }
            else {
                'book=' + [Uri]::EscapeDataString("/book/$($sample.entry)")
            }
            $probeUrl = [Uri]::EscapeDataString('https://example.com/blocked.png')
            $versionQuery = if ($sample.manifest) {
                ''
            }
            else {
                $entryPath = Join-Path $bookRoot ([string]$sample.entry)
                '&version=' + (Get-FileHash -LiteralPath $entryPath -Algorithm SHA256).Hash.ToLowerInvariant()
            }
            $url = "http://127.0.0.1:$port/reader/atha-reader.html?$sourceQuery&verify=1&probe=$probeUrl&state=$($sample.id)&persist=1$versionQuery"
            Invoke-AgentBrowser @('--session', $session, '--allowed-domains', '127.0.0.1', 'open', $url)
            Invoke-AgentBrowser @('--session', $session, 'set', 'viewport', '780', '1680')
            [void](Get-AgentBrowserScriptValue -Session $session -Script 'localStorage.clear(); true')
            Invoke-AgentBrowser @('--session', $session, 'reload')
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
                if ($sample.id -eq 'math-history-r1') {
                    if ($theme -eq 'light') {
                        $annotationProbe = Get-AgentBrowserScriptValue -Session $session -Script @'
(() => {
  const point = globalThis.__athaReaderDiagnostics.selectionProbe();
  return point && [point.startX, point.startY, point.endX, point.endY].map(Math.round);
})()
'@ | ConvertFrom-Json
                        if (@($annotationProbe).Count -ne 4) { throw 'No visible text was available for annotation.' }
                        Invoke-AgentBrowser @('--session', $session, 'mouse', 'move', [string]$annotationProbe[0], [string]$annotationProbe[1])
                        Invoke-AgentBrowser @('--session', $session, 'mouse', 'down', 'left')
                        Invoke-AgentBrowser @('--session', $session, 'mouse', 'move', [string]$annotationProbe[2], [string]$annotationProbe[3])
                        Invoke-AgentBrowser @('--session', $session, 'mouse', 'up', 'left')
                        [void](Get-AgentBrowserScriptValue -Session $session -Script "document.documentElement.setAttribute('data-reader-tools', ''); true")
                        Invoke-AgentBrowser @('--session', $session, 'click', '.notes > summary')
                        Invoke-AgentBrowser @('--session', $session, 'fill', '#annotation-note', '真实阅读笔记')
                        Invoke-AgentBrowser @('--session', $session, 'click', '#add-annotation')
                        Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "(() => { const value = globalThis.__athaReaderDiagnostics.snapshot().annotations; return value.active.length === 1 && value.overlayCount === 1 && value.active[0].sourceAnchor.contentHash.length === 64; })()")
                        Invoke-AgentBrowser @('--session', $session, 'select', '#font-size', '40')
                        Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "globalThis.__athaReaderDiagnostics.snapshot().annotations.overlayCount === 1")
                        Invoke-AgentBrowser @('--session', $session, 'select', '#font-size', '32')
                        Invoke-AgentBrowser @('--session', $session, 'fill', '#annotation-note', '已更新的真实阅读笔记')
                        Invoke-AgentBrowser @('--session', $session, 'click', '#save-annotation-note')
                        Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "globalThis.__athaReaderDiagnostics.snapshot().annotations.active[0]?.note === '已更新的真实阅读笔记'")
                        Invoke-AgentBrowser @('--session', $session, 'screenshot', (Join-Path $screenshots 'math-history-r1-light-annotation.png'))
                        Invoke-AgentBrowser @('--session', $session, 'click', '.notes > summary')
                    }
                    else {
                        Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "(() => { const value = globalThis.__athaReaderDiagnostics.snapshot().annotations; return value.active.length === 1 && value.overlayCount === 1 && value.active[0].note === '已更新的真实阅读笔记'; })()")
                        [void](Get-AgentBrowserScriptValue -Session $session -Script "document.documentElement.setAttribute('data-reader-tools', ''); true")
                        Invoke-AgentBrowser @('--session', $session, 'click', '.notes > summary')
                        Invoke-AgentBrowser @('--session', $session, 'click', '#go-annotation')
                        Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "(() => { const value = globalThis.__athaReaderDiagnostics.snapshot(); const anchor = JSON.parse(value.annotations.active[0].sourceAnchor.canonicalLocator).start; const current = JSON.parse(value.navigation.current).start; return anchor.section === current.section && anchor.offset === current.offset; })()")
                        Invoke-AgentBrowser @('--session', $session, 'screenshot', (Join-Path $screenshots 'math-history-r1-dark-annotation.png'))
                        Invoke-AgentBrowser @('--session', $session, 'click', '#delete-annotation')
                        $deletedAnnotation = Get-AgentBrowserScriptValue -Session $session -Script 'globalThis.__athaReaderDiagnostics.snapshot().annotations' | ConvertFrom-Json
                        if (@($deletedAnnotation.active).Count -ne 0 -or $deletedAnnotation.tombstones -ne 1 -or $deletedAnnotation.overlayCount -ne 0) {
                            throw "Annotation soft delete failed: $($deletedAnnotation | ConvertTo-Json -Compress -Depth 6)"
                        }
                        Invoke-AgentBrowser @('--session', $session, 'click', '.notes > summary')
                    }
                }
                $screenshot = Join-Path $screenshots "$($sample.id)-$theme.png"
                Invoke-AgentBrowser @('--session', $session, 'screenshot', $screenshot)
                [void](Get-AgentBrowserScriptValue -Session $session -Script "document.documentElement.setAttribute('data-reader-tools', ''); true")
                Invoke-AgentBrowser @('--session', $session, 'click', '.search > summary')
                Invoke-AgentBrowser @('--session', $session, 'fill', '#search-query', [string]$sample.searchQuery)
                Invoke-AgentBrowser @('--session', $session, 'click', '#search-form button[type=submit]')
                Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "globalThis.__athaReaderDiagnostics.snapshot().search.status === 'complete'")
                $searchState = Get-AgentBrowserScriptValue -Session $session -Script 'globalThis.__athaReaderDiagnostics.snapshot().search' | ConvertFrom-Json
                if ($searchState.count -ne [int]$sample.expectedSearchResults -or @($searchState.sections).Count -ne [int]$sample.expectedSearchSections) {
                    throw "Search results changed for $($sample.id): $($searchState | ConvertTo-Json -Compress)"
                }
                if ($sample.id -eq 'math-history-r1') {
                    $searchScreenshot = Join-Path $screenshots "math-history-r1-$theme-search.png"
                    Invoke-AgentBrowser @('--session', $session, 'screenshot', $searchScreenshot)
                    if ($theme -eq 'light') {
                        [void](Get-AgentBrowserScriptValue -Session $session -Script "document.documentElement.dataset.theme = 'dark'; true")
                        $explicitDark = Get-AgentBrowserScriptValue -Session $session -Script "(() => { const panel = getComputedStyle(document.querySelector('.search-panel')); const input = getComputedStyle(document.querySelector('#search-query')); const status = document.querySelector('#annotations-status'); status.dataset.error = 'true'; const errorColor = getComputedStyle(status).color; delete status.dataset.error; return panel.backgroundColor === 'rgb(32, 44, 48)' && panel.color === 'rgb(233, 238, 238)' && input.backgroundColor === 'rgb(32, 44, 48)' && input.color === 'rgb(233, 238, 238)' && errorColor === 'rgb(255, 179, 173)'; })()"
                        if ($explicitDark -ne 'true') { throw 'Explicit dark search theme is unreadable under a light system theme.' }
                        Invoke-AgentBrowser @('--session', $session, 'screenshot', (Join-Path $screenshots 'math-history-r1-explicit-dark-search.png'))
                        [void](Get-AgentBrowserScriptValue -Session $session -Script "delete document.documentElement.dataset.theme; true")
                    }
                }
                $lastResult = @($searchState.results)[-1]
                Invoke-AgentBrowser @('--session', $session, 'select', '#search-results', [string]$lastResult.id)
                Invoke-AgentBrowser @('--session', $session, 'click', '#go-search-result')
                Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', 'globalThis.__athaReaderDiagnostics.snapshot().search.lastJump?.visible === true')
                $firstResult = @($searchState.results)[0]
                Invoke-AgentBrowser @('--session', $session, 'select', '#search-results', [string]$firstResult.id)
                Invoke-AgentBrowser @('--session', $session, 'click', '#go-search-result')
                Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', '(() => { const search = globalThis.__athaReaderDiagnostics.snapshot().search; const first = JSON.parse(search.results[0].locator).start; return search.lastJump?.visible === true && search.lastJump.section === first.section && search.lastJump.offset === first.offset; })()')
                Invoke-AgentBrowser @('--session', $session, 'click', '.search > summary')
                Invoke-AgentBrowser @('--session', $session, 'errors')
                Invoke-AgentBrowser @('--session', $session, 'network', 'requests', '--filter', 'example.com')
            }
            if ($sample.id -eq 'math-history-r1') {
                Invoke-AgentBrowser @('--session', $session, 'click', '.progress > summary')
                $positionBefore = Get-AgentBrowserScriptValue -Session $session -Script "document.querySelector('#position').textContent" | ConvertFrom-Json
                Invoke-AgentBrowser @('--session', $session, 'click', '#next')
                Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "document.querySelector('#position').textContent !== '$positionBefore'")
                Invoke-AgentBrowser @('--session', $session, 'click', '.directory > summary')
                Invoke-AgentBrowser @('--session', $session, 'click', '#add-bookmark')
                Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "document.querySelectorAll('#toc option[data-bookmark-id]').length === 1 && document.querySelector('#add-bookmark').getAttribute('aria-pressed') === 'true'")
                Invoke-AgentBrowser @('--session', $session, 'click', '.preferences > summary')
                [void](Get-AgentBrowserScriptValue -Session $session -Script "(() => { const value = document.querySelector('#brightness'); value.value = '85'; value.dispatchEvent(new Event('input', { bubbles: true })); value.dispatchEvent(new Event('change', { bubbles: true })); return true; })()")
                Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "JSON.parse(localStorage.getItem('atha.reader.application.v1')).preferences.brightness === 85")
                Invoke-AgentBrowser @('--session', $session, 'select', '#font-size', '24')
                Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "JSON.parse(localStorage.getItem('atha.reader.application.v1')).preferences.fontSize === 24")
                Invoke-AgentBrowser @('--session', $session, 'select', '#theme', 'dark')
                Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "document.documentElement.dataset.theme === 'dark' && JSON.parse(localStorage.getItem('atha.reader.application.v1')).preferences.theme === 'dark'")
                $savedPosition = Get-AgentBrowserScriptValue -Session $session -Script "document.querySelector('#position').textContent" | ConvertFrom-Json
                $restoreUrl = $url.Replace('verify=1&', '')
                Invoke-AgentBrowser @('--session', $session, 'open', $restoreUrl)
                Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "document.documentElement.dataset.status === 'pass'")
                $restored = Get-AgentBrowserScriptValue -Session $session -Script @'
(() => ({
  position: document.querySelector('#position').textContent,
  fontSize: document.querySelector('#font-size').value,
  brightness: document.querySelector('#brightness').value,
  theme: document.querySelector('#theme').value,
  bookmarks: document.querySelectorAll('#toc option[data-bookmark-id]').length,
  ...(() => {
    const key = Object.keys(localStorage).find((value) => value.startsWith('atha.reader.annotations.'));
    const items = JSON.parse(localStorage.getItem(key)).items;
    return {
      annotationActive: items.filter((item) => item.deletedAt === null).length,
      annotationTombstones: items.filter((item) => item.deletedAt !== null).length,
    };
  })(),
                }))()
'@ | ConvertFrom-Json
                if ($restored.position -ne $savedPosition -or $restored.fontSize -ne '24' -or $restored.brightness -ne '85' -or $restored.theme -ne 'dark' -or $restored.bookmarks -ne 1 -or $restored.annotationActive -ne 0 -or $restored.annotationTombstones -ne 1) {
                    throw "Persistent reader state did not restore. expectedPosition=$savedPosition actual=$($restored | ConvertTo-Json -Compress)"
                }
                [void](Get-AgentBrowserScriptValue -Session $session -Script "document.documentElement.setAttribute('data-reader-tools', ''); true")
                Invoke-AgentBrowser @('--session', $session, 'click', '.progress > summary')
                Invoke-AgentBrowser @('--session', $session, 'click', '#next')
                Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "document.querySelector('#position').textContent !== '$savedPosition'")
                Invoke-AgentBrowser @('--session', $session, 'click', '.directory > summary')
                [void](Get-AgentBrowserScriptValue -Session $session -Script @'
(() => {
  const toc = document.querySelector('#toc');
  const bookmark = toc.querySelector('option[data-bookmark-id]');
  toc.value = bookmark.value;
  toc.dispatchEvent(new Event('change', { bubbles: true }));
  return true;
})()
'@)
                Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "document.querySelector('#position').textContent === '$savedPosition' && document.querySelector('#add-bookmark').getAttribute('aria-pressed') === 'true'")
                Invoke-AgentBrowser @('--session', $session, 'click', '#add-bookmark')
                Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "document.querySelectorAll('#toc option[data-bookmark-id]').length === 0 && document.querySelector('#add-bookmark').getAttribute('aria-pressed') === 'false'")
                $bookmarkCount = [int](Get-AgentBrowserScriptValue -Session $session -Script "document.querySelectorAll('#toc option[data-bookmark-id]').length")
                if ($bookmarkCount -ne 0) { throw 'Bookmark deletion did not persist in memory.' }
                [void](Get-AgentBrowserScriptValue -Session $session -Script @'
(() => {
  const key = Object.keys(localStorage).find((value) => value.startsWith('atha.reader.progress.'));
  if (!key) return false;
  localStorage.setItem(key, '{');
  return true;
})()
'@)
                Invoke-AgentBrowser @('--session', $session, 'reload')
                Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "document.documentElement.dataset.status === 'pass'")
                $fallbackPosition = Get-AgentBrowserScriptValue -Session $session -Script "document.querySelector('#position').textContent" | ConvertFrom-Json
                if (-not $fallbackPosition.StartsWith('1 / ')) { throw 'Corrupt progress did not fall back safely.' }
                $persistenceScreenshot = Join-Path $screenshots 'math-history-r1-persistence.png'
                Invoke-AgentBrowser @('--session', $session, 'screenshot', $persistenceScreenshot)
                [void](Get-AgentBrowserScriptValue -Session $session -Script 'localStorage.clear(); true')
            }
            if ($sample.id -eq 'math-history-r1') {
                Write-Host '  Verifying WebView2 state across host processes...'
                Invoke-ReaderHost -BookRoot $bookRoot -Sample $sample -StateProbe write
                Invoke-ReaderHost -BookRoot $bookRoot -Sample $sample -StateProbe read
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
