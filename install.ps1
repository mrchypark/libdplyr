# libdplyr installation script for Windows
# Usage: Irm https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.ps1 | iex

param(
    [string]$Version = "",
    [string]$InstallDir = "$env:LOCALAPPDATA\libdplyr\bin",
    [switch]$Help
)

$ErrorActionPreference = "Stop"
$REPO = "mrchypark/libdplyr"
$BINARY_NAME = "libdplyr.exe"
$DEFAULT_VERSION = "0.1.0"

function Write-Info { param([string]$Message); Write-Host "[INFO] $Message" -ForegroundColor Blue }
function Write-Success { param([string]$Message); Write-Host "[SUCCESS] $Message" -ForegroundColor Green }
function Write-Warning { param([string]$Message); Write-Host "[WARNING] $Message" -ForegroundColor Yellow }
function Write-Error { param([string]$Message); Write-Host "[ERROR] $Message" -ForegroundColor Red }

function Show-Help {
    Write-Host "libdplyr installation script for Windows"
    Write-Host "Usage: .\install.ps1 [OPTIONS]"
    Write-Host "Options:"
    Write-Host "  -Version VERSION     Install specific version (default: latest)"
    Write-Host "  -InstallDir DIR      Installation directory"
    Write-Host "  -Help               Show this help message"
}

function Get-LatestVersion {
    try {
        $response = Invoke-RestMethod -Uri "https://api.github.com/repos/$REPO/releases/latest"
        return $response.tag_name -replace '^v', ''
    } catch {
        Write-Warning "Could not fetch latest version"
        return $null
    }
}

function Install-Binary {
    param([string]$Version, [string]$InstallDirectory)
    
    $platform = "windows-x86_64"
    $filename = "libdplyr-v$Version-$platform"
    $archiveName = "$filename.zip"
    $downloadUrl = "https://github.com/$REPO/releases/download/v$Version/$archiveName"
    
    Write-Info "Downloading libdplyr v$Version..."
    
    $tempDir = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }
    $archivePath = Join-Path $tempDir $archiveName
    
    Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath -UseBasicParsing
    Expand-Archive -Path $archivePath -DestinationPath $tempDir -Force
    
    $binaryPath = Get-ChildItem -Path $tempDir -Name $BINARY_NAME -Recurse | Select-Object -First 1
    if (-not $binaryPath) { throw "Binary not found in archive" }
    
    if (-not (Test-Path $InstallDirectory)) {
        New-Item -ItemType Directory -Path $InstallDirectory -Force | Out-Null
    }
    
    $destinationPath = Join-Path $InstallDirectory $BINARY_NAME
    Copy-Item -Path (Join-Path $tempDir $binaryPath) -Destination $destinationPath -Force
    Remove-Item -Path $tempDir -Recurse -Force
    
    Write-Success "libdplyr v$Version installed successfully!"
    return $destinationPath
}

function Add-ToPath {
    param([string]$Directory)
    
    $currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($currentPath -split ';' -contains $Directory) { return }
    
    $newPath = if ($currentPath) { "$currentPath;$Directory" } else { $Directory }
    [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
    $env:PATH = "$env:PATH;$Directory"
    
    Write-Success "Added to PATH successfully!"
}

if ($Help) { Show-Help; return }

Write-Info "Starting libdplyr installation..."

$versionToInstall = if ($Version) { $Version } else { 
    $latest = Get-LatestVersion
    if ($latest) { $latest } else { $DEFAULT_VERSION }
}

Write-Info "Installing version: $versionToInstall"

try {
    $binaryPath = Install-Binary -Version $versionToInstall -InstallDirectory $InstallDir
    Add-ToPath -Directory $InstallDir
    
    Write-Success "Installation complete!"
    Write-Host "Run 'libdplyr --help' to get started"
} catch {
    Write-Error "Installation failed: $($_.Exception.Message)"
    exit 1
}