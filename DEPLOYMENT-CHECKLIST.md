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
- [ ] Add the following records:

| Type | Name | Value | Proxy | TTL | Notes |
|------|------|-------|-------|-----|-------|
| CNAME | `@` | `cname.vercel-dns.com` | OFF | Auto | Main site (Vercel) |
| A | `api` | CONTROL_PLANE_IP | Optional | Auto | API server |
| A | `admin` | CONTROL_PLANE_IP | Optional | Auto | Admin panel |
| A | `dash` | CONTROL_PLANE_IP | Optional | Auto | User dashboard |

> **Note**: Root domain (`dvaar.io`) points to Vercel for the marketing site. Subdomains point to your server. Docs and blog are at `dvaar.io/docs` and `dvaar.io/blog` (handled by Vercel).

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

  dig dvaar.io +short
  # Should return: Vercel IP (76.76.21.21 or similar)
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

### 10.2 Test CLI Login (Device Flow)
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

### 10.3 Test Tunnel
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

## Phase 11: Vercel Deployment (Marketing Site)

The main site (`dvaar.io`) with docs and blog is deployed separately on Vercel.

### 11.1 Create Vercel Project
- [ ] Go to [vercel.com](https://vercel.com) and sign in
- [ ] Click **"Add New..."** → **"Project"**
- [ ] Import your `dvaar_site` repository (or create one)
- [ ] Framework Preset: **Next.js**
- [ ] Click **"Deploy"**

### 11.2 Configure Custom Domain
- [ ] Go to Project **Settings** → **Domains**
- [ ] Add domain: `dvaar.io`
- [ ] Vercel will show you the required DNS records
- [ ] Verify DNS is configured (from Phase 5.1):
  ```
  CNAME @ -> cname.vercel-dns.com
  ```
- [ ] Wait for SSL certificate provisioning

### 11.3 Site Structure
The NextJS site should have these routes:

| Path | Content |
|------|---------|
| `/` | Landing page |
| `/docs` | Documentation (MDX or similar) |
| `/docs/*` | Individual doc pages |
| `/blog` | Blog listing |
| `/blog/*` | Individual blog posts |
| `/pricing` | Pricing page |

### 11.4 Verify Deployment
- [ ] Check main site: `https://dvaar.io`
- [ ] Check docs: `https://dvaar.io/docs`
- [ ] Check blog: `https://dvaar.io/blog`

---

## Phase 12: Stripe Integration

### 12.1 Create Stripe Account
- [ ] Go to [dashboard.stripe.com](https://dashboard.stripe.com)
- [ ] Sign up or log in
- [ ] Complete business verification (required for live mode)

### 12.2 Create Products and Prices
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

### 12.3 Get API Keys
- [ ] Go to **Developers** → **API keys**
- [ ] Copy **Secret key** (starts with `sk_live_` or `sk_test_`)
  ```
  STRIPE_SECRET_KEY=sk_live_xxxxxxxxxxxxxxxxxx
  ```

> **Note**: Use `sk_test_` keys for testing, `sk_live_` for production.

### 12.4 Configure Webhook
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

### 12.5 Configure Customer Portal
- [ ] Go to **Settings** → **Billing** → **Customer portal**
- [ ] Enable the portal
- [ ] Configure allowed actions:
  - [x] Allow customers to update subscriptions
  - [x] Allow customers to cancel subscriptions
  - [x] Allow customers to update payment methods
- [ ] Click **Save**

### 12.6 Add Stripe Secrets to Server
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

### 12.7 Verify Stripe Integration
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

### 12.8 Test Webhook (Stripe CLI - Optional)
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
  - STRIPE_SECRET_KEY
  - STRIPE_WEBHOOK_SECRET
  - STRIPE_HOBBY_PRICE_ID
  - STRIPE_PRO_PRICE_ID
  - SSH private key

### Monitoring Setup
- [ ] Set up uptime monitoring for:
  - https://dvaar.io (Vercel - marketing site)
  - https://api.dvaar.io/api/health (Hetzner - API)
  - https://admin.dvaar.io (Hetzner - admin panel)

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
| Marketing Site | Vercel | Free (Hobby) |
| Domain (dvaar.io) | Varies | ~$12/yr |
| Domain (dvaar.app) | Varies | ~$15/yr |
| **Total** | | **~€17/mo** |

---

## Next Steps After Deployment

1. **Add Edge Nodes** - Scale with additional servers in different regions
2. **Set up monitoring** - Grafana, Prometheus, or simple uptime checks
3. **Documentation** - API docs at dvaar.io/docs
4. **Email notifications** - Set up transactional emails for billing events
5. **Analytics** - Add usage analytics and dashboards

---

*Last updated: January 2025*
