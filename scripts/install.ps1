# Installs the llx binary on Windows from a GitHub release:
#
#   irm https://raw.githubusercontent.com/rsaryev/llx/main/scripts/install.ps1 | iex
#
# Environment:
#   LLX_VERSION      a release such as 0.2.0 (default: the latest)
#   LLX_INSTALL_DIR  where llx.exe goes (default: %LOCALAPPDATA%\Programs\llx)
#
# It downloads llx-x86_64-pc-windows-msvc.zip and SHA256SUMS, refuses an archive whose
# SHA-256 does not match, and adds the folder to the user's PATH.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$repo = 'rsaryev/llx'
$dir = if ($env:LLX_INSTALL_DIR) { $env:LLX_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\llx' }
$base = if ($env:LLX_VERSION) {
    "https://github.com/$repo/releases/download/v$($env:LLX_VERSION.TrimStart('v'))"
} else {
    "https://github.com/$repo/releases/latest/download"
}

if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64' -and $env:PROCESSOR_ARCHITEW6432 -ne 'AMD64') {
    throw "llx install: no Windows build for $env:PROCESSOR_ARCHITECTURE yet"
}
$archive = 'llx-x86_64-pc-windows-msvc.zip'

$tmp = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    Write-Host "Downloading $archive"
    Invoke-WebRequest -UseBasicParsing -Uri "$base/$archive" -OutFile (Join-Path $tmp $archive)
    Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile (Join-Path $tmp 'SHA256SUMS')

    $line = Get-Content (Join-Path $tmp 'SHA256SUMS') | Where-Object { ($_ -split '\s+\*?')[1] -eq $archive }
    if (-not $line) { throw "llx install: SHA256SUMS lists no $archive" }
    $expected = ($line -split '\s+')[0].ToLower()
    $actual = (Get-FileHash -Algorithm SHA256 (Join-Path $tmp $archive)).Hash.ToLower()
    if ($actual -ne $expected) { throw "llx install: checksum mismatch for ${archive}: expected $expected, got $actual" }

    Expand-Archive -Path (Join-Path $tmp $archive) -DestinationPath (Join-Path $tmp 'x') -Force
    New-Item -ItemType Directory -Path $dir -Force | Out-Null
    Copy-Item (Join-Path $tmp 'x\llx.exe') (Join-Path $dir 'llx.exe') -Force
    # The short name, unless another program already has it.
    $short = Join-Path $dir 'lx.exe'
    if (-not (Test-Path $short) -or ((Get-Item $short).Length -eq (Get-Item (Join-Path $dir 'llx.exe')).Length)) {
        Copy-Item (Join-Path $dir 'llx.exe') $short -Force
    }
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
Write-Host "Installed llx to $dir\llx.exe"

$userPath = [string][Environment]::GetEnvironmentVariable('Path', 'User')
if (-not (($userPath -split ';') -contains $dir)) {
    [Environment]::SetEnvironmentVariable('Path', ($userPath.TrimEnd(';') + ";$dir").TrimStart(';'), 'User')
    $env:Path += ";$dir"
    Write-Host "Added $dir to your user PATH (new terminals see it)."
}

Write-Host ''
Write-Host 'Next steps:'
Write-Host '  llx           # a session: type /login to sign in (ChatGPT or OpenRouter)'
Write-Host '  llx doctor    # check the sign-ins, the models and the WebView2 runtime'
