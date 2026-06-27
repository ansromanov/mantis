#!/usr/bin/env pwsh
# mantis installer for Windows.
#
# Downloads the prebuilt mantis.exe from GitHub Releases, verifies its SHA-256
# checksum, and installs it on your PATH.
#
# Usage (PowerShell):
#   irm https://raw.githubusercontent.com/ansromanov/mantis/main/install.ps1 | iex
#
# From cmd.exe:
#   powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/ansromanov/mantis/main/install.ps1 | iex"
#
# Environment overrides:
#   $env:MANTIS_VERSION      release tag to install (default: latest), e.g. "v0.2.0"
#   $env:MANTIS_INSTALL_DIR  directory to install into (default: auto-detected)

$ErrorActionPreference = 'Stop'
$ProgressPreference    = 'SilentlyContinue'   # suppresses slow Invoke-WebRequest progress bar

$Repo    = 'ansromanov/mantis'
$BinName = 'mantis'
$Version    = if ($env:MANTIS_VERSION)    { $env:MANTIS_VERSION }    else { 'latest' }
$InstallDir = if ($env:MANTIS_INSTALL_DIR){ $env:MANTIS_INSTALL_DIR } else { $null }

function Write-Step { Write-Host "==> $args" }
function Write-Warn { Write-Host "warning: $args" -ForegroundColor Yellow }

# Only x86_64 Windows binaries are released; ARM64 users should use cargo install.
$cpu = $env:PROCESSOR_ARCHITECTURE
if ($cpu -notin @('AMD64', 'x86_64')) {
    Write-Error ("Unsupported CPU architecture: $cpu. " +
                 "Only x86_64 Windows is supported. " +
                 "Use 'cargo install mantis' instead.")
    exit 1
}
$Asset = "${BinName}-windows-x86_64.exe"

# Resolve download base URL.
$BaseUrl = if ($Version -eq 'latest') {
    "https://github.com/$Repo/releases/latest/download"
} else {
    "https://github.com/$Repo/releases/download/$Version"
}

# Pick an install directory.
if (-not $InstallDir) {
    $CargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
    $CargoBin  = Join-Path $CargoHome 'bin'
    if (Test-Path $CargoBin) {
        $InstallDir = $CargoBin
    } else {
        $InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\mantis'
    }
}

$TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $TmpDir | Out-Null

try {
    $TmpBin  = Join-Path $TmpDir $Asset
    $TmpSums = Join-Path $TmpDir 'SHA256SUMS'

    Write-Step "Downloading $Asset ($Version)"
    try {
        Invoke-WebRequest -Uri "$BaseUrl/$Asset" -OutFile $TmpBin -UseBasicParsing
    } catch {
        Write-Error "Failed to download ${BaseUrl}/${Asset}: $_"
        exit 1
    }

    Write-Step 'Verifying checksum'
    try {
        Invoke-WebRequest -Uri "$BaseUrl/SHA256SUMS" -OutFile $TmpSums -UseBasicParsing
    } catch {
        Write-Error "Failed to download checksum file from ${BaseUrl}/SHA256SUMS: $_"
        exit 1
    }

    $Actual = (Get-FileHash -Path $TmpBin -Algorithm SHA256).Hash.ToLower()

    # Match lines of the form: <hash>  [*]<asset>
    $Pattern  = "^([0-9a-f]{64})\s+\*?$([regex]::Escape($Asset))\s*$"
    $SumsLine = Get-Content $TmpSums | Where-Object { $_ -match $Pattern } | Select-Object -First 1
    if (-not $SumsLine) {
        Write-Error "No checksum for '$Asset' found in SHA256SUMS."
        exit 1
    }
    $Expected = ($SumsLine -split '\s+')[0].ToLower()

    if ($Actual -ne $Expected) {
        Write-Error ("Checksum mismatch for ${Asset}:`n" +
                     "  expected: $Expected`n" +
                     "  actual:   $Actual")
        exit 1
    }

    # Install the binary.
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir | Out-Null
    }
    $Dest = Join-Path $InstallDir "${BinName}.exe"
    Move-Item -Force -Path $TmpBin -Destination $Dest

    Write-Step "Installed $BinName to $Dest"

    # Add install dir to the user PATH if it is not already there.
    $UserPath = [System.Environment]::GetEnvironmentVariable('PATH', [System.EnvironmentVariableTarget]::User)
    if ($UserPath -notlike "*$InstallDir*") {
        [System.Environment]::SetEnvironmentVariable(
            'PATH',
            "$InstallDir;$UserPath",
            [System.EnvironmentVariableTarget]::User
        )
        Write-Warn "$InstallDir added to your PATH. Restart your terminal for it to take effect."
    }

    Write-Host ''
    Write-Host "Run '$BinName' to browse the current directory (press '?' for help)." -ForegroundColor Green
} finally {
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
}
