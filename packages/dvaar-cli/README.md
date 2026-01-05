# dvaar

Fast, secure tunneling for developers. Drop-in replacement for ngrok.

[![npm version](https://badge.fury.io/js/dvaar.svg)](https://www.npmjs.com/package/dvaar)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Installation

```bash
npm install -g dvaar
```

Or use directly with npx:

```bash
npx dvaar http 3000
```

## Quick Start

```bash
# 1. Login with GitHub (uses Device Flow)
dvaar login

# 2. Expose your local server
dvaar http 3000

# That's it! Your local server is now accessible globally.
```

Example output:

```
┌   dvaar
│
◇  Connected to server
│
◇  Tunnel Active
│  Public URL: https://quick-fox-847.dvaar.app
│  Forwarding: http://localhost:3000
│
◆  Waiting for requests... (Ctrl+C to stop)

  14:23:15     GET /api/users              200    42ms     1.2KB
  14:23:16    POST /api/login              200    89ms     256B
  14:23:18     GET /dashboard              200    12ms    45.3KB
```

## Commands

### `dvaar login [TOKEN]`

Authenticate with Dvaar using GitHub OAuth.

```bash
# Interactive login (opens GitHub Device Flow)
dvaar login

# Or provide a token directly
dvaar login <your-token>
```

Example login flow:

```
┌   dvaar login
│
◇  Connected to GitHub
│
◇  One-Time Code
│  Copy this code: ABCD-1234
│
│  Then paste it at: https://github.com/login/device
│
◇  Open GitHub in your browser?
│  Yes / No
│
◇  Waiting for authentication...
│
◇  Authenticated with GitHub
│
◇  Logged in as you@example.com
│
└  You're all set! Run `dvaar http <port>` to create a tunnel.
```

### `dvaar http <TARGET> [OPTIONS]`

Create an HTTP tunnel to expose your local server.

```bash
# Expose a port
dvaar http 3000

# Expose with custom subdomain (Hobby+ plan)
dvaar http 3000 --domain myapp
# => https://myapp.dvaar.app

# Serve static files
dvaar http ./dist

# Run in background (daemon mode)
dvaar http 3000 -d

# Enable basic authentication
dvaar http 3000 --auth user:password

# Override Host header
dvaar http 3000 --host-header myapp.local

# Use HTTPS for upstream
dvaar http 3000 --use-tls
```

**Options:**
- `-d, --detach` - Run in background (daemon mode)
- `--domain <subdomain>` - Request a specific subdomain
- `--auth <user:pass>` - Enable basic authentication
- `--host-header <host>` - Override Host header sent to upstream
- `--use-tls` - Use HTTPS for upstream connection

### `dvaar ls`

List all active tunnels.

```bash
dvaar ls
# ID       PORT   URL                          STATUS
# abc123   3000   https://myapp.dvaar.app      active
# def456   8080   https://api.dvaar.app        active
```

### `dvaar stop <ID>`

Stop a running tunnel.

```bash
dvaar stop abc123
```

### `dvaar logs <ID> [-f]`

View tunnel logs.

```bash
# View logs
dvaar logs abc123

# Follow logs in real-time
dvaar logs abc123 -f
```

### `dvaar usage`

Check your bandwidth usage and current plan.

```bash
dvaar usage
```

```
┌   dvaar usage
│
◇  Usage data retrieved
│
◇  Plan: Free
◇  Bandwidth Used: 2.50 GB
◇  Bandwidth Limit: unlimited
│
└  Done
```

### `dvaar upgrade [PLAN]`

Upgrade your plan to Hobby or Pro.

```bash
# Interactive plan selection
dvaar upgrade

# Or specify plan directly
dvaar upgrade hobby   # $5/month
dvaar upgrade pro     # $15/month
```

Running `dvaar upgrade` shows an interactive selector:

```
┌   dvaar upgrade
│
◇  Pricing
│  ┌─────────────┬─────────────┬─────────────┬─────────────┐
│  │   Feature   │    Free     │   Hobby     │     Pro     │
│  ├─────────────┼─────────────┼─────────────┼─────────────┤
│  │ Price       │     $0      │   $5/mo     │   $15/mo    │
│  ├─────────────┼─────────────┼─────────────┼─────────────┤
│  │ Tunnels/hr  │      5      │     20      │    100      │
│  │ Requests/m  │     60      │    600      │   3000      │
│  ├─────────────┼─────────────┼─────────────┼─────────────┤
│  │ Custom sub  │      ✗      │     ✓       │     ✓       │
│  │ Reserved    │      ✗      │     ✓       │     ✓       │
│  └─────────────┴─────────────┴─────────────┴─────────────┘
│
◇  Full details: https://dvaar.io/#pricing
│
◆  Select a plan to upgrade
│  ● Hobby - $5/month (20 tunnels/hr, 600 req/min, custom subdomains)
│  ○ Pro - $15/month (100 tunnels/hr, 3000 req/min, 5 team seats)
│
◇  Checkout session created
│
◇  Checkout URL
│  https://checkout.stripe.com/pay/cs_xxx
│
└  Complete payment in your browser to activate your plan
```

Use arrow keys to navigate, Enter to select. Opens Stripe checkout in your browser.

## Alternative Installation Methods

### curl (macOS/Linux)

```bash
curl -sSL https://dvaar.io/install.sh | bash
```

### Homebrew (macOS)

```bash
brew install strawberry-labs/dvaar
```

### Windows (PowerShell)

```powershell
irm https://dvaar.io/install.ps1 | iex
```

## How It Works

Dvaar creates a secure WebSocket tunnel between your local machine and our edge servers. When traffic hits your public URL (e.g., `https://myapp.dvaar.app`), it's forwarded through the tunnel to your local server.

```
Internet → dvaar.app → WebSocket Tunnel → localhost:3000
```

## Beautiful Request Logging

Dvaar shows real-time, color-coded request logs in your terminal:

```
  14:23:15     GET /api/users              200    42ms     1.2KB
  14:23:16    POST /api/login              200    89ms     256B
  14:23:17     PUT /api/profile            200    65ms     512B
  14:23:18  DELETE /api/session            204    23ms       0B
  14:23:19     GET /dashboard              200    12ms    45.3KB
  14:23:20     GET /api/data               500   234ms     128B
```

- **Green** status codes for 2xx success
- **Cyan** status codes for 3xx redirects
- **Yellow** status codes for 4xx client errors
- **Red** status codes for 5xx server errors
- Color-coded response times (green < 100ms, yellow > 500ms, red > 1s)

## Requirements

- Node.js 16+ (for npm installation)
- macOS, Linux, or Windows

## Documentation

Full documentation at [dvaar.io/docs](https://dvaar.io/docs)

## Support

- Documentation: [dvaar.io/docs](https://dvaar.io/docs)
- Issues: [github.com/strawberry-labs/dvaar/issues](https://github.com/strawberry-labs/dvaar/issues)
- Twitter: [@dvaar_io](https://twitter.com/dvaar_io)
- Discord: [discord.gg/dvaar](https://discord.gg/dvaar)

## License

MIT - see [LICENSE](https://github.com/strawberry-labs/dvaar/blob/main/LICENSE)
