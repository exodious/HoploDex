/// <reference types="vitest/config" />
import { availableParallelism } from "node:os";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { hoplodexNotices } from "./scripts/licenses/vitePlugin.mjs";

const host = process.env.TAURI_DEV_HOST;

// Defines __HOPLODEX_PROGRAM__, checks the bundled packages, and writes
// dist/notices.json and dist/NOTICES.txt (010 contracts/notices.md). One
// collector serves the main build and the strength worker's.
const notices = hoplodexNotices();

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), notices.main],

  worker: { plugins: () => [notices.worker] },

  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    // #77 step 10: the suite is jsdom-bound, so wall time follows the worker
    // count. Vitest's default leaves one CPU idle (availableParallelism() - 1,
    // 3 workers on 4 CPUs); use them all. Threads start faster than forks,
    // which counts most on Windows. Each file still gets its own worker
    // (isolation stays on); no test touches process-wide state.
    pool: "threads",
    maxWorkers: availableParallelism(),
  },

  // Vite options tailored for Tauri development
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
