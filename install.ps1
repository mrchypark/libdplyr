# libdplyr installation script for Windows
# Supports Windows 10/11 with PowerShell 5.1+

param(
    [string]$Version = "0.1.0",
    [string]$InstallDir = "$env:USERPROFILE\.local\bin",
    [switch]$Latest,
    [switch]$Help
)

# Configuration
$REPO = "example/libdplyr"  # Change this to your actual GitHub repository path

# Color definitions for console output
$Colors = @{
    Red = "Red"
    Green = "Green"
    Yellow = "Yellow"
    Blue = "Blue"
    White = "White"
}

# Logging functions
function Write-Info {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor $Colors.Blue
}

function Write-Success {
    param([string]$Message)
    Write-Host "[SUCCESS] $Message" -ForegroundColor $Colors.Green
}

function Write-Warning {
    param([string]$Message)
    Write-Host "[WARNING] $Message" -ForegroundColor $Colors.Yellow
}

function Write-Error {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor $Colors.Red
}

# Show help
function Show-Help {
    Write-Host @"
libdplyr installation script for Windows

Usage:
  .\install.ps1 [options]

Options:
  -Version <VERSION>      Install specific version
  -InstallDir <DIRECTORY> Specify installation directory (default: $env:USERPROFILE\.local\bin)
  -Latest                 Force check for latest version
  -Help                   Show this help message

Examples:
  .\install.ps1                           # Install latest version
  .\install.ps1 -Version 0.2.0           # Install specific version
  .\install.ps1 -InstallDir "C:\Tools"   # Custom installation directory

Environment variables:
  LIBDPLYR_INSTALL_DIR    Installation directory override

"@
}

# Detect system architecture
function Get-SystemArchitecture {
    $arch = $env:PROCESSOR_ARCHITECTURE
    switch ($arch) {
        "AMD64" { return "x86_64" }
        "ARM64" { return "aarch64" }
        default {
            Write-Error "Unsupported architecture: $arch"
            exit 1
        }
    }
}

# Get latest version from GitHub API
function Get-LatestVersion {
    try {
        Write-Info "Fetching latest version from GitHub..."
        $response = Invoke-RestMethod -Uri "https://api.github.com/repos/$REPO/releases/latest" -Method Get
        $latestVersion = $response.tag_name -replace '^v', ''
        Write-Info "Latest version: $latestVersion"
        return $latestVersion
    }
    catch {
        Write-Warning "Could not fetch latest version. Using default version $Version."
        return $Version
    }
}

# Generate download URL
function Get-DownloadUrl {
    param([string]$Version, [string]$Architecture)
    
    $filename = "libdplyr-windows-$Architecture.exe.zip"
    $url = "https://github.com/$REPO/releases/download/v$Version/$filename"
    Write-Info "Download URL: $url"
    return $url
}

# Download and install binary
function Install-Binary {
    param([string]$DownloadUrl, [string]$InstallDir, [string]$Version)
    
    # Create temporary directory
    $tempDir = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }
    Write-Info "Temporary directory: $tempDir"
    
    try {
        # Download
        Write-Info "Downloading libdplyr v$Version..."
        $zipPath = Join-Path $tempDir "libdplyr.zip"
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $zipPath -UseBasicParsing
        
        # Extract
        Write-Info "Extracting archive..."
        Expand-Archive -Path $zipPath -DestinationPath $tempDir -Force
        
        # Create install directory
        if (-not (Test-Path $InstallDir)) {
            New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        }
        
        # Copy binary
        Write-Info "Installing to $InstallDir..."
        $exePath = Get-ChildItem -Path $tempDir -Filter "*.exe" | Select-Object -First 1
        if ($exePath) {
            Copy-Item -Path $exePath.FullName -Destination (Join-Path $InstallDir "libdplyr.exe") -Force
        } else {
            Write-Error "Could not find libdplyr.exe in the downloaded archive"
            exit 1
        }
        
        Write-Success "libdplyr v$Version has been successfully installed!"
    }
    finally {
        # Clean up temporary files
        Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# Check PATH and provide guidance
function Test-PathConfiguration {
    param([string]$InstallDir)
    
    $currentPath = $env:PATH -split ';'
    if ($InstallDir -notin $currentPath) {
        Write-Warning "$InstallDir is not in your PATH."
        Write-Host ""
        Write-Host "To add it to your PATH permanently, run one of the following:"
        Write-Host ""
        Write-Host "For current user only:"
        Write-Host "  [Environment]::SetEnvironmentVariable('PATH', `$env:PATH + ';$InstallDir', 'User')" -ForegroundColor Cyan
        Write-Host ""
        Write-Host "For system-wide (requires admin):"
        Write-Host "  [Environment]::SetEnvironmentVariable('PATH', `$env:PATH + ';$InstallDir', 'Machine')" -ForegroundColor Cyan
        Write-Host ""
        Write-Host "Or for the current session only:"
        Write-Host "  `$env:PATH += ';$InstallDir'" -ForegroundColor Cyan
        Write-Host ""
    }
}

# Verify installation
function Test-Installation {
    param([string]$InstallDir)
    
    $exePath = Join-Path $InstallDir "libdplyr.exe"
    if (Test-Path $exePath) {
        Write-Success "Installation verified: $exePath"
        
        # Check version
        try {
            $versionOutput = & $exePath --version 2>$null
            if ($versionOutput) {
                $installedVersion = ($versionOutput | Select-String '\d+\.\d+\.\d+').Matches[0].Value
                Write-Success "Installed version: $installedVersion"
            }
        }
        catch {
            Write-Warning "Could not verify version, but binary exists"
        }
        
        Write-Host ""
        Write-Host "Usage:"
        Write-Host "  libdplyr --help"
        Write-Host "  echo 'data %>% select(name, age)' | libdplyr --dialect postgresql"
    }
    else {
        Write-Error "Installation verification failed"
        exit 1
    }
}

# Main function
function Main {
    # Show help if requested
    if ($Help) {
        Show-Help
        exit 0
    }
    
    # Check for environment variable override
    if ($env:LIBDPLYR_INSTALL_DIR) {
        $InstallDir = $env:LIBDPLYR_INSTALL_DIR
    }
    
    Write-Info "Starting libdplyr installation script"
    
    # Detect system architecture
    $architecture = Get-SystemArchitecture
    Write-Info "Detected architecture: $architecture"
    
    # Get latest version if requested or using default
    if ($Latest -or $Version -eq "0.1.0") {
        $Version = Get-LatestVersion
    }
    
    # Generate download URL
    $downloadUrl = Get-DownloadUrl -Version $Version -Architecture $architecture
    
    # Execute installation
    Install-Binary -DownloadUrl $downloadUrl -InstallDir $InstallDir -Version $Version
    
    # Check PATH configuration
    Test-PathConfiguration -InstallDir $InstallDir
    
    # Verify installation
    Test-Installation -InstallDir $InstallDir
    
    Write-Success "Installation completed!"
}

# Execute main function
try {
    Main
}
catch {
    Write-Error "Installation failed: $($_.Exception.Message)"
    exit 1
}