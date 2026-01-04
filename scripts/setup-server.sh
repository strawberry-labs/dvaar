#!/bin/bash
set -euo pipefail

# Dvaar Server Setup Script
# Usage: curl -sSL https://raw.githubusercontent.com/YOUR_REPO/dvaar/main/scripts/setup-server.sh | bash -s -- [control-plane|edge] [OPTIONS]

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() { echo -e "${GREEN}[+]${NC} $1"; }
warn() { echo -e "${YELLOW}[!]${NC} $1"; }
error() { echo -e "${RED}[x]${NC} $1"; exit 1; }

# Parse arguments
NODE_TYPE="${1:-control-plane}"
CONTROL_PLANE_IP="${CONTROL_PLANE_IP:-}"
BASE_DOMAIN="${BASE_DOMAIN:-dvaar.io}"
TUNNEL_DOMAIN="${TUNNEL_DOMAIN:-dvaar.app}"
GITHUB_REPO="${GITHUB_REPO:-}"

if [[ "$NODE_TYPE" != "control-plane" && "$NODE_TYPE" != "edge" ]]; then
    error "Usage: $0 [control-plane|edge]"
fi

log "Setting up Dvaar $NODE_TYPE node..."

# Detect public IP
PUBLIC_IP=$(curl -s https://ifconfig.me || curl -s https://api.ipify.org)
log "Detected public IP: $PUBLIC_IP"

# Update system
log "Updating system packages..."
apt-get update && apt-get upgrade -y

# Install dependencies
log "Installing dependencies..."
apt-get install -y \
    ca-certificates \
    curl \
    gnupg \
    lsb-release \
    ufw \
    fail2ban

# Install Docker
if ! command -v docker &> /dev/null; then
    log "Installing Docker..."
    curl -fsSL https://get.docker.com | sh
    systemctl enable docker
    systemctl start docker
fi

# Install Docker Compose plugin
if ! docker compose version &> /dev/null; then
    log "Installing Docker Compose..."
    apt-get install -y docker-compose-plugin
fi

# Configure firewall
log "Configuring firewall..."
ufw default deny incoming
ufw default allow outgoing
ufw allow ssh
ufw allow 80/tcp
ufw allow 443/tcp
ufw allow 6000/tcp  # Internal node-to-node
ufw --force enable

# Create directory structure
log "Creating directory structure..."
mkdir -p /opt/dvaar
cd /opt/dvaar

# Generate secrets if control plane
if [[ "$NODE_TYPE" == "control-plane" ]]; then
    POSTGRES_PASSWORD=$(openssl rand -base64 32 | tr -dc 'a-zA-Z0-9' | head -c 32)
    CLUSTER_SECRET=$(openssl rand -base64 32 | tr -dc 'a-zA-Z0-9' | head -c 32)
    ADMIN_TOKEN=$(openssl rand -base64 32 | tr -dc 'a-zA-Z0-9' | head -c 32)

    log "Generated secrets (SAVE THESE!):"
    echo "========================================="
    echo "POSTGRES_PASSWORD=$POSTGRES_PASSWORD"
    echo "CLUSTER_SECRET=$CLUSTER_SECRET"
    echo "ADMIN_TOKEN=$ADMIN_TOKEN"
    echo "========================================="

    # Create .env file
    cat > .env << EOF
# Dvaar Control Plane Configuration
NODE_TYPE=control-plane
NODE_IP=$PUBLIC_IP
BASE_DOMAIN=$BASE_DOMAIN
TUNNEL_DOMAIN=$TUNNEL_DOMAIN
PUBLIC_URL=https://api.$BASE_DOMAIN

# Database
POSTGRES_PASSWORD=$POSTGRES_PASSWORD

# Cluster
CLUSTER_SECRET=$CLUSTER_SECRET

# Admin
ADMIN_TOKEN=$ADMIN_TOKEN

# GitHub OAuth (configure these)
GITHUB_CLIENT_ID=
GITHUB_CLIENT_SECRET=

# Container Registry
GITHUB_REPO=$GITHUB_REPO
VERSION=latest
EOF

    # Download docker-compose and Caddyfile
    log "Downloading configuration files..."
    curl -sSL "https://raw.githubusercontent.com/${GITHUB_REPO:-dvaar/dvaar}/main/docker/docker-compose.yml" -o docker-compose.yml
    curl -sSL "https://raw.githubusercontent.com/${GITHUB_REPO:-dvaar/dvaar}/main/docker/Caddyfile" -o Caddyfile

else
    # Edge node setup
    if [[ -z "$CONTROL_PLANE_IP" ]]; then
        error "CONTROL_PLANE_IP is required for edge nodes"
    fi

    cat > .env << EOF
# Dvaar Edge Node Configuration
NODE_TYPE=edge
NODE_IP=$PUBLIC_IP
BASE_DOMAIN=$BASE_DOMAIN
TUNNEL_DOMAIN=$TUNNEL_DOMAIN
PUBLIC_URL=https://api.$BASE_DOMAIN

# Control Plane Connection
CONTROL_PLANE_IP=$CONTROL_PLANE_IP
CLUSTER_SECRET=${CLUSTER_SECRET:-CHANGE_ME}

# Redis (connect to control plane)
REDIS_URL=redis://$CONTROL_PLANE_IP:6379

# Database (connect to control plane)
DATABASE_URL=postgres://dvaar:CHANGE_ME@$CONTROL_PLANE_IP:5432/dvaar

# Container Registry
GITHUB_REPO=$GITHUB_REPO
VERSION=latest
EOF

    # Create edge-specific docker-compose
    cat > docker-compose.yml << 'EOF'
version: '3.8'

services:
  dvaar:
    image: ghcr.io/${GITHUB_REPO:-dvaar/dvaar}:${VERSION:-latest}
    container_name: dvaar-server
    restart: unless-stopped
    ports:
      - "8080:8080"
      - "6000:6000"
    environment:
      HOST: 0.0.0.0
      PORT: 8080
      INTERNAL_PORT: 6000
      BASE_DOMAIN: ${BASE_DOMAIN}
      TUNNEL_DOMAIN: ${TUNNEL_DOMAIN}
      PUBLIC_URL: ${PUBLIC_URL}
      NODE_IP: ${NODE_IP}
      CLUSTER_SECRET: ${CLUSTER_SECRET}
      DATABASE_URL: ${DATABASE_URL}
      REDIS_URL: ${REDIS_URL}
      ADMIN_TOKEN: ${ADMIN_TOKEN:-}
      RUST_LOG: info,dvaar_server=debug

  caddy:
    image: caddy:2-alpine
    container_name: dvaar-caddy
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    environment:
      BASE_DOMAIN: ${BASE_DOMAIN}
      TUNNEL_DOMAIN: ${TUNNEL_DOMAIN}
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy_data:/data
      - caddy_config:/config
    depends_on:
      - dvaar

volumes:
  caddy_data:
  caddy_config:
EOF

    curl -sSL "https://raw.githubusercontent.com/${GITHUB_REPO:-dvaar/dvaar}/main/docker/Caddyfile" -o Caddyfile
fi

# Create systemd service for auto-start
cat > /etc/systemd/system/dvaar.service << EOF
[Unit]
Description=Dvaar Tunnel Service
Requires=docker.service
After=docker.service

[Service]
Type=oneshot
RemainAfterExit=yes
WorkingDirectory=/opt/dvaar
ExecStart=/usr/bin/docker compose up -d
ExecStop=/usr/bin/docker compose down
TimeoutStartSec=0

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable dvaar

log "========================================="
log "Setup complete!"
log ""
log "Next steps:"
if [[ "$NODE_TYPE" == "control-plane" ]]; then
    log "1. Configure GitHub OAuth in /opt/dvaar/.env"
    log "2. Set up DNS records for $BASE_DOMAIN:"
    log "   - api.$BASE_DOMAIN -> $PUBLIC_IP"
    log "   - admin.$BASE_DOMAIN -> $PUBLIC_IP"
    log "   - dash.$BASE_DOMAIN -> $PUBLIC_IP"
    log "   (Note: $BASE_DOMAIN root should point to Vercel)"
    log "3. Set up DNS records for $TUNNEL_DOMAIN:"
    log "   - *.$TUNNEL_DOMAIN -> $PUBLIC_IP (DNS only, no proxy!)"
    log "4. Start services: cd /opt/dvaar && docker compose up -d"
    log "5. Access admin at: https://admin.$BASE_DOMAIN"
else
    log "1. Update CLUSTER_SECRET and DATABASE_URL password in /opt/dvaar/.env"
    log "2. Set up DNS: *.$TUNNEL_DOMAIN -> $PUBLIC_IP (or use GeoDNS)"
    log "3. Start services: cd /opt/dvaar && docker compose up -d"
fi
log "========================================="
