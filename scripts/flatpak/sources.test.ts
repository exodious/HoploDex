// @vitest-environment node
// The offline source lists against the lockfiles (011 US3-2, 011 FR-015,
// 011 SC-004; 011 data-model.md "Offline source lists"). `npm test` runs it, so
// a lockfile change without `npm run flatpak:sources` fails every test run, not
// only the Flatpak build. It needs no Python and no network. The comparison is
// `sources.mjs`; the fixtures under `fixtures/sources-*/` are tiny lockfiles
// and lists, each wrong in one way.
import crypto from "node:crypto";
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
    it("passes, the generators' inline and script entries and a link entry included", () => {
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
