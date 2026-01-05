import { FeatureSelector } from "@/components/feature-selector";
import { Section } from "@/components/section";
import { codeToHtml } from "shiki";

interface FeatureOption {
  id: number;
  title: string;
  description: string;
  code: string;
}

const featureOptions: FeatureOption[] = [
  {
    id: 1,
    title: "Quick Start",
    description: "Expose your local server to the internet with one command.",
    code: `# Install Dvaar (choose your method)
curl -sSL https://dvaar.io/install.sh | bash  # macOS/Linux
npm install -g dvaar                           # npm

# Login with GitHub (uses Device Flow)
dvaar login
# ! First, copy your one-time code: ABCD-1234
# Press Enter to open github.com/login/device

# Expose port 3000 to the internet
dvaar http 3000
# => https://quick-fox-847.dvaar.app

# Your local server is now accessible globally!`,
  },
  {
    id: 2,
    title: "Custom Subdomains",
    description: "Choose your own subdomain for memorable URLs.",
    code: `# Expose with a custom subdomain (Hobby+ plan)
dvaar http 3000 --domain myapp
# => https://myapp.dvaar.app

# Use your own domain via CNAME (Hobby+ plan)
# First, add CNAME record:
# api.example.com CNAME myapp.dvaar.app

# Then run with your custom subdomain
dvaar http 3000 --domain myapp
# Access via: https://api.example.com`,
  },
  {
    id: 3,
    title: "Background Mode",
    description: "Run tunnels as background daemons with full management.",
    code: `# Start tunnel in background (daemon mode)
dvaar http 3000 -d
# => Tunnel started in background (id: abc123)

# List all active tunnels
dvaar ls
# ID       PORT   URL                          STATUS
# abc123   3000   https://myapp.dvaar.app      active
# def456   8080   https://api.dvaar.app        active

# View real-time logs
dvaar logs abc123

# Stop a specific tunnel
dvaar stop abc123`,
  },
  {
    id: 4,
    title: "Advanced Options",
    description: "Fine-tune your tunnel with authentication and headers.",
    code: `# Enable basic authentication for security
dvaar http 3000 --auth username:password
# Visitors must enter credentials to access

# Serve static files directly
dvaar http ./dist
# => https://quick-fox-847.dvaar.app

# Override Host header for upstream
dvaar http 3000 --host-header myapp.local

# Connect to upstream via HTTPS
dvaar http 3000 --use-tls

# Check your bandwidth usage
dvaar usage
# Plan: Free | Used: 2.5 GB

# Upgrade your plan
dvaar upgrade
# Or: dvaar upgrade hobby`,
  },
];

export async function Examples() {
  const features = await Promise.all(
    featureOptions.map(async (feature) => ({
      ...feature,
      code: await codeToHtml(feature.code, {
        lang: "bash",
        theme: "github-dark",
      }),
    }))
  );

  return (
    <Section id="examples">
      <div className="border-x border-t">
        <FeatureSelector features={features} />
      </div>
    </Section>
  );
}
