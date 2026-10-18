// @vitest-environment node
// 010 FR-012, research.md §6: the real vite.config.ts wires the notices plugin
// into both bundles, the main build (`plugins`) and the strength worker's
// (`worker.plugins`), so removing either one fails here instead of silently
// dropping the bundled-package check or the withdrawn-data check from it.
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { loadConfigFromFile } from "vite";

const root = resolve(__dirname, "../..");

async function loadConfig() {
  const loaded = await loadConfigFromFile(
    { command: "build", mode: "production" },
    resolve(root, "vite.config.ts"),
    root,
  );
  if (loaded === null) throw new Error("vite.config.ts was not found");
  return loaded.config;
}

function flatten(plugins: unknown): { name?: string }[] {
  return (Array.isArray(plugins) ? plugins.flat(Infinity) : [plugins]).filter(
    (p): p is { name?: string } => typeof p === "object" && p !== null,
  );
}

describe("010 research.md §6 - vite.config.ts wires the notices plugin into both bundles", () => {
  it("010 FR-012: plugins holds the main notices plugin", async () => {
    const config = await loadConfig();
    const names = flatten(config.plugins).map((p) => p.name);
    expect(names).toContain("hoplodex-notices");
  });

  it("010 FR-012: worker.plugins holds the worker notices plugin", async () => {
    const config = await loadConfig();
    const workerPlugins = config.worker?.plugins;
    expect(typeof workerPlugins).toBe("function");
    const names = flatten((workerPlugins as () => unknown)()).map((p) => p.name);
    expect(names).toContain("hoplodex-notices-worker");
  });
});
