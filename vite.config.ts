import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "path";

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],

  // Configure as Multi-Page App (MPA) for spotlight window
  appType: 'mpa',

  // Multiple entry points for build
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        spotlight: resolve(__dirname, 'spotlight.html'),
      },
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  clearScreen: false,
  // Tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
