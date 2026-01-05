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

### 2.4 Create Production Environment (Optional)
- [ ] Go to repository **Settings** → **Environments**
- [ ] Click **"New environment"**
- [ ] Name: `production`
- [ ] Click **"Configure environment"**
- [ ] (Optional, paid plans only) Enable "Required reviewers" for deployment approval
- [ ] Can restrict deployment to specific branches (e.g., `main` only)

> **Note**: Environment protection rules like "Required reviewers" require GitHub Team or Enterprise. On the free plan, you still get environment secrets/variables which is sufficient.

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
- [ ] Check **"Enable Device Flow"** (required for CLI authentication)
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

## Phase 4: SSH Deploy Key Setup

> **IMPORTANT**: Create dedicated deploy keys - never use personal SSH keys for deployment.

### 4.1 Generate Deploy Key
- [ ] On your **local machine**, generate a new key pair:
  ```bash
  # Generate deploy key (no passphrase for CI/CD automation)
  ssh-keygen -t ed25519 -C "dvaar-deploy" -f ~/.ssh/dvaar_deploy -N ""

  # This creates two files:
  # ~/.ssh/dvaar_deploy      (private key - goes in GitHub secrets)
  # ~/.ssh/dvaar_deploy.pub  (public key - goes on Hetzner + servers)
  ```

