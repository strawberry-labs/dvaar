#!/bin/bash
set -euo pipefail

# Quick script to add a new edge node in under 5 minutes
# Usage: ./add-edge-node.sh <new-server-ip> [ssh-key-path]

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

log() { echo -e "${GREEN}[+]${NC} $1"; }
error() { echo -e "${RED}[x]${NC} $1"; exit 1; }

NEW_SERVER_IP="${1:-}"
SSH_KEY="${2:-~/.ssh/id_rsa}"

if [[ -z "$NEW_SERVER_IP" ]]; then
    error "Usage: $0 <new-server-ip> [ssh-key-path]"
fi

# Load control plane config
if [[ ! -f /opt/dvaar/.env ]]; then
    error "Run this from the control plane server"
fi

source /opt/dvaar/.env

log "Adding edge node: $NEW_SERVER_IP"

# Copy setup script and run on remote
log "Copying setup script to new server..."
ssh -i "$SSH_KEY" -o StrictHostKeyChecking=no "root@$NEW_SERVER_IP" "mkdir -p /opt/dvaar"

# Create remote .env with proper values
log "Configuring edge node..."
ssh -i "$SSH_KEY" "root@$NEW_SERVER_IP" bash << EOF
set -e

# Install Docker if not present
if ! command -v docker &> /dev/null; then
    curl -fsSL https://get.docker.com | sh
    systemctl enable docker
    systemctl start docker
fi

# Install Docker Compose
apt-get update && apt-get install -y docker-compose-plugin

# Create config
mkdir -p /opt/dvaar
cd /opt/dvaar

cat > .env << 'ENVEOF'
NODE_TYPE=edge
NODE_IP=$NEW_SERVER_IP
BASE_DOMAIN=$BASE_DOMAIN
PUBLIC_URL=$PUBLIC_URL
CLUSTER_SECRET=$CLUSTER_SECRET
REDIS_URL=redis://$NODE_IP:6379
DATABASE_URL=postgres://dvaar:$POSTGRES_PASSWORD@$NODE_IP:5432/dvaar
GITHUB_REPO=$GITHUB_REPO
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
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy_data:/data
    depends_on:
      - dvaar

volumes:
  caddy_data:
COMPOSEEOF

cat > Caddyfile << 'CADDYEOF'
*.${BASE_DOMAIN} {
    reverse_proxy dvaar:8080
}
CADDYEOF

# Configure firewall
ufw allow 80/tcp
ufw allow 443/tcp
ufw allow 6000/tcp
ufw --force enable

# Start services
docker compose pull
docker compose up -d

echo "Edge node started!"
EOF

log "Edge node $NEW_SERVER_IP is now running!"
log ""
log "Don't forget to:"
log "1. Add DNS record: Update your wildcard DNS or add GeoDNS"
log "2. Add to CI/CD: Update EDGE_NODES in GitHub repo variables"
log ""
log "Test with: curl -H 'Host: test.$BASE_DOMAIN' http://$NEW_SERVER_IP/health"
