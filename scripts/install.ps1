# Dvaar CLI Installer for Windows
# Usage: irm https://dvaar.io/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Version = if ($env:DVAAR_VERSION) { $env:DVAAR_VERSION } else { "latest" }
$GithubRepo = "strawberry-labs/dvaar"
$InstallDir = "$env:USERPROFILE\.dvaar\bin"

function Write-Info { param($Message) Write-Host "==> " -ForegroundColor Blue -NoNewline; Write-Host $Message }
function Write-Success { param($Message) Write-Host "==> " -ForegroundColor Green -NoNewline; Write-Host $Message }
function Write-Warn { param($Message) Write-Host "==> " -ForegroundColor Yellow -NoNewline; Write-Host $Message }
function Write-Err { param($Message) Write-Host "==> " -ForegroundColor Red -NoNewline; Write-Host $Message; exit 1 }

# Get latest version
function Get-LatestVersion {
    if ($Version -eq "latest") {
        try {
            $release = Invoke-RestMethod "https://api.github.com/repos/$GithubRepo/releases/latest"
            $script:Version = $release.tag_name -replace "^v", ""
        } catch {
            Write-Err "Failed to get latest version: $_"
        }
    }
}

# Download and install
function Install-Dvaar {
    $arch = if ([Environment]::Is64BitOperatingSystem) { "x64" } else { "x86" }
    $downloadUrl = "https://github.com/$GithubRepo/releases/download/v$Version/dvaar-windows-$arch.zip"
    $tempDir = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }
    $zipPath = Join-Path $tempDir "dvaar.zip"

    Write-Info "Downloading dvaar v$Version for windows-$arch..."

    try {
        Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath -UseBasicParsing
    } catch {
        Remove-Item $tempDir -Recurse -Force
        Write-Err "Download failed: $_"
    }

    Write-Info "Extracting..."
    Expand-Archive -Path $zipPath -DestinationPath $tempDir -Force

    Write-Info "Installing to $InstallDir..."
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    Move-Item (Join-Path $tempDir "dvaar.exe") (Join-Path $InstallDir "dvaar.exe") -Force
    Remove-Item $tempDir -Recurse -Force
}

# Add to PATH
function Add-ToPath {
    $currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($currentPath -notlike "*$InstallDir*") {
        Write-Info "Adding $InstallDir to PATH..."
        [Environment]::SetEnvironmentVariable("PATH", "$currentPath;$InstallDir", "User")
        $env:PATH = "$env:PATH;$InstallDir"
        Write-Warn "Restart your terminal for PATH changes to take effect."
    }
}

# Verify installation
function Test-Installation {
    try {
        $version = & "$InstallDir\dvaar.exe" --version 2>&1
        Write-Success "dvaar v$Version installed successfully!"
        Write-Host ""
        Write-Host $version
    } catch {
        Write-Success "dvaar v$Version installed to $InstallDir\dvaar.exe"
    }
}

# Print next steps
function Show-NextSteps {
    Write-Host ""
    Write-Host "Dvaar is fully managed through the CLI - no dashboard needed!" -ForegroundColor Blue
    Write-Host ""
    Write-Host ([char]0x2501 * 60)
    Write-Host ""
    Write-Host "Getting started:"
    Write-Host ""
    Write-Host "  " -NoNewline; Write-Host "dvaar login" -ForegroundColor Green -NoNewline; Write-Host "            Authenticate with GitHub"
    Write-Host "  " -NoNewline; Write-Host "dvaar http 3000" -ForegroundColor Green -NoNewline; Write-Host "        Expose local port 3000"
    Write-Host "  " -NoNewline; Write-Host "dvaar http ./dist" -ForegroundColor Green -NoNewline; Write-Host "      Serve a static directory"
    Write-Host ""
    Write-Host "Tunnel management:"
    Write-Host ""
    Write-Host "  " -NoNewline; Write-Host "dvaar ls" -ForegroundColor Green -NoNewline; Write-Host "               List active tunnels"
    Write-Host "  " -NoNewline; Write-Host "dvaar stop <id>" -ForegroundColor Green -NoNewline; Write-Host "        Stop a tunnel"
    Write-Host "  " -NoNewline; Write-Host "dvaar logs <id>" -ForegroundColor Green -NoNewline; Write-Host "        View tunnel logs"
    Write-Host ""
    Write-Host "Account & billing:"
    Write-Host ""
    Write-Host "  " -NoNewline; Write-Host "dvaar usage" -ForegroundColor Green -NoNewline; Write-Host "            View bandwidth usage"
    Write-Host "  " -NoNewline; Write-Host "dvaar upgrade" -ForegroundColor Green -NoNewline; Write-Host "          Upgrade your plan"
    Write-Host "  " -NoNewline; Write-Host "dvaar billing" -ForegroundColor Green -NoNewline; Write-Host "          Manage subscription & invoices"
    Write-Host ""
    Write-Host "Maintenance:"
    Write-Host ""
    Write-Host "  " -NoNewline; Write-Host "dvaar update" -ForegroundColor Green -NoNewline; Write-Host "           Update to latest version"
    Write-Host "  " -NoNewline; Write-Host "dvaar uninstall" -ForegroundColor Green -NoNewline; Write-Host "        Remove dvaar from your system"
    Write-Host ""
    Write-Host ([char]0x2501 * 60)
    Write-Host ""
    Write-Host "Documentation: https://dvaar.io/docs"
    Write-Host ""
}

# Main
function Main {
    Write-Host ""
    Write-Host "  ____"
    Write-Host " |  _ \__   ____ _  __ _ _ __"
    Write-Host " | | | \ \ / / _`` |/ _`` | '__|"
    Write-Host " | |_| |\ V / (_| | (_| | |"
    Write-Host " |____/  \_/ \__,_|\__,_|_|"
    Write-Host ""
    Write-Host " Localhost Tunnel Service"
    Write-Host ""

    Get-LatestVersion
    Install-Dvaar
    Add-ToPath
    Test-Installation
    Show-NextSteps
}

Main
