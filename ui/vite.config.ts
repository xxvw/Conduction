import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri v2: dev server は固定ポート、strict port、clear screen off。
// @see https://tauri.app/v1/guides/getting-started/setup/vite
export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
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
