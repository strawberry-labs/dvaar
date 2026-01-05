# Dvaar

**Dvaar** (द्वार) — *"Gateway"* in Sanskrit

Expose your localhost to the internet. Fast, secure, and simple.

```bash
# Install
curl -sSL https://dvaar.io/install.sh | bash

# Expose port 3000
dvaar http 3000
# => https://quick-fox-847.dvaar.app
```

## Features

- **Instant HTTPS** — Get a public URL in seconds
- **Custom Subdomains** — `myapp.dvaar.app` instead of random strings
- **Custom Domains** — Use your own domain via CNAME
- **WebSocket Support** — Full duplex communication
- **Request Inspection** — See all requests in real-time
- **Background Mode** — Run tunnels as daemons
- **Multi-Region** — Edge nodes for low latency globally

## Installation

### macOS / Linux

```bash
curl -sSL https://dvaar.io/install.sh | bash
```

### Windows

```powershell
irm https://dvaar.io/install.ps1 | iex
```

### Homebrew (macOS)

```bash
brew install strawberry-labs/dvaar
```

## Quick Start

### 1. Login

```bash
dvaar login
```

Opens browser for GitHub OAuth. Token is saved to `~/.dvaar/config.yml`.

### 2. Start a Tunnel

```bash
# Expose a local port
dvaar http 3000

# Expose with custom subdomain (Hobby+ plan)
dvaar http 3000 --domain myapp

# Expose a local server
dvaar http localhost:8080

# Serve static files
dvaar http ./dist
```

### 3. Background Mode

```bash
# Run in background
dvaar http 3000 -d

# List active tunnels
dvaar ls

# View logs
dvaar logs <id>

# Stop a tunnel
dvaar stop <id>
```

## CLI Reference

```
dvaar <COMMAND>

Commands:
  login     Authenticate with Dvaar
  http      Create an HTTP tunnel
  ls        List active tunnels
  stop      Stop a tunnel
  logs      View tunnel logs
  usage     Show bandwidth usage
  upgrade   Upgrade your plan

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### `dvaar http`

```
dvaar http <TARGET> [OPTIONS]

Arguments:
  <TARGET>  Port, URL, or path to serve
            Examples: 3000, localhost:8080, ./dist

Options:
  -d, --domain <NAME>         Request specific subdomain
  --custom-domain <DOMAIN>    Use your own domain (requires CNAME setup)
  --host-header <HOST>        Override Host header sent to upstream
  --auth <USER:PASS>          Enable basic auth
  -d, --detach                Run in background
  --use-tls                   Connect to upstream via HTTPS
```

## Pricing

| Plan | Price | Subdomains | Tunnels | Bandwidth |
|------|-------|------------|---------|-----------|
| **Free** | $0/mo | Random only | 3 | 1 GB/mo |
| **Hobby** | $5/mo | Custom + CNAME | 5 | 100 GB/mo |
| **Pro** | $15/mo | Reserved + CNAME | 20 | 1 TB/mo |

**Subdomain Types:**
- **Random** (Free): `quick-fox-847.dvaar.app` — changes each session
- **Custom** (Hobby): `myapp.dvaar.app` — pick any available name
- **Reserved** (Pro): `myapp.dvaar.app` — locked to your account forever

## Custom Domain Setup

1. Add CNAME record:
   ```
   api.yoursite.com  CNAME  myapp.dvaar.app
   ```

2. Start tunnel with custom domain:
   ```bash
   dvaar http 3000 --domain myapp --custom-domain api.yoursite.com
   ```

3. Access via `https://api.yoursite.com`

## Architecture

```
┌──────────────┐     WebSocket      ┌──────────────┐
│   CLI        │◄──────────────────►│   Dvaar      │
│ (your laptop)│                    │   Server     │
└──────────────┘                    └──────────────┘
       │                                   │
       │ HTTP                              │ HTTPS
       ▼                                   ▼
┌──────────────┐                    ┌──────────────┐
│  localhost   │                    │   Public     │
│    :3000     │                    │   Internet   │
└──────────────┘                    └──────────────┘
```

**How it works:**
1. CLI connects to `api.dvaar.io` via WebSocket
2. Server assigns subdomain (e.g., `myapp.dvaar.app`)
3. Public requests to `myapp.dvaar.app` are forwarded through the tunnel
4. CLI proxies requests to your local server and returns responses

## Self-Hosting

Dvaar can be self-hosted on your own infrastructure.

```bash
# Clone the repo
git clone https://github.com/yourusername/dvaar.git
cd dvaar

# Copy environment template
cp .env.example .env

# Edit configuration
nano .env

# Start with Docker
cd docker
docker compose up -d
```

See [DEPLOYMENT.md](./DEPLOYMENT.md) for full production deployment guide.

## Development

### Prerequisites

- Rust 1.75+
- PostgreSQL 15+
- Redis 7+

### Setup

```bash
# Clone
git clone https://github.com/yourusername/dvaar.git
cd dvaar

# Setup environment
cp .env.example .env
# Edit .env with your database credentials

# Run migrations
cd dvaar_server
sqlx database create
sqlx migrate run

# Run server
cargo run -p dvaar_server

# In another terminal, run CLI
cargo run -p dvaar_cli -- http 3000
```

### Project Structure

```
dvaar/
├── dvaar_common/     # Shared protocol library
│   └── src/lib.rs    # MessagePack protocol types
├── dvaar_server/     # Edge server
│   ├── src/
│   │   ├── main.rs
│   │   ├── config.rs
│   │   ├── db.rs
│   │   ├── redis.rs
│   │   └── routes/
│   │       ├── auth.rs      # GitHub OAuth
│   │       ├── tunnel.rs    # WebSocket handler
│   │       ├── ingress.rs   # Public request handler
│   │       ├── proxy.rs     # Node-to-node proxy
│   │       └── admin.rs     # Admin dashboard
│   └── migrations/
├── dvaar_cli/        # CLI client
│   └── src/
│       ├── main.rs
│       ├── config.rs
│       ├── commands/
│       │   ├── login.rs
│       │   ├── http.rs
│       │   └── session.rs
│       └── tunnel/
│           └── client.rs
├── docker/
│   ├── Dockerfile
│   ├── docker-compose.yml
│   └── Caddyfile
├── scripts/
│   ├── setup-server.sh
│   └── add-edge-node.sh
└── .github/
    └── workflows/
        └── deploy.yml
```

## Domains

| Domain | Purpose |
|--------|---------|
| `dvaar.io` | Main website, API, admin |
| `dvaar.app` | Tunnel URLs (`*.dvaar.app`) |
| `dvaar.link` | Backup tunnel domain |
| `dvaar.to` | Reserved |
| `dvaar.dev` | Reserved |

## API

### Health Check

```bash
curl https://api.dvaar.io/health
```

### Get User

```bash
curl -H "Authorization: Bearer <token>" https://api.dvaar.io/api/user
```

### Get Usage

```bash
curl -H "Authorization: Bearer <token>" https://api.dvaar.io/api/usage
```

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing`)
5. Open a Pull Request

## License

MIT License. See [LICENSE](./LICENSE) for details.

## Acknowledgments

- Inspired by [ngrok](https://ngrok.com), [Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/), and [localtunnel](https://localtunnel.me)
- Built with [Rust](https://www.rust-lang.org/), [Axum](https://github.com/tokio-rs/axum), and [Tokio](https://tokio.rs/)

---

**Dvaar** — The gateway to your localhost.

[Website](https://dvaar.io) · [Documentation](https://dvaar.io/docs) · [Status](https://status.dvaar.io)
