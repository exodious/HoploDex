import fs from "node:fs";
import path from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createEstimator, strengthDictionary } from "./estimate";
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

describe("the strength hint's word lists (010 research.md §2)", () => {
  it("has no list from the English Wikipedia, which can't ship (010 US3-3, 010 FR-015)", () => {
    expect(Object.keys(strengthDictionary()).filter((key) => /wikipedia/i.test(key))).toEqual([]);
  });

  it("has every other list the package's entry modules gave it, so none disappears silently (010 FR-015)", () => {
    // `@zxcvbn-ts/language-en` and `-common` 4.1.3's `dist/index.mjs` keys,
    // less `wikipedia-en`.
    expect(Object.keys(strengthDictionary()).sort()).toEqual(
      [
        "commonWords-en",
        "firstnames-en",
        "lastnames-en",
        "cardinalNumbers-en",
        "ordinalNumbers-en",
        "daysOfWeek-en",
        "months-en",
        "seasons-en",
        "timePeriods-en",
        "rainbowColors-en",
        "directions-en",
        "intermediateDirections-en",
        "sizeProgression-en",
        "militaryAlphabet-en",
        "planets-en",
        "zodiacSigns-en",
        "chineseZodiac-en",
        "passwords-common",
        "diceware-common",
      ].sort(),
    );
  });

  it("keeps the answers it gave: 'password' scores 0 with a suggestion, a five-word phrase 4 (003 FR-003)", () => {
    const estimate = createEstimator();
    expect(estimate("password")).toEqual({ score: 0, suggestion: expect.stringMatching(/\S/) });
    expect(estimate("vivid otter ledger crane mosaic").score).toBe(4);
  });
});
