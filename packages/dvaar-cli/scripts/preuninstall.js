#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const os = require("os");

// Clean up downloaded binary
const binDir = path.join(__dirname, "..", "bin");
const isWindows = os.platform() === "win32";
const binaryPath = path.join(binDir, isWindows ? "dvaar.exe" : "dvaar");

try {
  if (fs.existsSync(binaryPath)) {
    fs.unlinkSync(binaryPath);
    console.log("Cleaned up dvaar binary.");
  }
} catch (e) {
  // Ignore cleanup errors
}
