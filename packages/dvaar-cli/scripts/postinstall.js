#!/usr/bin/env node

const https = require("https");
const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");
const os = require("os");
const zlib = require("zlib");

const GITHUB_REPO = "strawberry-labs/dvaar";
const BINARY_NAME = "dvaar";

function getPlatform() {
  const platform = os.platform();
  const arch = os.arch();

  let osPart;
  switch (platform) {
    case "darwin":
      osPart = "darwin";
      break;
    case "linux":
      osPart = "linux";
      break;
    case "win32":
      osPart = "windows";
      break;
    default:
      throw new Error(`Unsupported platform: ${platform}`);
  }

  let archPart;
  switch (arch) {
    case "x64":
      archPart = "x64";
      break;
    case "arm64":
      archPart = "arm64";
      break;
    default:
      throw new Error(`Unsupported architecture: ${arch}`);
  }

  return `${osPart}-${archPart}`;
}

function getLatestVersion() {
  return new Promise((resolve, reject) => {
    const options = {
      hostname: "api.github.com",
      path: `/repos/${GITHUB_REPO}/releases/latest`,
      headers: { "User-Agent": "dvaar-cli-npm" },
    };

    https
      .get(options, (res) => {
        let data = "";
        res.on("data", (chunk) => (data += chunk));
        res.on("end", () => {
          try {
            const release = JSON.parse(data);
            resolve(release.tag_name.replace(/^v/, ""));
          } catch (e) {
            reject(e);
          }
        });
      })
      .on("error", reject);
  });
}

function downloadFile(url, destPath) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(destPath);

    const request = (url) => {
      https
        .get(url, { headers: { "User-Agent": "dvaar-cli-npm" } }, (res) => {
          if (res.statusCode === 302 || res.statusCode === 301) {
            request(res.headers.location);
            return;
          }
          if (res.statusCode !== 200) {
            reject(new Error(`Download failed: ${res.statusCode}`));
            return;
          }
          res.pipe(file);
          file.on("finish", () => {
            file.close();
            resolve();
          });
        })
        .on("error", (err) => {
          fs.unlink(destPath, () => {});
          reject(err);
        });
    };

    request(url);
  });
}

function extractTarGz(tarPath, destDir) {
  // Use tar command (available on macOS, Linux, and modern Windows)
  try {
    execSync(`tar -xzf "${tarPath}" -C "${destDir}"`, { stdio: "inherit" });
  } catch (e) {
    throw new Error(`Failed to extract: ${e.message}`);
  }
}

function extractZip(zipPath, destDir) {
  // Use unzip command or PowerShell on Windows
  const platform = os.platform();
  try {
    if (platform === "win32") {
      execSync(
        `powershell -command "Expand-Archive -Path '${zipPath}' -DestinationPath '${destDir}' -Force"`,
        { stdio: "inherit" }
      );
    } else {
      execSync(`unzip -o "${zipPath}" -d "${destDir}"`, { stdio: "inherit" });
    }
  } catch (e) {
    throw new Error(`Failed to extract: ${e.message}`);
  }
}

async function main() {
  console.log("Installing dvaar CLI...");

  const platform = getPlatform();
  const isWindows = os.platform() === "win32";
  const binDir = path.join(__dirname, "..", "bin");
  const binaryPath = path.join(binDir, isWindows ? "dvaar.exe" : "dvaar");

  // Create bin directory
  if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir, { recursive: true });
  }

  try {
    const version = await getLatestVersion();
    console.log(`Downloading dvaar v${version} for ${platform}...`);

    const ext = isWindows ? "zip" : "tar.gz";
    const downloadUrl = `https://github.com/${GITHUB_REPO}/releases/download/v${version}/dvaar-${platform}.${ext}`;
    const archivePath = path.join(binDir, `dvaar.${ext}`);

    await downloadFile(downloadUrl, archivePath);

    console.log("Extracting...");
    if (isWindows) {
      extractZip(archivePath, binDir);
    } else {
      extractTarGz(archivePath, binDir);
    }

    // Cleanup archive
    fs.unlinkSync(archivePath);

    // Make executable on Unix
    if (!isWindows) {
      fs.chmodSync(binaryPath, 0o755);
    }

    console.log(`dvaar v${version} installed successfully!`);
  } catch (e) {
    console.error(`Installation failed: ${e.message}`);
    console.error("");
    console.error("You can install manually:");
    console.error("  curl -sSL https://dvaar.io/install.sh | bash");
    process.exit(1);
  }
}

main();
