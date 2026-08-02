# Description: Serve the repository reader and one local sample to Agent Browser over loopback.

[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$BookRoot,
    [ValidateRange(1024, 65535)]
    [int]$Port = 18766
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$readerRoot = (Resolve-Path (Join-Path $repoRoot 'reader')).Path
$bookRootPath = (Resolve-Path -LiteralPath $BookRoot).Path
$separator = [IO.Path]::DirectorySeparatorChar

foreach ($root in @($readerRoot, $bookRootPath)) {
    $item = Get-Item -LiteralPath $root
    if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "Validation root must be a real directory: $root"
    }
}

function Get-SafeFile {
    param(
        [string]$Root,
        [string]$Relative
    )

    if ([string]::IsNullOrWhiteSpace($Relative) -or
        $Relative.Contains([char]0) -or
        $Relative.Contains('\') -or
        $Relative.Contains(':')) {
        throw 'Invalid validation path.'
    }
    $segments = @($Relative.Split('/') | Where-Object { $_ -ne '' })
    if ($segments.Count -eq 0 -or @($segments | Where-Object { $_ -in @('.', '..') }).Count -gt 0) {
        throw 'Invalid validation path.'
    }
    $candidate = [IO.Path]::GetFullPath((Join-Path $Root ([string]::Join($separator, $segments))))
    $rootPrefix = $Root.TrimEnd($separator) + $separator
    if (-not $candidate.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Validation path escaped its root.'
    }
    $current = $Root
    foreach ($segment in $segments) {
        $current = Join-Path $current $segment
        $item = Get-Item -LiteralPath $current -ErrorAction Stop
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw 'Validation path contains a reparse point.'
        }
    }
    if ($item.PSIsContainer -or -not $item.FullName.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Validation path is not a file under its root.'
    }
    return $item.FullName
}

function Get-ContentType {
    param([string]$Path)

    switch ([IO.Path]::GetExtension($Path).ToLowerInvariant()) {
        '.html' { 'text/html; charset=utf-8' }
        '.xhtml' { 'text/plain; charset=utf-8' }
        '.css' { 'text/css; charset=utf-8' }
        '.js' { 'text/javascript; charset=utf-8' }
        '.mjs' { 'text/javascript; charset=utf-8' }
        '.json' { 'application/json; charset=utf-8' }
        '.svg' { 'image/svg+xml' }
        '.png' { 'image/png' }
        '.jpg' { 'image/jpeg' }
        '.jpeg' { 'image/jpeg' }
        '.gif' { 'image/gif' }
        '.webp' { 'image/webp' }
        default { throw 'Unsupported validation media type.' }
    }
}

function Write-Response {
    param(
        [System.Net.HttpListenerContext]$Context,
        [int]$StatusCode,
        [string]$ContentType,
        [byte[]]$Body
    )

    $Context.Response.StatusCode = $StatusCode
    $Context.Response.ContentType = $ContentType
    $Context.Response.ContentLength64 = $Body.Length
    if ($Context.Request.HttpMethod -ne 'HEAD') {
        $Context.Response.OutputStream.Write($Body, 0, $Body.Length)
    }
    $Context.Response.Close()
}

$listener = [Net.HttpListener]::new()
$listener.Prefixes.Add("http://127.0.0.1:$Port/")
$listener.Start()
try {
    while ($listener.IsListening) {
        $context = $listener.GetContext()
        try {
            if ($context.Request.HttpMethod -notin @('GET', 'HEAD')) {
                Write-Response $context 405 'text/plain; charset=utf-8' ([Text.Encoding]::UTF8.GetBytes('method not allowed'))
                continue
            }
            $escapedPath = ($context.Request.RawUrl -split '\?', 2)[0]
            $path = [Uri]::UnescapeDataString($escapedPath)
            if ($path -eq '/health') {
                Write-Response $context 200 'text/plain; charset=utf-8' ([Text.Encoding]::UTF8.GetBytes('ok'))
                continue
            }
            $readerFiles = @{
                '/reader/atha-reader.html' = 'atha-reader.html'
                '/reader/atha-reader.css' = 'atha-reader.css'
            }
            if ($readerFiles.ContainsKey($path)) {
                $file = Get-SafeFile $readerRoot $readerFiles[$path]
                if ($path.EndsWith('.html')) {
                    $context.Response.Headers['Content-Security-Policy'] = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
                }
                Write-Response $context 200 (Get-ContentType $file) ([IO.File]::ReadAllBytes($file))
                continue
            }
            if ($path -eq '/reader/atha-reader.mjs') {
                $source = @(
                    'web/content.mjs',
                    'web/pagination.mjs',
                    'web/session.mjs',
                    'web/diagnostics.mjs',
                    'web/app.mjs'
                ) | ForEach-Object {
                    $file = Get-SafeFile $readerRoot $_
                    [IO.File]::ReadAllText($file)
                }
                $body = [Text.Encoding]::UTF8.GetBytes([string]::Join("`n", $source))
                Write-Response $context 200 'text/javascript; charset=utf-8' $body
                continue
            }
            if ($path.StartsWith('/book/', [StringComparison]::Ordinal)) {
                $file = Get-SafeFile $bookRootPath $path.Substring(6)
                $context.Response.Headers['Access-Control-Allow-Origin'] = "http://127.0.0.1:$Port"
                Write-Response $context 200 (Get-ContentType $file) ([IO.File]::ReadAllBytes($file))
                continue
            }
            Write-Response $context 404 'text/plain; charset=utf-8' ([Text.Encoding]::UTF8.GetBytes('not found'))
        }
        catch {
            if ($context.Response.OutputStream.CanWrite) {
                Write-Response $context 400 'text/plain; charset=utf-8' ([Text.Encoding]::UTF8.GetBytes('bad request'))
            }
        }
    }
}
finally {
    $listener.Stop()
    $listener.Close()
}
