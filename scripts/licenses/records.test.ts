// The committed records under licenses/ and their checks (010 research.md §7,
// §8; 010 data-model.md; 010 FR-009, FR-010): written first, red until records.mjs
// exports what is used here. Failure messages are asserted word for word as
// 010 contracts/notices.md "Failure messages" gives them; where the contract
// is silent a test checks the words that name the entry and the problem.
//
// The API these tests assume (fixtures/README.md has the fixtures):
//
//   readRecords(licensesDir)
//     -> { handRecorded, exceptions, clarifications, nativeCode, failures: string[] }
//        The entries as written. An absent file is []. A file that is not
//        sorted by its first field, or an entry with an unknown field, a bad
//        `what`, is a failure naming the file and the entry.
//   readCommittedText(file) -> string   (the file less its `Source:` first line;
//        throws an Error naming the file and `Source:` when the line is missing)
//
//   checkHandRecorded(entries, ctx) -> { failures, components }
//   checkExceptions(entries, ctx)   -> { failures, warnings, used, resolved }
//   checkClarifications(entries, ctx) -> { failures, warnings, used, resolved }
//   checkNativeCode(entries, ctx)   -> { failures, warnings, used }
//   unusedAcrossTargets(records, perTarget) -> string[]   (failures)
//
//   ctx = { root, triple, allTargets?, env?,
//           crates: [{ name, version, folder, license, features?, nativeBuild? }],
//           npm:    [{ name, version, folder, license }] }
//     root: the repository root (a `licenses/texts/...` path resolves against it);
//     the OS comes from the triple; `features` are the carrier's enabled
//     features; `nativeBuild` lists the `cc`/`cmake` build dependencies enabled
//     for the triple. `used` lists component names (exceptions, clarifications)
//     or crate names (native code) that the target ships; `resolved` maps a
//     component to its [{ file?, text }].
//   perTarget = [{ exceptions: used[], clarifications: used[], nativeCode: used[] }, ...]
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import {
  checkClarifications,
  checkExceptions,
  checkHandRecorded,
  checkNativeCode,
  readCommittedText,
  readRecords,
  unusedAcrossTargets,
} from "./records.mjs";

const fixtures = path.join(path.dirname(fileURLToPath(import.meta.url)), "fixtures");
const project = path.join(fixtures, "project");
const LINUX = "x86_64-unknown-linux-gnu";
const WINDOWS = "x86_64-pc-windows-msvc";
const MACOS = "aarch64-apple-darwin";

type Crate = {
  name: string;
  version: string;
  folder: string;
  license: string;
  features?: string[];
  nativeBuild?: string[];
};
type Pkg = { name: string; version: string; folder: string; license: string };

const crate = (
  name: string,
  version: string,
  license: string,
  extra: Partial<Crate> = {},
): Crate => ({
  name,
  version,
  license,
  folder: path.join(project, "registry", `${name}-${version}`),
  ...extra,
});
const npm = (name: string, version: string, license = "MIT"): Pkg => ({
  name,
  version,
  license,
  folder: path.join(project, "modules", name),
});

const carrier = (extra: Partial<Crate> = {}) =>
  crate("libsqlite3-sys", "0.38.1", "MIT", {
    features: ["bundled-sqlcipher", "default"],
    nativeBuild: ["cc"],
    ...extra,
  });
const bazSys = () => crate("baz-sys", "0.1.0", "MIT", { nativeBuild: ["cc"] });
const prebuilt = () => crate("prebuilt-lib-sys", "0.1.0", "MIT");
const webview = () => crate("webview2-com-sys", "0.38.2", "MIT");

/** The crates and packages the good fixture ships for a target (fixtures/cargo's trees). */
function shipped(triple: string) {
  const crates = [
    crate("single", "1.0.0", "MIT"),
    crate("needs-exception", "1.0.0", "MIT"),
    crate("unplaceable", "1.0.0", "MIT OR Apache-2.0"),
    bazSys(),
    carrier(),
  ];
  if (triple === LINUX) crates.push(crate("linux-only", "1.0.0", "MIT"));
  if (triple === WINDOWS) crates.push(crate("windows-only", "1.0.0", "MIT"), prebuilt(), webview());
  const packages = [
    npm("@zxcvbn-ts/language-en", "4.1.1"),
    npm("@zxcvbn-ts/language-common", "4.1.3"),
    npm("no-license-file", "1.0.0"),
    npm("spdx-only", "1.0.0"),
    npm("@fontsource-variable/example-font", "5.0.0", "OFL-1.1"),
  ];
  return { crates, npm: packages };
}

function ctx(triple: string, over: Record<string, unknown> = {}) {
  return { root: project, triple, ...shipped(triple), ...over };
}

const goodDir = path.join(project, "licenses");
const lazy = <T>(make: () => T) => {
  let value: T | undefined;
  return () => (value ??= make());
};
const good = lazy(() => readRecords(goodDir));
const caseRoot = (name: string) => path.join(fixtures, "records", name);
const readCase = (name: string) => readRecords(path.join(caseRoot(name), "licenses"));
const sha256 = (text: string) => createHash("sha256").update(text).digest("hex");
const read = (...parts: string[]) => readFileSync(path.join(...parts), "utf8");
const texts = (items: { text: string }[]) => items.map((t) => t.text);

