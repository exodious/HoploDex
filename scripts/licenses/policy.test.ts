// The license policy (010 research.md §5, §7, §9): the allow list is the one
// src-tauri/deny.toml holds, the preference order is 010 research.md §5's, and a
// license allowed for particular packages or entries is allowed for nothing else
// (constitution, "Licensing").
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { ALLOWED, EXCEPTIONS, isAllowed, PREFERENCE } from "./policy.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

/** The `allow` array of deny.toml's `[licenses]`, read without a TOML library. */
function denyAllow(): string[] {
  const toml = readFileSync(path.join(repoRoot, "src-tauri/deny.toml"), "utf8");
  const section = /^\[licenses\]\s*$([\s\S]*?)(?=^\[|(?![\s\S]))/m.exec(toml);
  const array = section && /^allow\s*=\s*\[([\s\S]*?)\]/m.exec(section[1]);
  if (!array) throw new Error("deny.toml has no [licenses] allow list");
  return [...array[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

describe("010 research.md §9 - the allow list is in step with deny.toml", () => {
  it("ALLOWED equals deny.toml's allow list, both ways", () => {
    const deny = new Set(denyAllow());
    const missingFromScript = [...deny].filter((id) => !ALLOWED.has(id)).sort();
    const missingFromDeny = [...ALLOWED].filter((id) => !deny.has(id)).sort();
    expect({ missingFromScript, missingFromDeny }).toEqual({
      missingFromScript: [],
      missingFromDeny: [],
    });
  });
});

describe("010 research.md §5 - the preference order", () => {
  it("is exactly the order the research gives", () => {
    expect(PREFERENCE).toEqual([
      "MIT",
      "Apache-2.0",
      "BSD-3-Clause",
      "BSD-2-Clause",
      "ISC",
      "Zlib",
      "Unicode-3.0",
      "BSL-1.0",
      "0BSD",
      "MIT-0",
      "CC0-1.0",
      "MPL-2.0",
      "Apache-2.0 WITH LLVM-exception",
    ]);
  });

  it("holds only allowed licenses", () => {
    for (const id of PREFERENCE) expect(ALLOWED.has(id), id).toBe(true);
  });
});

describe("010 FR-008 research.md §7 - exceptions are scoped", () => {
  it("allows OFL-1.1 for the fontsource packages and for nothing else", () => {
    expect(isAllowed("OFL-1.1", { package: "@fontsource/inter" })).toBe(true);
    expect(isAllowed("OFL-1.1", { package: "@fontsource-variable/big-shoulders-display" })).toBe(
      true,
    );
    expect(isAllowed("OFL-1.1", { package: "fontsource-fake" })).toBe(false);
    expect(isAllowed("OFL-1.1", { package: "@fontsource-evil/x" })).toBe(false);
    expect(isAllowed("OFL-1.1", { entry: "SQLite" })).toBe(false);
    expect(isAllowed("OFL-1.1", {})).toBe(false);
  });

  it("allows blessing, CC-BY-4.0, ODC-By-1.0 and LicenseRef-public-domain only for the entries named", () => {
    expect(isAllowed("blessing", { entry: "SQLite" })).toBe(true);
    expect(isAllowed("CC-BY-4.0", { entry: "EFF Large Wordlist for Passphrases" })).toBe(true);
    expect(isAllowed("ODC-By-1.0", { entry: "OpenSubtitles 2024 word frequencies" })).toBe(true);
    for (const entry of [
      "Moby Word Lists names",
      "1990 US Census surnames",
      "Ten million passwords",
    ]) {
      expect(isAllowed("LicenseRef-public-domain", { entry }), entry).toBe(true);
    }
  });

  it("never allows them for a package or a crate, or for another entry", () => {
    for (const license of ["blessing", "CC-BY-4.0", "ODC-By-1.0", "LicenseRef-public-domain"]) {
      expect(isAllowed(license, { package: "some-package" }), license).toBe(false);
      expect(isAllowed(license, { package: "@fontsource/inter" }), license).toBe(false);
      expect(isAllowed(license, { entry: "Some other entry" }), license).toBe(false);
      expect(isAllowed(license, {}), license).toBe(false);
      expect(ALLOWED.has(license), license).toBe(false);
    }
    expect(isAllowed("blessing", { entry: "EFF Large Wordlist for Passphrases" })).toBe(false);
    expect(isAllowed("CC-BY-4.0", { entry: "SQLite" })).toBe(false);
  });

  it("allows the shared list for any subject, and refuses an unknown license", () => {
    expect(isAllowed("MIT", { package: "x" })).toBe(true);
    expect(isAllowed("MIT", { entry: "x" })).toBe(true);
    expect(isAllowed("MIT", {})).toBe(true);
    expect(isAllowed("Proprietary", { package: "x" })).toBe(false);
  });

  it("gives every exception a reason and scopes it to packages or entries, not both", () => {
    for (const exception of EXCEPTIONS) {
      expect(exception.reason, exception.license).toMatch(/\S/);
      expect(Boolean(exception.packages) !== Boolean(exception.entries), exception.license).toBe(
        true,
      );
    }
  });
});
