#!/bin/bash
set -euo pipefail

# Setup Cloudflare Origin Certificates for Dvaar
# Run this ONCE on the control plane after generating certs in Cloudflare dashboard

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log() { echo -e "${GREEN}[+]${NC} $1"; }
warn() { echo -e "${YELLOW}[!]${NC} $1"; }
error() { echo -e "${RED}[x]${NC} $1"; exit 1; }
info() { echo -e "${CYAN}[i]${NC} $1"; }

CERTS_DIR="/opt/dvaar/certs"

echo ""
echo "========================================="
echo "Cloudflare Origin Certificate Setup"
echo "========================================="
echo ""

# Check if running on server
if [[ ! -d /opt/dvaar ]]; then
    error "Run this script on the control plane server (where /opt/dvaar exists)"
fi

# Create certs directory
mkdir -p "$CERTS_DIR"
chmod 700 "$CERTS_DIR"

# Check if certs already exist
if [[ -f "$CERTS_DIR/origin.pem" ]] && [[ -f "$CERTS_DIR/origin-key.pem" ]]; then
    warn "Origin certificates already exist at $CERTS_DIR"
    read -p "Overwrite? (y/N): " confirm
    if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
        log "Keeping existing certificates"
        exit 0
    fi
fi

echo ""
info "Follow these steps to generate your origin certificate:"
echo ""
echo "1. Go to Cloudflare Dashboard > your domain (dvaar.app)"
echo "2. Navigate to: SSL/TLS > Origin Server"
echo "3. Click 'Create Certificate'"
echo "4. Settings:"
echo "   - Private key type: RSA (2048)"
echo "   - Hostnames: *.dvaar.app, dvaar.app"
echo "   - Certificate validity: 15 years"
echo "5. Click 'Create'"
echo "6. Copy the CERTIFICATE (begins with -----BEGIN CERTIFICATE-----)"
echo ""

read -p "Press Enter when ready to paste the certificate..."
echo "Paste the certificate (end with Ctrl+D on a new line):"
cat > "$CERTS_DIR/origin.pem"
echo ""

log "Certificate saved"
echo ""
echo "Now paste the PRIVATE KEY (begins with -----BEGIN PRIVATE KEY-----):"
echo "Paste the private key (end with Ctrl+D on a new line):"
cat > "$CERTS_DIR/origin-key.pem"
echo ""

log "Private key saved"

# Set secure permissions
chmod 644 "$CERTS_DIR/origin.pem"
chmod 600 "$CERTS_DIR/origin-key.pem"

log "Permissions set (cert: 644, key: 600)"

# Restart Caddy to pick up new certs
log "Restarting Caddy..."
cd /opt/dvaar
docker compose restart caddy

echo ""
log "========================================="
log "Origin certificates installed!"
log ""
log "Next steps:"
log "1. Enable Cloudflare Proxy (orange cloud) for all DNS records:"
log "   - *.dvaar.app"
log "   - dvaar.app"
log ""
log "2. Set SSL mode to 'Full' or 'Full (strict)':"
log "   Cloudflare Dashboard > SSL/TLS > Overview"
log ""
log "3. For edge nodes, run add-edge-node.sh"
log "   (it will automatically copy these certs)"
log "========================================="
