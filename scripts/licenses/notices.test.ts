// The notices generator: the components, notices.json, NOTICES.txt, the program
// block and the command line (010 research.md §5, §6, §10, §13; 010
// contracts/notices.md; 010 FR-008, FR-011, SC-001, SC-002): written first, red
// until notices.mjs exports what is used here. fixtures/README.md says what
// each fixture shows; no test but the last group needs cargo or the network.
//
// The API these tests assume of notices.mjs (every path and both command
// runners are parameters; the defaults, for the real repository, are the ones
// the build uses):
//
//   programBlock({ root?, git?, now? })
//     -> { name, version, copyrightYears, copyrightHolder, sourceUrl }
//        `root` holds package.json (default: the repository); `git(args)`
//        returns a git command's stdout, run in `root`, and throws or returns
//        "" when there is no git (default: the real git); `now` is a Date, the
//        build's own year when there is no git (default: the current time).
//        The year is from `git log -1 --format=%cs`.
//
//   buildNotices({ target, root, lockFile, modulesDir, licensesDir,
//                  licenseFile, cargo, git, now, openSslDir, allTargets?,
//                  development?, program? })
//     -> { document, failures: string[], warnings: string[] }
//        Never throws for a check that fails: every failure is a string in
//        `failures` (each as 010 contracts/notices.md "Failure messages" gives
//        it) and `document` still holds what was found. `lockFile` is the
//        npm lockfile, `modulesDir` stands for node_modules, `licenseFile` is
//        LICENSE; a `licenses/texts/...` path in a record resolves against
//        the folder above `licensesDir`. `cargo(args)` returns stdout, `target`
//        is a triple, `openSslDir` defaults to OPENSSL_DIR. `allTargets` is the
//        --all-targets check of one target (OpenSSL's version isn't read).
//        `development: true` adds `"development": true` to the document.
//
//   renderJson(document) -> string   (JSON.stringify(document, null, 2) + "\n")
//   renderText(document) -> string   (NOTICES.txt)
//
//   main(argv, deps) -> Promise<number>   (the exit code)
//        argv is process.argv.slice(2). deps holds everything buildNotices
//        takes except `target`, plus `env` (default: process.env), `hostTriple`
//        (the target when neither --target nor TAURI_ENV_TARGET_TRIPLE says),
//        and `stdout(text)`/`stderr(text)` (each call is whole lines, each
//        ending in "\n"). Failures go to stderr, one per line; the summary to
//        stdout. Exit codes: 0 all checks passed, 1 any failure or a bad
//        option.
//        --check [--target T]            "notices: T N components; 0 failures"
//        --check --all-targets           "notices: L N, W N, M N components; 0 failures"
//        --out <dir> [--target T]        writes notices.json and NOTICES.txt to <dir>
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, statSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { fakeCargo, PROJECT, TRIPLES } from "./fixtures/fakeCargo.mjs";
import { buildNotices, main, programBlock, renderJson, renderText } from "./notices.mjs";
import { isAllowed } from "./policy.mjs";
import { npmShipped, targetTriple } from "./shipped.mjs";
import { choose } from "./spdx.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const fixtures = path.join(here, "fixtures");
const repo = path.resolve(here, "..", "..");
const [LINUX, WINDOWS, MACOS] = TRIPLES;
const record = (name: string) => path.join(fixtures, "records", name, "licenses");

type Component = {
  name: string;
  version: string;
  kind: string;
  license: string;
  declared?: string;
  texts: string[];
  source: string;
  carrier?: { kind: string; name: string; version: string };
};
type Doc = {
  format: number;
  program: Record<string, string>;
  target: string;
  gpl: string;
  additionalPermission: string;
  components: Component[];
  texts: Record<string, string>;
  development?: boolean;
};
type Built = { document: Doc; failures: string[]; warnings: string[] };

/** The good fixture's inputs; `over` replaces some. */
function inputs(over: Record<string, unknown> = {}) {
  return {
    root: PROJECT,
    lockFile: path.join(PROJECT, "lock.json"),
    modulesDir: path.join(PROJECT, "modules"),
    licensesDir: path.join(PROJECT, "licenses"),
    licenseFile: path.join(PROJECT, "LICENSE"),
    cargo: fakeCargo(),
    git: () => "2026-10-18\n",
    now: new Date("2026-10-18T12:00:00Z"),
    openSslDir: path.join(fixtures, "openssl", "3.5.4"),
    ...over,
  };
}
const build = (target: string, over: Record<string, unknown> = {}) =>
  buildNotices({ target, ...inputs(over) }) as Built;
const good = (target = LINUX) => build(target).document;

const read = (...parts: string[]) => readFileSync(path.join(...parts), "utf8");
/** A committed text less its `Source: <url>` first line. */
const committed = (file: string) => read(file).split("\n").slice(1).join("\n");
const project = (...parts: string[]) => path.join(PROJECT, ...parts);
const pkgText = (name: string, file: string) => read(PROJECT, "modules", name, file);
const crateText = (name: string, version: string, file: string) =>
  read(PROJECT, "registry", `${name}-${version}`, file);

const find = (doc: Doc, name: string) => doc.components.find((c) => c.name === name);
const textsOf = (doc: Doc, c: Component | undefined) => (c?.texts ?? []).map((id) => doc.texts[id]);
const idOf = (c: Component) => `${c.name}@${c.version}`;
/** A document's components with each text id replaced by its text (ids differ between documents). */
const resolved = (doc: Doc) => doc.components.map((c) => ({ ...c, texts: textsOf(doc, c) }));
const sorted = (list: string[]) => [...list].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
const packaged = (doc: Doc) => doc.components.filter((c) => c.source !== "hand-recorded");