### 4.2 View Public Key
- [ ] Copy your public key (you'll need this for Hetzner):
  ```bash
  cat ~/.ssh/dvaar_deploy.pub
  ```

### 4.3 Security Notes
- [ ] Store deploy key backup in password manager (1Password, Bitwarden, etc.)
- [ ] Never commit keys to git
- [ ] Set reminder to rotate keys every 6-12 months

---

## Phase 5: Hetzner Server Provisioning

### 5.1 Create Hetzner Account
- [ ] Go to [accounts.hetzner.com](https://accounts.hetzner.com)
- [ ] Sign up or log in
- [ ] Add payment method
- [ ] Go to [console.hetzner.cloud](https://console.hetzner.cloud)

### 5.2 Create Project
- [ ] Click **"+ New Project"**
- [ ] Name: `dvaar`
- [ ] Click **"Add project"**
- [ ] Click into the project

### 5.3 Add SSH Key
- [ ] Go to **Security** → **SSH Keys**
- [ ] Click **"Add SSH Key"**
- [ ] Paste your deploy key public key (from Phase 4.2)
- [ ] Name: `dvaar-deploy`
- [ ] Click **"Add SSH Key"**

### 5.4 Create Control Plane Server
- [ ] Click **"Add Server"**
- [ ] **Location**: Choose based on your primary user base
  | Location | Code | Best For |
  |----------|------|----------|
  | Falkenstein, DE | fsn1 | Central Europe |
  | Nuremberg, DE | nbg1 | Central Europe |
  | Helsinki, FI | hel1 | Nordic/Eastern Europe |
  | Ashburn, US | ash | US East Coast |
  | Hillsboro, US | hil | US West Coast |

- [ ] **Image**: Ubuntu 24.04
- [ ] **Type**: Shared vCPU → **CPX21** (recommended for control plane)
  | Spec | Value |
  |------|-------|
  | vCPU | 3 AMD |
  | RAM | 4 GB |
  | SSD | 80 GB |
  | Traffic | 20 TB |
  | Price | ~€8.50/mo |

- [ ] **Networking**: Public IPv4 (default)
- [ ] **SSH Keys**: Select your `dvaar-deploy` key
- [ ] **Name**: `dvaar-control`
- [ ] Click **"Create & Buy now"**

### 5.5 Create Edge Node Servers (2 nodes)

Create two edge servers in different locations for geographic distribution:

#### Edge Node 1
- [ ] Click **"Add Server"**
- [ ] **Location**: Different from control plane (e.g., `ash` for US East)
- [ ] **Image**: Ubuntu 24.04
- [ ] **Type**: Shared vCPU → **CPX11** (sufficient for edge nodes)
  | Spec | Value |
  |------|-------|
  | vCPU | 2 AMD |
  | RAM | 2 GB |
  | SSD | 40 GB |
  | Traffic | 20 TB |
  | Price | ~€4.50/mo |

- [ ] **Networking**: Public IPv4 (default)
- [ ] **SSH Keys**: Select your `dvaar-deploy` key
- [ ] **Name**: `dvaar-edge-1`
- [ ] Click **"Create & Buy now"**

#### Edge Node 2
- [ ] Click **"Add Server"**
- [ ] **Location**: Third location (e.g., `hil` for US West or `hel1` for Nordic)
- [ ] **Image**: Ubuntu 24.04
- [ ] **Type**: Shared vCPU → **CPX11**
- [ ] **Networking**: Public IPv4 (default)
- [ ] **SSH Keys**: Select your `dvaar-deploy` key
- [ ] **Name**: `dvaar-edge-2`
- [ ] Click **"Create & Buy now"**

### 5.6 Record All Server IPs
- [ ] Wait for all servers to be "Running" (1-2 minutes each)
- [ ] Record all IPv4 addresses:
  ```
  Control Plane IP: xxx.xxx.xxx.xxx  (dvaar-control)
  Edge Node 1 IP:   yyy.yyy.yyy.yyy  (dvaar-edge-1)
  Edge Node 2 IP:   zzz.zzz.zzz.zzz  (dvaar-edge-2)
  ```

### 5.7 Verify SSH Access to All Servers
- [ ] Test SSH to control plane
  ```bash
  ssh -i ~/.ssh/dvaar_deploy root@CONTROL_PLANE_IP
  exit
  ```

- [ ] Test SSH to edge node 1
  ```bash
  ssh -i ~/.ssh/dvaar_deploy root@EDGE_NODE_1_IP
  exit
  ```

- [ ] Test SSH to edge node 2
  ```bash
  ssh -i ~/.ssh/dvaar_deploy root@EDGE_NODE_2_IP
  exit
  ```

---

## Phase 6: DNS Configuration

### 6.1 Configure dvaar.io DNS Records
- [ ] Log into your DNS provider (Cloudflare, Namecheap, etc.)
- [ ] Navigate to DNS settings for `dvaar.io`
- [ ] Add the following records:

| Type | Name | Value | Proxy | TTL | Notes |
|------|------|-------|-------|-----|-------|
| CNAME | `@` | `cname.vercel-dns.com` | OFF | Auto | Main site (Vercel) |
| A | `api` | CONTROL_PLANE_IP | Optional | Auto | API server |
| A | `admin` | CONTROL_PLANE_IP | Optional | Auto | Admin panel |
| A | `dash` | CONTROL_PLANE_IP | Optional | Auto | User dashboard |

> **Note**: Root domain (`dvaar.io`) points to Vercel for the marketing site. Subdomains point to your server. Docs and blog are at `dvaar.io/docs` and `dvaar.io/blog` (handled by Vercel).

### 6.2 Configure dvaar.app DNS Records
- [ ] Navigate to DNS settings for `dvaar.app`
- [ ] **For initial setup** (control plane only), add:

| Type | Name | Value | Proxy | TTL |
|------|------|-------|-------|-----|
| A | `@` | CONTROL_PLANE_IP | **OFF** | Auto |
| A | `*` | CONTROL_PLANE_IP | **OFF** | Auto |

> **IMPORTANT**:
> - Proxy must be **DISABLED** (DNS only / gray cloud in Cloudflare) - WebSockets need direct access
> - This points all traffic to control plane initially
> - After setting up edge nodes (Phase 11), update DNS to include all nodes for load distribution

### 6.3 Verify DNS Propagation
- [ ] Wait 5-10 minutes for propagation
- [ ] Verify records:
  ```bash
  dig api.dvaar.io +short
  # Should return: CONTROL_PLANE_IP

  dig test.dvaar.app +short
  # Should return: CONTROL_PLANE_IP

  dig dvaar.io +short
  # Should return: Vercel IP (76.76.21.21 or similar)
  ```

---

## Phase 7: Server Setup

### 7.1 SSH into Control Plane
- [ ] Connect to server
  ```bash
  ssh root@CONTROL_PLANE_IP
  ```

### 7.2 Copy Scripts to Server
Since the repo is private, copy the scripts from your local machine:

- [ ] From your **local machine** (not the server):
  ```bash
  # Copy setup scripts to server
  scp scripts/setup-server.sh root@CONTROL_PLANE_IP:/tmp/
  scp scripts/add-edge-node.sh root@CONTROL_PLANE_IP:/tmp/
  ```

### 7.3 Run Setup Script
- [ ] SSH into the server:
  ```bash
  ssh root@CONTROL_PLANE_IP
  ```

- [ ] Set environment variables:
  ```bash
  export GITHUB_REPO="YOUR_USERNAME/dvaar"
  export BASE_DOMAIN="dvaar.io"
  export TUNNEL_DOMAIN="dvaar.app"
  ```

- [ ] Run the setup script:
  ```bash
  bash /tmp/setup-server.sh control-plane
  ```

- [ ] **IMPORTANT**: Save the generated secrets displayed at the end:
  ```
  POSTGRES_PASSWORD=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
  CLUSTER_SECRET=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
  ADMIN_TOKEN=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
  ```

### 7.4 Configure GitHub OAuth
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

### 7.5 Verify Configuration
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

## Phase 8: Configure GitHub Secrets

### 8.1 Add SSH Private Key
- [ ] Go to repository **Settings** → **Secrets and variables** → **Actions**
- [ ] Click **"New repository secret"**
- [ ] Name: `SSH_PRIVATE_KEY`
- [ ] Value: Contents of your **private** deploy key
  ```bash
  # Get private key content (run locally)
  cat ~/.ssh/dvaar_deploy
  ```
- [ ] Click **"Add secret"**

### 8.2 Add Control Plane Host
- [ ] Click **"New repository secret"**
- [ ] Name: `CONTROL_PLANE_HOST`
- [ ] Value: Your control plane IP address (from Phase 5.6)
- [ ] Click **"Add secret"**

### 8.3 Verify Secrets
- [ ] Confirm secrets are listed:
  - `SSH_PRIVATE_KEY`
  - `CONTROL_PLANE_HOST`

---

## Phase 9: Initial Deployment

### 9.1 Trigger CI/CD Pipeline
- [ ] From your local machine:
  ```bash
  cd /path/to/dvaar
  git commit --allow-empty -m "Trigger initial deployment"
  git push
  ```

### 9.2 Monitor Build
- [ ] Go to repository → **Actions** tab
- [ ] Click on the running workflow
- [ ] Watch "Build" job (first build takes ~5-7 minutes)
- [ ] Watch "Deploy to Control Plane" job

### 9.3 Verify Deployment on Server
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

## Phase 10: Verification

### 10.1 Test Health Endpoint
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

### 10.2 Test Admin Access
- [ ] Test admin metrics (use your ADMIN_TOKEN)
  ```bash
  curl -s -H "Authorization: Bearer YOUR_ADMIN_TOKEN" \
    https://admin.dvaar.io/api/metrics | jq
  ```

### 10.3 Test OAuth Flow
- [ ] Open browser to `https://api.dvaar.io/api/auth/github`
- [ ] Should redirect to GitHub authorization page
- [ ] Authorize the app
- [ ] Should redirect back with user info or token

### 10.4 Test SSL Certificates
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

## Phase 11: Edge Node Setup

Now that the control plane is running, set up the two edge nodes.

### 11.1 Copy Deploy Key to Control Plane
- [ ] From your local machine, copy the deploy key:
  ```bash
  scp ~/.ssh/dvaar_deploy root@CONTROL_PLANE_IP:~/.ssh/dvaar_deploy
  ssh root@CONTROL_PLANE_IP "chmod 600 ~/.ssh/dvaar_deploy"
  ```

### 11.2 Set Up Edge Node 1
- [ ] SSH into control plane:
  ```bash
  ssh root@CONTROL_PLANE_IP
  ```

- [ ] Run add-edge-node script (already copied in Phase 7.2):
  ```bash
  cd /opt/dvaar
  cp /tmp/add-edge-node.sh .
  chmod +x add-edge-node.sh
  ./add-edge-node.sh EDGE_NODE_1_IP
  ```

- [ ] Verify edge node 1 is running:
  ```bash
  ssh -i ~/.ssh/dvaar_deploy root@EDGE_NODE_1_IP "cd /opt/dvaar && docker compose ps"
  ```

### 11.3 Set Up Edge Node 2
- [ ] From control plane, run:
  ```bash
  ./add-edge-node.sh EDGE_NODE_2_IP
  ```

- [ ] Verify edge node 2 is running:
  ```bash
  ssh -i ~/.ssh/dvaar_deploy root@EDGE_NODE_2_IP "cd /opt/dvaar && docker compose ps"
  ```

### 11.4 Configure GitHub Actions for Edge Nodes
- [ ] Go to repository **Settings** → **Variables** → **Actions**
- [ ] Click **"New repository variable"**
- [ ] Name: `EDGE_NODES`
- [ ] Value (JSON array of edge node IPs):
  ```json
  ["EDGE_NODE_1_IP", "EDGE_NODE_2_IP"]
  ```
- [ ] Click **"Add variable"**

### 11.5 Update DNS for Load Balancing

> **IMPORTANT**: Without this step, all traffic goes to control plane and edge nodes are unused!

#### Option A: Round-Robin DNS (Simple, Free)
- [ ] Update DNS records for `*.dvaar.app` to include all nodes:
  ```
  A  @  CONTROL_PLANE_IP
  A  @  EDGE_NODE_1_IP
  A  @  EDGE_NODE_2_IP
  A  *  CONTROL_PLANE_IP
  A  *  EDGE_NODE_1_IP
  A  *  EDGE_NODE_2_IP
  ```
- DNS will rotate between all IPs (not geo-aware, but distributes load)

#### Option B: Cloudflare Load Balancing (~$5/mo)
- [ ] Go to Cloudflare → Traffic → Load Balancing
- [ ] Create a pool with all three server IPs
- [ ] Enable health checks (HTTP to `/health`)
- [ ] Create load balancer for `*.dvaar.app`
- [ ] Set steering policy to "Geo" or "Dynamic"
- Provides automatic failover + geographic routing

#### Option C: Keep Control Plane Only (Not recommended)
- Skip this step if you want all traffic through control plane
- Edge nodes will only be used for tunnels that explicitly connect to them

### 11.6 Verify Edge Nodes
- [ ] Test health on each edge node:
  ```bash
  curl -s https://api.dvaar.io/api/health | jq  # Control plane

  # Edge nodes should respond on their IPs (via direct request)
  curl -sk https://EDGE_NODE_1_IP/health
  curl -sk https://EDGE_NODE_2_IP/health
  ```

---

## Phase 12: CLI Setup (Optional - For Testing)

### 12.1 Build CLI Locally
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

### 12.2 Test CLI Login (Device Flow)
- [ ] Run login command
  ```bash
  dvaar login
  ```

- [ ] You should see:
  ```
  Authenticating with GitHub...

  ! First, copy your one-time code: XXXX-XXXX

  Press Enter to open https://github.com/login/device in your browser...
  ```

- [ ] Press Enter, browser opens to GitHub
- [ ] Enter the code shown in terminal
- [ ] Authorize the Dvaar app
- [ ] Terminal shows: `Logged in as your@email.com`

### 12.3 Test Tunnel
- [ ] Start a local server
  ```bash
  python3 -m http.server 8000 &
  ```

- [ ] Create tunnel
  ```bash
  dvaar http 8000
  ```

- [ ] Verify tunnel works
  ```bash
  # Use the URL printed by dvaar cli
  curl https://YOUR-SUBDOMAIN.dvaar.app
  ```

---

## Phase 13: Vercel Deployment (Marketing Site)

The main site (`dvaar.io`) with docs and blog is deployed separately on Vercel.

### 13.1 Create Vercel Project
- [ ] Go to [vercel.com](https://vercel.com) and sign in
- [ ] Click **"Add New..."** → **"Project"**
- [ ] Import your `dvaar_site` repository (or create one)
- [ ] Framework Preset: **Next.js**
- [ ] Click **"Deploy"**

### 13.2 Configure Custom Domain
- [ ] Go to Project **Settings** → **Domains**
- [ ] Add domain: `dvaar.io`
- [ ] Vercel will show you the required DNS records
- [ ] Verify DNS is configured (from Phase 5.1):
  ```
  CNAME @ -> cname.vercel-dns.com
  ```
- [ ] Wait for SSL certificate provisioning

### 13.3 Site Structure
The NextJS site should have these routes:

| Path | Content |
|------|---------|
| `/` | Landing page |
| `/docs` | Documentation (MDX or similar) |
| `/docs/*` | Individual doc pages |
| `/blog` | Blog listing |
| `/blog/*` | Individual blog posts |
| `/pricing` | Pricing page |

### 13.4 Verify Deployment
- [ ] Check main site: `https://dvaar.io`
- [ ] Check docs: `https://dvaar.io/docs`
- [ ] Check blog: `https://dvaar.io/blog`

---

## Phase 14: Stripe Integration

### 14.1 Create Stripe Account
- [ ] Go to [dashboard.stripe.com](https://dashboard.stripe.com)
- [ ] Sign up or log in
- [ ] Complete business verification (required for live mode)

### 14.2 Create Products and Prices
- [ ] Go to **Products** → **Add product**
- [ ] Create **Hobby** product:
  - Name: `Hobby`
  - Description: `For developers who need custom domains`
  - Price: `$5/month` (recurring)
  - Click **Save product**
  - Copy the **Price ID** (starts with `price_`)
    ```
    STRIPE_HOBBY_PRICE_ID=price_xxxxxxxxxxxxxxxxxx
    ```

- [ ] Create **Pro** product:
  - Name: `Pro`
  - Description: `For teams and production workloads`
  - Price: `$15/month` (recurring)
  - Click **Save product**
  - Copy the **Price ID**
    ```
    STRIPE_PRO_PRICE_ID=price_xxxxxxxxxxxxxxxxxx
    ```

### 14.3 Get API Keys
- [ ] Go to **Developers** → **API keys**
- [ ] Copy **Secret key** (starts with `sk_live_` or `sk_test_`)
  ```
  STRIPE_SECRET_KEY=sk_live_xxxxxxxxxxxxxxxxxx
  ```

> **Note**: Use `sk_test_` keys for testing, `sk_live_` for production.

### 14.4 Configure Webhook
- [ ] Go to **Developers** → **Webhooks**
- [ ] Click **Add endpoint**
- [ ] Endpoint URL: `https://api.dvaar.io/api/billing/webhook`
- [ ] Select events to listen for:
  - `checkout.session.completed`
  - `customer.subscription.updated`
  - `customer.subscription.deleted`
  - `invoice.payment_failed`
- [ ] Click **Add endpoint**
- [ ] Click on the endpoint → **Reveal** signing secret
- [ ] Copy **Signing secret** (starts with `whsec_`)
  ```
  STRIPE_WEBHOOK_SECRET=whsec_xxxxxxxxxxxxxxxxxx
  ```

### 14.5 Configure Customer Portal
- [ ] Go to **Settings** → **Billing** → **Customer portal**
- [ ] Enable the portal
- [ ] Configure allowed actions:
  - [x] Allow customers to update subscriptions
  - [x] Allow customers to cancel subscriptions
  - [x] Allow customers to update payment methods
- [ ] Click **Save**

### 14.6 Add Stripe Secrets to Server
- [ ] SSH into server
  ```bash
  ssh root@CONTROL_PLANE_IP
  ```

- [ ] Edit environment file
  ```bash
  nano /opt/dvaar/.env
  ```

- [ ] Add Stripe configuration:
  ```bash
  STRIPE_SECRET_KEY=sk_live_xxxxxxxxxxxxxxxxxx
  STRIPE_WEBHOOK_SECRET=whsec_xxxxxxxxxxxxxxxxxx
  STRIPE_HOBBY_PRICE_ID=price_xxxxxxxxxxxxxxxxxx
  STRIPE_PRO_PRICE_ID=price_xxxxxxxxxxxxxxxxxx
  ```

- [ ] Save and exit (Ctrl+X, Y, Enter)

- [ ] Restart the server
  ```bash
  cd /opt/dvaar
  docker compose restart dvaar
  ```

### 14.7 Verify Stripe Integration
- [ ] Test webhook endpoint
  ```bash
  curl -X POST https://api.dvaar.io/api/billing/webhook \
    -H "Content-Type: application/json" \
    -d '{}'
  # Should return 400 "Missing signature" (correct - signature required)
  ```

- [ ] Test plans endpoint
  ```bash
  curl https://api.dvaar.io/api/billing/plans | jq
  ```
  Expected output:
  ```json
  {
    "plans": [
      {"id": "free", "name": "Free", "price": 0, ...},
      {"id": "hobby", "name": "Hobby", "price": 5, ...},
      {"id": "pro", "name": "Pro", "price": 15, ...}
    ]
  }
  ```

- [ ] Test checkout flow via CLI
  ```bash
  dvaar upgrade hobby
  # Should open Stripe checkout page in browser
  ```

### 14.8 Test Webhook (Stripe CLI - Optional)
- [ ] Install Stripe CLI: [stripe.com/docs/stripe-cli](https://stripe.com/docs/stripe-cli)
- [ ] Login: `stripe login`
- [ ] Forward webhooks to local:
  ```bash
  stripe listen --forward-to localhost:3000/api/billing/webhook
  ```
- [ ] Trigger test event:
  ```bash
  stripe trigger checkout.session.completed
  ```

---

## Post-Deployment Checklist

### Security Hardening

#### SSH Hardening
- [ ] Disable password authentication:
  ```bash
  sudo nano /etc/ssh/sshd_config
  # Set: PasswordAuthentication no
  # Set: PermitRootLogin prohibit-password
  sudo systemctl restart sshd
  ```

- [ ] Change default SSH port (optional but recommended):
  ```bash
  sudo nano /etc/ssh/sshd_config
  # Set: Port 2222  (or another high port)
  sudo ufw allow 2222/tcp
  sudo ufw delete allow ssh
  sudo systemctl restart sshd
  ```

#### Fail2ban Configuration
- [ ] Configure fail2ban for SSH:
  ```bash
  sudo nano /etc/fail2ban/jail.local
  ```
  ```ini
  [sshd]
  enabled = true
  port = ssh
  filter = sshd
  logpath = /var/log/auth.log
  maxretry = 3
  bantime = 3600
  findtime = 600
  ```
  ```bash
  sudo systemctl restart fail2ban
  sudo fail2ban-client status sshd
  ```

#### Hetzner Cloud Firewall
- [ ] Go to Hetzner Cloud Console → Firewalls
- [ ] Create firewall with rules:
  | Direction | Protocol | Port | Source |
  |-----------|----------|------|--------|
  | Inbound | TCP | 22 (or custom) | Your IP only |
  | Inbound | TCP | 80 | Any |
  | Inbound | TCP | 443 | Any |
  | Inbound | TCP | 6000 | Control plane IP (for edge nodes) |
- [ ] Apply firewall to your servers

#### Automatic Security Updates
- [ ] Enable unattended upgrades:
  ```bash
  sudo apt install unattended-upgrades
  sudo dpkg-reconfigure -plow unattended-upgrades
  ```

---

### Database Backup Procedures

#### Automated Daily Backups
- [ ] Create backup script on control plane:
  ```bash
  sudo nano /opt/dvaar/backup.sh
  ```
  ```bash
  #!/bin/bash
  set -euo pipefail

  BACKUP_DIR="/opt/dvaar/backups"
  TIMESTAMP=$(date +%Y%m%d_%H%M%S)
  RETENTION_DAYS=7

  mkdir -p "$BACKUP_DIR"

  # Backup PostgreSQL
  docker compose exec -T postgres pg_dump -U dvaar dvaar | gzip > "$BACKUP_DIR/db_$TIMESTAMP.sql.gz"

  # Backup .env file
  cp /opt/dvaar/.env "$BACKUP_DIR/env_$TIMESTAMP.bak"

  # Delete backups older than retention period
  find "$BACKUP_DIR" -name "db_*.sql.gz" -mtime +$RETENTION_DAYS -delete
  find "$BACKUP_DIR" -name "env_*.bak" -mtime +$RETENTION_DAYS -delete

  echo "Backup completed: $TIMESTAMP"
  ```
  ```bash
  chmod +x /opt/dvaar/backup.sh
  ```

- [ ] Set up daily cron job:
  ```bash
  sudo crontab -e
  # Add this line (runs at 3 AM daily):
  0 3 * * * /opt/dvaar/backup.sh >> /var/log/dvaar-backup.log 2>&1
  ```

- [ ] Test backup manually:
  ```bash
  /opt/dvaar/backup.sh
  ls -la /opt/dvaar/backups/
  ```

#### Manual Backup Before Updates
- [ ] Before any major update:
  ```bash
  cd /opt/dvaar

  # Backup database
  docker compose exec -T postgres pg_dump -U dvaar dvaar > backup_before_update.sql

  # Backup current images
  docker images | grep dvaar

  # Note current version
  docker compose ps
  ```

#### Restore from Backup
- [ ] To restore database:
  ```bash
  cd /opt/dvaar

  # Stop the application
  docker compose stop dvaar

  # Restore database
  gunzip -c /opt/dvaar/backups/db_TIMESTAMP.sql.gz | \
    docker compose exec -T postgres psql -U dvaar dvaar

  # Restart application
  docker compose start dvaar
  ```

---

### Rollback Procedures

#### Quick Rollback (Same Version, Config Issue)
- [ ] If something breaks after a config change:
  ```bash
  cd /opt/dvaar

  # Restore previous .env
  cp /opt/dvaar/backups/env_TIMESTAMP.bak .env

  # Restart services
  docker compose restart
  ```

#### Version Rollback (Bad Deployment)
- [ ] To rollback to a previous Docker image version:
  ```bash
  cd /opt/dvaar

  # Edit .env to specify previous version
  nano .env
  # Change: VERSION=latest
  # To:     VERSION=v1.2.3  (previous working version)

  # Or edit docker-compose.yml directly:
  # image: ghcr.io/strawberry-labs/dvaar:v1.2.3

  # Pull and restart
  docker compose pull
  docker compose up -d
  ```

- [ ] To find available versions:
  ```bash
  # Check GitHub releases
  curl -s https://api.github.com/repos/strawberry-labs/dvaar/releases | jq '.[].tag_name'

  # Or check GitHub Container Registry
  # https://github.com/strawberry-labs/dvaar/pkgs/container/dvaar
  ```

#### Full Disaster Recovery
- [ ] If server is completely compromised:
  1. Provision new server from Hetzner
  2. Run setup script: `curl -sSL https://raw.githubusercontent.com/strawberry-labs/dvaar/main/scripts/setup-server.sh | bash`
  3. Restore `.env` from backup (password manager or backup location)
  4. Restore database from backup
  5. Update DNS to point to new server
  6. Update GitHub secrets with new server IP

---

### Monitoring Setup

#### Uptime Monitoring (Free Options)
- [ ] Set up [UptimeRobot](https://uptimerobot.com) (free tier: 50 monitors):
  - Monitor: `https://dvaar.io` (marketing site)
  - Monitor: `https://api.dvaar.io/health` (API health)
  - Monitor: `https://admin.dvaar.io` (admin panel)
  - Set alert contacts (email, Slack, etc.)

- [ ] Or use [Better Uptime](https://betterstack.com/better-uptime):
  - Similar setup with nicer status pages

#### Server Monitoring
- [ ] Install netdata for real-time server metrics:
  ```bash
  bash <(curl -Ss https://my-netdata.io/kickstart.sh)
  # Access at: http://YOUR_IP:19999
  ```

- [ ] Or use Hetzner's built-in monitoring:
  - Go to Hetzner Cloud Console → Server → Metrics
  - Enable alerts for CPU, memory, disk

#### Log Aggregation (Optional)
- [ ] For production, consider:
  - [Grafana Loki](https://grafana.com/oss/loki/) - free, self-hosted
  - [Papertrail](https://www.papertrail.com/) - easy setup, free tier
  - [Logtail](https://betterstack.com/logtail) - modern, free tier

#### Docker Log Management
- [ ] Configure Docker log rotation:
  ```bash
  sudo nano /etc/docker/daemon.json
  ```
  ```json
  {
    "log-driver": "json-file",
    "log-opts": {
      "max-size": "10m",
      "max-file": "3"
    }
  }
  ```
  ```bash
  sudo systemctl restart docker
  ```

---

### Secrets Management

#### Store Secrets Securely
- [ ] Save all secrets in a password manager (1Password, Bitwarden, etc.):
  - `POSTGRES_PASSWORD`
  - `CLUSTER_SECRET`
  - `ADMIN_TOKEN`
  - `GITHUB_CLIENT_ID`
  - `GITHUB_CLIENT_SECRET`
  - `STRIPE_SECRET_KEY`
  - `STRIPE_WEBHOOK_SECRET`
  - `STRIPE_HOBBY_PRICE_ID`
  - `STRIPE_PRO_PRICE_ID`
  - SSH deploy key (private key)

#### Secret Rotation Schedule
- [ ] Set calendar reminders:
  | Secret | Rotation Frequency | Notes |
  |--------|-------------------|-------|
  | SSH deploy key | Every 6-12 months | Update all servers + GitHub |
  | ADMIN_TOKEN | Every 6 months | Update .env on server |
  | Stripe keys | Only if compromised | Regenerate in Stripe dashboard |
  | GitHub OAuth | Only if compromised | Regenerate in GitHub settings |
  | POSTGRES_PASSWORD | Every 12 months | Requires DB update + .env |
  | CLUSTER_SECRET | Every 12 months | Update control plane + all edge nodes |

#### If a Secret is Compromised
- [ ] Immediate actions:
  1. Rotate the compromised secret immediately
  2. Check logs for unauthorized access
  3. Update all locations using that secret
  4. If SSH key: remove old public key from all servers
  5. If Stripe: check for unauthorized transactions

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
| Control Plane (CPX21) | Hetzner | ~€8.50/mo |
| Edge Node 1 (CPX11) | Hetzner | ~€4.50/mo |
| Edge Node 2 (CPX11) | Hetzner | ~€4.50/mo |
| Marketing Site | Vercel | Free (Hobby) |
| Domain (dvaar.io) | Varies | ~$12/yr (~€1/mo) |
| Domain (dvaar.app) | Varies | ~$15/yr (~€1.25/mo) |
| **Total** | | **~€20/mo** |

> **Note**: Hetzner prices are approximate and may vary. All servers include 20TB traffic/month which is more than sufficient for most use cases.

---

## Next Steps After Deployment

1. **Add Edge Nodes** - Scale with additional servers in different regions
2. **Set up monitoring** - Grafana, Prometheus, or simple uptime checks
3. **Documentation** - API docs at dvaar.io/docs
4. **Email notifications** - Set up transactional emails for billing events
5. **Analytics** - Add usage analytics and dashboards

---

*Last updated: January 2025*
