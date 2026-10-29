import type { NextConfig } from "next";
import path from "node:path";

const nextConfig: NextConfig = {
  transpilePackages: ["@lightninglabs/l402-ai"],
  turbopack: { root: path.resolve(__dirname, "../..") },
};

export default nextConfig;
