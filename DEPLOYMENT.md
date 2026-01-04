# Dvaar Production Deployment Guide

## Table of Contents
1. [Domain Structure](#domain-structure)
2. [Server Recommendations](#server-recommendations)
3. [Pricing Tiers](#pricing-tiers)
4. [Single Server Setup](#single-server-setup)
5. [Adding Edge Nodes (5-min process)](#adding-edge-nodes)
6. [CI/CD Pipeline](#cicd-pipeline)
7. [DNS Configuration](#dns-configuration)
8. [Monitoring & Admin](#monitoring--admin)
9. [Multi-Provider Setup](#multi-provider-setup)

---

## Domain Structure

| Domain | Purpose | Hosted On |
|--------|---------|-----------|
| **dvaar.io** | Main site, docs, blog | Vercel |
| **api.dvaar.io** | API server | Hetzner |
| **admin.dvaar.io** | Admin panel | Hetzner |
| **dash.dvaar.io** | User dashboard | Hetzner |
| **dvaar.app** | Tunnel URLs (`*.dvaar.app`) | Hetzner |
| dvaar.link | Backup tunnel domain | Reserve |
| dvaar.to | Backup | Reserve |
| dvaar.dev | Backup | Reserve |

**URL Examples:**
- Main site: `https://dvaar.io` (Vercel)
- Docs: `https://dvaar.io/docs` (Vercel)
- Blog: `https://dvaar.io/blog` (Vercel)
- API: `https://api.dvaar.io` (Hetzner)
- Admin: `https://admin.dvaar.io` (Hetzner)
- Dashboard: `https://dash.dvaar.io` (Hetzner)
- Tunnel: `https://myapp.dvaar.app` (Hetzner)

---

## Server Recommendations

### Single Server (Start Here)
For starting out, run everything on one Hetzner dedicated server:

| Server | Specs | Price | Use Case |
|--------|-------|-------|----------|
| **AX42** (Recommended) | AMD Ryzen 5 3600, 64GB RAM, 2x512GB NVMe | ~€52/mo | Perfect for MVP, handles 1000+ concurrent tunnels |
| AX52 | AMD Ryzen 7 3700X, 64GB RAM, 2x1TB NVMe | ~€67/mo | More headroom for growth |
| AX102 | AMD Ryzen 9 5950X, 128GB RAM, 2x1.92TB NVMe | ~€130/mo | High traffic, 10k+ tunnels |

**Why Dedicated over VPS?**
- Consistent performance (no noisy neighbors)
- Better network throughput (1 Gbps guaranteed)
- More cost-effective at scale
- NVMe storage for fast Redis/Postgres

### Scaling: Edge Nodes
When you need to scale (>5000 concurrent tunnels or geo-distribution):

| Provider | Server | Specs | Price | Region |
|----------|--------|-------|-------|--------|
| Hetzner | CAX21 | 4 vCPU ARM, 8GB RAM | ~€6/mo | EU (Falkenstein, Helsinki) |
| Hetzner | CAX31 | 8 vCPU ARM, 16GB RAM | ~€11/mo | EU |
| Hetzner | CPX31 | 4 vCPU AMD, 8GB RAM | ~€14/mo | US (Ashburn) |
| Vultr | VC2 | 2 vCPU, 4GB RAM | ~$24/mo | Global (25 regions) |
| DigitalOcean | Basic | 2 vCPU, 4GB RAM | ~$24/mo | Global |

**Architecture:**
```
                    ┌─────────────────────────────────────┐
                    │     Control Plane (AX42)            │
                    │  ┌─────────┐ ┌─────────┐ ┌───────┐  │
                    │  │ Postgres│ │  Redis  │ │ Dvaar │  │
                    │  └─────────┘ └─────────┘ └───────┘  │
                    └─────────────────────────────────────┘
                                     │
              ┌──────────────────────┼──────────────────────┐
              ▼                      ▼                      ▼
    ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
    │ Edge EU (CAX21) │    │ Edge US (CPX31) │    │ Edge Asia (Vultr)│
    │     Dvaar       │    │     Dvaar       │    │     Dvaar        │
    └─────────────────┘    └─────────────────┘    └─────────────────┘
```

---

## Pricing Tiers

Recommended pricing structure:

### Free Tier
- **Price:** $0/mo
- Random subdomains only (e.g., `happy-fox-123.dvaar.app`)
- 3 concurrent tunnels
- 1GB bandwidth/month
- Community support

### Hobby
- **Price:** $5/mo
- Custom subdomains (e.g., `myapp.dvaar.app`) - pick any available name
- Custom domains via CNAME (e.g., `api.yoursite.com` → `myapp.dvaar.app`)
- 5 concurrent tunnels
- 100GB bandwidth/month
- Email support

### Pro
- **Price:** $15/mo
- Reserved subdomains (guaranteed yours, never expires)
- Custom domains via CNAME (unlimited)
- 20 concurrent tunnels
- 1TB bandwidth/month
- Priority support
- Webhook notifications
- IP allowlisting

**Subdomain Types Explained:**
- **Random subdomain** (Free): System-generated like `quick-fox-847.dvaar.app`, changes each session
- **Custom subdomain** (Hobby+): You pick the name like `myapp.dvaar.app`, first-come-first-serve
- **Reserved subdomain** (Pro): Your subdomain is locked to your account, even when not tunneling

**Custom Domain Setup (Hobby & Pro):**
1. Add CNAME record: `api.yoursite.com` → `myapp.dvaar.app`
2. In CLI: `dvaar http 8080 --domain myapp --custom-domain api.yoursite.com`
3. Dvaar verifies DNS and routes traffic

---

## Single Server Setup

### Prerequisites
1. Hetzner account with AX42 server
2. Domains pointed to Hetzner DNS or Cloudflare (dvaar.io + dvaar.app)
3. GitHub account (for OAuth + CI/CD)

### Step 1: Order Server

1. Go to [Hetzner Robot](https://robot.hetzner.com)
2. Order **AX42** in your preferred location (Falkenstein recommended)
3. Choose **Ubuntu 22.04** as OS
4. Wait for provisioning (~15 min)

### Step 2: Initial Server Setup

SSH into your server:
```bash
ssh root@YOUR_SERVER_IP
```

Run the setup script:
```bash
curl -sSL https://raw.githubusercontent.com/YOUR_GITHUB_USER/dvaar/main/scripts/setup-server.sh | bash -s -- control-plane
```

**Save the generated secrets!** The script outputs:
- `POSTGRES_PASSWORD`
- `CLUSTER_SECRET`
- `ADMIN_TOKEN`

### Step 3: Configure GitHub OAuth

1. Go to [GitHub Developer Settings](https://github.com/settings/developers)
2. Create new OAuth App:
   - **Application name:** Dvaar
   - **Homepage URL:** `https://dvaar.io`
   - **Authorization callback URL:** `https://api.dvaar.io/api/auth/github/callback`
3. Copy Client ID and Client Secret

Edit `/opt/dvaar/.env`:
```bash
nano /opt/dvaar/.env
```

Add:
```env
GITHUB_CLIENT_ID=your_client_id
GITHUB_CLIENT_SECRET=your_client_secret
```

### Step 4: Configure DNS

**For dvaar.io (main brand):**
| Type | Name | Value | Proxy |
|------|------|-------|-------|
| A | @ | YOUR_SERVER_IP | Yes (if Cloudflare) |
| A | www | YOUR_SERVER_IP | Yes |
| A | api | YOUR_SERVER_IP | Yes |
| A | admin | YOUR_SERVER_IP | Yes |
| A | dash | YOUR_SERVER_IP | Yes |
| A | docs | YOUR_SERVER_IP | Yes |

**For dvaar.app (tunnel URLs):**
| Type | Name | Value | Proxy |
|------|------|-------|-------|
| A | @ | YOUR_SERVER_IP | Yes |
| A | * | YOUR_SERVER_IP | **No** (must be DNS-only for tunnels) |

**Important:** Wildcard record on dvaar.app must NOT be proxied through Cloudflare!

### Step 5: Start Services

```bash
cd /opt/dvaar
docker compose up -d
```

Check status:
```bash
docker compose ps
docker compose logs -f
```

### Step 6: Verify

1. **Health check:** `curl https://api.dvaar.io/health`
2. **Admin panel:** Open `https://admin.dvaar.io` and enter your `ADMIN_TOKEN`
3. **Test login:** `dvaar login` from your local machine

---

## Adding Edge Nodes

**Time: ~5 minutes per node**

### Option 1: Quick Script (From Control Plane)

SSH to your control plane and run:
```bash
cd /opt/dvaar
./scripts/add-edge-node.sh NEW_SERVER_IP
```

### Option 2: Manual Setup

1. **Order server** (CAX21 for EU, CPX31 for US)

2. **SSH and run:**
```bash
export CONTROL_PLANE_IP=your_control_plane_ip
export CLUSTER_SECRET=your_cluster_secret
export BASE_DOMAIN=dvaar.io
export TUNNEL_DOMAIN=dvaar.app

curl -sSL https://raw.githubusercontent.com/YOUR_USER/dvaar/main/scripts/setup-server.sh | bash -s -- edge
```

3. **Update DNS:**
   - Add A record or update GeoDNS

4. **Add to CI/CD** (see below)

### GeoDNS Setup (Optional)

For automatic geo-routing, use Cloudflare Load Balancer or AWS Route53:

```
*.dvaar.app
  ├── EU users → edge-eu.dvaar.io (Hetzner Falkenstein)
  ├── US users → edge-us.dvaar.io (Hetzner Ashburn)
  └── Asia users → edge-asia.dvaar.io (Vultr Singapore)
```

---

## CI/CD Pipeline

### GitHub Actions Setup

1. **Create repository secrets** (Settings → Secrets → Actions):
   - `SSH_PRIVATE_KEY`: Your deployment SSH key
   - `CONTROL_PLANE_HOST`: Control plane IP

2. **Create repository variables** (Settings → Variables → Actions):
   - `EDGE_NODES`: JSON array of edge nodes
   ```json
   [
     {"name": "eu-1", "host": "1.2.3.4"},
     {"name": "us-1", "host": "5.6.7.8"}
   ]
   ```

3. **Create environment** (Settings → Environments):
   - Name: `production`
   - Add protection rules if needed

### Deployment Flow

```
Push to main → Build Docker image → Push to GHCR → Deploy to control plane → Deploy to edge nodes
```

### Manual Deploy

```bash
# From your local machine
git push origin main

# Or trigger manually
gh workflow run deploy.yml
```

### Adding New Edge Node to CI/CD

1. Update `EDGE_NODES` variable:
```json
[
  {"name": "eu-1", "host": "1.2.3.4"},
  {"name": "us-1", "host": "5.6.7.8"},
  {"name": "asia-1", "host": "9.10.11.12"}  // New node
]
```

2. Next deploy will automatically include the new node

---

## DNS Configuration

### Cloudflare (Recommended)

**dvaar.io (main brand):**
```
Type    Name    Content         Proxy   TTL
A       @       YOUR_IP         Yes     Auto
A       www     YOUR_IP         Yes     Auto
A       api     YOUR_IP         Yes     Auto
A       admin   YOUR_IP         Yes     Auto
A       dash    YOUR_IP         Yes     Auto
A       docs    YOUR_IP         Yes     Auto
```

**dvaar.app (tunnel URLs):**
```
Type    Name    Content         Proxy   TTL
A       @       YOUR_IP         Yes     Auto
A       *       YOUR_IP         No      Auto    ← IMPORTANT: DNS only for tunnels!
```

### Hetzner DNS

```bash
# Using hcloud CLI

# dvaar.io
hcloud dns zone create --name dvaar.io
hcloud dns record create --zone dvaar.io --type A --name @ --value YOUR_IP
hcloud dns record create --zone dvaar.io --type A --name api --value YOUR_IP
hcloud dns record create --zone dvaar.io --type A --name admin --value YOUR_IP
hcloud dns record create --zone dvaar.io --type A --name dash --value YOUR_IP
hcloud dns record create --zone dvaar.io --type A --name docs --value YOUR_IP

# dvaar.app
hcloud dns zone create --name dvaar.app
hcloud dns record create --zone dvaar.app --type A --name @ --value YOUR_IP
hcloud dns record create --zone dvaar.app --type A --name '*' --value YOUR_IP
```

### SSL Certificates

Caddy handles SSL automatically via Let's Encrypt. No configuration needed!

For high-traffic, consider Cloudflare SSL in "Full (strict)" mode.

---

## Monitoring & Admin

### Admin Dashboard

Access at: `https://admin.dvaar.io`

**Metrics available:**
- Active tunnels
- Total users
- DAU (Daily Active Users)
- MAU (Monthly Active Users)
- Bandwidth usage
- Node status
- Uptime

### Health Endpoints

```bash
# Basic health
curl https://api.dvaar.io/health

# Detailed health (with admin token)
curl -H "Authorization: Bearer YOUR_ADMIN_TOKEN" https://admin.dvaar.io/api/health

# Metrics
curl -H "Authorization: Bearer YOUR_ADMIN_TOKEN" https://admin.dvaar.io/api/metrics
```

### Logs

```bash
# All services
docker compose logs -f

# Specific service
docker compose logs -f dvaar

# Last 100 lines
docker compose logs --tail 100 dvaar
```

### Alerts (Optional)

Add to `/opt/dvaar/docker-compose.yml`:
```yaml
  healthcheck:
    image: curlimages/curl:latest
    command: |
      sh -c 'while true; do
        if ! curl -sf http://dvaar:8080/health; then
          curl -X POST "https://api.telegram.org/bot$TELEGRAM_BOT_TOKEN/sendMessage" \
            -d "chat_id=$TELEGRAM_CHAT_ID" \
            -d "text=Dvaar health check failed!"
        fi
        sleep 60
      done'
    environment:
      TELEGRAM_BOT_TOKEN: ${TELEGRAM_BOT_TOKEN}
      TELEGRAM_CHAT_ID: ${TELEGRAM_CHAT_ID}
```

---

## Multi-Provider Setup

### Adding Vultr Edge Node

```bash
# 1. Create server via Vultr API or console
# 2. SSH and setup
ssh root@VULTR_IP

# Install Docker
curl -fsSL https://get.docker.com | sh

# Create config
mkdir -p /opt/dvaar && cd /opt/dvaar

cat > .env << EOF
NODE_TYPE=edge
NODE_IP=VULTR_IP
BASE_DOMAIN=dvaar.io
TUNNEL_DOMAIN=dvaar.app
CLUSTER_SECRET=your_cluster_secret
REDIS_URL=redis://CONTROL_PLANE_IP:6379
DATABASE_URL=postgres://dvaar:password@CONTROL_PLANE_IP:5432/dvaar
GITHUB_REPO=your/repo
VERSION=latest
EOF

# Start (same docker-compose as other edge nodes)
docker compose up -d
```

### Adding DigitalOcean Edge Node

Same process as Vultr. Use their 1-click Docker droplet for faster setup.

### Adding AWS/GCP

For AWS, use EC2 t3.medium (~$30/mo) or Lightsail ($20/mo).
For GCP, use e2-medium (~$25/mo).

Same setup script works on any Linux server with Docker.

---

## Troubleshooting

### Container won't start
```bash
docker compose logs dvaar
# Check for database connection issues
```

### SSL certificate issues
```bash
docker compose logs caddy
# Delete cert data and restart
docker compose down
docker volume rm dvaar_caddy_data
docker compose up -d
```

### Tunnel not connecting
```bash
# Check if tunnel endpoint is accessible
curl -v https://api.dvaar.io/_dvaar/tunnel

# Check WebSocket upgrade
curl -i -N -H "Connection: Upgrade" -H "Upgrade: websocket" \
  https://api.dvaar.io/_dvaar/tunnel
```

### Redis connection issues (edge nodes)
```bash
# Ensure port 6379 is open on control plane
# On control plane:
ufw allow from EDGE_NODE_IP to any port 6379
```

### Database connection issues (edge nodes)
```bash
# Ensure port 5432 is open on control plane
# On control plane:
ufw allow from EDGE_NODE_IP to any port 5432

# Update pg_hba.conf for remote connections
echo "host all all EDGE_NODE_IP/32 md5" >> /var/lib/docker/volumes/dvaar_postgres_data/_data/pg_hba.conf
docker compose restart postgres
```

---

## Quick Reference

### Commands

```bash
# Start all services
docker compose up -d

# Stop all services
docker compose down

# View logs
docker compose logs -f

# Restart specific service
docker compose restart dvaar

# Update to latest version
docker compose pull && docker compose up -d

# Check resource usage
docker stats
```

### File Locations

```
/opt/dvaar/
├── .env                 # Configuration
├── docker-compose.yml   # Service definitions
├── Caddyfile           # Reverse proxy config
└── scripts/
    └── add-edge-node.sh
```

### Ports

| Port | Service | Description |
|------|---------|-------------|
| 80 | Caddy | HTTP (redirects to HTTPS) |
| 443 | Caddy | HTTPS |
| 8080 | Dvaar | Internal HTTP/WebSocket |
| 6000 | Dvaar | Node-to-node communication |
| 5432 | Postgres | Database (control plane only) |
| 6379 | Redis | Cache/routing (control plane only) |

---

## Cost Estimation

### Starting (Single Server)
- Hetzner AX42: €52/mo
- Domains: ~€20/year (2 domains)
- **Total: ~€54/mo**

### Growth (+ 2 Edge Nodes)
- Control plane: €52/mo
- 2x CAX21: €12/mo
- **Total: ~€64/mo**

### Scale (+ 5 Edge Nodes Global)
- Control plane AX52: €67/mo
- 2x CAX31 (EU): €22/mo
- 2x CPX31 (US): €28/mo
- 1x Vultr (Asia): $24/mo
- **Total: ~€140/mo**

At 1000 paying users ($5/mo avg): **$5000 revenue, €140 cost = 97% margin**
