# Installs the lxsurf binary on Windows from a GitHub release:
#
#   irm https://raw.githubusercontent.com/rsaryev/decision-loop/master/scripts/install.ps1 | iex
#
# Environment:
#   LXSURF_VERSION      a release such as 0.2.0 (default: the latest)
#   LXSURF_INSTALL_DIR  where lxsurf.exe goes (default: %LOCALAPPDATA%\Programs\lxsurf)
#
# It downloads lxsurf-x86_64-pc-windows-msvc.zip and SHA256SUMS, refuses an archive whose
# SHA-256 does not match, and adds the folder to the user's PATH.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$repo = 'rsaryev/decision-loop'
$dir = if ($env:LXSURF_INSTALL_DIR) { $env:LXSURF_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\lxsurf' }
$base = if ($env:LXSURF_VERSION) {
    "https://github.com/$repo/releases/download/v$($env:LXSURF_VERSION.TrimStart('v'))"
} else {
    "https://github.com/$repo/releases/latest/download"
}

if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64' -and $env:PROCESSOR_ARCHITEW6432 -ne 'AMD64') {
    throw "lxsurf install: no Windows build for $env:PROCESSOR_ARCHITECTURE yet"
}
$archive = 'lxsurf-x86_64-pc-windows-msvc.zip'

$tmp = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    Write-Host "Downloading $archive"
    Invoke-WebRequest -UseBasicParsing -Uri "$base/$archive" -OutFile (Join-Path $tmp $archive)
    Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile (Join-Path $tmp 'SHA256SUMS')

    $line = Get-Content (Join-Path $tmp 'SHA256SUMS') | Where-Object { ($_ -split '\s+\*?')[1] -eq $archive }
    if (-not $line) { throw "lxsurf install: SHA256SUMS lists no $archive" }
    $expected = ($line -split '\s+')[0].ToLower()
    $actual = (Get-FileHash -Algorithm SHA256 (Join-Path $tmp $archive)).Hash.ToLower()
    if ($actual -ne $expected) { throw "lxsurf install: checksum mismatch for ${archive}: expected $expected, got $actual" }

    Expand-Archive -Path (Join-Path $tmp $archive) -DestinationPath (Join-Path $tmp 'x') -Force
    New-Item -ItemType Directory -Path $dir -Force | Out-Null
    Copy-Item (Join-Path $tmp 'x\lxsurf.exe') (Join-Path $dir 'lxsurf.exe') -Force
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
Write-Host "Installed lxsurf to $dir\lxsurf.exe"

$userPath = [string][Environment]::GetEnvironmentVariable('Path', 'User')
if (-not (($userPath -split ';') -contains $dir)) {
    [Environment]::SetEnvironmentVariable('Path', ($userPath.TrimEnd(';') + ";$dir").TrimStart(';'), 'User')
    $env:Path += ";$dir"
    Write-Host "Added $dir to your user PATH (new terminals see it)."
}

Write-Host ''
Write-Host 'Next steps:'
Write-Host '  lxsurf login     # sign in with OpenRouter (or set OPENROUTER_API_KEY)'
Write-Host '  lxsurf doctor    # check the key, the models and the WebView2 runtime'
