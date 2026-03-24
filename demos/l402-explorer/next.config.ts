import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  transpilePackages: ["bolt402-ai-sdk"],
  serverExternalPackages: ["bolt402-wasm"],
};

export default nextConfig;
