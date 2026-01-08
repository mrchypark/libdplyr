# Installation Guide

## 🚀 Quick Install (Recommended)

**Linux/macOS:**
```bash
# Install latest version
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash

# Install to custom directory
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash -s -- --dir=$HOME/bin

# Install specific version
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash -s -- --version=v0.1.0
```

**Windows (PowerShell):**
```powershell
# Install latest version
Irm https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.ps1 | iex

# Install to custom directory
Irm https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.ps1 | iex -Dir "C:\Tools"
```

## 📦 Supported Platforms

| Platform | Architecture | Status | Installation Method |
|----------|-------------|--------|-------------------|
| **Linux** | x86_64 | ✅ Fully Supported | `curl -sSL ... \| bash` |
| **Linux** | ARM64 (aarch64) | ✅ Fully Supported | `curl -sSL ... \| bash` |
| **macOS** | Intel (x86_64) | ✅ Fully Supported | `curl -sSL ... \| bash` |
| **macOS** | Apple Silicon (ARM64) | ✅ Fully Supported | `curl -sSL ... \| bash` |
| **Windows** | x86_64 | ✅ Fully Supported | `Irm ... \| iex` |
| **Windows** | ARM64 | ✅ Fully Supported | `Irm ... \| iex` |

## 🛠 Advanced Installation

### With Version Management
```bash
# Download advanced installer
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/scripts/install-advanced.sh -o install-advanced.sh
chmod +x install-advanced.sh

# List available versions
./install-advanced.sh --list-versions

# Install specific version with auto-update
./install-advanced.sh --version=v0.1.0 --auto-update

# Install to custom directory with force reinstall
./install-advanced.sh --dir=$HOME/tools --force
```

### Install via Cargo
```bash
# Use as library
cargo add libdplyr

# Install CLI tool from source
cargo install libdplyr
```

### Build from Source
```bash
git clone https://github.com/mrchypark/libdplyr.git
cd libdplyr
cargo build --release

# The binary will be available at target/release/libdplyr
# Copy it to a directory in your PATH
cp target/release/libdplyr /usr/local/bin/  # Linux/macOS
```

## 🔧 Installation Options & Troubleshooting

### Script Options

| Option | install.sh | install.ps1 | Description |
|--------|------------|-------------|-------------|
| `--help` / `-h` | ✅ | `-Help` | Show detailed help message |
| `--version VER` / `-v VER` | ✅ | `-Version VER` | Install specific version (e.g., v1.0.0) |
| `--dir PATH` / `-d PATH` | ✅ | `-Dir PATH` | Custom installation directory |
| `--dry-run` | ✅ | `-DryRun` | Preview installation without changes |
| `--debug` | ✅ | `-Debug` | Enable verbose debug output |

### Common Issues

**1. Permission Denied**
```bash
# Option 1: Install to user directory (Recommended)
./install.sh --dir $HOME/.local/bin

# Option 2: Use sudo (Not recommended)
sudo ./install.sh
```

**2. Command Not Found**
If `libdplyr` is not found after installation, you need to add the installation directory to your PATH:
```bash
export PATH="$HOME/.local/bin:$PATH"
# Add to ~/.bashrc or ~/.zshrc to make permanent
```

**3. Network Issues**
Use debug mode to diagnose connectivity:
```bash
./install.sh --debug
```

**4. Verify Installation**
```bash
libdplyr --version
# Should output: libdplyr x.y.z
```
