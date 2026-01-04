# Dvaar Production Deployment Checklist

Complete guide to deploying Dvaar from zero to production.

---

## Phase 1: Local Git Setup

### 1.1 Initial Commit
- [ ] Navigate to project directory
  ```bash
  cd /path/to/dvaar
  ```

- [ ] Stage all files
  ```bash
  git add .
  ```

- [ ] Create initial commit
  ```bash
  git commit -m "Initial commit: Dvaar tunneling service"
  ```

---

## Phase 2: GitHub Repository Setup

### 2.1 Create Repository
- [ ] Go to [github.com/new](https://github.com/new)
- [ ] Repository name: `dvaar`
- [ ] Visibility: **Private** (recommended for production infrastructure)
- [ ] Do NOT initialize with README, .gitignore, or license (we have our own)
- [ ] Click "Create repository"

### 2.2 Push Code
- [ ] Add remote origin
  ```bash
  git remote add origin git@github.com:YOUR_USERNAME/dvaar.git
  ```

- [ ] Rename branch to main
  ```bash
  git branch -M main
  ```

- [ ] Push to GitHub
  ```bash
  git push -u origin main
  ```

### 2.3 Configure GitHub Actions Permissions
- [ ] Go to repository **Settings** → **Actions** → **General**
- [ ] Scroll to "Workflow permissions"
- [ ] Select **"Read and write permissions"**
- [ ] Check **"Allow GitHub Actions to create and approve pull requests"**
- [ ] Click **Save**

### 2.4 Create Production Environment
- [ ] Go to repository **Settings** → **Environments**
- [ ] Click **"New environment"**
- [ ] Name: `production`
- [ ] Click **"Configure environment"**
- [ ] (Optional) Enable "Required reviewers" for deployment approval
- [ ] Click **"Save protection rules"**

---

## Phase 3: GitHub OAuth Application

### 3.1 Create OAuth App
- [ ] Go to [github.com/settings/developers](https://github.com/settings/developers)
- [ ] Click **"OAuth Apps"** tab
- [ ] Click **"New OAuth App"**

### 3.2 Configure OAuth App
- [ ] **Application name**: `Dvaar`
- [ ] **Homepage URL**: `https://dvaar.io`
- [ ] **Application description**: `Secure tunneling service` (optional)
- [ ] **Authorization callback URL**: `https://api.dvaar.io/api/auth/github/callback`
- [ ] Click **"Register application"**

### 3.3 Save Credentials
- [ ] Copy **Client ID** and save securely
  ```
  Client ID: Iv1.xxxxxxxxxxxxxxxx
  ```

- [ ] Click **"Generate a new client secret"**
- [ ] Copy **Client Secret** immediately (shown only once)
  ```
  Client Secret: xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
  ```

- [ ] Store both values securely (you'll need them for server setup)

---

## Phase 4: Hetzner Server Provisioning

### 4.1 Create Hetzner Account
- [ ] Go to [accounts.hetzner.com](https://accounts.hetzner.com)
- [ ] Sign up or log in
- [ ] Add payment method
- [ ] Go to [console.hetzner.cloud](https://console.hetzner.cloud)

### 4.2 Create Project
- [ ] Click **"+ New Project"**
- [ ] Name: `dvaar`
- [ ] Click **"Add project"**
- [ ] Click into the project

### 4.3 Add SSH Key
- [ ] Go to **Security** → **SSH Keys**
- [ ] Click **"Add SSH Key"**
- [ ] Paste your public key:
  ```bash
  # Get your public key (run locally)
  cat ~/.ssh/id_ed25519.pub
  # or
  cat ~/.ssh/id_rsa.pub
  ```
- [ ] Name: `deploy-key`
- [ ] Click **"Add SSH Key"**

### 4.4 Create Control Plane Server
- [ ] Click **"Add Server"**
- [ ] **Location**: Choose based on your target users
  | Location | Code | Latency Target |
  |----------|------|----------------|
  | Falkenstein, DE | fsn1 | Europe |
  | Nuremberg, DE | nbg1 | Europe |
  | Helsinki, FI | hel1 | Europe/Nordic |
  | Ashburn, US | ash | North America |
  | Hillsboro, US | hil | US West Coast |

- [ ] **Image**: Ubuntu 24.04
- [ ] **Type**: Shared vCPU → **CPX21**
  | Spec | Value |
  |------|-------|
  | vCPU | 3 AMD |
  | RAM | 4 GB |
  | SSD | 80 GB |
  | Traffic | 20 TB |
  | Price | ~€15/mo |

- [ ] **Networking**: Public IPv4 (default)
- [ ] **SSH Keys**: Select your `deploy-key`
- [ ] **Name**: `dvaar-control`
- [ ] Click **"Create & Buy now"**

### 4.5 Record Server IP
- [ ] Wait for server to be "Running" (1-2 minutes)
- [ ] Copy the **IPv4 address**
  ```
  Control Plane IP: xxx.xxx.xxx.xxx
  ```

### 4.6 Verify SSH Access
- [ ] Test SSH connection
  ```bash
  ssh root@CONTROL_PLANE_IP
  # Should connect without password prompt
  exit
  ```

---

## Phase 5: DNS Configuration

### 5.1 Configure dvaar.io DNS Records
- [ ] Log into your DNS provider (Cloudflare, Namecheap, etc.)
- [ ] Navigate to DNS settings for `dvaar.io`
- [ ] Add the following A records:

| Type | Name | Value | Proxy | TTL |
|------|------|-------|-------|-----|
| A | `@` | CONTROL_PLANE_IP | Optional | Auto |
| A | `api` | CONTROL_PLANE_IP | Optional | Auto |
| A | `admin` | CONTROL_PLANE_IP | Optional | Auto |
| A | `dash` | CONTROL_PLANE_IP | Optional | Auto |

### 5.2 Configure dvaar.app DNS Records
- [ ] Navigate to DNS settings for `dvaar.app`
- [ ] Add the following A records:

| Type | Name | Value | Proxy | TTL |
|------|------|-------|-------|-----|
| A | `@` | CONTROL_PLANE_IP | **OFF** | Auto |
| A | `*` | CONTROL_PLANE_IP | **OFF** | Auto |

> **IMPORTANT**: dvaar.app must have proxy DISABLED (DNS only / gray cloud in Cloudflare). WebSocket connections require direct access.

### 5.3 Verify DNS Propagation
- [ ] Wait 5-10 minutes for propagation
- [ ] Verify records:
  ```bash
  dig api.dvaar.io +short
  # Should return: CONTROL_PLANE_IP

  dig test.dvaar.app +short
  # Should return: CONTROL_PLANE_IP
  ```

---

## Phase 6: Server Setup

### 6.1 SSH into Control Plane
- [ ] Connect to server
  ```bash
  ssh root@CONTROL_PLANE_IP
  ```

### 6.2 Run Setup Script
- [ ] Set environment variables
  ```bash
  export GITHUB_REPO="YOUR_USERNAME/dvaar"
  export BASE_DOMAIN="dvaar.io"
  export TUNNEL_DOMAIN="dvaar.app"
  ```

- [ ] Run the setup script
  ```bash
  curl -sSL "https://raw.githubusercontent.com/${GITHUB_REPO}/main/scripts/setup-server.sh" | bash -s -- control-plane
  ```

- [ ] **IMPORTANT**: Save the generated secrets displayed at the end:
  ```
  POSTGRES_PASSWORD=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
  CLUSTER_SECRET=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
  ADMIN_TOKEN=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
  ```

### 6.3 Configure GitHub OAuth
- [ ] Edit the environment file
  ```bash
  nano /opt/dvaar/.env
  ```

- [ ] Add your GitHub OAuth credentials:
  ```bash
  GITHUB_CLIENT_ID=Iv1.xxxxxxxxxxxxxxxx
  GITHUB_CLIENT_SECRET=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
  ```

- [ ] Save and exit (Ctrl+X, Y, Enter)

### 6.4 Verify Configuration
- [ ] Check .env file has all required values:
  ```bash
  cat /opt/dvaar/.env | grep -E "^[A-Z]"
  ```

  Required values:
  - [ ] `NODE_IP` - Server's public IP
  - [ ] `BASE_DOMAIN` - dvaar.io
  - [ ] `TUNNEL_DOMAIN` - dvaar.app
  - [ ] `POSTGRES_PASSWORD` - Generated password
  - [ ] `CLUSTER_SECRET` - Generated secret
  - [ ] `ADMIN_TOKEN` - Generated token
  - [ ] `GITHUB_CLIENT_ID` - Your OAuth client ID
  - [ ] `GITHUB_CLIENT_SECRET` - Your OAuth client secret
  - [ ] `GITHUB_REPO` - YOUR_USERNAME/dvaar

---

## Phase 7: GitHub Secrets Configuration

### 7.1 Add SSH Private Key
- [ ] Go to repository **Settings** → **Secrets and variables** → **Actions**
- [ ] Click **"New repository secret"**
- [ ] Name: `SSH_PRIVATE_KEY`
- [ ] Value: Your SSH private key
  ```bash
  # Get your private key (run locally)
  cat ~/.ssh/id_ed25519
  # or
  cat ~/.ssh/id_rsa
  ```
- [ ] Click **"Add secret"**

### 7.2 Add Control Plane Host
- [ ] Click **"New repository secret"**
- [ ] Name: `CONTROL_PLANE_HOST`
- [ ] Value: Your control plane IP address
- [ ] Click **"Add secret"**

### 7.3 Verify Secrets
- [ ] Confirm both secrets are listed:
  - `SSH_PRIVATE_KEY`
  - `CONTROL_PLANE_HOST`

---

## Phase 8: Initial Deployment

### 8.1 Trigger CI/CD Pipeline
- [ ] From your local machine:
  ```bash
  cd /path/to/dvaar
  git commit --allow-empty -m "Trigger initial deployment"
  git push
  ```

### 8.2 Monitor Build
- [ ] Go to repository → **Actions** tab
- [ ] Click on the running workflow
- [ ] Watch "Build" job (first build takes ~5-7 minutes)
- [ ] Watch "Deploy to Control Plane" job

### 8.3 Verify Deployment on Server
- [ ] SSH into server
  ```bash
  ssh root@CONTROL_PLANE_IP
  ```

- [ ] Check container status
  ```bash
  cd /opt/dvaar
  docker compose ps
  ```

  Expected output - all containers "Up":
  - dvaar-postgres
  - dvaar-redis
  - dvaar-server
  - dvaar-caddy

- [ ] Check logs for errors
  ```bash
  docker compose logs dvaar --tail 50
  ```

---

## Phase 9: Verification

### 9.1 Test Health Endpoint
- [ ] Test API health
  ```bash
  curl -s https://api.dvaar.io/api/health | jq
  ```

  Expected response:
  ```json
  {
    "status": "healthy",
    "db": "ok",
    "redis": "ok",
    "tunnels": 0
  }
  ```

### 9.2 Test Admin Access
- [ ] Test admin metrics (use your ADMIN_TOKEN)
  ```bash
  curl -s -H "Authorization: Bearer YOUR_ADMIN_TOKEN" \
    https://admin.dvaar.io/api/metrics | jq
  ```

### 9.3 Test OAuth Flow
- [ ] Open browser to `https://api.dvaar.io/api/auth/github`
- [ ] Should redirect to GitHub authorization page
- [ ] Authorize the app
- [ ] Should redirect back with user info or token

### 9.4 Test SSL Certificates
- [ ] Verify HTTPS is working
  ```bash
  curl -I https://api.dvaar.io/api/health
  # Should show HTTP/2 200 and valid SSL
  ```

- [ ] Verify wildcard SSL
  ```bash
  curl -I https://test.dvaar.app
  # Should show HTTP/2 (404 is OK - tunnel doesn't exist)
  ```

---

## Phase 10: CLI Setup (Optional - For Testing)

### 10.1 Build CLI Locally
- [ ] Build release binary
  ```bash
  cd /path/to/dvaar
  cargo build --release --bin dvaar
  ```

- [ ] Copy to PATH
  ```bash
  cp target/release/dvaar /usr/local/bin/
  # or
  cp target/release/dvaar ~/.local/bin/
  ```

### 10.2 Configure CLI
- [ ] Login with GitHub
  ```bash
  dvaar login
  ```

- [ ] Start a test tunnel
  ```bash
  # Start a local server first
  python3 -m http.server 8000 &

  # Create tunnel
  dvaar http 8000
  ```

- [ ] Verify tunnel works
  ```bash
  # Use the URL printed by dvaar cli
  curl https://YOUR-SUBDOMAIN.dvaar.app
  ```

---

## Post-Deployment Checklist

### Security Hardening
- [ ] Change default SSH port (optional)
- [ ] Set up fail2ban alerts
- [ ] Enable Hetzner firewall in addition to UFW
- [ ] Set up monitoring (Uptime Kuma, Grafana, etc.)

### Backup Configuration
- [ ] Save all secrets securely (password manager, etc.):
  - POSTGRES_PASSWORD
  - CLUSTER_SECRET
  - ADMIN_TOKEN
  - GITHUB_CLIENT_ID
  - GITHUB_CLIENT_SECRET
  - SSH private key

### Monitoring Setup
- [ ] Set up uptime monitoring for:
  - https://api.dvaar.io/api/health
  - https://admin.dvaar.io

---

## Troubleshooting

### Build Fails
```bash
# Check GitHub Actions logs
# Common issues:
# - Missing secrets
# - Dockerfile syntax errors
# - Rust compilation errors
```

### Deployment Fails
```bash
# SSH to server and check:
ssh root@CONTROL_PLANE_IP
cd /opt/dvaar
docker compose logs --tail 100
```

### SSL Certificate Issues
```bash
# Caddy auto-provisions SSL. Check logs:
docker compose logs caddy --tail 50

# Common issues:
# - DNS not propagated yet (wait longer)
# - Cloudflare proxy enabled on dvaar.app (disable it)
```

### Database Connection Issues
```bash
# Check postgres is healthy
docker compose ps postgres
docker compose logs postgres --tail 20
```

### Redis Connection Issues
```bash
# Check redis is healthy
docker compose ps redis
docker compose logs redis --tail 20
```

---

## Cost Summary

| Resource | Provider | Cost |
|----------|----------|------|
| Control Plane (CPX21) | Hetzner | ~€15/mo |
| Domain (dvaar.io) | Varies | ~$12/yr |
| Domain (dvaar.app) | Varies | ~$15/yr |
| **Total** | | **~€18/mo** |

---

## Next Steps After Deployment

1. **Add Edge Nodes** - Scale with additional servers in different regions
2. **Set up monitoring** - Grafana, Prometheus, or simple uptime checks
3. **Configure Stripe** - Enable paid tiers when ready
4. **Build landing page** - NextJS site at dvaar.io
5. **Documentation** - API docs at dvaar.io/docs

---

*Last updated: January 2025*
