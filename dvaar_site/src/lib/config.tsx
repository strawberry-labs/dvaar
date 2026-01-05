import { Icons } from "@/components/icons";
import {
  GlobeIcon,
  LockIcon,
  ZapIcon,
  TerminalIcon,
  CloudIcon,
  ShieldIcon,
} from "lucide-react";

export const BLUR_FADE_DELAY = 0.15;

export const siteConfig = {
  name: "Dvaar",
  description: "Expose your localhost to the internet. Fast, secure, and simple.",
  cta: "Get Started",
  url: process.env.NEXT_PUBLIC_APP_URL || "https://dvaar.io",
  keywords: [
    "Localhost Tunnel",
    "ngrok alternative",
    "Expose localhost",
    "WebSocket tunneling",
    "HTTPS tunnel",
    "Developer tools",
  ],
  links: {
    email: "support@dvaar.io",
    twitter: "https://twitter.com/dvaar_io",
    discord: "https://discord.gg/dvaar",
    github: "https://github.com/strawberry-labs/dvaar",
    instagram: "https://instagram.com/dvaar_io",
  },
  hero: {
    title: "Dvaar",
    description:
      "Expose your localhost to the internet in seconds. Get instant HTTPS URLs, custom subdomains, and WebSocket support with a single command.",
    cta: "Get Started",
    ctaDescription: "curl -sSL https://dvaar.io/install.sh | bash",
  },
  features: [
    {
      name: "Instant HTTPS",
      description:
        "Get a secure public URL for your local server in seconds. No configuration needed.",
      icon: <LockIcon className="h-6 w-6" />,
    },
    {
      name: "Custom Subdomains",
      description:
        "Choose your own subdomain like myapp.dvaar.app instead of random strings.",
      icon: <GlobeIcon className="h-6 w-6" />,
    },
    {
      name: "WebSocket Support",
      description:
        "Full duplex WebSocket communication for real-time applications.",
      icon: <ZapIcon className="h-6 w-6" />,
    },
    {
      name: "Simple CLI",
      description:
        "One command to expose any port. Background mode, logs, and session management built-in.",
      icon: <TerminalIcon className="h-6 w-6" />,
    },
    {
      name: "Multi-Region",
      description:
        "Edge nodes around the world for low latency access from anywhere.",
      icon: <CloudIcon className="h-6 w-6" />,
    },
    {
      name: "Request Inspection",
      description:
        "See all incoming requests in real-time. Debug webhooks and API calls instantly.",
      icon: <ShieldIcon className="h-6 w-6" />,
    },
  ],
  pricing: [
    {
      name: "Free",
      price: { monthly: "$0", yearly: "$0" },
      frequency: { monthly: "month", yearly: "year" },
      description: "Perfect for personal projects and testing.",
      features: [
        "5 tunnels per hour",
        "60 requests per minute",
        "Random subdomains",
        "Community support",
      ],
      cta: "Get Started",
    },
    {
      name: "Hobby",
      price: { monthly: "$5", yearly: "$50" },
      frequency: { monthly: "month", yearly: "year" },
      description: "For developers who need custom domains.",
      features: [
        "20 tunnels per hour",
        "600 requests per minute",
        "Custom subdomains",
        "Reserved subdomains",
        "CNAME support",
        "Email support",
      ],
      cta: "Upgrade via CLI",
      popular: true,
    },
    {
      name: "Pro",
      price: { monthly: "$15", yearly: "$150" },
      frequency: { monthly: "month", yearly: "year" },
      description: "For teams and production workloads.",
      features: [
        "100 tunnels per hour",
        "3000 requests per minute",
        "Custom subdomains",
        "Reserved subdomains",
        "CNAME support",
        "5 team members",
        "Priority support",
      ],
      cta: "Upgrade via CLI",
    },
  ],
  footer: {
    socialLinks: [
      {
        icon: <Icons.github className="h-5 w-5" />,
        url: "https://github.com/strawberry-labs/dvaar",
      },
      {
        icon: <Icons.twitter className="h-5 w-5" />,
        url: "https://twitter.com/dvaar_io",
      },
    ],
    links: [
      { text: "Pricing", url: "#pricing" },
      { text: "Documentation", url: "/docs" },
      { text: "Status", url: "https://status.dvaar.io" },
    ],
    bottomText: "Copyright 2026 Strawberry Labs LLC. All rights reserved.",
    brandText: "DVAAR",
  },

  testimonials: [
    {
      id: 1,
      text: "Dvaar has completely replaced ngrok for our team. The custom subdomains and simple CLI make sharing local dev environments a breeze.",
      name: "Sarah Chen",
      company: "DevStack Labs",
      image:
        "https://images.unsplash.com/photo-1494790108377-be9c29b29330?w=500&auto=format&fit=crop&q=60&ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxzZWFyY2h8NHx8cG9ydHJhaXR8ZW58MHx8MHx8fDA%3D",
    },
    {
      id: 2,
      text: "Testing webhooks from Stripe and GitHub has never been easier. Dvaar's request inspection saves hours of debugging time.",
      name: "Marcus Johnson",
      company: "PayFlow",
      image:
        "https://images.unsplash.com/photo-1500648767791-00dcc994a43e?w=500&auto=format&fit=crop&q=60&ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxzZWFyY2h8MTh8fHBvcnRyYWl0fGVufDB8fDB8fHww",
    },
    {
      id: 3,
      text: "The WebSocket support is rock solid. We use Dvaar daily for testing our real-time collaboration features.",
      name: "Emily Rodriguez",
      company: "CollabSpace",
      image:
        "https://images.unsplash.com/photo-1507003211169-0a1dd7228f2d?w=500&auto=format&fit=crop&q=60&ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxzZWFyY2h8MTJ8fHBvcnRyYWl0fGVufDB8fDB8fHww",
    },
    {
      id: 4,
      text: "Finally, a tunneling solution that just works. Install, run, done. No complex configuration needed.",
      name: "Alex Kim",
      company: "RapidDev",
      image:
        "https://images.unsplash.com/photo-1438761681033-6461ffad8d80?w=500&auto=format&fit=crop&q=60&ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxzZWFyY2h8Mjh8fHBvcnRyYWl0fGVufDB8fDB8fHww",
    },
    {
      id: 5,
      text: "We switched from Cloudflare Tunnel to Dvaar for development. The CLI is much simpler and the subdomain feature is perfect.",
      name: "James Wilson",
      company: "CloudNative Inc",
      image:
        "https://images.unsplash.com/photo-1472099645785-5658abf4ff4e?w=500&auto=format&fit=crop&q=60&ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxzZWFyY2h8MzJ8fHBvcnRyYWl0fGVufDB8fDB8fHww",
    },
    {
      id: 6,
      text: "Sharing my local app with clients for demos is now one command. Dvaar has become essential to my workflow.",
      name: "Priya Patel",
      company: "DesignForward",
      image:
        "https://images.unsplash.com/photo-1544005313-94ddf0286df2?w=500&auto=format&fit=crop&q=60&ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxzZWFyY2h8NDB8fHBvcnRyYWl0fGVufDB8fDB8fHww",
    },
    {
      id: 7,
      text: "The background mode and session management make Dvaar perfect for CI/CD testing environments.",
      name: "David Park",
      company: "AutoDeploy",
      image:
        "https://images.unsplash.com/photo-1506794778202-cad84cf45f1d?w=500&auto=format&fit=crop&q=60&ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxzZWFyY2h8NDR8fHBvcnRyYWl0fGVufDB8fDB8fHww",
    },
    {
      id: 8,
      text: "Multi-region support means our global team can all access dev servers with minimal latency. Game changer!",
      name: "Lisa Zhang",
      company: "GlobalTech",
      image:
        "https://images.unsplash.com/photo-1534528741775-53994a69daeb?w=500&auto=format&fit=crop&q=60&ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxzZWFyY2h8NTJ8fHBvcnRyYWl0fGVufDB8fDB8fHww",
    },
    {
      id: 9,
      text: "Built with Rust means Dvaar is blazing fast and uses minimal resources. Love the performance!",
      name: "Tom Anderson",
      company: "PerfLabs",
      image:
        "https://images.unsplash.com/photo-1507003211169-0a1dd7228f2d?w=500&auto=format&fit=crop&q=60&ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxzZWFyY2h8NTZ8fHBvcnRyYWl0fGVufDB8fDB8fHww",
    },
    {
      id: 10,
      text: "The self-hosting option is amazing. We run Dvaar on our own infrastructure for complete control.",
      name: "Maria Garcia",
      company: "SecureOps",
      image:
        "https://images.unsplash.com/photo-1531746020798-e6953c6e8e04?w=500&auto=format&fit=crop&q=60&ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxzZWFyY2h8NjR8fHBvcnRyYWl0fGVufDB8fDB8fHww",
    },
    {
      id: 11,
      text: "Free tier is generous enough for personal projects. Only upgraded when my startup needed custom domains.",
      name: "Kevin Lee",
      company: "IndieHacker",
      image:
        "https://images.unsplash.com/photo-1500648767791-00dcc994a43e?w=500&auto=format&fit=crop&q=60&ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxzZWFyY2h8NzB8fHBvcnRyYWl0fGVufDB8fDB8fHww",
    },
    {
      id: 12,
      text: "Dvaar's documentation is excellent. Had tunnels running in under a minute on my first try.",
      name: "Rachel Moore",
      company: "CodeAcademy",
      image:
        "https://images.unsplash.com/photo-1494790108377-be9c29b29330?w=500&auto=format&fit=crop&q=60&ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxzZWFyY2h8NzZ8fHBvcnRyYWl0fGVufDB8fDB8fHww",
    },
    {
      id: 13,
      text: "Basic auth support lets us share staging environments securely with clients. Very useful feature!",
      name: "Daniel Brown",
      company: "ClientFirst",
      image:
        "https://images.unsplash.com/photo-1506794778202-cad84cf45f1d?w=500&auto=format&fit=crop&q=60&ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxzZWFyY2h8ODJ8fHBvcnRyYWl0fGVufDB8fDB8fHww",
    },
    {
      id: 14,
      text: "Static file serving is brilliant. One command to share a build folder publicly - perfect for design reviews.",
      name: "Sophie Turner",
      company: "PixelPerfect",
      image:
        "https://images.unsplash.com/photo-1534528741775-53994a69daeb?w=500&auto=format&fit=crop&q=60&ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxzZWFyY2h8ODh8fHBvcnRyYWl0fGVufDB8fDB8fHww",
    },
    {
      id: 15,
      text: "Cross-platform support means everyone on our team can use it regardless of OS. macOS, Linux, Windows - all covered.",
      name: "Mike Chen",
      company: "CrossPlatform",
      image:
        "https://images.unsplash.com/photo-1507003211169-0a1dd7228f2d?w=500&auto=format&fit=crop&q=60&ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxzZWFyY2h8OTR8fHBvcnRyYWl0fGVufDB8fDB8fHww",
    },
  ],
};

export type SiteConfig = typeof siteConfig;
