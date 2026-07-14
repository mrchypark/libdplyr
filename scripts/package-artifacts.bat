@echo off
REM Artifact Packaging Script (Windows)
REM R4-AC3: Platform-specific extension binary packaging with metadata and checksums

setlocal enabledelayedexpansion

REM Configuration
if "%VERSION%"=="" (
    for /f "tokens=*" %%i in ('git describe --tags --always --dirty 2^>nul') do set VERSION=%%i
    if "!VERSION!"=="" set VERSION=dev
)

if "%BUILD_DIR%"=="" set BUILD_DIR=build
if "%PACKAGE_DIR%"=="" set PACKAGE_DIR=packages

set PLATFORM=windows
set ARCH=x86_64
set PLATFORM_ARCH=%PLATFORM%-%ARCH%
set EXTENSION_NAME=dplyr

set DUCKDB_VERSION_RAW=%DUCKDB_VERSION%
if not defined DUCKDB_VERSION_RAW (
    echo DUCKDB_VERSION is required for manual packaging. Set it to the exact 1.5.x build version ^(for example, 1.5.4^).
    exit /b 1
)

set DUCKDB_VERSION=!DUCKDB_VERSION_RAW!
if /i "!DUCKDB_VERSION:~0,1!"=="v" set DUCKDB_VERSION=!DUCKDB_VERSION:~1!
echo(!DUCKDB_VERSION!| %SystemRoot%\System32\findstr.exe /r /x "1\.5\.[0-9][0-9]*" >nul
if errorlevel 1 (
    echo Invalid DUCKDB_VERSION: '!DUCKDB_VERSION!'. Expected an exact 1.5.x semantic version ^(for example, 1.5.4^).
    exit /b 1
)

echo libdplyr Artifact Packaging
echo =================================
echo Version: %VERSION%
echo Platform: %PLATFORM_ARCH%
echo Build Directory: %BUILD_DIR%
echo Package Directory: %PACKAGE_DIR%
echo.

REM =============================================================================
REM Validation
REM =============================================================================

echo 🔍 Validating Build Artifacts
echo ------------------------------

REM Check if build directory exists
if not exist "%BUILD_DIR%" (
    echo ❌ Build directory not found: %BUILD_DIR%
    echo Please run the build process first
    exit /b 1
)

REM Check for extension file
set EXTENSION_FILE=%BUILD_DIR%\Release\%EXTENSION_NAME%.duckdb_extension
if not exist "%EXTENSION_FILE%" (
    set EXTENSION_FILE=%BUILD_DIR%\%EXTENSION_NAME%.duckdb_extension
)

if not exist "%EXTENSION_FILE%" (
    echo ❌ Extension file not found: %EXTENSION_FILE%
    echo Please build the extension first
    exit /b 1
)

echo ✅ Extension file found: %EXTENSION_FILE%

REM Get file size
for %%A in ("%EXTENSION_FILE%") do set EXTENSION_SIZE=%%~zA
echo Extension size: %EXTENSION_SIZE% bytes

REM =============================================================================
REM Package Directory Setup
REM =============================================================================

echo.
echo 📁 Setting up Package Directory
echo --------------------------------

set PACKAGE_ROOT=%PACKAGE_DIR%\%VERSION%
set PLATFORM_PACKAGE=%PACKAGE_ROOT%\%PLATFORM_ARCH%

if not exist "%PLATFORM_PACKAGE%" mkdir "%PLATFORM_PACKAGE%"
echo ✅ Created package directory: %PLATFORM_PACKAGE%

REM =============================================================================
REM Copy and Rename Extension
REM =============================================================================

echo.
echo 📋 Copying Extension Binary
echo ----------------------------

set PACKAGED_EXTENSION=%PLATFORM_PACKAGE%\%EXTENSION_NAME%-%PLATFORM_ARCH%.duckdb_extension
copy "%EXTENSION_FILE%" "%PACKAGED_EXTENSION%" >nul

echo ✅ Extension copied to: %PACKAGED_EXTENSION%

REM =============================================================================
REM Generate Metadata
REM =============================================================================

echo.
echo 📊 Generating Metadata
echo -----------------------

REM Get build timestamp
for /f "tokens=*" %%i in ('powershell -command "Get-Date -Format 'yyyy-MM-ddTHH:mm:ssZ'"') do set BUILD_TIMESTAMP=%%i

REM Get git information
for /f "tokens=*" %%i in ('git rev-parse HEAD 2^>nul') do set GIT_COMMIT=%%i
if "%GIT_COMMIT%"=="" set GIT_COMMIT=unknown

for /f "tokens=*" %%i in ('git rev-parse --abbrev-ref HEAD 2^>nul') do set GIT_BRANCH=%%i
if "%GIT_BRANCH%"=="" set GIT_BRANCH=unknown

for /f "tokens=*" %%i in ('git describe --tags --exact-match 2^>nul') do set GIT_TAG=%%i
if "%GIT_TAG%"=="" set GIT_TAG=

REM Get tool versions
for /f "tokens=*" %%i in ('rustc --version 2^>nul') do set RUST_VERSION=%%i
if "%RUST_VERSION%"=="" set RUST_VERSION=unknown

for /f "tokens=1" %%i in ('cmake --version 2^>nul') do set CMAKE_VERSION=%%i
if "%CMAKE_VERSION%"=="" set CMAKE_VERSION=unknown

REM Get libdplyr version
set LIBDPLYR_VERSION=unknown
if exist "libdplyr_c\Cargo.toml" (
    for /f "tokens=3 delims= " %%i in ('findstr "^version = " libdplyr_c\Cargo.toml') do (
        set LIBDPLYR_VERSION=%%i
        set LIBDPLYR_VERSION=!LIBDPLYR_VERSION:"=!
    )
)

REM Create metadata JSON
(
echo {
echo   "extension": {
echo     "name": "%EXTENSION_NAME%",
echo     "version": "%VERSION%",
echo     "platform": "%PLATFORM%",
echo     "architecture": "%ARCH%",
echo     "platform_arch": "%PLATFORM_ARCH%",
echo     "filename": "%EXTENSION_NAME%-%PLATFORM_ARCH%.duckdb_extension",
echo     "size_bytes": %EXTENSION_SIZE%,
echo     "size_human": "%EXTENSION_SIZE% bytes"
echo   },
echo   "build": {
echo     "timestamp": "%BUILD_TIMESTAMP%",
echo     "git_commit": "%GIT_COMMIT%",
echo     "git_branch": "%GIT_BRANCH%",
echo     "git_tag": "%GIT_TAG%",
echo     "build_type": "Release"
echo   },
echo   "versions": {
echo     "libdplyr": "%LIBDPLYR_VERSION%",
echo     "rust": "%RUST_VERSION%",
echo     "cmake": "%CMAKE_VERSION%",
echo     "duckdb_build_version": "%DUCKDB_VERSION%"
echo   },
echo   "compatibility": {
echo     "duckdb_min_version": "%DUCKDB_VERSION%",
echo     "duckdb_max_version": "%DUCKDB_VERSION%",
echo     "abi_version": "1",
echo     "api_version": "1"
echo   },
echo   "features": {
echo     "dplyr_keywords": true,
echo     "table_functions": true,
echo     "error_handling": true,
echo     "caching": true,
echo     "debug_logging": true
echo   },
echo   "requirements": {
echo     "minimum_memory_mb": 64,
echo     "recommended_memory_mb": 256,
echo     "disk_space_mb": 10
echo   }
echo }
) > "%PLATFORM_PACKAGE%\metadata.json"

echo ✅ Metadata generated: %PLATFORM_PACKAGE%\metadata.json

REM =============================================================================
REM Generate Installation Instructions
REM =============================================================================

echo.
echo 📖 Generating Installation Instructions
echo ----------------------------------------

(
echo # DuckDB dplyr Extension Installation
echo.
echo ## Platform: %PLATFORM_ARCH%
echo **Version**: %VERSION%
echo **Build Date**: %BUILD_TIMESTAMP%
echo.
echo ## Prerequisites
echo.
echo - **DuckDB binary version**: Exactly %DUCKDB_VERSION% ^(declared by the required DUCKDB_VERSION packaging input^)
echo - **Source-tested DuckDB versions**: 1.5.0 and 1.5.4
echo - **Operating System**: Windows ^(x86_64 architecture^)
echo - **Memory**: At least 64MB available RAM
echo - **Disk Space**: At least 10MB free space
echo.
echo ## Installation Steps
echo.
echo ### 1. Download the Extension
echo Download the extension file: `%EXTENSION_NAME%-%PLATFORM_ARCH%.duckdb_extension`
echo.
echo ### 2. Verify the Download ^(Recommended^)
echo ```cmd
echo REM Check file size
echo dir %EXTENSION_NAME%-%PLATFORM_ARCH%.duckdb_extension
echo.
echo REM Verify checksum ^(see checksums.txt^)
echo certutil -hashfile %EXTENSION_NAME%-%PLATFORM_ARCH%.duckdb_extension SHA256
echo ```
echo.
echo ### 3. Load the Extension in DuckDB
echo.
echo #### Option A: Load from File Path
echo DuckDB derives the extension entry point from the filename, so first copy the
echo versioned distribution artifact to the canonical filename:
echo ```cmd
echo copy %EXTENSION_NAME%-%PLATFORM_ARCH%.duckdb_extension %EXTENSION_NAME%.duckdb_extension
echo ```
echo.
echo Start DuckDB with `duckdb.exe -unsigned`, then run:
echo.
echo ```sql
echo -- Load the extension
echo LOAD 'C:\path\to\%EXTENSION_NAME%.duckdb_extension';
echo.
echo -- Verify it loaded successfully
echo SELECT 'Extension loaded successfully' as status;
echo ```
echo.
echo #### Option B: Install to DuckDB Extensions Directory
echo ```cmd
echo REM Use a duckdb.exe version that exactly matches this package
echo copy %EXTENSION_NAME%-%PLATFORM_ARCH%.duckdb_extension %EXTENSION_NAME%.duckdb_extension
echo set "CANONICAL_EXTENSION=%%CD:\=/%%/%EXTENSION_NAME%.duckdb_extension"
echo set "SQL_EXTENSION_PATH=%%CANONICAL_EXTENSION:'=''%%"
echo duckdb.exe -unsigned -bail :memory: -c "FORCE INSTALL '%%SQL_EXTENSION_PATH%%'; LOAD %EXTENSION_NAME%;"
echo ```
echo.
echo ## Usage Examples
echo.
echo ### Basic dplyr Operations
echo ```sql
echo -- Load the extension
echo LOAD 'C:\path\to\%EXTENSION_NAME%.duckdb_extension';
echo SET allow_parser_override_extension = 'fallback';
echo.
echo -- Create sample data
echo CREATE TABLE mtcars AS
echo SELECT * FROM 'https://raw.githubusercontent.com/tidyverse/dplyr/main/data-raw/mtcars.csv';
echo.
echo -- Use implicit pipeline syntax (%%^>%%)
echo mtcars %%^>%%
echo        select^(mpg, cyl, hp^) %%^>%%
echo        filter^(mpg ^> 20^) %%^>%%
echo        arrange^(desc^(hp^)^);
echo ```
echo.
echo ### Table Function Syntax
echo ```sql
echo -- Alternative syntax using table function
echo SELECT * FROM dplyr^('mtcars %%^>%%
echo                      select^(mpg, cyl^) %%^>%%
echo                      filter^(cyl == 4^)'^);
echo ```
echo.
echo ## Troubleshooting
echo.
echo ### Common Issues
echo.
echo 1. **Extension fails to load**
echo    - Confirm DuckDB is exactly version %DUCKDB_VERSION%
echo    - Verify file permissions
echo    - Ensure correct platform/architecture
echo.
echo 2. **"Function not found" errors**
echo    - Confirm extension is loaded: `LOAD 'C:\path\to\%EXTENSION_NAME%.duckdb_extension';`
echo    - Check for typos in dplyr syntax
echo.
echo 3. **Performance issues**
echo    - Enable caching for repeated queries
echo    - Check available memory
echo    - Consider query complexity
echo.
echo ### Debug Mode
echo ```cmd
echo REM Enable debug logging
echo set DPLYR_DEBUG=1
echo duckdb.exe -unsigned your_database.db
echo ```
echo.
echo ## Version Information
echo.
echo - **Extension Version**: %VERSION%
echo - **Build Commit**: %GIT_COMMIT%
echo - **Required DuckDB binary version**: %DUCKDB_VERSION% ^(exact match^)
echo - **Source-tested DuckDB versions**: 1.5.0 and 1.5.4
echo - **Platform**: %PLATFORM_ARCH%
echo - **Build Date**: %BUILD_TIMESTAMP%
echo.
echo For more information, visit the project repository.
) > "%PLATFORM_PACKAGE%\INSTALL.md"

echo ✅ Installation guide generated: %PLATFORM_PACKAGE%\INSTALL.md

REM =============================================================================
REM Generate Checksums
REM =============================================================================

echo.
echo 🔐 Generating Checksums
echo -----------------------

cd /d "%PLATFORM_PACKAGE%"

REM Generate checksums
(
echo # Checksums for %EXTENSION_NAME%-%PLATFORM_ARCH%.duckdb_extension
echo # Generated on %BUILD_TIMESTAMP%
echo.
) > checksums.txt

REM SHA256 in the portable "digest  filename" format
set "EXTENSION_SHA256="
for /f "delims=" %%i in ('powershell -NoProfile -Command "^(Get-FileHash -Algorithm SHA256 -LiteralPath '%EXTENSION_NAME%-%PLATFORM_ARCH%.duckdb_extension'^).Hash.ToLowerInvariant^(^)"') do set "EXTENSION_SHA256=%%i"
if not defined EXTENSION_SHA256 (
    echo ❌ Failed to calculate the extension SHA256 checksum
    exit /b 1
)
echo !EXTENSION_SHA256!  %EXTENSION_NAME%-%PLATFORM_ARCH%.duckdb_extension>> checksums.txt

cd /d "%~dp0.."

echo ✅ Checksums generated: %PLATFORM_PACKAGE%\checksums.txt

REM =============================================================================
REM Create Archive
REM =============================================================================

echo.
echo 📦 Creating Archive
echo -------------------

set ARCHIVE_NAME=%EXTENSION_NAME%-%VERSION%-%PLATFORM_ARCH%

cd /d "%PACKAGE_ROOT%"

REM Create ZIP archive
if exist "%ARCHIVE_NAME%.zip" del "%ARCHIVE_NAME%.zip"
powershell -command "Compress-Archive -Path '%PLATFORM_ARCH%\*' -DestinationPath '%ARCHIVE_NAME%.zip'"

if exist "%ARCHIVE_NAME%.zip" (
    echo ✅ ZIP archive created: %PACKAGE_ROOT%\%ARCHIVE_NAME%.zip

    REM Generate archive checksum in the same portable format
    set "ARCHIVE_SHA256="
    for /f "delims=" %%i in ('powershell -NoProfile -Command "^(Get-FileHash -Algorithm SHA256 -LiteralPath '%ARCHIVE_NAME%.zip'^).Hash.ToLowerInvariant^(^)"') do set "ARCHIVE_SHA256=%%i"
    if not defined ARCHIVE_SHA256 (
        echo ❌ Failed to calculate the archive SHA256 checksum
        exit /b 1
    )
    echo !ARCHIVE_SHA256!  %ARCHIVE_NAME%.zip> "%ARCHIVE_NAME%.zip.sha256"
    echo ✅ Archive checksum: %PACKAGE_ROOT%\%ARCHIVE_NAME%.zip.sha256
)

cd /d "%~dp0.."

REM =============================================================================
REM Generate Release Summary
REM =============================================================================

echo.
echo 📋 Generating Release Summary
echo ------------------------------

(
echo # Release Summary: %EXTENSION_NAME% %VERSION% ^(%PLATFORM_ARCH%^)
echo.
echo ## Build Information
echo - **Version**: %VERSION%
echo - **Platform**: %PLATFORM_ARCH%
echo - **Build Date**: %BUILD_TIMESTAMP%
echo - **Git Commit**: %GIT_COMMIT%
echo - **Git Branch**: %GIT_BRANCH%
echo.
echo ## Package Contents
echo - `%EXTENSION_NAME%-%PLATFORM_ARCH%.duckdb_extension` - Main extension binary ^(%EXTENSION_SIZE% bytes^)
echo - `metadata.json` - Detailed build and compatibility information
echo - `INSTALL.md` - Installation and usage instructions
echo - `checksums.txt` - File integrity verification
echo.
echo ## Archives
echo - `%ARCHIVE_NAME%.zip` - Compressed archive ^(Windows^)
echo.
echo ## Compatibility
echo - **Required DuckDB binary version**: %DUCKDB_VERSION% ^(exact match^)
echo - **Source-tested DuckDB versions**: 1.5.0 and 1.5.4
echo - **Platform**: %PLATFORM% ^(%ARCH%^)
echo - **ABI Version**: 1
echo - **API Version**: 1
echo.
echo ## Features
echo - ✅ Implicit pipeline syntax (%%^>%%)
echo - ✅ Table function interface
echo - ✅ Error handling with codes
echo - ✅ Query result caching
echo - ✅ Debug logging support
echo.
echo ## Installation
echo 1. Download the ZIP archive for Windows
echo 2. Extract the extension binary
echo 3. Copy the versioned binary to `%EXTENSION_NAME%.duckdb_extension`
echo 4. Load in DuckDB: `LOAD 'C:\path\to\%EXTENSION_NAME%.duckdb_extension';`
echo 5. Example: `SELECT * FROM dplyr^('data %%^>%% select^(col^)'^);`
echo.
echo ## Verification
echo ```cmd
echo REM Verify checksum
echo certutil -hashfile %EXTENSION_NAME%-%PLATFORM_ARCH%.duckdb_extension SHA256
echo.
echo REM Stage the canonical filename required by DuckDB
echo copy %EXTENSION_NAME%-%PLATFORM_ARCH%.duckdb_extension %EXTENSION_NAME%.duckdb_extension
echo.
echo REM Test loading
echo duckdb.exe -unsigned -c "LOAD './%EXTENSION_NAME%.duckdb_extension'; SELECT 'OK' as status;"
echo ```
echo.
echo ---
echo Generated by libdplyr packaging system
) > "%PACKAGE_ROOT%\release-summary-%PLATFORM_ARCH%.md"

echo ✅ Release summary: %PACKAGE_ROOT%\release-summary-%PLATFORM_ARCH%.md

REM =============================================================================
REM Final Summary
REM =============================================================================

echo.
echo 🎉 Packaging Complete
echo =====================

echo ✅ Successfully packaged %EXTENSION_NAME% %VERSION% for %PLATFORM_ARCH%
echo.
echo Package location: %PLATFORM_PACKAGE%
echo Archive location: %PACKAGE_ROOT%
echo.
echo Files created:
echo   📦 %EXTENSION_NAME%-%PLATFORM_ARCH%.duckdb_extension ^(%EXTENSION_SIZE% bytes^)
echo   📊 metadata.json
echo   📖 INSTALL.md
echo   🔐 checksums.txt
echo   📋 release-summary-%PLATFORM_ARCH%.md

if exist "%PACKAGE_ROOT%\%ARCHIVE_NAME%.zip" (
    for %%A in ("%PACKAGE_ROOT%\%ARCHIVE_NAME%.zip") do set ZIP_SIZE=%%~zA
    echo   📦 %ARCHIVE_NAME%.zip ^(!ZIP_SIZE! bytes^)
)

echo.
echo Next steps:
echo   1. Test the packaged extension
echo   2. Upload to release repository
echo   3. Update documentation
echo.
echo 🚀 Ready for distribution!
