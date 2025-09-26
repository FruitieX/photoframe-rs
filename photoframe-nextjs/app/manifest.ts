import type { MetadataRoute } from "next";

export const dynamic = "force-static";

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: "photoframe-rs",
    short_name: "photoframe",
    description:
      "Configure and monitor your e-ink photo frames powered by photoframe-rs.",
    start_url: "/",
    display: "standalone",
    background_color: "#1c2024",
    theme_color: "#1c2024",
    icons: [
      {
        src: "/android-chrome-192x192.png",
        sizes: "192x192",
        type: "image/png",
        purpose: "maskable",
      },
      {
        src: "/android-chrome-512x512.png",
        sizes: "512x512",
        type: "image/png",
        purpose: "maskable",
      },
    ],
    categories: ["photos", "utilities", "productivity"],
    shortcuts: [
      {
        name: "Frames",
        short_name: "Frames",
        url: "/frames",
      },
      {
        name: "Sources",
        short_name: "Sources",
        url: "/sources",
      },
    ],
  };
}
