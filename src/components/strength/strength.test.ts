import fs from "node:fs";
import path from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { StrengthAnswer } from "./estimate";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetModules();
  vi.doUnmock("./spawn");
});

describe("the passphrase strength worker (#102)", () => {
  it("answers each request with zxcvbn's estimate, under the request's id", async () => {
    const scope = {
      onmessage: null as ((event: { data: unknown }) => void) | null,
      postMessage: vi.fn<(answer: StrengthAnswer) => void>(),
    };
    vi.stubGlobal("self", scope);
    await import("./worker");

    scope.onmessage?.({ data: { id: 7, value: "password" } });
    scope.onmessage?.({ data: { id: 8, value: "vivid otter ledger crane mosaic" } });

    expect(scope.postMessage.mock.calls).toEqual([
      [{ id: 7, estimate: { score: 0, suggestion: expect.stringMatching(/\S/) } }],
      [{ id: 8, estimate: { score: 4, suggestion: expect.any(String) } }],
    ]);
  });

  it("is the only place zxcvbn is built: the page's main thread never imports it", () => {
    const root = path.resolve(__dirname, "../..");
    const files = fs
      .readdirSync(root, { recursive: true, encoding: "utf8" })
      .filter((file) => /\.tsx?$/.test(file) && !/\.test\.tsx?$/.test(file))
      .map((file) => file.split(path.sep).join("/"));
    // Value and dynamic imports, not `import type`, which leaves nothing at
    // run time.
    const importing = (pattern: RegExp) =>
      files.filter((file) =>
        [
          ...fs
            .readFileSync(path.join(root, file), "utf8")
            .matchAll(/(?:^import (?!type\b)[^;]*?from |\bimport\()\s*"([^"]+)"/gm),
        ].some(([, from]) => pattern.test(from)),
      );

    expect(importing(/^@zxcvbn-ts\//)).toEqual(["components/strength/estimate.ts"]);
    expect(importing(/\/estimate$/).sort()).toEqual([
      "components/strength/worker.ts",
      "test/inProcessStrengthWorker.ts",
    ]);
  });

  it("answers null, once and for all, when the worker can't run, so the hint shows only its advice", async () => {
    let failWorker: (() => void) | undefined;
    const posted = vi.fn();
    vi.doMock("./spawn", () => ({
      spawnStrengthWorker: () => {
        const worker = {
          onerror: null as ((event: Partial<ErrorEvent>) => void) | null,
          postMessage: posted,
          terminate: vi.fn(),
        };
        failWorker = () => worker.onerror?.({ message: "blocked", preventDefault: () => {} });
        return worker;
      },
    }));
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const { estimateStrength } = await import("./client");

    const pending = estimateStrength("password");
    expect(posted).toHaveBeenCalledTimes(1);
    failWorker?.();

    await expect(pending).resolves.toBeNull();
    await expect(estimateStrength("password")).resolves.toBeNull();
    expect(posted).toHaveBeenCalledTimes(1);
    expect(error).toHaveBeenCalledTimes(1);
    error.mockRestore();
  });
});
