import { Icons } from "@/components/icons";
import {
  Drawer,
  DrawerContent,
  DrawerDescription,
  DrawerFooter,
  DrawerHeader,
  DrawerTitle,
  DrawerTrigger,
} from "@/components/ui/drawer";
import { siteConfig } from "@/lib/config";
import Link from "next/link";
import { IoMenuSharp } from "react-icons/io5";

const navLinks = [
  { href: "https://github.com/strawberry-labs/dvaar", label: "GitHub", external: true },
  { href: "/docs", label: "Docs", external: false },
  { href: "/blog", label: "Blog", external: false },
];

export function MobileDrawer() {
  return (
    <Drawer>
      <DrawerTrigger>
        <IoMenuSharp className="text-2xl" />
      </DrawerTrigger>
      <DrawerContent>
        <DrawerHeader className="px-6">
          <Link
            href="/"
            title="brand-logo"
            className="relative mr-6 flex items-center space-x-2"
          >
            <Icons.logo className="w-auto h-[40px]" />
            <DrawerTitle>{siteConfig.name}</DrawerTitle>
          </Link>
          <DrawerDescription>{siteConfig.description}</DrawerDescription>
        </DrawerHeader>
        <DrawerFooter className="flex flex-col gap-2">
          {navLinks.map((link) => (
            <Link
              key={link.label}
              href={link.href}
              target={link.external ? "_blank" : undefined}
              rel={link.external ? "noopener noreferrer" : undefined}
              className="w-full py-3 px-4 text-center text-foreground hover:bg-muted rounded-lg transition-colors cursor-pointer"
            >
              {link.label}
            </Link>
          ))}
        </DrawerFooter>
      </DrawerContent>
    </Drawer>
  );
}