// What the good fixture ships (fixtures/README.md): the npm packages that are
// neither `dev` nor `devOptional`, the crates in each triple's recorded tree
// less `hoplodex`, and the hand-recorded entries for the triple's OS.
const NPM = [
  "@fontsource-variable/example-font@5.0.0",
  "@scope/shipped-scoped@2.0.0",
  "@zxcvbn-ts/language-common@4.1.3",
  "@zxcvbn-ts/language-en@4.1.1",
  "choice-pkg@1.0.0",
  "no-license-file@1.0.0",
  "shipped-mit@1.0.0",
  "spdx-only@1.0.0",
  "with-notice-pkg@1.0.0",
];
const CRATES = [
  "baz-sys@0.1.0",
  "both-in-one@0.4.42",
  "dual@1.0.0",
  "libsqlite3-sys@0.38.1",
  "licenses-dir@1.0.0",
  "needs-exception@1.0.0",
  "pointer-and-license@1.0.0",
  "single@1.0.0",
  "unplaceable@1.0.0",
  "with-notice@1.0.0",
];
const CRATES_BY_TARGET: Record<string, string[]> = {
  [LINUX]: [...CRATES, "linux-only@1.0.0"],
  [WINDOWS]: [...CRATES, "prebuilt-lib-sys@0.1.0", "webview2-com-sys@0.38.2", "windows-only@1.0.0"],
  [MACOS]: CRATES,
};
const HAND = [
  "EFF Large Wordlist for Passphrases",
  "OpenSubtitles 2024 word frequencies",
  "SQLCipher",
  "SQLite",
];
const HAND_BY_TARGET: Record<string, string[]> = {
  [LINUX]: HAND,
  [WINDOWS]: [...HAND, "Microsoft WebView2 loader", "OpenSSL"],
  [MACOS]: HAND,
};
const COUNT: Record<string, number> = Object.fromEntries(
  TRIPLES.map((t) => [t, NPM.length + CRATES_BY_TARGET[t].length + HAND_BY_TARGET[t].length]),
);

/** A cargo runner whose `cargo tree` leaves one crate out of the shipped tree (and nothing else). */
function withoutCrate(name: string) {
  const inner = fakeCargo();
  return (args: string[]) => {
    const out = inner(args);
    // The build-dependency query (`-e normal,build`) keeps its crates: only the shipped tree changes
    if (args[0] !== "tree" || args.some((a) => a.split(",").includes("build"))) return out;
    return out
      .split("\n")
      .filter((line: string) => !line.startsWith(`${name} `))
      .join("\n");
  };
}

describe("010 US2-1 - the components are exactly what the audit counts as shipped", () => {
  it.each(TRIPLES)("%s: every check passes on the good fixtures", (triple) => {
    const built = build(triple);
    expect(built.failures).toEqual([]);
  });

  it.each(TRIPLES)(
    "%s: the npm packages, the tree's crates and this OS's hand-recorded entries",
    (triple) => {
      const doc = good(triple);
      const npm = doc.components.filter((c) => ["npm", "font"].includes(c.kind));
      const crates = doc.components.filter((c) => c.kind === "crate");
      const hand = doc.components.filter((c) => c.source === "hand-recorded");
      expect(sorted(npm.map(idOf))).toEqual(sorted(NPM));
      expect(sorted(crates.map(idOf))).toEqual(sorted(CRATES_BY_TARGET[triple]));
      expect(sorted(hand.map((c) => c.name))).toEqual(sorted(HAND_BY_TARGET[triple]));
      expect(doc.components).toHaveLength(COUNT[triple]);
    },
  );

  it("takes the npm packages from npmShipped, so the audit and the notices share one set", () => {
    const lock = JSON.parse(read(PROJECT, "lock.json"));
    const shipped = npmShipped(lock).map((p: { name: string; version: string }) =>
      idOf(p as never),
    );
    const doc = good();
    expect(
      sorted(
        packaged(doc)
          .filter((c) => c.kind !== "crate")
          .map(idOf),
      ),
    ).toEqual(sorted(shipped));
  });

  it("lists no dev or devOptional package", () => {
    const names = good().components.map((c) => c.name);
    expect(names).not.toContain("dev-tool");
    expect(names).not.toContain("types-peer");
  });

  it("lists a crate only the triple ships in that triple's document alone", () => {
    expect(find(good(LINUX), "linux-only")).toBeDefined();
    expect(find(good(WINDOWS), "linux-only")).toBeUndefined();
    expect(find(good(MACOS), "linux-only")).toBeUndefined();
    expect(find(good(WINDOWS), "windows-only")).toBeDefined();
    expect(find(good(LINUX), "windows-only")).toBeUndefined();
    expect(find(good(MACOS), "windows-only")).toBeUndefined();
  });

  it("lists a crate the metadata knows but the tree doesn't ship in no document", () => {
    for (const triple of TRIPLES) {
      expect(find(good(triple), "extra")).toBeUndefined();
      expect(find(good(triple), "no-license")).toBeUndefined();
    }
  });

  it("lists a hand-recorded entry only on the operating systems it names", () => {
    expect(find(good(LINUX), "OpenSSL")).toBeUndefined();
    expect(find(good(MACOS), "Microsoft WebView2 loader")).toBeUndefined();
    expect(find(good(WINDOWS), "OpenSSL")).toBeDefined();
    expect(find(good(WINDOWS), "Microsoft WebView2 loader")).toBeDefined();
  });

  it("adding a crate to the tree adds its notice, and nothing else changes", () => {
    const base = good();
    const added = build(LINUX, { cargo: fakeCargo({ variant: "extra-crate" }) });
    expect(added.failures).toEqual([]);
    const extra = find(added.document, "extra");
    expect(extra).toMatchObject({ version: "1.0.0", kind: "crate", license: "MIT" });
    expect(textsOf(added.document, extra)).toEqual([crateText("extra", "1.0.0", "LICENSE")]);
    expect(resolved(added.document).filter((c) => c.name !== "extra")).toEqual(resolved(base));
    expect({ ...added.document, components: [], texts: {} }).toEqual({
      ...base,
      components: [],
      texts: {},
    });
  });

  it("removing a crate from the tree removes its notice, and nothing else changes", () => {
    const base = good();
    const removed = build(LINUX, { cargo: withoutCrate("single") });
    expect(removed.failures).toEqual([]);
    expect(find(removed.document, "single")).toBeUndefined();
    expect(resolved(removed.document)).toEqual(resolved(base).filter((c) => c.name !== "single"));
    expect({ ...removed.document, components: [], texts: {} }).toEqual({
      ...base,
      components: [],
      texts: {},
    });
  });

  it("adding a crate to another triple's tree leaves this triple's document alone", () => {
    const extra = fakeCargo({ variant: "extra-crate" });
    expect(resolved(build(MACOS, { cargo: extra }).document)).toEqual(resolved(good(MACOS)));
  });

  it("fails a path package other than hoplodex, naming it", () => {
    const built = build(LINUX, { cargo: fakeCargo({ variant: "path-package" }) });
    expect(built.failures.some((f) => f.includes("rogue"))).toBe(true);
  });
});

