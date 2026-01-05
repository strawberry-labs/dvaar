# Dvaar Deployment Scripts Explained

This document explains how the Dvaar deployment system works in simple terms, designed to be listened to via text-to-speech.

---

## Introduction

Dvaar is a tunneling service that lets developers expose their local servers to the internet. Think of it like ngrok, but self-hosted. The deployment system has four main scripts that work together. Let me walk you through each one so you understand exactly what happens when you deploy Dvaar.

---

## Part One: The Release Workflow

The release workflow lives in a file called release dot yml inside the github workflows folder. This script runs automatically whenever you push a git tag that starts with the letter v, like v1.0.0 or v2.3.1.

### What triggers it

When you run git tag v1.0.0 and then git push origin v1.0.0, GitHub detects this new tag and kicks off the release workflow. The v prefix is the signal that tells GitHub this is a version release.

### The build phase

The first thing the workflow does is build the Dvaar command line tool for five different platforms. It creates separate builds for Linux on Intel processors, Linux on ARM processors like the newer Macs or Raspberry Pis, macOS on Intel, macOS on Apple Silicon, and Windows.

For most platforms, it uses regular cargo build, which is Rust's build command. However, for Linux ARM, it uses a tool called cross, which is a special cross-compilation tool. Cross-compilation means building software on one type of computer that will run on a different type. So even though GitHub's Linux runners are Intel-based, cross lets us build ARM binaries.

Each build runs in parallel across different GitHub runner machines. The Linux builds run on Ubuntu, the macOS builds run on macOS, and the Windows build runs on Windows. This parallel execution means all five builds happen at the same time, making the whole process faster.

After each build completes, the script packages the binary. For Unix systems like Linux and macOS, it creates a tar.gz compressed archive. For Windows, it creates a zip file. These archives are then uploaded as build artifacts.

### The release phase

Once all five builds complete successfully, the release phase begins. This job downloads all the artifacts from the build phase, moves them into a single release folder, and generates SHA-256 checksums. Checksums are like digital fingerprints. Users can verify their download is authentic and uncorrupted by comparing the checksum.

Then it creates an actual GitHub release using a tool called action-gh-release. This creates the release page you see on GitHub with download links for all the binaries. It also automatically generates release notes based on the commits since the last release.

### Publishing to package managers

After the release is created, two more jobs run in parallel.

First, publish to npm. This publishes a Node.js wrapper package. It updates the version number in package.json to match the git tag, then runs npm publish. This lets users install Dvaar using npm install globally.

Second, update Homebrew. This updates the Homebrew formula so macOS users can run brew install strawberry-labs/dvaar. It checks out a separate repository called homebrew-dvaar, downloads the new release files, calculates their checksums, generates a new formula file with the updated version and checksums, then commits and pushes that change.

---

## Part Two: The Deploy Workflow

The deploy workflow lives in deploy dot yml. Unlike the release workflow which publishes the CLI, the deploy workflow handles the server-side infrastructure.

### When it runs

This workflow triggers on pushes to the main branch or when version tags are pushed. It also runs on pull requests to main, but for pull requests it only builds without deploying.

### Building the Docker image

The first job builds a Docker image for the Dvaar server. It uses Docker Buildx, which is an extended build tool that supports advanced features like build caching.

The workflow logs into GitHub Container Registry, which is GitHub's Docker image hosting service. Then it extracts metadata to generate image tags. For a push to main, it tags the image as main. For a version tag like v1.2.3, it creates multiple tags: the full version, just the major and minor version, and a short git commit hash.

Then it builds and pushes the image. The caching is important here. It uses GitHub Actions cache to store and retrieve build layers. This means if you only changed one small file, most of the build can be skipped because the layers are cached.

### Deploying to the control plane

After the build succeeds, and only if this is a push to main or a version tag, the workflow deploys to the control plane server.

It uses SSH to connect to the server. The server address comes from a secret called CONTROL_PLANE_HOST, and authentication uses an SSH private key stored in secrets.

Once connected, it runs three commands. First, docker compose pull downloads the newest image. Second, docker compose up -d starts or restarts the containers in detached mode. The remove-orphans flag cleans up any old containers that are no longer defined. Third, docker system prune cleans up unused images to save disk space.

### Deploying to edge nodes

The last job deploys to edge nodes, which are additional servers in different geographic regions. This uses a matrix strategy.

The edge nodes are defined in a repository variable called EDGE_NODES, stored as JSON. For each node in that list, it SSHs in and runs similar commands. The main difference is it only starts the dvaar and caddy services, not the database services, since edge nodes connect back to the control plane's database.

