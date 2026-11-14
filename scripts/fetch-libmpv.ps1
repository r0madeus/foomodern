<#
.SYNOPSIS
  Downloads the LGPL libmpv build for Windows into src-tauri/lib.

.DESCRIPTION
  Picks the newest "mpv-dev-lgpl-x86_64-<date>-git-<hash>.7z" asset from
  zhongfly/mpv-winbuild (the plain x86_64 build, not "v3", which needs newer
  CPU instructions), extracts libmpv-2.dll and creates the MSVC import
  library mpv.lib that the Rust build links against.

  Works with Windows PowerShell 5.1 and PowerShell 7. Needs the MSVC build
  tools (already required by Rust on Windows) and either the built-in
  tar.exe with 7z support or 7-Zip (winget install 7zip.7zip).

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/fetch-libmpv.ps1
#>
param(
    [string]$Dest = (Join-Path $PSScriptRoot '..\src-tauri\lib'),
    [string]$Arch = 'x86_64',
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$Dest = [IO.Path]::GetFullPath($Dest)
New-Item -ItemType Directory -Force -Path $Dest | Out-Null
$dllOut = Join-Path $Dest 'libmpv-2.dll'
$libOut = Join-Path $Dest 'mpv.lib'
if ((Test-Path $dllOut) -and (Test-Path $libOut) -and -not $Force) {
    Write-Host "libmpv already present in $Dest (use -Force to re-download)."
    exit 0
}

# --- 1. Find the release asset -------------------------------------------
$headers = @{ 'User-Agent' = 'neomodern-fetch-libmpv'; 'Accept' = 'application/vnd.github+json' }
if ($env:GITHUB_TOKEN) { $headers['Authorization'] = "Bearer $($env:GITHUB_TOKEN)" }
$release = Invoke-RestMethod -Headers $headers -Uri 'https://api.github.com/repos/zhongfly/mpv-winbuild/releases/latest'
$pattern = "^mpv-dev-lgpl-$Arch-\d{8}-git-[0-9a-f]+\.7z$"
$asset = $release.assets | Where-Object { $_.name -match $pattern } | Select-Object -First 1
if (-not $asset) {
    Write-Host "Assets in release $($release.tag_name):"
    $release.assets | ForEach-Object { Write-Host "  $($_.name)" }
    throw "No asset matches $pattern"
}
Write-Host "Using $($asset.name) from release $($release.tag_name)"

# --- 2. Download and extract ---------------------------------------------
$work = Join-Path ([IO.Path]::GetTempPath()) ("neomodern-libmpv-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $work | Out-Null
try {
    $archive = Join-Path $work $asset.name
    Invoke-WebRequest -UseBasicParsing -Headers @{ 'User-Agent' = 'neomodern-fetch-libmpv' } -Uri $asset.browser_download_url -OutFile $archive
    $extract = Join-Path $work 'x'
    New-Item -ItemType Directory -Force -Path $extract | Out-Null

    $sevenZip = (Get-Command 7z -ErrorAction SilentlyContinue).Source
    if (-not $sevenZip -and (Test-Path "$env:ProgramFiles\7-Zip\7z.exe")) { $sevenZip = "$env:ProgramFiles\7-Zip\7z.exe" }
    if ($sevenZip) {
        & $sevenZip x -y "-o$extract" $archive | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "7-Zip failed with exit code $LASTEXITCODE" }
    } else {
        & tar.exe -xf $archive -C $extract
        if ($LASTEXITCODE -ne 0) { throw "tar.exe could not extract the .7z archive. Install 7-Zip (winget install 7zip.7zip) and run again." }
    }

    $dll = Get-ChildItem -Path $extract -Recurse -Filter 'libmpv-2.dll' | Select-Object -First 1
    if (-not $dll) { throw "libmpv-2.dll not found in $($asset.name)" }

    # --- 3. MSVC tools -----------------------------------------------------
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) { throw "vswhere.exe not found; install the Visual Studio C++ build tools (needed by Rust anyway)." }
    $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vs) { throw "MSVC x64 build tools not found." }
    $toolsBin = Get-ChildItem -Path (Join-Path $vs 'VC\Tools\MSVC') -Directory |
        Sort-Object Name -Descending | Select-Object -First 1 |
        ForEach-Object { Join-Path $_.FullName 'bin\Hostx64\x64' }
    $libExe = Join-Path $toolsBin 'lib.exe'
    $dumpbin = Join-Path $toolsBin 'dumpbin.exe'

    # --- 4. Import library -------------------------------------------------
    $def = Get-ChildItem -Path $extract -Recurse -Filter '*.def' | Select-Object -First 1
    if ($def) {
        $defPath = $def.FullName
    } else {
        # No .def shipped: build one from the DLL's export table.
        $defPath = Join-Path $work 'mpv.def'
        $names = & $dumpbin /nologo /exports $dll.FullName |
            ForEach-Object { if ($_ -match '^\s+\d+\s+[0-9A-Fa-f]+\s+[0-9A-Fa-f]{8}\s+(\S+)') { $Matches[1] } }
        if (-not $names) { throw "dumpbin found no exports in libmpv-2.dll" }
        (@('LIBRARY libmpv-2', 'EXPORTS') + $names) | Set-Content -Encoding ASCII -Path $defPath
    }
    & $libExe /nologo "/def:$defPath" /name:libmpv-2.dll "/out:$libOut" /machine:x64 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "lib.exe failed with exit code $LASTEXITCODE" }

    Copy-Item -Force $dll.FullName $dllOut
    @(
        "asset: $($asset.name)",
        "release: $($release.tag_name)",
        "url: $($asset.browser_download_url)",
        "license: LGPL-2.1-or-later (mpv LGPL build); source: https://github.com/mpv-player/mpv"
    ) | Set-Content -Encoding UTF8 -Path (Join-Path $Dest 'SOURCE.txt')
    Write-Host "libmpv ready in $Dest"
} finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
