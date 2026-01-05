#!/usr/bin/env node

const { spawn } = require("child_process");
const path = require("path");
const os = require("os");

const isWindows = os.platform() === "win32";
const binaryName = isWindows ? "dvaar.exe" : "dvaar";
const binaryPath = path.join(__dirname, binaryName);

const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: true,
});

child.on("error", (err) => {
  if (err.code === "ENOENT") {
    console.error("dvaar binary not found. Try reinstalling:");
    console.error("  npm uninstall -g dvaar && npm install -g dvaar");
    process.exit(1);
  }
  throw err;
});

child.on("exit", (code) => {
  process.exit(code || 0);
});
