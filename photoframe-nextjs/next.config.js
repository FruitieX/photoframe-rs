/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  // Export a static site for embedding into the Rust binary
  output: "export",
  distDir: "out",
  images: { unoptimized: true },
};
module.exports = nextConfig;
