"use client";

import { MobileDrawer } from "@/components/mobile-drawer";
import { siteConfig } from "@/lib/config";
import Image from "next/image";
import Link from "next/link";

const navLinks = [
  { href: "https://github.com/strawberry-labs/dvaar", label: "GitHub", external: true },
  { href: "/docs", label: "Docs", external: false },
  { href: "/blog", label: "Blog", external: false },
];

export function Header() {
  return (
    <header className="sticky top-0 z-50 bg-background/60 backdrop-blur">
      <div className="mx-auto container max-w-[var(--container-max-width)]">
        <div className="flex justify-between items-center px-6 lg:px-12 py-3 border-x">
          <Link
            href="/"
            title="brand-logo"
            className="flex items-center space-x-2"
          >
            <Image src="/dvaar-logo.svg" alt="Dvaar Logo" width={32} height={32} className="w-auto h-8" />
            <span className="font-semibold text-lg">{siteConfig.name}</span>
          </Link>

          {/* Desktop Navigation */}
          <nav className="hidden lg:flex items-center space-x-10">
            {navLinks.map((link) => (
              <Link
                key={link.label}
                href={link.href}
                target={link.external ? "_blank" : undefined}
                rel={link.external ? "noopener noreferrer" : undefined}
                className="text-xl font-medium text-foreground/70 hover:text-foreground transition-colors cursor-pointer"
              >
                {link.label}
              </Link>
            ))}
          </nav>

          {/* Mobile Navigation */}
          <div className="cursor-pointer block lg:hidden">
            <MobileDrawer />
          </div>
        </div>
        <hr className="border-x border-border" />
      </div>
    </header>
  );
}
