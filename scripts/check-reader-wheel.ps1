# Description: Measure reader wheel acceptance and input-to-stable latency in a real browser.

[CmdletBinding()]
param(
    [ValidateRange(1024, 65535)]
    [int]$Port = 18775
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$bookRoot = (Resolve-Path (Join-Path $repoRoot 'fixtures/local/category-coproduct-5-6')).Path
$serverScript = Join-Path $PSScriptRoot 'Serve-ReaderValidation.ps1'
$session = "atha-reader-wheel-$PID"
$server = $null

function Invoke-AgentBrowser {
    param([string[]]$Arguments)

    & agent-browser @Arguments
    if ($LASTEXITCODE -ne 0) { throw "agent-browser failed with exit code $LASTEXITCODE." }
}

function Get-AgentBrowserScriptValue {
    param([string]$Script)

    $output = @($Script | & agent-browser --session $session eval --stdin)
    if ($LASTEXITCODE -ne 0) { throw "agent-browser eval failed with exit code $LASTEXITCODE." }
    return [string]::Join("`n", $output).Trim()
}

try {
    $startInfo = [Diagnostics.ProcessStartInfo]::new('pwsh')
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    foreach ($argument in @('-NoProfile', '-File', $serverScript, '-BookRoot', $bookRoot, '-Port', [string]$Port)) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    $server = [Diagnostics.Process]::Start($startInfo)
    $ready = $false
    for ($attempt = 0; $attempt -lt 50; $attempt++) {
        try {
            if ((Invoke-WebRequest -UseBasicParsing "http://127.0.0.1:$Port/health" -TimeoutSec 1).StatusCode -eq 200) {
                $ready = $true
                break
            }
        }
        catch {
            Start-Sleep -Milliseconds 100
        }
    }
    if ($server.HasExited) { throw "Reader validation server exited with code $($server.ExitCode)." }
    if (-not $ready) { throw 'Reader validation server did not become ready.' }

    $entry = '/book/EPUB/text/ch008.xhtml'
    $entryPath = Join-Path $bookRoot 'EPUB/text/ch008.xhtml'
    $version = (Get-FileHash -LiteralPath $entryPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $url = "http://127.0.0.1:$Port/reader/atha-reader.html?book=$([Uri]::EscapeDataString($entry))&search-probe=1&state=wheel-check&version=$version"
    Invoke-AgentBrowser @('--session', $session, '--allowed-domains', '127.0.0.1', 'open', $url)
    Invoke-AgentBrowser @('--session', $session, 'set', 'viewport', '780', '1680')
    Invoke-AgentBrowser @('--session', $session, 'reload')
    Invoke-AgentBrowser @('--session', $session, 'wait', '--fn', "document.documentElement.dataset.status === 'pass' || Boolean(document.documentElement.dataset.error)")
    $startup = Get-AgentBrowserScriptValue "({ status: document.documentElement.dataset.status, error: document.documentElement.dataset.error || null })" | ConvertFrom-Json
    if ($startup.status -ne 'pass') { throw "Reader startup failed: $($startup | ConvertTo-Json -Compress)" }

    $media = Get-AgentBrowserScriptValue @'
(async () => {
  const point = await globalThis.__athaReaderDiagnostics.mediaPoint('ordinary');
  await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  return { point, position: document.querySelector('#position').textContent };
})()
'@ | ConvertFrom-Json
    Invoke-AgentBrowser @('--session', $session, 'mouse', 'move', [string][Math]::Round($media.point.x), [string][Math]::Round($media.point.y))
    Invoke-AgentBrowser @('--session', $session, 'mouse', 'wheel', '100')
    $actualMediaAccepted = Get-AgentBrowserScriptValue @'
(async () => {
  const before = __POSITION__;
  for (let frame = 0; frame < 30; frame += 1) {
    if (document.querySelector('#position').textContent !== before) return true;
    await new Promise(requestAnimationFrame);
  }
  return false;
})()
'@.Replace('__POSITION__', (ConvertTo-Json ([string]$media.position) -Compress)) | ConvertFrom-Json

    $result = Get-AgentBrowserScriptValue "globalThis.__athaReaderDiagnostics.wheelProbe()" | ConvertFrom-Json
    $result | Add-Member -NotePropertyName actualMediaAccepted -NotePropertyValue $actualMediaAccepted
    $result | ConvertTo-Json -Depth 5
    if (-not $actualMediaAccepted) { throw 'Actual mouse wheel input was lost over book media.' }
    $presentTargets = @($result.targets.psobject.Properties.Value | Where-Object present)
    if ($presentTargets.Count -eq 0 -or @($presentTargets | Where-Object { -not $_.accepted -or -not $_.defaultPrevented }).Count -gt 0) {
        throw 'Wheel input was lost over book media.'
    }
    if ($result.repeatedAccepted -ne 4 -or $result.repeatedDefaultPrevented -ne 4) {
        throw 'Repeated discrete wheel input was not accepted one-for-one.'
    }
    if ($null -eq $result.inputToStableP95Ms -or $result.inputToStableP95Ms -gt 50) {
        throw "Wheel input-to-stable P95 exceeded 50ms: $($result.inputToStableP95Ms)ms."
    }
}
finally {
    & agent-browser --session $session close | Out-Null
    if ($server -and -not $server.HasExited) {
        $server.Kill($true)
        $server.WaitForExit()
    }
}