describe("010 research.md §7 - reading the records", () => {
  it("reads the four files as written, with no failure", () => {
    expect(good().failures).toEqual([]);
    expect(good().handRecorded.map((e: { name: string }) => e.name)).toEqual([
      "EFF Large Wordlist for Passphrases",
      "Microsoft WebView2 loader",
      "OpenSSL",
      "OpenSubtitles 2024 word frequencies",
      "SQLCipher",
      "SQLite",
    ]);
    expect(good().exceptions.map((e: { component: string }) => e.component)).toEqual([
      "crate:needs-exception",
      "npm:no-license-file",
      "npm:spdx-only",
    ]);
    expect(good().clarifications).toHaveLength(2);
    expect(good().nativeCode.map((e: { crate: string }) => e.crate)).toEqual([
      "baz-sys",
      "libsqlite3-sys",
      "prebuilt-lib-sys",
      "webview2-com-sys",
    ]);
  });

  it("fails each record file that is absent, naming it (010 FR-009, 010 research.md §5)", () => {
    const records = readCase("absent-files");
    expect(records.nativeCode).toHaveLength(4);
    expect(records.failures).toEqual(
      ["hand-recorded.json", "exceptions.json", "clarify.json"].map(
        (name) =>
          `${name}: the file is missing; every record file must exist (write an empty list as []).`,
      ),
    );
  });

  it("reads a present file holding [] as no entries, with no failure", () => {
    const records = readCase("hand-empty-os");
    expect(records.exceptions).toEqual([]);
    expect(records.clarifications).toEqual([]);
    expect(records.nativeCode).toEqual([]);
  });

  it("fails a hand-recorded `os` that is empty, naming the file and the entry (010 FR-009)", () => {
    expect(readCase("hand-empty-os").failures).toEqual([
      'hand-recorded.json: entry "SQLite" has no "os"; list at least one of linux, windows, macos.',
    ]);
  });

  it("fails a hand-recorded `os` outside linux, windows and macos, naming the value (010 FR-009)", () => {
    expect(readCase("hand-unknown-os").failures).toEqual([
      'hand-recorded.json: entry "SQLite" has os "freebsd"; each must be linux, windows or macos.',
    ]);
  });

  it("fails a hand-recorded `kind` outside compiled-in, linked and data (010 FR-009)", () => {
    expect(readCase("hand-unknown-kind").failures).toEqual([
      'hand-recorded.json: entry "SQLite" has kind "bundled"; it must be compiled-in, linked or data.',
    ]);
  });

  it("fails a hand-recorded entry or an exception with no texts (010 FR-009, 010 FR-010)", () => {
    expect(readCase("hand-empty-texts").failures).toEqual([
      'hand-recorded.json: entry "SQLite" has no texts; list at least one.',
    ]);
    expect(readCase("exception-empty-texts").failures).toEqual([
      'exceptions.json: entry "crate:needs-exception" has no texts; list at least one.',
    ]);
  });

  it("fails a clarification with no files (010 FR-009, 010 FR-010)", () => {
    expect(readCase("clarify-no-files").failures).toEqual([
      'clarify.json: entry "crate:unplaceable" has no files; list at least one.',
    ]);
  });

  it("fails a native-code `what: hand-recorded:<name>` that names no hand-recorded entry (010 FR-009)", () => {
    expect(readCase("native-hand-unknown").failures).toEqual([
      'native-code.json: entry "libsqlite3-sys" has what "hand-recorded:No such entry", but hand-recorded.json has no entry "No such entry".',
    ]);
  });

  it("fails a hand-recorded file that isn't sorted, naming the file and the entry", () => {
    const { failures } = readCase("hand-unsorted");
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("hand-recorded.json");
    expect(failures[0]).toContain("SQLCipher");
  });

  it("fails an exceptions file that isn't sorted, naming the file and the entry", () => {
    const { failures } = readCase("exceptions-unsorted");
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("exceptions.json");
    expect(failures[0]).toContain("crate:needs-exception");
  });

  it("fails an unknown field, naming the file, the entry and the field", () => {
    const hand = readCase("hand-unknown-field").failures;
    expect(hand).toHaveLength(1);
    expect(hand[0]).toContain("hand-recorded.json");
    expect(hand[0]).toContain("SQLite");
    expect(hand[0]).toContain("licence");
    const clarify = readCase("clarify-unknown-field").failures;
    expect(clarify).toHaveLength(1);
    expect(clarify[0]).toContain("clarify.json");
    expect(clarify[0]).toContain("crate:unplaceable");
    expect(clarify[0]).toContain("note");
  });

  it("fails an entry with a required field missing, naming the file, the entry and the field", () => {
    const { failures } = readCase("missing-fields");
    expect(failures).toHaveLength(3);
    const has = (file: string, entry: string, field: string) =>
      failures.some(
        (f: string) => f.includes(file) && f.includes(entry) && f.includes(`"${field}"`),
      );
    expect(has("hand-recorded.json", "SQLite", "texts")).toBe(true);
    expect(has("hand-recorded.json", "SQLite", "reason")).toBe(true);
    expect(has("exceptions.json", "crate:needs-exception", "declared")).toBe(true);
  });

  it("fails a native-code entry whose `what` is not one of the four", () => {
    const { failures } = readCase("native-bad-what");
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("native-code.json");
    expect(failures[0]).toContain("baz-sys");
    expect(failures[0]).toContain("something-else");
  });
});

// Shape failures that need only one malformed entry are written to a scratch folder
// (fixtures/README.md, "Records"): a string is written as it is, anything else as JSON.
function scratchRecords(files: Record<string, unknown>) {
  const dir = mkdtempSync(path.join(os.tmpdir(), "hoplodex-records-"));
  const all: Record<string, unknown> = {
    "hand-recorded.json": [],
    "exceptions.json": [],
    "clarify.json": [],
    "native-code.json": [],
    ...files,
  };
  for (const [name, value] of Object.entries(all)) {
    writeFileSync(path.join(dir, name), typeof value === "string" ? value : JSON.stringify(value));
  }
  return readRecords(dir);
}

