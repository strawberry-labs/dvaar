"use client";

import { Section } from "@/components/section";
import { cn } from "@/lib/utils";
import { motion } from "framer-motion";
import { CheckIcon, CopyIcon } from "lucide-react";
import { useState } from "react";

const ease = [0.16, 1, 0.3, 1];

const installCommands = [
  {
    id: "curl",
    label: "curl",
    command: "curl -sSL https://dvaar.io/install | bash",
  },
  {
    id: "npm",
    label: "npm",
    command: "npm install -g dvaar",
  },
  {
    id: "brew",
    label: "brew",
    command: "brew install dvaar-io/tap/dvaar",
  },
  {
    id: "cargo",
    label: "cargo",
    command: "cargo install dvaar",
  },
  {
    id: "windows",
    label: "windows",
    command: "irm https://dvaar.io/install.ps1 | iex",
  },
];

function InstallCommand() {
  const [activeTab, setActiveTab] = useState("curl");
  const [copied, setCopied] = useState(false);

  const activeCommand = installCommands.find((cmd) => cmd.id === activeTab);

  const copyToClipboard = async () => {
    if (activeCommand) {
      await navigator.clipboard.writeText(activeCommand.command);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <motion.div
      className="w-full max-w-3xl border border-border rounded-lg bg-[#141414]"
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: 0.4, duration: 0.8, ease }}
    >
      {/* Tabs */}
      <div className="flex border-b border-border">
        {installCommands.map((cmd) => (
          <button
            key={cmd.id}
            onClick={() => setActiveTab(cmd.id)}
            className={cn(
              "px-6 py-4 text-base font-medium transition-colors relative cursor-pointer",
              activeTab === cmd.id
                ? "text-foreground"
                : "text-muted-foreground hover:text-foreground"
            )}
          >
            {cmd.label}
            {activeTab === cmd.id && (
              <motion.div
                className="absolute bottom-0 left-0 right-0 h-px bg-foreground"
                layoutId="activeTab"
              />
            )}
          </button>
        ))}
      </div>

      {/* Command */}
      <div className="flex items-center justify-between p-6 font-mono text-lg">
        <code className="text-muted-foreground">
          {activeCommand?.command.split(" ")[0]}{" "}
          <span className="text-foreground font-semibold">
            {activeCommand?.command.split(" ").slice(1).join(" ")}
          </span>
        </code>
        <button
          onClick={copyToClipboard}
          className="ml-4 p-2 text-muted-foreground hover:text-foreground transition-colors cursor-pointer"
          aria-label="Copy to clipboard"
        >
          {copied ? (
            <CheckIcon className="h-5 w-5" />
          ) : (
            <CopyIcon className="h-5 w-5" />
          )}
        </button>
      </div>
    </motion.div>
  );
}

function HeroContent() {
  return (
    <div className="flex w-full max-w-3xl flex-col items-start">
      <motion.h1
        className="text-left text-4xl font-semibold leading-tight text-foreground sm:text-5xl md:text-6xl tracking-tight font-mono"
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.8, ease }}
      >
        Expose your localhost to the internet
      </motion.h1>
      <motion.p
        className="mt-6 text-left max-w-2xl leading-relaxed text-muted-foreground text-lg sm:text-xl"
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.2, duration: 0.8, ease }}
      >
        Instant HTTPS URLs for your local server. Custom subdomains,
        WebSocket support, and request inspection built-in.
      </motion.p>
    </div>
  );
}

export function Hero() {
  return (
    <Section id="hero">
      <div className="flex flex-col items-start justify-center w-full p-6 lg:p-12 border-x min-h-[60vh]">
        <HeroContent />
        <div className="mt-10 w-full">
          <InstallCommand />
        </div>
      </div>
    </Section>
  );
}
