import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Vite config for the ORouter React rewrite. API + SSE go to Rust; i18n
// literals are copied into public/ by scripts/sync-public.mjs.
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      "/api": { target: process.env.RUST_BACKEND ?? "http://127.0.0.1:20130", changeOrigin: true },
      "/v1": { target: process.env.RUST_BACKEND ?? "http://127.0.0.1:20130", changeOrigin: true, ws: false },
      "/health": { target: process.env.RUST_BACKEND ?? "http://127.0.0.1:20130", changeOrigin: true },
    },
  },
  resolve: {
    alias: { "@": "/src" },
  },
});
