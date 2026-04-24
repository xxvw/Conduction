import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Tauri v2: dev server は固定ポート、strict port、clear screen off。
// @see https://v2.tauri.app/start/frontend/vite/
const __dirname = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
  resolve: {
    alias: {
      "@": resolve(__dirname, "src"),
      "@design": resolve(__dirname, "design-system"),
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: false,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "es2022",
    sourcemap: !!process.env.TAURI_DEBUG,
    minify: process.env.TAURI_DEBUG ? false : "esbuild",
  },
}));