describe("010 US2-2 - a failed check names the component and every failure is collected", () => {
  it("fails a license the policy doesn't allow, in the contract's words", () => {
    const built = build(LINUX, { cargo: fakeCargo({ variant: "gpl-crate" }) });
    expect(built.failures).toContain(
      "license: crate quux 2.0.0 is under GPL-2.0-only, which isn't allowed.",
    );
  });

  it("fails a crate with no text for its license and no exception, naming the crate and triple", () => {
    const built = build(LINUX, { licensesDir: record("native-unlisted") });
    expect(built.failures).toContainEqual(
      expect.stringMatching(
        /^crate needs-exception 1\.0\.0 \(x86_64-unknown-linux-gnu\): no license text for MIT/,
      ),
    );
    expect(built.failures).toContainEqual(
      expect.stringMatching(/^npm spdx-only 1\.0\.0: no license text for MIT/),
    );
  });

  it("passes the records' failures through word for word, and lists every one", () => {
    const built = build(LINUX, { licensesDir: record("native-unlisted") });
    const native = (crate: string) =>
      `native code: crate ${crate} 0.1.0 (${LINUX}) builds C (cc) and isn't in licenses/native-code.json. Review what it compiles or links and record it.`;
    expect(built.failures).toContain(native("baz-sys"));
    expect(
      built.failures.some((f) => f.startsWith("native code: crate libsqlite3-sys 0.38.1")),
    ).toBe(true);
    expect(built.failures.length).toBeGreaterThanOrEqual(4);
    expect(built.failures.every((f) => !f.includes("\n"))).toBe(true);
  });

  it("still returns the document it could make, for the development server", () => {
    const built = build(LINUX, { licensesDir: record("native-unlisted") });
    expect(find(built.document, "shipped-mit")).toBeDefined();
    expect(find(built.document, "single")).toBeDefined();
  });

  it("fails a hand-recorded entry whose carrier changed version", () => {
    const built = build(LINUX, { cargo: fakeCargo({ variant: "carrier-0.39.0" }) });
    expect(built.failures).toContain(
      'hand-recorded "SQLCipher": its carrier libsqlite3-sys is 0.39.0, recorded 0.38.1. Review the entry (its version, text and carrier) and update it.',
    );
  });

  it("fails a hand-recorded entry whose carrier lacks its feature", () => {
    const built = build(LINUX, { cargo: fakeCargo({ variant: "no-sqlcipher-feature" }) });
    expect(built.failures).toContain(
      'hand-recorded "SQLCipher": carrier libsqlite3-sys is shipped without feature bundled-sqlcipher.',
    );
  });

  it("fails a Windows build that links OpenSSL 1.1, in the contract's words", () => {
    const built = build(WINDOWS, { openSslDir: path.join(fixtures, "openssl", "1.1.1w") });
    expect(built.failures).toContainEqual(
      expect.stringContaining(
        'hand-recorded "OpenSSL": the Windows build links OpenSSL 1.1.1w; HoploDex needs OpenSSL 3',
      ),
    );
  });

  it("fails a Windows build with no OPENSSL_DIR, naming it", () => {
    const built = build(WINDOWS, { openSslDir: undefined });
    expect(built.failures.some((f) => f.includes("OPENSSL_DIR"))).toBe(true);
  });

  it("reads the OpenSSL version from the build's headers on Windows", () => {
    const openssl = find(good(WINDOWS), "OpenSSL");
    expect(openssl?.version).toBe("3.5.4");
  });

  it("under --all-targets reads no OpenSSL version and doesn't need OPENSSL_DIR", () => {
    const built = build(WINDOWS, { openSslDir: undefined, allTargets: true });
    expect(built.failures).toEqual([]);
    expect(find(built.document, "OpenSSL")?.version).toBe(
      "3.x (the version the Windows build links)",
    );
  });
});

