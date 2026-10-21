/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed dev-server port and serves the build from `dist`. In a browser the dev
// server passes `/api` and `/auth` to a local platform server (octopuson-platform-server); without
// one the interface runs on sample data.
declare const process: { env: Record<string, string | undefined> };
const host = process.env.TAURI_DEV_HOST;
const server = process.env.PLATFORM_SERVER ?? "http://127.0.0.1:8796";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1470,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1471 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
    proxy: {
      "/api": { target: server, changeOrigin: false },
      "/auth": { target: server, changeOrigin: false },
    },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    target: "es2022",
    sourcemap: false,
    chunkSizeWarningLimit: 1500,
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
  },
});
