# Dvaar Scripts Documentation

This document explains all scripts used for deploying and distributing Dvaar.

---

## Table of Contents

1. [Server Scripts](#server-scripts)
   - [setup-server.sh](#setup-serversh)
   - [add-edge-node.sh](#add-edge-nodesh)
2. [CLI Distribution Scripts](#cli-distribution-scripts)
   - [install.sh](#installsh)
   - [install.ps1](#installps1)
3. [Distribution Channels](#distribution-channels)
   - [Homebrew](#homebrew)
   - [npm](#npm)
   - [Cargo](#cargo)
4. [CI/CD Automation](#cicd-automation)

---

## Server Scripts

### setup-server.sh

**Location**: `scripts/setup-server.sh`

**Purpose**: Bootstrap a new server (control plane or edge node) with everything needed to run Dvaar.

**Usage**:
```bash
# For control plane (first server with DB + Redis)
curl -sSL https://raw.githubusercontent.com/YOUR_REPO/dvaar/main/scripts/setup-server.sh | bash -s -- control-plane

# For edge node (connects to existing control plane)
CONTROL_PLANE_IP=x.x.x.x curl -sSL https://raw.githubusercontent.com/YOUR_REPO/dvaar/main/scripts/setup-server.sh | bash -s -- edge
```

**What it does**:

1. **System Setup**
   - Updates packages
   - Installs dependencies (ca-certificates, curl, ufw, fail2ban)

2. **Docker Installation**
   - Installs Docker Engine
   - Installs Docker Compose plugin
   - Enables Docker to start on boot

3. **Firewall Configuration**
   - Allows SSH (port 22)
   - Allows HTTP (port 80)
   - Allows HTTPS (port 443)
   - Allows internal node communication (port 6000)
   - Blocks everything else

4. **For Control Plane**:
   - Generates secure random passwords for:
     - PostgreSQL
     - Cluster secret (node-to-node auth)
     - Admin panel token
   - Creates `/opt/dvaar/.env` with all configuration
   - Downloads docker-compose.yml and Caddyfile

5. **For Edge Node**:
   - Creates minimal `.env` pointing to control plane
   - Creates edge-specific docker-compose (no DB/Redis)
   - Configures connection to shared DB and Redis

6. **Creates systemd service** for auto-start on reboot

**Output**: Prints generated secrets (SAVE THESE!) and next steps.

---

### add-edge-node.sh

**Location**: `scripts/add-edge-node.sh`

**Purpose**: Quick 5-minute setup to add a new edge node from the control plane.

**Usage** (run from control plane):
```bash
./scripts/add-edge-node.sh <new-server-ip> [ssh-key-path]
```

**What it does**:

1. Loads existing config from `/opt/dvaar/.env`
2. SSHs into the new server
3. Installs Docker
4. Creates edge node configuration using control plane's secrets
5. Starts the containers
6. Prints DNS/CI update reminders

**Example**:
```bash
# From control plane server
./scripts/add-edge-node.sh 95.216.xx.xx

# Output:
# [+] Adding edge node: 95.216.xx.xx
# [+] Copying setup script to new server...
# [+] Configuring edge node...
# [+] Edge node 95.216.xx.xx is now running!
#
# Don't forget to:
# 1. Add DNS record: Update your wildcard DNS or add GeoDNS
# 2. Add to CI/CD: Update EDGE_NODES in GitHub repo variables
```

---

## CLI Distribution Scripts

### install.sh

**Location**: `scripts/install.sh` (also served at `https://dvaar.io/install`)

**Purpose**: One-line installer for macOS and Linux users.

**Usage**:
```bash
curl -sSL https://dvaar.io/install | bash
```

**What it does**:

1. Detects OS (macOS, Linux) and architecture (x64, arm64)
2. Downloads the correct pre-built binary from GitHub Releases
3. Installs to `/usr/local/bin/dvaar` (or `~/.local/bin` if no sudo)
4. Verifies the binary works
5. Prints success message with next steps

**Fallback**: If no pre-built binary exists, suggests using cargo install.

---

### install.ps1

**Location**: `scripts/install.ps1`

**Purpose**: PowerShell installer for Windows users.

**Usage** (PowerShell):
```powershell
irm https://dvaar.io/install.ps1 | iex
```

**What it does**:

1. Downloads Windows binary from GitHub Releases
2. Installs to `%USERPROFILE%\.dvaar\bin`
3. Adds to PATH
4. Verifies installation

---

## Distribution Channels

### Homebrew

**Location**: Separate repo `dvaar/homebrew-tap`

**Installation**:
```bash
brew tap dvaar/tap
brew install dvaar
```

**Formula** (`Formula/dvaar.rb`):
```ruby
class Dvaar < Formula
  desc "Expose your localhost to the internet"
  homepage "https://dvaar.io"
  version "0.1.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/strawberry-labs/dvaar/releases/download/v#{version}/dvaar-darwin-arm64.tar.gz"
      sha256 "SHASUM_HERE"
    else
      url "https://github.com/strawberry-labs/dvaar/releases/download/v#{version}/dvaar-darwin-x64.tar.gz"
      sha256 "SHASUM_HERE"
    end
  end

  on_linux do
    url "https://github.com/strawberry-labs/dvaar/releases/download/v#{version}/dvaar-linux-x64.tar.gz"
    sha256 "SHASUM_HERE"
  end

  def install
    bin.install "dvaar"
  end

  test do
    system "#{bin}/dvaar", "--version"
  end
end
```

**Update Process**: GitHub Action builds release → updates formula → users run `brew upgrade dvaar`

---

### npm

**Location**: `packages/dvaar-cli/` or published to npm as `@dvaar/cli`

**Installation**:
```bash
npm install -g @dvaar/cli
# or
npx @dvaar/cli http 3000
```

**How it works**: npm package is a thin wrapper that:
1. Downloads the correct binary on `postinstall`
2. Provides a bin entry that executes the native binary

**package.json**:
```json
{
  "name": "@dvaar/cli",
  "version": "0.1.0",
  "bin": { "dvaar": "./bin/dvaar" },
  "scripts": {
    "postinstall": "node scripts/install.js"
  }
}
```

---

### Cargo

**Installation**:
```bash
cargo install dvaar
```

**How it works**: Publishes to crates.io, users compile from source.

**Cargo.toml** additions for publishing:
```toml
[package]
name = "dvaar"
version = "0.1.0"
license = "MIT"
repository = "https://github.com/strawberry-labs/dvaar"
description = "Expose your localhost to the internet"
keywords = ["tunnel", "localhost", "ngrok", "http"]
categories = ["command-line-utilities", "web-programming"]
```

---

## CI/CD Automation

### Release Workflow

**Location**: `.github/workflows/release.yml`

**Triggers**: On git tag push (`v*`)

**What it does**:

1. **Build binaries** for all platforms:
   - `dvaar-darwin-arm64` (macOS Apple Silicon)
   - `dvaar-darwin-x64` (macOS Intel)
   - `dvaar-linux-x64` (Linux)
   - `dvaar-linux-arm64` (Linux ARM)
   - `dvaar-windows-x64.exe` (Windows)

2. **Create GitHub Release** with all binaries

3. **Update Homebrew formula** with new version and SHA

4. **Publish to npm** (`@dvaar/cli`)

5. **Publish to crates.io** (`dvaar`)

### Server Deploy Workflow

**Location**: `.github/workflows/deploy.yml`

**Triggers**: On push to `main` or version tags

**What it does**:

1. Builds Docker image
2. Pushes to GitHub Container Registry
3. SSHs to control plane, pulls and restarts
4. SSHs to each edge node, pulls and restarts

---

## Edge Node Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Control Plane                            │
│                    (Hetzner FSN1)                           │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
│  │ Postgres │  │  Redis   │  │  Dvaar   │  │  Caddy   │   │
│  │   :5432  │  │  :6379   │  │  :8080   │  │  :443    │   │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘   │
└─────────────────────────────────────────────────────────────┘
        │                │
        │ DB Connection  │ Redis Connection
        │                │
┌───────┴────────┐  ┌────┴───────────┐  ┌─────────────────┐
│   Edge Node 1  │  │   Edge Node 2  │  │   Edge Node 3   │
│  (Hetzner HEL) │  │  (Hetzner ASH) │  │  (Vultr SYD)    │
│  ┌──────────┐  │  │  ┌──────────┐  │  │  ┌──────────┐   │
│  │  Dvaar   │  │  │  │  Dvaar   │  │  │  │  Dvaar   │   │
│  │  :8080   │  │  │  │  :8080   │  │  │  │  :8080   │   │
│  └──────────┘  │  │  └──────────┘  │  │  └──────────┘   │
│  ┌──────────┐  │  │  ┌──────────┐  │  │  ┌──────────┐   │
│  │  Caddy   │  │  │  │  Caddy   │  │  │  │  Caddy   │   │
│  │  :443    │  │  │  │  :443    │  │  │  │  :443    │   │
│  └──────────┘  │  │  └──────────┘  │  │  └──────────┘   │
└────────────────┘  └────────────────┘  └─────────────────┘
```

**How it works**:

1. **User creates tunnel** → Connects to nearest edge node via WebSocket
2. **Edge node registers route** in shared Redis: `route:myapp` → `{node_ip: "edge1", port: 6000}`
3. **Request comes in** to `myapp.dvaar.app`
4. **DNS routes to edge node** (GeoDNS or round-robin)
5. **Edge checks Redis** for route
   - If local: forwards via WebSocket
   - If remote: proxies to correct node via internal port 6000
6. **Response flows back** through the same path

---

## Quick Reference

| Script | Where to Run | Purpose |
|--------|--------------|---------|
| `setup-server.sh control-plane` | New VPS | First-time control plane setup |
| `setup-server.sh edge` | New VPS | First-time edge node setup |
| `add-edge-node.sh` | Control plane | Quick add another edge |
| `install.sh` | User's machine | Install CLI (curl) |
| `install.ps1` | User's machine | Install CLI (Windows) |

---

## Environment Variables Reference

### Control Plane `.env`

```bash
NODE_TYPE=control-plane
NODE_IP=x.x.x.x              # This server's public IP
BASE_DOMAIN=dvaar.io
TUNNEL_DOMAIN=dvaar.app
PUBLIC_URL=https://api.dvaar.io

POSTGRES_PASSWORD=xxx        # Generated
CLUSTER_SECRET=xxx           # Generated (shared with edges)
ADMIN_TOKEN=xxx              # Generated

GITHUB_CLIENT_ID=xxx         # From GitHub OAuth app
GITHUB_CLIENT_SECRET=xxx     # From GitHub OAuth app

GITHUB_REPO=your/repo        # For Docker image pulls
VERSION=latest
```

### Edge Node `.env`

```bash
NODE_TYPE=edge
NODE_IP=y.y.y.y              # This edge's public IP
BASE_DOMAIN=dvaar.io
TUNNEL_DOMAIN=dvaar.app
PUBLIC_URL=https://api.dvaar.io

CLUSTER_SECRET=xxx           # Same as control plane
DATABASE_URL=postgres://dvaar:xxx@CONTROL_PLANE_IP:5432/dvaar
REDIS_URL=redis://CONTROL_PLANE_IP:6379

GITHUB_REPO=your/repo
VERSION=latest
```