describe("010 FR-011 - notices.json", () => {
  const COMPONENT_KEYS = [
    "name",
    "version",
    "kind",
    "license",
    "declared",
    "texts",
    "source",
    "carrier",
  ];

  it("is format 1, with its keys in the contract's order", () => {
    const doc = good();
    expect(doc.format).toBe(1);
    expect(Object.keys(doc)).toEqual([
      "format",
      "program",
      "target",
      "gpl",
      "additionalPermission",
      "components",
      "texts",
    ]);
    expect(Object.keys(doc.program)).toEqual([
      "name",
      "version",
      "copyrightYears",
      "copyrightHolder",
      "sourceUrl",
    ]);
    for (const c of doc.components) {
      expect(Object.keys(c), idOf(c)).toEqual(COMPONENT_KEYS.filter((k) => k in c));
    }
  });

  it("holds the target, the GPL text and the additional permission as they are in their files", () => {
    const doc = good(WINDOWS);
    expect(doc.target).toBe(WINDOWS);
    expect(doc.gpl).toBe(read(PROJECT, "LICENSE"));
    expect(doc.additionalPermission).toBe(read(PROJECT, "licenses", "additional-permission.txt"));
  });

  it("is the document written with two-space indent and a final newline", () => {
    const doc = good();
    const json = renderJson(doc) as string;
    expect(json).toBe(`${JSON.stringify(doc, null, 2)}\n`);
    expect(json.endsWith("}\n")).toBe(true);
    expect(json).not.toContain("\r");
    expect(JSON.parse(json)).toEqual(doc);
  });

  it("is byte-identical across two runs over the same inputs", () => {
    const a = renderJson(build(LINUX).document);
    const b = renderJson(build(LINUX, { cargo: fakeCargo() }).document);
    expect(a).toBe(b);
    expect(renderText(build(WINDOWS).document)).toBe(renderText(build(WINDOWS).document));
  });

  it("orders the components by name in lower case, then version", () => {
    const names = good(WINDOWS).components.map((c) => [c.name.toLowerCase(), c.version]);
    const expected = [...names].sort((a, b) =>
      a[0] !== b[0] ? (a[0] < b[0] ? -1 : 1) : a[1] < b[1] ? -1 : a[1] > b[1] ? 1 : 0,
    );
    expect(names).toEqual(expected);
    expect(good().components[0].name).toBe("@fontsource-variable/example-font");
  });

  it("stores each distinct text once, every id is used and every referenced id exists", () => {
    const doc = good(WINDOWS);
    const used = doc.components.flatMap((c) => c.texts);
    expect(Object.keys(doc.texts).sort()).toEqual([...new Set(used)].sort());
    const values = Object.values(doc.texts);
    expect(new Set(values).size).toBe(values.length);
    expect(values.every((t) => typeof t === "string" && t.length > 0)).toBe(true);
    // The fixtures share some texts between components (the same MIT or Apache text)
    expect(used.length).toBeGreaterThan(values.length);
  });

  it("gives every component at least one text", () => {
    for (const c of good(WINDOWS).components) expect(c.texts.length, idOf(c)).toBeGreaterThan(0);
  });

  it("names a choice's chosen license, with `declared` only for a choice", () => {
    const lock = JSON.parse(read(PROJECT, "lock.json")).packages;
    const metadata = JSON.parse(read(fixtures, "cargo", "metadata.json")).packages;
    const declaredOf = new Map<string, string>();
    for (const c of metadata) declaredOf.set(`crate:${c.name}`, c.license);
    for (const [location, p] of Object.entries<{ license: string }>(lock)) {
      if (location) declaredOf.set(`npm:${location.replace(/^node_modules\//, "")}`, p.license);
    }
    const doc = good();
    let choices = 0;
    for (const c of packaged(doc)) {
      const declared = declaredOf.get(`${c.kind === "crate" ? "crate" : "npm"}:${c.name}`)!;
      const chosen = choose(declared, (l: string) => isAllowed(l, { package: c.name }));
      expect(c.license, idOf(c)).toBe(chosen);
      if (declared.includes(" OR ")) {
        choices++;
        expect(c.declared, idOf(c)).toBe(declared);
      } else {
        expect("declared" in c, idOf(c)).toBe(false);
      }
    }
    expect(choices).toBeGreaterThanOrEqual(4);
    expect(find(doc, "choice-pkg")).toMatchObject({
      license: "MIT",
      declared: "Apache-2.0 OR MIT",
    });
    expect(find(doc, "dual")).toMatchObject({ license: "MIT", declared: "MIT OR Apache-2.0" });
  });

  it("says where each text came from", () => {
    const doc = good();
    const source = (name: string) => find(doc, name)?.source;
    expect(source("shipped-mit")).toBe("package");
    expect(source("single")).toBe("package");
    expect(source("needs-exception")).toBe("exception");
    expect(source("no-license-file")).toBe("exception");
    expect(source("spdx-only")).toBe("exception");
    expect(source("unplaceable")).toBe("clarified");
    expect(source("@fontsource-variable/example-font")).toBe("clarified");
    for (const name of HAND) expect(source(name), name).toBe("hand-recorded");
    for (const c of doc.components) {
      expect(["package", "exception", "clarified", "hand-recorded"]).toContain(c.source);
    }
  });

  it("calls an @fontsource package a font, a crate a crate, and a hand-recorded entry its recorded kind", () => {
    const doc = good(WINDOWS);
    expect(find(doc, "@fontsource-variable/example-font")?.kind).toBe("font");
    expect(find(doc, "shipped-mit")?.kind).toBe("npm");
    expect(find(doc, "single")?.kind).toBe("crate");
    expect(find(doc, "SQLCipher")?.kind).toBe("compiled-in");
    expect(find(doc, "OpenSSL")?.kind).toBe("linked");
    expect(find(doc, "EFF Large Wordlist for Passphrases")?.kind).toBe("data");
  });

  it("has a carrier on a hand-recorded component and nowhere else", () => {
    const doc = good(WINDOWS);
    for (const c of doc.components) {
      expect("carrier" in c, idOf(c)).toBe(c.source === "hand-recorded");
    }
    expect(find(doc, "SQLCipher")?.carrier).toEqual({
      kind: "crate",
      name: "libsqlite3-sys",
      version: "0.38.1",
    });
    expect(find(doc, "OpenSubtitles 2024 word frequencies")?.carrier).toEqual({
      kind: "npm",
      name: "@zxcvbn-ts/language-en",
      version: "4.1.1",
    });
  });

  it("gives a package its license file, whole and verbatim", () => {
    const doc = good();
    expect(textsOf(doc, find(doc, "shipped-mit"))).toEqual([pkgText("shipped-mit", "license")]);
    expect(textsOf(doc, find(doc, "single"))).toEqual([crateText("single", "1.0.0", "LICENSE")]);
  });

  it("gives a choice only the chosen license's text", () => {
    const doc = good();
    expect(textsOf(doc, find(doc, "choice-pkg"))).toEqual([pkgText("choice-pkg", "LICENSE_MIT")]);
    expect(textsOf(doc, find(doc, "dual"))).toEqual([crateText("dual", "1.0.0", "LICENSE-MIT")]);
  });

  it("puts the license text first, then the NOTICE text", () => {
    const doc = good();
    expect(textsOf(doc, find(doc, "with-notice-pkg"))).toEqual([
      pkgText("with-notice-pkg", "LICENSE"),
      pkgText("with-notice-pkg", "NOTICE.md"),
    ]);
    const crate = textsOf(doc, find(doc, "with-notice"));
    expect(crate[0]).toBe(crateText("with-notice", "1.0.0", "LICENSE"));
    expect(crate).toContain(crateText("with-notice", "1.0.0", "NOTICE"));
  });

  it("gives an exception's committed text, less its Source line", () => {
    const doc = good();
    expect(textsOf(doc, find(doc, "needs-exception"))).toEqual([
      committed(project("licenses", "texts", "needs-exception", "LICENSE.txt")),
    ]);
    expect(textsOf(doc, find(doc, "spdx-only"))).toEqual([
      committed(project("licenses", "texts", "spdx-only", "LICENSE.txt")),
    ]);
  });

  it("gives a clarification's files in place of what the matcher finds, in the record's order", () => {
    const doc = good();
    expect(textsOf(doc, find(doc, "unplaceable"))).toEqual([
      crateText("unplaceable", "1.0.0", "LICENSE"),
    ]);
    expect(textsOf(doc, find(doc, "@fontsource-variable/example-font"))).toEqual([
      "Copyright 2020 The Example Font Project Authors (https://example.invalid/fonts)",
      committed(project("licenses", "texts", "example-font", "OFL.txt")),
    ]);
  });

  it("gives a hand-recorded entry its texts in the record's order", () => {
    const doc = good(WINDOWS);
    const language = "@zxcvbn-ts/language-en";
    expect(textsOf(doc, find(doc, "OpenSubtitles 2024 word frequencies"))).toEqual([
      pkgText(language, "NOTICE.md"),
      pkgText(language, "THIRD_PARTY_LICENSES.md"),
    ]);
    expect(textsOf(doc, find(doc, "SQLCipher"))).toEqual([
      crateText("libsqlite3-sys", "0.38.1", "sqlcipher/LICENSE"),
    ]);
    expect(textsOf(doc, find(doc, "SQLite"))).toEqual([
      committed(project("licenses", "texts", "sqlite", "blessing.txt")),
    ]);
    expect(find(doc, "SQLite")?.license).toBe("blessing");
    expect(textsOf(doc, find(doc, "OpenSSL"))).toEqual([
      committed(project("licenses", "texts", "openssl", "LICENSE.txt")),
    ]);
  });

  it("adds `development: true` last, and only when asked", () => {
    expect("development" in good()).toBe(false);
    const doc = build(LINUX, { development: true }).document;
    expect(doc.development).toBe(true);
    expect(Object.keys(doc).at(-1)).toBe("development");
    expect(JSON.parse(renderJson(doc) as string).development).toBe(true);
  });

  it("holds the program block it was given, the same one programBlock makes", () => {
    const doc = good();
    expect(doc.program).toEqual(programBlock({ root: PROJECT, git: () => "2026-10-18\n" }));
  });
});

