import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { SHADER_CACHE, isolateAppData, removeSandbox } from "./sandbox";

const saved = { ...process.env };

afterEach(async () => {
  await removeSandbox();
  process.env = { ...saved };
});

describe("the E2E sandbox", () => {
  // Under software rendering, a sandbox's own empty shader cache cost every
  // spec about a second the first time a dialog was drawn (#102).
  it.runIf(process.platform === "linux")(
    "shares one Mesa shader cache, outside every sandbox, between sessions",
    async () => {
      const first = isolateAppData();
      expect(process.env.XDG_CACHE_HOME?.startsWith(first)).toBe(true);
      expect(process.env.MESA_SHADER_CACHE_DIR).toBe(SHADER_CACHE);
      await removeSandbox();

      const second = isolateAppData();
      expect(second).not.toBe(first);
      expect(process.env.MESA_SHADER_CACHE_DIR).toBe(SHADER_CACHE);
      for (const sandbox of [first, second]) {
        expect(path.relative(sandbox, SHADER_CACHE).startsWith("..")).toBe(true);
      }
      expect(SHADER_CACHE.split(path.sep)).toContain("node_modules");
    },
  );
});
