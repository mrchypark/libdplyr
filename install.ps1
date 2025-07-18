# libdplyr Installer Script for Windows
# This script installs libdplyr on Windows systems

# Version
$VERSION = "0.1.0"
$REPO_URL = "https://github.com/mrchyaprk/libdplyr"
$RELEASE_URL = "https://github.com/mrchyaprk/libdplyr/releases/download/v${VERSION}"

# Default installation directory
$DEFAULT_INSTALL_DIR = "$env:LOCALAPPDATA\Programs\libdplyr"
$INSTALL_DIR = $DEFAULT_INSTALL_DIR

# Show help message
function Show-Help {
    Write-Host "libdplyr Installer v$VERSION"
    Write-Host ""
    Write-Host "Usage: .\install.ps1 [OPTIONS]"
    Write-Host ""
    Write-Host "Options:"
    Write-Host "  -Dir PATH       Install libdplyr to PATH (default: $DEFAULT_INSTALL_DIR)"
    Write-Host "  -Help           Show this help message"
    Write-Host "  -Version        Show installer version"
    Write-Host ""
    Write-Host "Examples:"
    Write-Host "  .\install.ps1"
    Write-Host "  .\install.ps1 -Dir 'C:\Tools'"
}

# Parse command line arguments
function Parse-Args {
    param (
        [Parameter(ValueFromRemainingArguments=$true)]
        $RemainingArgs
    )
    
    for ($i = 0; $i -lt $RemainingArgs.Count; $i++) {
        $arg = $RemainingArgs[$i]
        
        if ($arg -eq "-Dir" -and $i+1 -lt $RemainingArgs.Count) {
            $script:INSTALL_DIR = $RemainingArgs[$i+1]
            $i++
        }
        elseif ($arg -eq "-Help") {
            Show-Help
            exit 0
        }
        elseif ($arg -eq "-Version") {
            Write-Host "libdplyr installer v$VERSION"
            exit 0
        }
        else {
            Write-Host "Error: Unknown option: $arg" -ForegroundColor Red
            Show-Help
            exit 1
        }
    }
}

# Detect architecture
function Detect-Architecture {
    $arch = [System.Environment]::GetEnvironmentVariable("PROCESSOR_ARCHITECTURE")
    
    if ($arch -eq "AMD64") {
        return "x86_64"
    }
    elseif ($arch -eq "ARM64") {
        return "aarch64"
    }
    else {
        Write-Host "Error: Unsupported architecture: $arch" -ForegroundColor Red
        Write-Host "This installer supports x86_64 and ARM64 architectures only."
        exit 1
    }
}

# Download libdplyr
function Download-Libdplyr {
    $arch = Detect-Architecture
    Write-Host "Downloading libdplyr v$VERSION for windows-$arch..." -ForegroundColor Blue
    
    $tempDir = [System.IO.Path]::GetTempPath() + [System.Guid]::NewGuid().ToString()
    New-Item -ItemType Directory -Path $tempDir | Out-Null
    
    $archive = "libdplyr-${VERSION}-windows-${arch}.zip"
    $downloadUrl = "${RELEASE_URL}/${archive}"
    $archivePath = "$tempDir\$archive"
    
    Write-Host "Download URL: $downloadUrl" -ForegroundColor Cyan
    
    try {
        # Download the archive
        Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath
        Write-Host "✓ Download complete" -ForegroundColor Green
        
        # Extract the archive
        Write-Host "Extracting..." -ForegroundColor Blue
        Expand-Archive -Path $archivePath -DestinationPath $tempDir -Force
        Write-Host "✓ Extraction complete" -ForegroundColor Green
        
        # Create installation directory if it doesn't exist
        if (-not (Test-Path $INSTALL_DIR)) {
            Write-Host "Creating installation directory: $INSTALL_DIR" -ForegroundColor Yellow
            New-Item -ItemType Directory -Path $INSTALL_DIR -Force | Out-Null
        }
        
        # Move the binary to the installation directory
        Write-Host "Installing to $INSTALL_DIR..." -ForegroundColor Blue
        Copy-Item -Path "$tempDir\libdplyr.exe" -Destination "$INSTALL_DIR\libdplyr.exe" -Force
        
        # Clean up
        Remove-Item -Path $tempDir -Recurse -Force
        
        Write-Host "✓ Installation complete" -ForegroundColor Green
        return $true
    }
    catch {
        Write-Host "Error: Failed to download or install libdplyr" -ForegroundColor Red
        Write-Host $_.Exception.Message
        
        # Clean up
        if (Test-Path $tempDir) {
            Remove-Item -Path $tempDir -Recurse -Force
        }
        
        return $false
    }
}

# Add to PATH
function Add-ToPath {
    $userPath = [System.Environment]::GetEnvironmentVariable("PATH", "User")
    
    if (-not $userPath.Contains($INSTALL_DIR)) {
        Write-Host "Adding libdplyr to your PATH..." -ForegroundColor Blue
        
        $newPath = "$userPath;$INSTALL_DIR"
        [System.Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
        
        # Also update the current session's PATH
        $env:PATH = "$env:PATH;$INSTALL_DIR"
        
        Write-Host "✓ Added to PATH" -ForegroundColor Green
    }
    else {
        Write-Host "✓ libdplyr is already in PATH" -ForegroundColor Green
    }
}

# Verify installation
function Verify-Installation {
    Write-Host "Verifying installation..." -ForegroundColor Blue
    
    $libdplyrPath = "$INSTALL_DIR\libdplyr.exe"
    
    if (Test-Path $libdplyrPath) {
        try {
            $versionOutput = & $libdplyrPath --version
            Write-Host "✓ libdplyr is working: $versionOutput" -ForegroundColor Green
        }
        catch {
            Write-Host "Error: libdplyr installation verification failed" -ForegroundColor Red
            Write-Host $_.Exception.Message
            exit 1
        }
    }
    else {
        Write-Host "Error: libdplyr executable not found at $libdplyrPath" -ForegroundColor Red
        exit 1
    }
    
    Write-Host "libdplyr has been successfully installed to $INSTALL_DIR\libdplyr.exe" -ForegroundColor Green
    Write-Host "Try it out:" -ForegroundColor Blue
    Write-Host "  echo 'select(name, age) %>% filter(age > 18)' | libdplyr --pretty"
}

# Main function
function Main {
    Write-Host "libdplyr Installer v$VERSION" -ForegroundColor Magenta
    Write-Host ""
    
    # Parse command line arguments
    Parse-Args $args
    
    # Download and install
    $success = Download-Libdplyr
    
    if (-not $success) {
        Write-Host "Error: Installation failed" -ForegroundColor Red
        exit 1
    }
    
    # Add to PATH
    Add-ToPath
    
    # Verify installation
    Verify-Installation
    
    Write-Host ""
    Write-Host "Thank you for installing libdplyr!" -ForegroundColor Magenta
}

# Run the main function
Main $args