describe("010 FR-002 - the program block", () => {
  const block = (git: (args: string[]) => string, now = new Date("2026-10-18T12:00:00Z")) =>
    programBlock({ root: PROJECT, git, now }) as Record<string, string>;

  it("takes the name, version, holder and source address", () => {
    const program = block(() => "2026-10-18\n");
    expect(Object.keys(program)).toEqual([
      "name",
      "version",
      "copyrightYears",
      "copyrightHolder",
      "sourceUrl",
    ]);
    expect(program.name).toBe("HoploDex");
    expect(program.version).toBe(JSON.parse(read(PROJECT, "package.json")).version);
    expect(program.copyrightHolder).toBe("exodious and the HoploDex contributors");
    // package.json says git+https://github.com/exodious/HoploDex.git
    expect(program.sourceUrl).toBe("https://github.com/exodious/HoploDex");
  });

  it("gives 2026 alone for a commit made in 2026", () => {
    expect(block(() => "2026-12-31\n").copyrightYears).toBe("2026");
  });

  it("gives 2026 to the commit's year for a later commit", () => {
    expect(block(() => "2028-01-02\n").copyrightYears).toBe("2026–2028");
  });

  it("asks git for the last commit's date, in the repository root", () => {
    const calls: string[][] = [];
    block((args) => {
      calls.push(args);
      return "2026-10-18\n";
    });
    expect(calls).toHaveLength(1);
    expect(calls[0]).toContain("log");
    expect(calls[0]).toContain("--format=%cs");
  });

  it("uses the build's own year when git throws", () => {
    const noGit = () => {
      throw new Error("not a git repository");
    };
    expect(block(noGit, new Date("2027-05-05T00:00:00Z")).copyrightYears).toBe("2026–2027");
    expect(block(noGit, new Date("2026-05-05T00:00:00Z")).copyrightYears).toBe("2026");
  });

  it("uses the build's own year when git prints nothing", () => {
    expect(block(() => "", new Date("2030-02-02T00:00:00Z")).copyrightYears).toBe("2026–2030");
  });

  it("reads the real package.json by default", () => {
    const program = programBlock() as Record<string, string>;
    const pkg = JSON.parse(readFileSync(path.join(repo, "package.json"), "utf8"));
    expect(program.name).toBe("HoploDex");
    expect(program.version).toBe(pkg.version);
    expect(program.sourceUrl).toBe("https://github.com/exodious/HoploDex");
    expect(program.copyrightYears).toMatch(/^2026(–\d{4})?$/);
  });
});