---

## Part Three: The Setup Server Script

The setup server script is called setup-server.sh. This is the script you run when setting up a brand new server for the first time.

### How to run it

You can run it in two modes: control-plane or edge. The control plane is your main server that runs the database, Redis, and the primary Dvaar server. Edge nodes are secondary servers that handle traffic in different regions.

### Initial system setup

When the script starts, it immediately enables strict error handling. This means if any command fails, the script stops instead of continuing with a broken state.

It detects the server's public IP address by calling external services like ifconfig.me. This IP is important because it gets written into configuration files.

Then it updates all system packages using apt-get. After that, it installs dependencies including CA certificates for HTTPS, curl for making web requests, GnuPG for cryptographic operations, ufw for the firewall, and fail2ban for protection against brute force attacks.

### Installing Docker

If Docker isn't already installed, the script downloads and runs Docker's official installation script from get.docker.com. This is the recommended way to install Docker on Linux. After installation, it enables Docker as a system service so it starts automatically on boot.

It also installs Docker Compose as a plugin, which lets you define multi-container applications in a single file.

### Configuring the firewall

The script sets up ufw, the Uncomplicated Firewall. It configures the default policy to deny all incoming traffic and allow all outgoing traffic. Then it explicitly allows SSH on port 22, HTTP on port 80, HTTPS on port 443, and an internal port 6000 for node-to-node communication. Finally it enables the firewall.

### Control plane specific setup

If you're setting up a control plane, the script generates three important secrets using openssl. The postgres password is for the database. The cluster secret is used for authentication between edge nodes and the control plane. The admin token lets you access admin endpoints.

These secrets are printed to the screen with a clear warning to save them. They're only generated once and if you lose them, you'll have trouble.

Then it creates a dot env file with all the configuration. This includes the node IP, domain names, database password, cluster secret, and placeholders for GitHub OAuth credentials that you fill in manually later.

It also downloads docker-compose.yml and Caddyfile from your GitHub repository. These define how the containers should run and how the web server should route traffic.

### Edge node specific setup

For edge nodes, the setup is simpler because they don't run their own database. Instead, they connect to the control plane's database and Redis.

The script creates a dot env file with connection strings pointing to the control plane IP. It also creates a simpler docker-compose file that only runs the Dvaar server and Caddy, without Postgres or Redis.

### Creating the systemd service

Finally, the script creates a systemd service file. Systemd is Linux's service manager. The service file tells Linux to start Dvaar automatically whenever the server boots. It depends on Docker being ready first, and it runs docker compose up to start the containers.

After creating the service, it reloads systemd to pick up the new configuration and enables the service.

### The final output

At the end, the script prints next steps specific to what you're setting up. For a control plane, it tells you to configure GitHub OAuth, set up DNS records for your domains, and start the services. For edge nodes, it reminds you to update the cluster secret and database password, then set up DNS.

---

## Part Four: The Add Edge Node Script

The add edge node script is called add-edge-node.sh. This is a convenience script that you run from your control plane server to quickly add a new edge node.

### Prerequisites

This script must be run from the control plane server because it reads the configuration from the control plane's dot env file. It needs the new server's IP address and optionally a path to your SSH key.

### How it works

First, it loads the control plane's configuration by sourcing the dot env file. This gives it access to variables like the cluster secret, database password, and domain names.

Then it SSHs into the new server. The script uses a here document, which is a way to send multiple commands over SSH. Inside this remote execution block, it installs Docker if needed, then creates the necessary configuration files.

The dot env file on the edge node gets populated with values from the control plane. The database URL points to the control plane's IP with the postgres password. The Redis URL also points to the control plane.

It creates a docker-compose file specific to edge nodes. This defines the Dvaar server container and Caddy container. The Dvaar container exposes ports 8080 for HTTP and 6000 for internal communication.

It creates a simple Caddyfile that proxies all requests for wildcard subdomains to the Dvaar server.

The script configures the firewall to allow HTTP, HTTPS, and the internal port 6000. Then it pulls the Docker images and starts the services.

### After completion

The script reminds you of two manual steps. First, update your DNS to point to this new server. You might use a wildcard record or GeoDNS to route users to the nearest server. Second, add this node to your CI/CD configuration by updating the EDGE_NODES variable in your GitHub repository settings.

It also provides a curl command to test that the new node is working correctly.

---

## Part Five: The Install Script

