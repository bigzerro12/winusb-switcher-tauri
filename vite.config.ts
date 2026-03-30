import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "path";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@shared": resolve(__dirname, "src/shared"),
    },
  },
  // Tauri expects a fixed port
  server: {
    port: 5173,
    strictPort: true,
    watch: {
      // Tell Vite to ignore watching src-tauri
      ignored: ["**/src-tauri/**"],
    },
  },
  // Output to out/renderer to match tauri.conf.json frontendDist
  build: {
    outDir: "out/renderer",
    emptyOutDir: true,
    rollupOptions: {
      input: resolve(__dirname, "index.html"),
    },
  },
  // Prevent Vite from obscuring Rust errors
  clearScreen: false,
});