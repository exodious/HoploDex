// @vitest-environment node
// The bundle's contents check (011 US3-3, 011 FR-017, 011 FR-018, 011 SC-005;
// 011 data-model.md "The bundle's contents", 011 research.md §13): the built
// app's `files/` holds exactly the allowed paths, plus what `flatpak
// build-finish` itself adds, and every library the binary needs is in the
// runtime. `contents.mjs`'s pure check takes the file list, `readelf -d`'s
// output and the runtime's library names; the fixtures under
// `fixtures/contents-*/` hold those three, each wrong in one way.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { checkContents } from "./contents.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));

const lines = (text: string) => text.split("\n").filter((line) => line.trim() !== "");

function check(name: string, extraFiles: string[] = []): string[] {
  const dir = path.join(HERE, "fixtures", `contents-${name}`);
  return checkContents({
    files: [...lines(fs.readFileSync(path.join(dir, "files.txt"), "utf8")), ...extraFiles],
    readelf: fs.readFileSync(path.join(dir, "readelf.txt"), "utf8"),
    runtimeLibraries: lines(fs.readFileSync(path.join(dir, "runtime-libraries.txt"), "utf8")),
  });
}

describe("011 US3-3 - the bundle holds what the contract lists, and nothing else", () => {
  it("passes the exact list plus what flatpak build-finish adds", () => {
    expect(check("ok")).toEqual([]);
  });

  it("passes the LICENSE flatpak-builder 1.4.8 records, which older builders leave out", () => {
    expect(check("ok", ["share/licenses/io.github.exodious.HoploDex/hoplodex/LICENSE"])).toEqual(
      [],
    );
  });

  it("011 FR-018: fails a shared library, naming it", () => {
    expect(check("shared-library")).toEqual([
      "lib/libfoo.so is a shared library in the bundle; the runtime provides every library (011 FR-018)",
    ]);
  });

  it("011 FR-018: fails a shared library under export/, naming it", () => {
    expect(check("stray-export")).toEqual([
      "export/lib/libbar.so is a shared library in the bundle; the runtime provides every library (011 FR-018)",
    ]);
  });

  it("011 FR-017: fails a stray file, naming it", () => {
    expect(check("stray-file")).toEqual([
      "share/doc/x is in the bundle but not in the allow list (011 FR-017)",
    ]);
  });

  it("011 FR-017: fails a missing icon size, naming it", () => {
    expect(check("missing-icon")).toEqual([
      "share/icons/hicolor/256x256/apps/io.github.exodious.HoploDex.png is missing from the bundle (011 FR-017)",
    ]);
  });

  it("011 FR-017: fails a missing NOTICES.txt, naming it", () => {
    expect(check("missing-notices")).toEqual([
      "share/licenses/io.github.exodious.HoploDex/NOTICES.txt is missing from the bundle (011 FR-017)",
    ]);
  });

  it("011 FR-018: fails a needed library the runtime lacks, naming it", () => {
    expect(check("unknown-needed")).toEqual([
      "bin/hoplodex needs libfoo.so.1, which the runtime does not have (011 FR-018)",
    ]);
  });

  it("011 FR-018: fails readelf output with no needed library at all", () => {
    expect(check("no-needed")).toEqual([
      "readelf -d printed no NEEDED library for bin/hoplodex, so its output was not understood (011 FR-018)",
    ]);
  });
});
