import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Vue rewrite of the ORouter dashboard. API + SSE go to the Rust backend.
export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  server: {
    port: 5174,
    proxy: {
      "/api": { target: process.env.RUST_BACKEND ?? "http://127.0.0.1:20130", changeOrigin: true },
      "/v1": { target: process.env.RUST_BACKEND ?? "http://127.0.0.1:20130", changeOrigin: true },
      "/health": { target: process.env.RUST_BACKEND ?? "http://127.0.0.1:20130", changeOrigin: true },
      "/providers": { target: process.env.RUST_BACKEND ?? "http://127.0.0.1:20130", changeOrigin: true },
    },
  },
});
