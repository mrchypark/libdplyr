# Installation Guide

## 🚀 Quick Install (Recommended)

**Linux/macOS:**
```bash
# Install latest version
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash

# Install to custom directory
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash -s -- --install-dir "$HOME/bin"

# Install specific version
curl -sSL https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.sh | bash -s -- --version 0.7.0
```

**Windows (PowerShell):**
```powershell
# Install latest version
Irm https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.ps1 | iex

# Install to custom directory
Invoke-WebRequest https://raw.githubusercontent.com/mrchypark/libdplyr/main/install.ps1 -OutFile install.ps1
.\install.ps1 -Version 0.7.0 -InstallDir "C:\Tools"
```

## 📦 Supported Platforms

| Platform | Architecture | Status | Installation Method |
|----------|-------------|--------|-------------------|
| **Linux** | x86_64 | ✅ Fully Supported | `curl -sSL ... \| bash` |
| **Linux** | ARM64 (aarch64) | No prebuilt CLI archive | See source build below |
| **macOS** | Intel (x86_64) | ✅ Fully Supported | `curl -sSL ... \| bash` |
| **macOS** | Apple Silicon (ARM64) | ✅ Fully Supported | `curl -sSL ... \| bash` |
| **Windows** | x86_64 | ✅ Fully Supported | `Irm ... \| iex` |
| **Windows** | ARM64 | No prebuilt CLI archive | See source build below |

CLI archives are attached to the [GitHub release](https://github.com/mrchypark/libdplyr/releases/tag/v0.7.0).
They are separate from the DuckDB extension binaries. `cli-checksums.sha256` covers
only CLI archives; `checksums.sha256` covers the extension binaries.
The release asset named `install.sh` installs the DuckDB extension. For the CLI,
use the repository-root installation scripts shown above.

## 🛠 Advanced Installation

### Install via Cargo

`libdplyr` is not currently published on crates.io. Use the release tag on GitHub:

```bash
# Use as library
cargo add libdplyr --git https://github.com/mrchypark/libdplyr --tag v0.7.0

# Install CLI tool from source
cargo install --locked --git https://github.com/mrchypark/libdplyr --tag v0.7.0 libdplyr
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
| `--help` | ✅ | `-Help` | Show detailed help message |
| `--version VER` | ✅ | `-Version VER` | Install specific version (e.g., 0.7.0, without `v`) |
| `--install-dir PATH` | ✅ | `-InstallDir PATH` | Custom installation directory |

### Common Issues

**1. Permission Denied**
```bash
# Option 1: Install to user directory (Recommended)
./install.sh --install-dir "$HOME/.local/bin"

# Option 2: Use sudo (Not recommended)
sudo ./install.sh
```

**2. Command Not Found**
If `libdplyr` is not found after installation, you need to add the installation directory to your PATH:
```bash
export PATH="$HOME/.local/bin:$PATH"
# Add to ~/.bashrc or ~/.zshrc to make permanent
```

**3. Verify Installation**
```bash
libdplyr --version
# Should output: libdplyr x.y.z
```
