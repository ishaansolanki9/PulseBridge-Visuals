$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

function Refresh-Path {
    $machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $env:Path = "$machinePath;$userPath;$env:USERPROFILE\.cargo\bin"
}

function Install-WithWinget([string]$PackageId, [string]$Label) {
    if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
        throw "$Label is missing and winget is unavailable. Install it, reopen PowerShell, and run this script again."
    }
    Write-Host "Installing $Label..."
    winget install --exact --id $PackageId --accept-package-agreements --accept-source-agreements --silent
    if ($LASTEXITCODE -ne 0) {
        throw "winget could not install $Label (exit code $LASTEXITCODE)."
    }
    Refresh-Path
}

if ($env:OS -ne "Windows_NT") {
    throw "This script must run on the Windows laptop that will build the installer."
}

$windowsBuild = [Environment]::OSVersion.Version.Build
if ($windowsBuild -lt 20348) {
    Write-Warning "This Windows build is older than 20348. PulseBridge will use output-device loopback if process-only capture is unavailable."
}

if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
    Install-WithWinget "OpenJS.NodeJS.LTS" "Node.js LTS"
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Install-WithWinget "Rustlang.Rustup" "Rust"
}

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
$vcToolsPath = $null
if (Test-Path $vswhere) {
    $vcToolsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
}
if (-not $vcToolsPath) {
    if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
        throw "Microsoft C++ Build Tools are missing. Install Visual Studio Build Tools with the Desktop development with C++ workload, then rerun this script."
    }
    Write-Host "Installing Microsoft C++ Build Tools and the Windows SDK. A Windows permission prompt may appear..."
    winget install --force --exact --id Microsoft.VisualStudio.2022.BuildTools `
        --accept-package-agreements --accept-source-agreements `
        --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools;includeRecommended"
    if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne 3010) {
        throw "Microsoft C++ Build Tools installation failed (exit code $LASTEXITCODE)."
    }
    if (Test-Path $vswhere) {
        $vcToolsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    }
    if (-not $vcToolsPath) {
        throw "C++ Build Tools were installed but are not visible yet. Restart Windows, reopen PowerShell, and run this script again."
    }
}

if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
    throw "npm is not available after setup. Reopen PowerShell and run this script again."
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "Cargo is not available after setup. Reopen PowerShell and run this script again."
}

rustup default stable
rustup component add clippy rustfmt

Write-Host "Installing locked project dependencies..."
npm ci

Write-Host "Running frontend and native checks..."
npm run lint
npm run typecheck
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml

Write-Host "Building the self-contained Windows application and NSIS installer..."
npm run tauri -- build --bundles nsis

$installer = Get-ChildItem -Path "src-tauri\target\release\bundle\nsis" -Filter "*.exe" |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
if (-not $installer) {
    throw "Build completed without producing an NSIS installer."
}

$readyDirectory = Join-Path $ProjectRoot "transfer-ready"
New-Item -ItemType Directory -Path $readyDirectory -Force | Out-Null
$readyInstaller = Join-Path $readyDirectory "PulseBridge Visuals Setup.exe"
Copy-Item -Path $installer.FullName -Destination $readyInstaller -Force
$hash = Get-FileHash -Path $readyInstaller -Algorithm SHA256
"$($hash.Hash)  PulseBridge Visuals Setup.exe" | Set-Content -Path (Join-Path $readyDirectory "PulseBridge Visuals Setup.sha256.txt")

Write-Host ""
Write-Host "Installer ready: $readyInstaller" -ForegroundColor Green
Write-Host "SHA-256: $($hash.Hash)"
Write-Host "Install it on the Windows DJ laptop; Node and Rust are not needed after installation."
