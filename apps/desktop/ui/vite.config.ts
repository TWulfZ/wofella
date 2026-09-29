import { fileURLToPath, URL } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// Tauri's devUrl (tauri.conf.json) expects exactly this port, so Vite must not fall back to another one.
const DEV_PORT = 1420;

export default defineConfig({
  // The router plugin must run before react so the generated route tree exists when JSX is transformed.
  plugins: [tanstackRouter({ target: "react", autoCodeSplitting: true }), react(), tailwindcss()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  clearScreen: false,
  server: {
    port: DEV_PORT,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
    // Type-level contract tests (mocks.test-d.ts) must run in the plain `pnpm test` gate too, not only with --typecheck.
    typecheck: { enabled: true, include: ["src/**/*.test-d.ts"] },
  },
});
