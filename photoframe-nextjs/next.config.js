/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  // Export a static site for embedding into the Rust binary
  output: "export",
  distDir: "out",
  images: { unoptimized: true },
  // Development-only proxy for backend API running on localhost:8080
  // This keeps production static export unchanged.
  async rewrites() {
    if (process.env.NODE_ENV === "development") {
      return [
        {
          source: "/api/:path*",
          destination: "http://localhost:8080/api/:path*",
        },
      ];
    }
    return [];
  },
};
module.exports = nextConfig;