describe("010 US2-5 - NOTICES.txt", () => {
  const RULE = "=".repeat(78);
  const DASHES = "-".repeat(78);
  const STATEMENT = [
    "HoploDex is free software: you can redistribute it and/or modify it under the",
    "terms of the GNU General Public License as published by the Free Software",
    "Foundation, version 3 only. It is distributed in the hope that it will be",
    "useful, but WITHOUT ANY WARRANTY, to the extent permitted by law; without even",
    "the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.",
    "See the GNU General Public License below for more details.",
  ].join("\n");
  const trimEnd = (text: string) => text.replace(/\n+$/, "");

  /** contracts/notices.md's layout, built from the document: sections are separated by one blank line. */
  function expected(doc: Doc) {
    const p = doc.program;
    const heading = (c: Component) =>
      c.declared === undefined
        ? `${c.name} ${c.version}  (${c.license})`
        : `${c.name} ${c.version}  (${c.license}, used under ${c.license} of ${c.declared})`;
    const sections = [
      `${p.name} ${p.version}\nCopyright (C) ${p.copyrightYears} ${p.copyrightHolder}`,
      STATEMENT,
      `Source code: ${p.sourceUrl}`,
      trimEnd(doc.additionalPermission),
      `This copy was built for ${doc.target}.`,
      `${RULE}\nThird-party components\n${RULE}`,
      ...doc.components.map(
        (c) =>
          `${DASHES}\n${heading(c)}\n${DASHES}\n${c.texts.map((id) => trimEnd(doc.texts[id])).join("\n\n")}`,
      ),
      `${RULE}\nGNU General Public License, version 3\n${RULE}`,
      trimEnd(doc.gpl),
    ];
    return `${sections.join("\n\n")}\n`;
  }

  it.each(TRIPLES)("%s: follows the contract's layout line for line", (triple) => {
    const doc = good(triple);
    expect(renderText(doc)).toBe(expected(doc));
  });

  it("begins with the program, its copyright and the GPL statement, and ends with LICENSE", () => {
    const text = renderText(good()) as string;
    expect(
      text.startsWith(
        "HoploDex 0.1.0\nCopyright (C) 2026 exodious and the HoploDex contributors\n\n",
      ),
    ).toBe(true);
    expect(text).toContain(
      "\n\nSource code: https://github.com/exodious/HoploDex\n\nAdditional permission under GNU GPL version 3 section 7\n",
    );
    expect(text).toContain(`\n\nThis copy was built for ${LINUX}.\n\n`);
    expect(text.endsWith(`${trimEnd(read(PROJECT, "LICENSE"))}\n`)).toBe(true);
  });

  it("names a choice as 'used under' its chosen license of the declared expression", () => {
    const text = renderText(good()) as string;
    expect(text).toContain("\nchoice-pkg 1.0.0  (MIT, used under MIT of Apache-2.0 OR MIT)\n");
    expect(text).toContain("\nshipped-mit 1.0.0  (MIT)\n");
  });

  it("repeats a text in each component that uses it", () => {
    const doc = good(WINDOWS);
    const text = renderText(doc) as string;
    const users = new Map<string, number>();
    for (const c of doc.components) {
      for (const id of new Set(c.texts)) users.set(id, (users.get(id) ?? 0) + 1);
    }
    let shared = 0;
    for (const [id, count] of users) {
      const body = trimEnd(doc.texts[id]);
      expect(text.split(body).length - 1, body.slice(0, 40)).toBeGreaterThanOrEqual(count);
      if (count > 1) shared++;
    }
    expect(shared).toBeGreaterThan(0);
  });

  it("is UTF-8 with LF line ends and no byte-order mark", () => {
    const text = renderText(good(WINDOWS)) as string;
    expect(text).not.toContain("\r");
    expect(text.charCodeAt(0)).not.toBe(0xfeff);
    expect(Buffer.from(text, "utf8").toString("utf8")).toBe(text);
  });

  it("wraps what it writes itself at 80 columns", () => {
    const lines = (renderText(good(WINDOWS)) as string).split("\n");
    const own = lines.filter(
      (l) => l === STATEMENT.split("\n").find((s) => s === l) || /^[-=]+$/.test(l),
    );
    expect(own.length).toBeGreaterThan(0);
    for (const line of own) expect(line.length).toBeLessThanOrEqual(80);
  });
});

