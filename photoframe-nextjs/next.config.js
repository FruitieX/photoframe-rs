/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  // Export a static site for embedding into the Rust binary
  output: "export",
  distDir: "out",
  images: { unoptimized: true },
  // Development-only proxy for backend API running on localhost:8080.
  // Static exports do not support rewrites, so only define them in dev.
  ...(process.env.NODE_ENV === "development" && {
    async rewrites() {
      return [
        {
          source: "/api/:path*",
          destination: "http://localhost:8080/api/:path*",
        },
      ];
    },
  }),
};
module.exports = nextConfig;
