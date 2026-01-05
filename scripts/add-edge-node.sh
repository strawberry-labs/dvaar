#!/bin/bash
set -euo pipefail

# Quick script to add a new edge node in under 5 minutes
# Usage: ./add-edge-node.sh <new-server-ip> [ssh-key-path]
# Run this from the CONTROL PLANE server

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() { echo -e "${GREEN}[+]${NC} $1"; }
warn() { echo -e "${YELLOW}[!]${NC} $1"; }
error() { echo -e "${RED}[x]${NC} $1"; exit 1; }

NEW_SERVER_IP="${1:-}"
SSH_KEY="${2:-~/.ssh/dvaar_deploy}"

if [[ -z "$NEW_SERVER_IP" ]]; then
    error "Usage: $0 <new-server-ip> [ssh-key-path]"
fi

# Load control plane config
if [[ ! -f /opt/dvaar/.env ]]; then
    error "Run this from the control plane server"
fi

source /opt/dvaar/.env

# Save control plane IP before it gets overwritten
CONTROL_PLANE_IP="$NODE_IP"

log "Adding edge node: $NEW_SERVER_IP"
log "Control plane: $CONTROL_PLANE_IP"

# Verify SSH connection first
log "Verifying SSH connection..."
if ! ssh -i "$SSH_KEY" -o ConnectTimeout=10 -o BatchMode=yes "root@$NEW_SERVER_IP" "echo 'SSH OK'" &>/dev/null; then
    error "Cannot connect to $NEW_SERVER_IP. Check SSH key and ensure server is accessible."
fi

# Copy setup script and run on remote
log "Configuring edge node..."
ssh -i "$SSH_KEY" "root@$NEW_SERVER_IP" bash << EOF
set -e

echo "[+] Installing dependencies..."

# Install Docker if not present
if ! command -v docker &> /dev/null; then
    echo "[+] Installing Docker..."
    curl -fsSL https://get.docker.com | sh
    systemctl enable docker
    systemctl start docker
fi

# Install Docker Compose
apt-get update -qq && apt-get install -y -qq docker-compose-plugin ufw

# Create config
mkdir -p /opt/dvaar
cd /opt/dvaar

echo "[+] Creating configuration..."

cat > .env << ENVEOF
NODE_TYPE=edge
NODE_IP=${NEW_SERVER_IP}
BASE_DOMAIN=${BASE_DOMAIN}
TUNNEL_DOMAIN=${TUNNEL_DOMAIN}
PUBLIC_URL=${PUBLIC_URL}
CLUSTER_SECRET=${CLUSTER_SECRET}
REDIS_URL=redis://${CONTROL_PLANE_IP}:6379
DATABASE_URL=postgres://dvaar:${POSTGRES_PASSWORD}@${CONTROL_PLANE_IP}:5432/dvaar
GITHUB_REPO=${GITHUB_REPO}
VERSION=${VERSION:-latest}
ENVEOF

# Create docker-compose for edge
cat > docker-compose.yml << 'COMPOSEEOF'
version: '3.8'

services:
  dvaar:
    image: ghcr.io/\${GITHUB_REPO}:\${VERSION:-latest}
    container_name: dvaar-server
    restart: unless-stopped
    ports:
      - "8080:8080"
      - "6000:6000"
    env_file: .env
    environment:
      HOST: 0.0.0.0
      PORT: 8080
      INTERNAL_PORT: 6000
      RUST_LOG: info,dvaar_server=debug

  caddy:
    image: caddy:2-alpine
    container_name: dvaar-caddy
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    environment:
      TUNNEL_DOMAIN: \${TUNNEL_DOMAIN}
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy_data:/data
    depends_on:
      - dvaar

volumes:
  caddy_data:
COMPOSEEOF

cat > Caddyfile << 'CADDYEOF'
*.{$TUNNEL_DOMAIN} {
    reverse_proxy dvaar:8080
}

{$TUNNEL_DOMAIN} {
    redir https://dvaar.io{uri} permanent
}
CADDYEOF

echo "[+] Configuring firewall..."
ufw --force reset
ufw default deny incoming
ufw default allow outgoing
ufw allow ssh
ufw allow 80/tcp
ufw allow 443/tcp
ufw allow 6000/tcp
ufw --force enable

echo "[+] Starting services..."
docker compose pull
docker compose up -d

echo "[+] Edge node started!"
EOF

log "========================================="
log "Edge node $NEW_SERVER_IP is now running!"
log ""
log "Next steps:"
log "1. Add DNS record: *.$TUNNEL_DOMAIN -> $NEW_SERVER_IP"
log "   (or configure GeoDNS for geographic routing)"
log "2. Add to CI/CD: Update EDGE_NODES in GitHub repo variables"
log "   Example: [{\"name\": \"edge-1\", \"host\": \"$NEW_SERVER_IP\"}]"
log ""
log "Test with:"
log "  curl -I https://test.$TUNNEL_DOMAIN"
log "  curl -H 'Host: test.$TUNNEL_DOMAIN' http://$NEW_SERVER_IP/health"
log "========================================="
