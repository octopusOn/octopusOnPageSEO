import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

import type { NextConfig } from "next";

import { localizedRewrites } from "./src/i18n/routes";

/** Each language's route names from its messages file, read when the portal is built. */
function routeNames(): [string, Record<string, string>][] {
  const directory = join(process.cwd(), "messages");
  return readdirSync(directory)
    .filter((name) => name.endsWith(".json"))
    .map((name) => [name.slice(0, -5), (JSON.parse(readFileSync(join(directory, name), "utf8")) as { routes: Record<string, string> }).routes]);
}

const nextConfig: NextConfig = {
  reactStrictMode: true,
  poweredByHeader: false,
  output: "standalone",
  // The shared server code is TypeScript in the workspace.
  transpilePackages: ["@octopusseo/cloud-core"],
  serverExternalPackages: ["pg"],
  // Addresses in each language to the page folders (src/i18n/routes.ts); the proxy has already
  // corrected the address.
  async rewrites() {
    return { beforeFiles: localizedRewrites(routeNames()), afterFiles: [], fallback: [] };
  },
  async headers() {
    return [
      {
        source: "/:path*",
        headers: [
          { key: "X-Content-Type-Options", value: "nosniff" },
          { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
          { key: "X-Frame-Options", value: "DENY" },
          { key: "Permissions-Policy", value: "camera=(), microphone=(), geolocation=(), payment=(self)" },
          { key: "Strict-Transport-Security", value: "max-age=63072000; includeSubDomains" },
        ],
      },
    ];
  },
};

export default nextConfig;