describe("010 US2-5 - the command line", () => {
  function run(argv: string[], over: Record<string, unknown> = {}) {
    const out: string[] = [];
    const err: string[] = [];
    const deps = {
      ...inputs(),
      env: {},
      hostTriple: LINUX,
      stdout: (text: string) => out.push(text),
      stderr: (text: string) => err.push(text),
      ...over,
    };
    return (main(argv, deps) as Promise<number>).then((code) => ({
      code,
      out: out.join(""),
      err: err.join(""),
    }));
  }
  const listing = (dir: string): string[] =>
    readdirSync(dir, { recursive: true, withFileTypes: true })
      .filter((e) => e.isFile())
      .map((e) => {
        const file = path.join(e.parentPath, e.name);
        return `${file} ${statSync(file).mtimeMs} ${statSync(file).size}`;
      })
      .sort();
  const tmp = () => mkdtempSync(path.join(os.tmpdir(), "hoplodex-notices-"));

  it("--check passes on the good fixtures, says so and writes nothing", async () => {
    const before = listing(PROJECT);
    const result = await run(["--check", "--target", LINUX]);
    expect(result.code).toBe(0);
    expect(result.err).toBe("");
    expect(result.out).toBe(`notices: ${LINUX} ${COUNT[LINUX]} components; 0 failures\n`);
    expect(listing(PROJECT)).toEqual(before);
  });

  it("--check exits 1 on any failure and prints every failure, one per line", async () => {
    const licensesDir = record("native-unlisted");
    const failures = build(LINUX, { licensesDir }).failures;
    expect(failures.length).toBeGreaterThan(1);
    const before = listing(PROJECT);
    const result = await run(["--check", "--target", LINUX], { licensesDir });
    expect(result.code).toBe(1);
    const lines = result.err.split("\n");
    for (const failure of failures) expect(lines, failure).toContain(failure);
    expect(result.out).not.toContain("0 failures");
    expect(listing(PROJECT)).toEqual(before);
  });

  it("--out writes notices.json and NOTICES.txt, the renderings of the same build", async () => {
    const dir = tmp();
    const result = await run(["--out", dir, "--target", WINDOWS]);
    expect(result.code).toBe(0);
    const doc = build(WINDOWS).document;
    expect(readdirSync(dir).sort()).toEqual(["NOTICES.txt", "notices.json"]);
    expect(read(dir, "notices.json")).toBe(renderJson(doc));
    expect(read(dir, "NOTICES.txt")).toBe(renderText(doc));
  });

  it("--out exits 1 on a failure, as --check does", async () => {
    const result = await run(["--out", tmp(), "--target", LINUX], {
      licensesDir: record("native-unlisted"),
    });
    expect(result.code).toBe(1);
    expect(result.err).toContain("native code: crate baz-sys 0.1.0");
  });

  it("takes the target from --target, else TAURI_ENV_TARGET_TRIPLE, else the host's triple", async () => {
    const builtFor = async (argv: string[], env: Record<string, string>) => {
      const dir = tmp();
      const result = await run(["--out", dir, ...argv], { env });
      expect(result.code).toBe(0);
      return JSON.parse(read(dir, "notices.json")).target;
    };
    expect(await builtFor([], {})).toBe(LINUX);
    expect(await builtFor([], { TAURI_ENV_TARGET_TRIPLE: MACOS })).toBe(MACOS);
    expect(await builtFor(["--target", WINDOWS], { TAURI_ENV_TARGET_TRIPLE: MACOS })).toBe(WINDOWS);
  });

  it("reads OPENSSL_DIR from the environment when it isn't given", async () => {
    const env = { OPENSSL_DIR: path.join(fixtures, "openssl", "1.1.1w") };
    const result = await run(["--check", "--target", WINDOWS], { env, openSslDir: undefined });
    expect(result.code).toBe(1);
    expect(result.err).toContain("OpenSSL 1.1.1w");
  });

  it("--check --all-targets checks the three triples and prints the counts", async () => {
    const result = await run(["--check", "--all-targets"], { openSslDir: undefined });
    expect(result.err).toBe("");
    expect(result.code).toBe(0);
    expect(result.out).toBe(
      `notices: ${LINUX} ${COUNT[LINUX]}, ${WINDOWS} ${COUNT[WINDOWS]}, ${MACOS} ${COUNT[MACOS]} components; 0 failures\n`,
    );
  });

  it("--all-targets fails an entry no target uses", async () => {
    for (const [name, text] of [
      ["exception-unused", "exception crate:gone-crate"],
      ["clarify-unused", "clarification crate:gone-crate"],
      ["native-unused", "gone-sys"],
    ]) {
      const result = await run(["--check", "--all-targets"], {
        licensesDir: record(name),
        openSslDir: undefined,
      });
      expect(result.code, name).toBe(1);
      const line = result.err.split("\n").find((l) => l.includes(text));
      expect(line, name).toContain("no target");
    }
  });

  it("a single-target --check leaves an entry no target uses alone", async () => {
    const result = await run(["--check", "--target", LINUX], {
      licensesDir: record("exception-unused"),
    });
    expect(result.err).not.toContain("gone-crate");
  });

  it("fails an option it doesn't know, naming it", async () => {
    const result = await run(["--check", "--frobnicate"]);
    expect(result.code).toBe(1);
    expect(result.err).toContain("--frobnicate");
  });
});

