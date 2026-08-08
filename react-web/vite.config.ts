import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Vite config for the orouter React rewrite.
// Dev proxy: API + SSE + i18n literals go to the Rust backend (default :20130).
// M3 will wire the real RUST_PORT; for M1 the proxy is a no-op until the
// backend exists, so the theme shell still runs standalone.
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      "/api": { target: process.env.RUST_BACKEND ?? "http://127.0.0.1:20130", changeOrigin: true },
      "/v1": { target: process.env.RUST_BACKEND ?? "http://127.0.0.1:20130", changeOrigin: true, ws: false },
      "/health": { target: process.env.RUST_BACKEND ?? "http://127.0.0.1:20130", changeOrigin: true },
      "/i18n": { target: process.env.RUST_BACKEND ?? "http://127.0.0.1:20130", changeOrigin: true },
    },
  },
  resolve: {
    alias: { "@": "/src" },
  },
});