describe("010 research.md §7 - the shape of a record file", () => {
  const sqlite = () => good().handRecorded.find((e: { name: string }) => e.name === "SQLite");
  const withCarrier = (carrier: Record<string, unknown>) => ({ ...sqlite(), carrier });
  const hand = (entry: unknown) => scratchRecords({ "hand-recorded.json": [entry] }).failures;

  it("fails a carrier that names both crate and npm (010 FR-009)", () => {
    expect(hand(withCarrier({ crate: "libsqlite3-sys", npm: "other", version: "0.38.1" }))).toEqual(
      ['hand-recorded.json: entry "SQLite" carrier must name exactly one of crate and npm.'],
    );
  });

  it("fails a carrier that names neither crate nor npm (010 FR-009)", () => {
    expect(hand(withCarrier({ version: "0.38.1" }))).toEqual([
      'hand-recorded.json: entry "SQLite" carrier must name exactly one of crate and npm.',
    ]);
  });

  it("fails a carrier with no version (010 FR-009)", () => {
    expect(hand(withCarrier({ crate: "libsqlite3-sys" }))).toEqual([
      'hand-recorded.json: entry "SQLite" carrier has no "version".',
    ]);
  });

  it("fails an unknown carrier field, naming it", () => {
    expect(
      hand(withCarrier({ crate: "libsqlite3-sys", version: "0.38.1", features: "x" })),
    ).toEqual(['hand-recorded.json: entry "SQLite" has an unknown carrier field "features".']);
  });

  it("fails an unknown field of a clarification's file, naming it", () => {
    const entry = {
      ...good().clarifications[0],
      files: [{ path: "LICENSE", sha256: "0".repeat(64), note: "x" }],
    };
    expect(scratchRecords({ "clarify.json": [entry] }).failures).toEqual([
      'clarify.json: entry "crate:unplaceable" has an unknown file field "note".',
    ]);
  });

  it("fails a clarification's file that is neither { path, sha256 } nor { text }", () => {
    const entry = { ...good().clarifications[0], files: [{ path: "LICENSE" }] };
    expect(scratchRecords({ "clarify.json": [entry] }).failures).toEqual([
      'clarify.json: entry "crate:unplaceable" has a file that is neither { path, sha256 } nor { text }.',
    ]);
  });

  it("fails an entry that appears twice, naming it", () => {
    expect(scratchRecords({ "hand-recorded.json": [sqlite(), sqlite()] }).failures).toEqual([
      'hand-recorded.json: entry "SQLite" appears twice.',
    ]);
  });

  it("fails an entry that isn't an object", () => {
    expect(scratchRecords({ "native-code.json": ["baz-sys"] }).failures).toEqual([
      "native-code.json: an entry isn't an object.",
    ]);
  });

  it("fails a list field that isn't a list, naming the field", () => {
    expect(hand({ ...sqlite(), os: "linux" })).toEqual([
      'hand-recorded.json: entry "SQLite" has "os", but it isn\'t a list.',
    ]);
    expect(hand({ ...sqlite(), texts: "licenses/texts/sqlite/blessing.txt" })).toEqual([
      'hand-recorded.json: entry "SQLite" has "texts", but it isn\'t a list.',
    ]);
  });

  it("fails a file that isn't valid JSON, naming the file and the parser's reason", () => {
    const { failures } = scratchRecords({ "exceptions.json": "[{ not json" });
    expect(failures).toHaveLength(1);
    expect(failures[0]).toMatch(/^exceptions\.json: can't be read as JSON \(.+\)\.$/);
  });

  it("fails a file that holds something other than a list, naming the file", () => {
    expect(scratchRecords({ "clarify.json": {} }).failures).toEqual([
      "clarify.json: must hold a list of entries.",
    ]);
  });
});

describe("010 FR-010 - a committed text keeps its first line out of the notice", () => {
  it("returns the text without its `Source: <url>` line", () => {
    const file = path.join(goodDir, "texts", "sqlite", "blessing.txt");
    expect(read(file).split("\n")[0]).toMatch(/^Source: https:\/\//);
    expect(readCommittedText(file)).toBe(read(file).split("\n").slice(1).join("\n"));
    expect(readCommittedText(file)).toMatch(/^May you do good and not evil\./);
  });

  it("throws, naming the file and `Source:`, when the first line isn't one", () => {
    const file = path.join(
      caseRoot("text-without-source"),
      "licenses",
      "texts",
      "sqlite",
      "blessing.txt",
    );
    expect(() => readCommittedText(file)).toThrow(/Source:/);
    expect(() => readCommittedText(file)).toThrow(/blessing\.txt/);
  });
});

describe("010 US2-4 - hand-recorded components", () => {
  const entries = () => good().handRecorded;

  it("lists the entries for the target's OS, with their texts and carrier", () => {
    const result = checkHandRecorded(entries(), ctx(LINUX));
    expect(result.failures).toEqual([]);
    expect(result.components.map((c: { name: string }) => c.name)).toEqual([
      "EFF Large Wordlist for Passphrases",
      "OpenSubtitles 2024 word frequencies",
      "SQLCipher",
      "SQLite",
    ]);
    const byName = (name: string) =>
      result.components.find((c: { name: string }) => c.name === name);
    expect(byName("SQLCipher")).toMatchObject({
      name: "SQLCipher",
      version: "4.14.0 community",
      license: "BSD-3-Clause",
      kind: "compiled-in",
      carrier: { kind: "crate", name: "libsqlite3-sys", version: "0.38.1" },
    });
    expect(texts(byName("SQLCipher").texts)).toEqual([
      read(project, "registry", "libsqlite3-sys-0.38.1", "sqlcipher", "LICENSE"),
    ]);
    expect(byName("EFF Large Wordlist for Passphrases").carrier).toEqual({
      kind: "npm",
      name: "@zxcvbn-ts/language-common",
      version: "4.1.3",
    });
  });

  it("reports the entries it applied for the OS as `used`, for the --all-targets check", () => {
    const names = (triple: string) =>
      [...checkHandRecorded(entries(), ctx(triple, { allTargets: true })).used].sort();
    expect(names(MACOS)).toEqual([
      "EFF Large Wordlist for Passphrases",
      "OpenSubtitles 2024 word frequencies",
      "SQLCipher",
      "SQLite",
    ]);
    expect(names(WINDOWS)).toContain("OpenSSL");
    expect(names(WINDOWS)).toHaveLength(6);
  });

  it("gives a package: text from the carrier's folder, in the record's order", () => {
    const { components } = checkHandRecorded(entries(), ctx(LINUX));
    const subtitles = components.find((c: { name: string }) => c.name.startsWith("OpenSubtitles"));
    const folder = path.join(project, "modules", "@zxcvbn-ts", "language-en");
    expect(texts(subtitles.texts)).toEqual([
      read(folder, "NOTICE.md"),
      read(folder, "THIRD_PARTY_LICENSES.md"),
    ]);
  });

  it("gives a committed text without its Source line", () => {
    const { components } = checkHandRecorded(entries(), ctx(LINUX));
    const sqlite = components.find((c: { name: string }) => c.name === "SQLite");
    expect(texts(sqlite.texts)).toEqual([
      "May you do good and not evil.\nMay you find forgiveness for yourself and forgive others.\nMay you share freely, never taking more than you give.\n",
    ]);
  });

  it("lists the Windows-only entries on Windows, and not elsewhere", () => {
    const names = (triple: string, over = {}) =>
      checkHandRecorded(entries(), ctx(triple, over)).components.map(
        (c: { name: string }) => c.name,
      );
    const env = { OPENSSL_DIR: path.join(fixtures, "openssl", "3.5.4") };
    expect(names(WINDOWS, { env })).toEqual([
      "EFF Large Wordlist for Passphrases",
      "Microsoft WebView2 loader",
      "OpenSSL",
      "OpenSubtitles 2024 word frequencies",
      "SQLCipher",
      "SQLite",
    ]);
    expect(names(MACOS)).not.toContain("OpenSSL");
    expect(names(MACOS)).not.toContain("Microsoft WebView2 loader");
  });

  it("fails when the carrier's version differs, naming the entry (contract message)", () => {
    const moved = carrier({ version: "0.39.0" });
    const { failures } = checkHandRecorded(entries(), ctx(LINUX, { crates: [moved] }));
    expect(failures).toContain(
      'hand-recorded "SQLCipher": its carrier crate libsqlite3-sys is 0.39.0, recorded 0.38.1. Review the entry (its version, text and carrier) and update it.',
    );
  });

  it("fails when the carrier's feature isn't enabled (contract message)", () => {
    const { failures } = checkHandRecorded(
      entries(),
      ctx(LINUX, { crates: [carrier({ features: ["default"] })] }),
    );
    expect(failures).toEqual([
      'hand-recorded "SQLCipher": carrier libsqlite3-sys is shipped without feature bundled-sqlcipher.',
    ]);
  });

  it("fails when a crate carrier isn't shipped for a listed OS", () => {
    const { failures } = checkHandRecorded(entries(), ctx(LINUX, { crates: [] }));
    for (const name of ["SQLCipher", "SQLite"]) {
      const failure = failures.find((f: string) => f.includes(`hand-recorded "${name}"`));
      expect(failure, name).toContain("libsqlite3-sys");
      expect(failure, name).toContain("isn't shipped");
    }
  });

  it("fails when an npm carrier isn't shipped for a listed OS", () => {
    const { failures } = checkHandRecorded(entries(), ctx(LINUX, { npm: [] }));
    const failure = failures.find((f: string) => f.includes("OpenSubtitles"));
    expect(failure).toContain("@zxcvbn-ts/language-en");
    expect(failure).toContain("isn't shipped");
  });

  it("fails a Windows entry whose carrier isn't shipped on Windows", () => {
    const env = { OPENSSL_DIR: path.join(fixtures, "openssl", "3.5.4") };
    const crates = shipped(WINDOWS).crates.filter((c) => c.name !== "webview2-com-sys");
    const { failures } = checkHandRecorded(entries(), ctx(WINDOWS, { crates, env }));
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain('hand-recorded "Microsoft WebView2 loader"');
    expect(failures[0]).toContain("webview2-com-sys");
  });

  it("fails a missing committed text, naming the entry and the file", () => {
    const { failures } = checkHandRecorded(
      readCase("hand-missing-text").handRecorded,
      ctx(LINUX, { root: caseRoot("hand-missing-text") }),
    );
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain('hand-recorded "SQLite"');
    expect(failures[0]).toContain("missing.txt");
  });

  it("fails a missing package: text, naming the entry and the file", () => {
    const language = npm("@zxcvbn-ts/language-en", "4.1.1");
    const withoutNotice = {
      ...language,
      folder: path.join(project, "modules", "@zxcvbn-ts", "language-common"),
    };
    const { failures } = checkHandRecorded(
      entries(),
      ctx(LINUX, { npm: [withoutNotice, npm("@zxcvbn-ts/language-common", "4.1.3")] }),
    );
    expect(failures.length).toBeGreaterThan(0);
    for (const failure of failures) {
      expect(failure).toContain('hand-recorded "OpenSubtitles 2024 word frequencies"');
    }
    expect(failures.some((f: string) => f.includes("NOTICE.md"))).toBe(true);
  });

  it("fails a committed text with no Source line, naming the entry", () => {
    const { failures } = checkHandRecorded(
      readCase("text-without-source").handRecorded,
      ctx(LINUX, { root: caseRoot("text-without-source") }),
    );
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain('hand-recorded "SQLite"');
    expect(failures[0]).toContain("Source:");
  });
});

describe("010 research.md §7 - the OpenSSL version comes from the build", () => {
  const entries = () => good().handRecorded;
  const v3 = path.join(fixtures, "openssl", "3.5.4");
  const v1 = path.join(fixtures, "openssl", "1.1.1w");
  const none = path.join(fixtures, "openssl", "no-version");
  const include = (dir: string) => path.join(dir, "include");
  const openssl = (result: { components: { name: string }[] }) =>
    result.components.find((c) => c.name === "OpenSSL") as { version: string } | undefined;
  const windows = (env: Record<string, string>) =>
    checkHandRecorded(entries(), ctx(WINDOWS, { env }));
  const refused = (env: Record<string, string>) => windows(env).failures;
  const PREFIX = "X86_64_PC_WINDOWS_MSVC_";
  const linksOpenSsl1 = (variable: string) =>
    `hand-recorded "OpenSSL": the Windows build links OpenSSL 1.1.1w (headers from ${variable}); HoploDex needs OpenSSL 3 (its license is compatible with GPLv3, 1.1's isn't).`;

  it("reads OPENSSL_VERSION_STR on a Windows check, and the version becomes the component's", () => {
    const result = windows({ OPENSSL_DIR: v3 });
    expect(result.failures).toEqual([]);
    expect(openssl(result)?.version).toBe("3.5.4");
  });

  it("fails OpenSSL 1.1.1w naming the variable it came from", () => {
    expect(refused({ OPENSSL_DIR: v1 })).toEqual([linksOpenSsl1("OPENSSL_DIR")]);
  });

  // libsqlite3-sys 0.38.1's build.rs, `env()` (lines 312-321): each name is read as
  // <TARGET>_<name> first, then <name>. Its include path (lines 159-189): both
  // OPENSSL_LIB_DIR and OPENSSL_INCLUDE_DIR set -> the include variable's value;
  // else OPENSSL_DIR is required, and the include path is OPENSSL_INCLUDE_DIR if
  // set, else <OPENSSL_DIR>/include. 010 research.md §7 has the same account.
  describe.each([
    ["OPENSSL_DIR", { OPENSSL_DIR: v3 }, "OPENSSL_DIR"],
    [
      "<TARGET>_OPENSSL_DIR over OPENSSL_DIR",
      { [`${PREFIX}OPENSSL_DIR`]: v3, OPENSSL_DIR: v1 },
      `${PREFIX}OPENSSL_DIR`,
    ],
    [
      "OPENSSL_INCLUDE_DIR over OPENSSL_DIR",
      { OPENSSL_DIR: v1, OPENSSL_INCLUDE_DIR: include(v3) },
      "OPENSSL_INCLUDE_DIR",
    ],
    [
      "<TARGET>_OPENSSL_INCLUDE_DIR over OPENSSL_INCLUDE_DIR",
      {
        OPENSSL_DIR: v1,
        [`${PREFIX}OPENSSL_INCLUDE_DIR`]: include(v3),
        OPENSSL_INCLUDE_DIR: include(v1),
      },
      `${PREFIX}OPENSSL_INCLUDE_DIR`,
    ],
    [
      "OPENSSL_LIB_DIR and OPENSSL_INCLUDE_DIR without OPENSSL_DIR",
      { OPENSSL_LIB_DIR: "unread", OPENSSL_INCLUDE_DIR: include(v3) },
      "OPENSSL_INCLUDE_DIR",
    ],
    [
      "<TARGET>_OPENSSL_LIB_DIR completing the pair",
      { [`${PREFIX}OPENSSL_LIB_DIR`]: "unread", OPENSSL_INCLUDE_DIR: include(v3) },
      "OPENSSL_INCLUDE_DIR",
    ],
    [
      "OPENSSL_INCLUDE_DIR with OPENSSL_DIR but no OPENSSL_LIB_DIR",
      { OPENSSL_DIR: v1, OPENSSL_INCLUDE_DIR: include(v3) },
      "OPENSSL_INCLUDE_DIR",
    ],
    [
      "OPENSSL_LIB_DIR with OPENSSL_DIR but no OPENSSL_INCLUDE_DIR",
      { OPENSSL_LIB_DIR: "unread", OPENSSL_DIR: v3 },
      "OPENSSL_DIR",
    ],
    [
      "another target's prefix",
      { AARCH64_PC_WINDOWS_MSVC_OPENSSL_DIR: v1, OPENSSL_DIR: v3 },
      "OPENSSL_DIR",
    ],
  ])("takes the headers from %s", (_name, env, variable) => {
    it(`reads 3.5.4 through ${variable}`, () => {
      const result = windows(env as Record<string, string>);
      expect(result.failures).toEqual([]);
      expect(openssl(result)?.version).toBe("3.5.4");
    });
  });

  describe.each([
    [
      "a 1.1.1w OPENSSL_INCLUDE_DIR over a 3.x OPENSSL_DIR",
      { OPENSSL_DIR: v3, OPENSSL_INCLUDE_DIR: include(v1) },
      "OPENSSL_INCLUDE_DIR",
    ],
    [
      "a 1.1.1w <TARGET>_OPENSSL_DIR over a 3.x OPENSSL_DIR",
      { [`${PREFIX}OPENSSL_DIR`]: v1, OPENSSL_DIR: v3 },
      `${PREFIX}OPENSSL_DIR`,
    ],
    [
      "a 1.1.1w <TARGET>_OPENSSL_INCLUDE_DIR over a 3.x pair",
      {
        [`${PREFIX}OPENSSL_INCLUDE_DIR`]: include(v1),
        OPENSSL_LIB_DIR: "unread",
        OPENSSL_INCLUDE_DIR: include(v3),
      },
      `${PREFIX}OPENSSL_INCLUDE_DIR`,
    ],
    [
      "a 1.1.1w OPENSSL_INCLUDE_DIR beside OPENSSL_LIB_DIR",
      { OPENSSL_LIB_DIR: "unread", OPENSSL_INCLUDE_DIR: include(v1) },
      "OPENSSL_INCLUDE_DIR",
    ],
  ])("refuses %s", (_name, env, variable) => {
    it(`names ${variable}`, () => {
      expect(refused(env as Record<string, string>)).toEqual([linksOpenSsl1(variable)]);
    });
  });

  it("fails a Windows check where no variable gives the build its headers, naming them", () => {
    const expected =
      "hand-recorded \"OpenSSL\": the OpenSSL version the Windows build links can't be read; set OPENSSL_DIR, or both OPENSSL_LIB_DIR and OPENSSL_INCLUDE_DIR (libsqlite3-sys's build.rs reads them, as X86_64_PC_WINDOWS_MSVC_<name> first).";
    expect(refused({})).toEqual([expected]);
    // build.rs needs OPENSSL_DIR or both of the pair: a lone include or lib variable stops it
    expect(refused({ OPENSSL_INCLUDE_DIR: include(v3) })).toEqual([expected]);
    expect(refused({ OPENSSL_LIB_DIR: "unread" })).toEqual([expected]);
    // another target's variable doesn't count
    expect(refused({ AARCH64_PC_WINDOWS_MSVC_OPENSSL_DIR: v3 })).toEqual([expected]);
  });

  it("fails a variable whose folder has no opensslv.h, naming the entry and the variable", () => {
    const failures = refused({ OPENSSL_DIR: fixtures });
    expect(failures).toEqual([
      `hand-recorded "OpenSSL": ${path.join(fixtures, "include", "openssl", "opensslv.h")} (opensslv.h, from OPENSSL_DIR) doesn't exist.`,
    ]);
  });

  it("fails an opensslv.h that names no OpenSSL version, naming the file and the variable", () => {
    const header = path.join(none, "include", "openssl", "opensslv.h");
    expect(refused({ OPENSSL_DIR: none })).toEqual([
      `hand-recorded "OpenSSL": opensslv.h (${header}, from OPENSSL_DIR) names no OpenSSL version.`,
    ]);
  });

  it("doesn't read the version under --all-targets: the text and the carrier only", () => {
    const result = checkHandRecorded(
      entries(),
      ctx(WINDOWS, { allTargets: true, env: { OPENSSL_DIR: v1 } }),
    );
    expect(result.failures).toEqual([]);
    expect(openssl(result)?.version).toBe("3.x (the version the Windows build links)");
    expect(
      texts(result.components.find((c: { name: string }) => c.name === "OpenSSL").texts),
    ).toHaveLength(1);
  });

  it("still checks the OpenSSL carrier under --all-targets", () => {
    const crates = shipped(WINDOWS).crates.filter((c) => c.name !== "libsqlite3-sys");
    const { failures } = checkHandRecorded(entries(), ctx(WINDOWS, { allTargets: true, crates }));
    expect(failures.some((f: string) => f.includes('hand-recorded "OpenSSL"'))).toBe(true);
  });

  it("ignores the OpenSSL variables on Linux and macOS", () => {
    for (const triple of [LINUX, MACOS]) {
      const env = { OPENSSL_DIR: v1, OPENSSL_INCLUDE_DIR: include(v1) };
      expect(checkHandRecorded(entries(), ctx(triple, { env })).failures).toEqual([]);
    }
  });
});

describe("010 US2-3 - notice exceptions", () => {
  const entries = () => good().exceptions;
  const needs = crate("needs-exception", "1.0.0", "MIT");

  it("passes when the component is still missing its text and its declared license is the same", () => {
    const result = checkExceptions(entries(), ctx(LINUX));
    expect(result.failures).toEqual([]);
    expect(result.warnings).toEqual([]);
    expect([...result.used].sort()).toEqual([
      "crate:needs-exception",
      "npm:no-license-file",
      "npm:spdx-only",
    ]);
  });

  it("resolves the committed text, less its Source line", () => {
    const { resolved } = checkExceptions(entries(), ctx(LINUX));
    const [text] = texts(resolved["crate:needs-exception"]);
    expect(text).toContain("Copyright (c) 2020 Needs Exception Authors");
    expect(text).not.toContain("Source:");
  });

  it("passes on a version bump alone (010 research.md §8)", () => {
    const result = checkExceptions(
      entries(),
      ctx(LINUX, { crates: [{ ...needs, version: "1.3.0" }] }),
    );
    expect(result.failures).toEqual([]);
    expect(result.used).toContain("crate:needs-exception");
  });

  it("fails when the component now ships a license file: remove the exception", () => {
    const shipsLicense = crate("ships-license", "1.0.0", "MIT");
    const { failures } = checkExceptions(
      readCase("exception-stale").exceptions,
      ctx(LINUX, { root: caseRoot("exception-stale"), crates: [shipsLicense] }),
    );
    expect(failures).toHaveLength(1);
    expect(failures[0]).toMatch(
      /^exception crate:ships-license: the (?:package|crate) now ships LICENSE; remove the exception\.$/,
    );
  });

  it("fails when the component ships the text of an alternative that isn't the chosen one (010 FR-018)", () => {
    // "MIT OR Apache-2.0" is chosen as MIT, but the crate ships only LICENSE-APACHE: the
    // notices would use that text, so the exception is stale all the same.
    const alternative = crate("ships-alternative", "1.0.0", "MIT OR Apache-2.0");
    const { failures } = checkExceptions(
      readCase("exception-stale-alternative").exceptions,
      ctx(LINUX, { root: caseRoot("exception-stale-alternative"), crates: [alternative] }),
    );
    expect(failures).toEqual([
      "exception crate:ships-alternative: the crate now ships LICENSE-APACHE; remove the exception.",
    ]);
  });

  it("fails an exception whose package folder is missing, without reading it (010 FR-008)", () => {
    const gone = {
      ...npm("no-license-file", "1.0.0"),
      folder: path.join(fixtures, "no-such-folder"),
    };
    const { failures } = checkExceptions(entries(), ctx(LINUX, { npm: [gone] }));
    expect(failures).toEqual([
      `exception npm:no-license-file: its folder ${gone.folder} doesn't exist (run npm ci).`,
    ]);
  });

  it("fails when the declared license changed, naming both expressions", () => {
    const changed = crate("changed-license", "1.0.0", "MIT OR Apache-2.0");
    const { failures } = checkExceptions(
      readCase("exception-declared-changed").exceptions,
      ctx(LINUX, { root: caseRoot("exception-declared-changed"), crates: [changed] }),
    );
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("exception crate:changed-license");
    expect(failures[0]).toContain("MIT OR Apache-2.0");
    expect(failures[0]).toContain('"MIT"');
  });

  it("fails a missing text file, naming the exception and the file", () => {
    const { failures } = checkExceptions(
      readCase("exception-missing-text").exceptions,
      ctx(LINUX, { root: caseRoot("exception-missing-text") }),
    );
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("exception crate:needs-exception");
    expect(failures[0]).toContain("missing.txt");
  });

  it("warns, and doesn't fail, when this target doesn't ship the component", () => {
    const result = checkExceptions(entries(), ctx(MACOS, { crates: [], npm: [] }));
    expect(result.failures).toEqual([]);
    expect(result.used).toEqual([]);
    for (const e of entries()) {
      expect(result.warnings.some((w: string) => w.includes(e.component))).toBe(true);
    }
  });
});

describe("010 research.md §8 - clarifications", () => {
  const entries = () => good().clarifications;
  const cratesOnly = (c: Crate[]) => ({ crates: c, npm: [] as Pkg[] });
  const unplaceable = crate("unplaceable", "1.0.0", "MIT OR Apache-2.0");

  it("passes, and gives the listed files' texts in place of what the matcher finds", () => {
    const result = checkClarifications(entries(), ctx(LINUX));
    expect(result.failures).toEqual([]);
    expect([...result.used].sort()).toEqual([
      "crate:unplaceable",
      "npm:@fontsource-variable/example-font",
    ]);
    expect(texts(result.resolved["crate:unplaceable"])).toEqual([
      read(project, "registry", "unplaceable-1.0.0", "LICENSE"),
    ]);
  });

  it("takes a literal { text } and a committed path (less its Source line) in order", () => {
    const { resolved } = checkClarifications(entries(), ctx(LINUX));
    const font = texts(resolved["npm:@fontsource-variable/example-font"]);
    expect(font).toHaveLength(2);
    expect(font[0]).toBe(
      "Copyright 2020 The Example Font Project Authors (https://example.invalid/fonts)",
    );
    expect(font[1]).toContain("SIL OPEN FONT LICENSE Version 1.1");
    expect(font[1]).not.toContain("Source:");
  });

  it("fails when a listed file's SHA-256 differs (contract message, with the new hash)", () => {
    const actual = sha256(read(project, "registry", "unplaceable-1.0.0", "LICENSE"));
    const { failures } = checkClarifications(
      readCase("clarify-hash-changed").clarifications,
      ctx(LINUX, { root: caseRoot("clarify-hash-changed"), ...cratesOnly([unplaceable]) }),
    );
    expect(failures).toEqual([
      `clarification crate:unplaceable: LICENSE changed (sha256 ${actual}). Review it.`,
    ]);
  });

  it("fails a listed file that is missing, naming the clarification and the file", () => {
    const { failures } = checkClarifications(
      readCase("clarify-missing-file").clarifications,
      ctx(LINUX, { root: caseRoot("clarify-missing-file"), ...cratesOnly([unplaceable]) }),
    );
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("clarification crate:unplaceable");
    expect(failures[0]).toContain("COPYING-MISSING");
  });

  it("warns, and doesn't fail, when this target doesn't ship the component", () => {
    const result = checkClarifications(entries(), ctx(MACOS, { crates: [], npm: [] }));
    expect(result.failures).toEqual([]);
    expect(result.used).toEqual([]);
    expect(result.warnings.some((w: string) => w.includes("crate:unplaceable"))).toBe(true);
  });
});

describe("010 research.md §7 - native code", () => {
  const entries = () => good().nativeCode;

  it("passes the good case on Linux and Windows, and records what each target used", () => {
    const linux = checkNativeCode(entries(), ctx(LINUX));
    expect(linux.failures).toEqual([]);
    expect([...linux.used].sort()).toEqual(["baz-sys", "libsqlite3-sys"]);
    const windows = checkNativeCode(entries(), ctx(WINDOWS));
    expect(windows.failures).toEqual([]);
    expect(windows.warnings).toEqual([]);
    expect([...windows.used].sort()).toEqual([
      "baz-sys",
      "libsqlite3-sys",
      "prebuilt-lib-sys",
      "webview2-com-sys",
    ]);
  });

  it("warns about a listed crate that this target doesn't ship", () => {
    const { warnings } = checkNativeCode(entries(), ctx(LINUX));
    expect(warnings.some((w: string) => w.includes("prebuilt-lib-sys"))).toBe(true);
    expect(warnings.some((w: string) => w.includes("webview2-com-sys"))).toBe(true);
  });

  it("fails a shipped crate with an enabled cc build dependency that isn't listed (contract message)", () => {
    const { failures } = checkNativeCode(
      readCase("native-unlisted").nativeCode,
      ctx(WINDOWS, { crates: [crate("baz-sys", "0.1.0", "MIT", { nativeBuild: ["cc"] })] }),
    );
    expect(failures).toEqual([
      "native code: crate baz-sys 0.1.0 (x86_64-pc-windows-msvc) builds C (cc) and isn't in licenses/native-code.json. Review what it compiles or links and record it.",
    ]);
  });

  it("names cmake when that is the build dependency", () => {
    const { failures } = checkNativeCode(
      [],
      ctx(LINUX, { crates: [crate("baz-sys", "0.1.0", "MIT", { nativeBuild: ["cmake"] })] }),
    );
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("native code: crate baz-sys 0.1.0 (x86_64-unknown-linux-gnu)");
    expect(failures[0]).toContain("(cmake)");
  });

  it("fails a shipped crate that carries a prebuilt library and isn't listed", () => {
    const { failures } = checkNativeCode([], ctx(WINDOWS, { crates: [prebuilt()] }));
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain(
      "native code: crate prebuilt-lib-sys 0.1.0 (x86_64-pc-windows-msvc)",
    );
    expect(failures[0]).toContain("isn't in licenses/native-code.json");
  });

  it("fails every unlisted crate, one line each", () => {
    const { failures } = checkNativeCode([], ctx(WINDOWS));
    expect(failures).toHaveLength(4);
  });

  it("doesn't look at a crate with neither a build dependency nor a prebuilt file", () => {
    const { failures } = checkNativeCode(
      [],
      ctx(LINUX, { crates: [crate("single", "1.0.0", "MIT")] }),
    );
    expect(failures).toEqual([]);
  });

  it("warns about a listed crate that no longer ships on this target", () => {
    const result = checkNativeCode(
      readCase("native-unused").nativeCode,
      ctx(LINUX, { crates: [] }),
    );
    expect(result.failures).toEqual([]);
    expect(result.used).toEqual([]);
    expect(result.warnings.some((w: string) => w.includes("gone-sys"))).toBe(true);
  });
});

describe("010 research.md §9 - an entry no target uses fails the --all-targets audit", () => {
  const everything = (
    over: Partial<
      Record<"exceptions" | "clarifications" | "nativeCode" | "handRecorded", string[]>
    > = {},
  ) => ({
    handRecorded: good().handRecorded.map((e: { name: string }) => e.name),
    exceptions: ["crate:needs-exception", "npm:no-license-file", "npm:spdx-only"],
    clarifications: ["crate:unplaceable", "npm:@fontsource-variable/example-font"],
    nativeCode: ["baz-sys", "libsqlite3-sys", "prebuilt-lib-sys", "webview2-com-sys"],
    ...over,
  });

  it("passes when some target uses every entry", () => {
    const perTarget = [
      everything({ exceptions: [], clarifications: [], nativeCode: ["baz-sys", "libsqlite3-sys"] }),
      everything(),
      { exceptions: [], clarifications: [], nativeCode: [] },
    ];
    expect(unusedAcrossTargets(good(), perTarget)).toEqual([]);
  });

  it("fails a hand-recorded entry no target ships, naming it (010 FR-009, 010 FR-010)", () => {
    const perTarget = [
      everything({ handRecorded: ["SQLCipher", "SQLite"] }),
      everything({ handRecorded: ["SQLCipher"] }),
    ];
    const failures = unusedAcrossTargets(good(), perTarget);
    expect(failures).toEqual(
      [
        "EFF Large Wordlist for Passphrases",
        "Microsoft WebView2 loader",
        "OpenSSL",
        "OpenSubtitles 2024 word frequencies",
      ].map(
        (name) =>
          `hand-recorded "${name}": no target ships it; remove the entry (licenses/hand-recorded.json).`,
      ),
    );
  });

  it("fails an exception no target uses, naming it", () => {
    const failures = unusedAcrossTargets(readCase("exception-unused"), [
      { exceptions: [], clarifications: [], nativeCode: [] },
      { exceptions: [], clarifications: [], nativeCode: [] },
    ]);
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("exception crate:gone-crate");
    expect(failures[0]).toContain("no target");
  });

  it("fails a clarification no target uses, naming it", () => {
    const failures = unusedAcrossTargets(readCase("clarify-unused"), [
      { exceptions: [], clarifications: [], nativeCode: [] },
    ]);
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("clarification crate:gone-crate");
    expect(failures[0]).toContain("no target");
  });

  it("fails a native-code entry no target ships, naming it", () => {
    const failures = unusedAcrossTargets(readCase("native-unused"), [
      { exceptions: [], clarifications: [], nativeCode: ["baz-sys"] },
    ]);
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("native code");
    expect(failures[0]).toContain("gone-sys");
    expect(failures[0]).toContain("no target");
  });

  it("passes an entry that one of three targets uses", () => {
    const perTarget = [
      { exceptions: [], clarifications: [], nativeCode: [] },
      { exceptions: [], clarifications: [], nativeCode: ["gone-sys"] },
      { exceptions: [], clarifications: [], nativeCode: [] },
    ];
    expect(unusedAcrossTargets(readCase("native-unused"), perTarget)).toEqual([]);
  });
});