The install script is called install.sh. This is what end users run to install the Dvaar command line tool on their machines.

### Running the installer

Users run this script by piping curl output to bash. The script downloads from your website and executes locally. The cool ASCII art banner at the start gives users visual feedback that they're installing Dvaar.

### Platform detection

The script detects the operating system and CPU architecture. It uses uname to get this information. For the OS, it maps darwin to macOS, linux to Linux, and various Windows identifiers to Windows. For architecture, it maps x86_64 and amd64 to x64, and arm64 and aarch64 to arm64.

This produces a platform string like darwin-arm64 or linux-x64 that matches the filenames from the release workflow.

### Version detection

If the user didn't specify a version via environment variable, the script calls the GitHub API to find the latest release. It parses the JSON response to extract the tag name and strips the v prefix.

### Determining install location

The script tries to install to /usr/local/bin first since that's in most users' PATH. If that directory isn't writable, it falls back to ~/.local/bin in the user's home directory, creating it if necessary.

### Downloading and installing

It downloads the appropriate tar.gz file from GitHub releases, extracts it to a temporary directory, then moves the binary to the install directory. If the install directory requires root access, it uses sudo for the move and chmod commands.

After installation, it verifies by trying to run dvaar --version. If the command isn't found, it checks whether the install directory is in the user's PATH and provides instructions to add it if needed.

### Next steps

The script ends by printing helpful next steps. It tells users to run dvaar login to authenticate with GitHub, then dvaar http 3000 to expose a local port. It also links to the documentation.

---

## How Everything Works Together

Now let's tie it all together so you understand the full flow.

### Developer experience

When you're developing Dvaar and want to release a new version, you update the version number, commit your changes, create a git tag like v1.2.3, and push both the commits and the tag. This triggers the release workflow which builds binaries for all platforms, creates a GitHub release with download links, publishes to npm, and updates the Homebrew formula.

### Server deployment

When you push to main or push a version tag, the deploy workflow builds a new Docker image and pushes it to GitHub Container Registry. Then it SSHs into your control plane server and pulls the new image. It does the same for each edge node in your configuration.

### Setting up new infrastructure

When you want to add a new server, you first provision the server from your cloud provider. Then you SSH in and run the setup server script. If it's a control plane, you save the generated secrets and configure OAuth. If it's an edge node, you can use the add edge node script from your control plane for faster setup.

### User installation

End users install the CLI by running the install script. This downloads the right binary for their platform, installs it, and shows them how to get started. They can then create tunnels to expose their local development servers.

---

## Key Concepts Summarized

The control plane is your main server. It runs the database, Redis for caching, and the primary Dvaar service. All authentication and billing goes through here.

Edge nodes are additional servers that can be placed in different geographic regions. They don't have their own databases. Instead, they connect to the control plane for data but handle tunnel traffic locally. This reduces latency for users in different parts of the world.

GitHub Container Registry stores your Docker images. Each image is tagged with the version or branch name, making it easy to deploy specific versions or roll back if needed.

Caddy is the web server that sits in front of Dvaar. It automatically provisions SSL certificates from Let's Encrypt and handles HTTPS termination. It also routes requests for different subdomains to the right tunnels.

The cluster secret is a shared password between your control plane and edge nodes. It ensures that only your authorized servers can join the cluster.

---

## Troubleshooting Tips

If a build fails, check the GitHub Actions logs. Common issues include missing secrets, Docker build errors, or Rust compilation failures.

If deployment fails, SSH into the server and check the Docker logs with docker compose logs. Look for connection errors, missing environment variables, or port conflicts.

If SSL certificates don't work, check that your DNS is properly configured. Caddy needs to reach your domain to verify ownership. Also make sure Cloudflare proxy is disabled for the tunnel domain because WebSocket connections need direct access.

If edge nodes can't connect, verify the cluster secret matches between the control plane and edge node. Also check that ports 6000, 5432, and 6379 are accessible from the edge node to the control plane.

---

## Conclusion

That's the complete picture of how Dvaar's deployment system works. The release workflow handles building and publishing the CLI. The deploy workflow handles building and deploying the server infrastructure. The setup server script prepares new servers for first-time use. The add edge node script quickly adds capacity in new regions. And the install script gets the CLI onto user machines.

Each script is designed to be relatively self-contained and idempotent, meaning you can run them multiple times without breaking things. The system uses GitHub Actions for automation, Docker for containerization, and Caddy for web serving. Together, these tools create a reliable, automated pipeline from code to production.
