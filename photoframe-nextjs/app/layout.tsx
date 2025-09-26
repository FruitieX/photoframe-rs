import type { Metadata, Viewport } from "next";
import "./globals.css";
import { ReactNode } from "react";
import ClientProvider from "./client-provider";

export const metadata: Metadata = {
  title: {
    default: "photoframe-rs",
    template: "%s | photoframe-rs",
  },
  description:
    "Configure and monitor your e-ink photo frames powered by photoframe-rs.",
  applicationName: "photoframe-rs",
  appleWebApp: {
    capable: true,
    title: "photoframe-rs",
    statusBarStyle: "black-translucent",
  },
  manifest: "/manifest.webmanifest",
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  themeColor: "#1c2024",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" className="h-full">
      <body className="h-full">
        <ClientProvider>{children}</ClientProvider>
      </body>
    </html>
  );
}