// The host's own trees, with the real cargo: each OS's test run checks its own
// notices (010 research.md §14). The records and texts are T021's, so this
// group is red until they are written.
const hasCargo = spawnSync("cargo", ["--version"], { stdio: "ignore" }).status === 0;
if (!hasCargo) {
  console.warn("notices.test: skipping the real-tree tests; cargo isn't on the PATH");
}

describe.skipIf(!hasCargo)("010 SC-001 - the host's notices against the real trees", () => {
  const triple = targetTriple() as string;
  const os = triple.includes("windows") ? "windows" : triple.includes("apple") ? "macos" : "linux";
  let built: Built;
  const real = () => (built ??= buildNotices({ target: triple }) as Built);

  it("passes every check for the host's target", { timeout: 300_000 }, () => {
    expect(real().failures).toEqual([]);
  });

  it("lists exactly the audit's shipped set", { timeout: 300_000 }, () => {
    const doc = real().document;
    const lock = JSON.parse(readFileSync(path.join(repo, "package-lock.json"), "utf8"));
    const npm = doc.components.filter((c) => ["npm", "font"].includes(c.kind));
    expect(sorted(npm.map(idOf))).toEqual(
      sorted(npmShipped(lock).map((p: { name: string; version: string }) => idOf(p as never))),
    );

    const tree = execFileSync(
      "cargo",
      [
        "tree",
        "--locked",
        "--manifest-path",
        "src-tauri/Cargo.toml",
        "-e",
        "normal,no-proc-macro",
        "--target",
        triple,
        "--features",
        "custom-protocol",
        "--prefix",
        "none",
        "-f",
        "{p}",
      ],
      { cwd: repo, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
    );
    const crates = new Set<string>();
    for (const line of tree.split("\n")) {
      const m = /^(\S+) v(\S+)/.exec(line);
      if (m && m[1] !== "hoplodex") crates.add(`${m[1]}@${m[2]}`);
    }
    const listed = doc.components.filter((c) => c.kind === "crate").map(idOf);
    expect(sorted(listed)).toEqual(sorted([...crates]));

    const records = JSON.parse(
      readFileSync(path.join(repo, "licenses", "hand-recorded.json"), "utf8"),
    );
    const hand = records
      .filter((e: { os: string[] }) => e.os.includes(os))
      .map((e: { name: string }) => e.name);
    expect(
      sorted(doc.components.filter((c) => c.source === "hand-recorded").map((c) => c.name)),
    ).toEqual(sorted(hand));
  });

  it(
    "carries the OpenSubtitles NOTICE.md word for word (010 US1-5, 010 FR-016)",
    { timeout: 300_000 },
    () => {
      const doc = real().document;
      const entry = find(doc, "OpenSubtitles 2024 word frequencies");
      const notice = readFileSync(
        path.join(repo, "node_modules", "@zxcvbn-ts", "language-en", "NOTICE.md"),
        "utf8",
      );
      expect(textsOf(doc, entry)).toContain(notice);
    },
  );

  it("writes a real NOTICES.txt (010 US2-5, 010 FR-011, 010 FR-019)", { timeout: 300_000 }, () => {
    const doc = real().document;
    const text = renderText(doc) as string;
    const version = JSON.parse(readFileSync(path.join(repo, "package.json"), "utf8")).version;
    expect(text).toMatch(
      new RegExp(
        `^HoploDex ${version.replaceAll(".", "\\.")}\\nCopyright \\(C\\) 2026(–\\d{4})? exodious and the HoploDex contributors\\n`,
      ),
    );
    const permission = readFileSync(
      path.join(repo, "licenses", "additional-permission.txt"),
      "utf8",
    );
    expect(text).toContain(
      `Source code: https://github.com/exodious/HoploDex\n\n${permission.replace(/\n+$/, "")}\n\n`,
    );
    // research.md §2's CC BY 4.0 attribution of the EFF list, word for word (line breaks aside)
    const attribution =
      'EFF Large Wordlist for Passphrases, by Joseph Bonneau for the Electronic Frontier Foundation (https://www.eff.org/deeplinks/2016/07/new-wordlists-random-passphrases), licensed under the Creative Commons Attribution 4.0 International License (https://creativecommons.org/licenses/by/4.0/). Changed: the dice numbers are removed, the hyphens are removed from "drop-down", "felt-tip" and "t-shirt", and "yo-yo" is replaced by "cheetah" (by github.com/dmuth/diceware, Apache-2.0).';
    expect(text.replace(/\s+/g, " ")).toContain(attribution);
    const license = readFileSync(path.join(repo, "LICENSE"), "utf8");
    expect(text.endsWith(`${license.replace(/\n+$/, "")}\n`)).toBe(true);
  });
});
