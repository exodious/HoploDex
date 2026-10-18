// Finding a package's or crate's license, NOTICE and attribution files
// (010 research.md §5, §6; 010 FR-010): written first, red until licenseFiles.mjs
// exports findTexts. The folders are scripts/licenses/fixtures/project/registry
// (crates) and .../modules (npm packages); fixtures/README.md says what each shows.
//
// The API these tests assume:
//   findTexts(folder, chosen, { declared, label })
//     -> { texts: [{ file, text }], failure: string | null }
//   `chosen` is the one license the notice names (policy `choose()`), `declared`
//   the package's own expression, `label` the failure message's subject
//   ("crate foo 1.2.0", "crate dlopen2 0.8.2 (x86_64-unknown-linux-gnu)").
//   `file` is the path inside the folder with forward slashes. On a failure
//   `texts` is empty. There is no fallback text.
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { findTexts, findTextsAmong } from "./licenseFiles.mjs";

const project = path.join(path.dirname(fileURLToPath(import.meta.url)), "fixtures", "project");
const crate = (name: string, version = "1.0.0") =>
  path.join(project, "registry", `${name}-${version}`);
const pkg = (name: string) => path.join(project, "modules", name);

type Found = { texts: { file: string; text: string }[]; failure: string | null };

function find(folder: string, chosen: string, declared = chosen, label = "crate fixture 1.0.0") {
  return findTexts(folder, chosen, { declared, label }) as Found;
}
const files = (found: Found) => found.texts.map((t) => t.file);
const content = (folder: string, file: string) => readFileSync(path.join(folder, file), "utf8");

describe("010 research.md §5 - a crate with one license gets every license file", () => {
  it("takes LICENSE, whole and verbatim", () => {
    const found = find(crate("single"), "MIT");
    expect(found.failure).toBeNull();
    expect(files(found)).toEqual(["LICENSE"]);
    expect(found.texts[0].text).toBe(content(crate("single"), "LICENSE"));
  });

  it("matches the names without regard to case (a package's `license` file)", () => {
    expect(files(find(pkg("shipped-mit"), "MIT"))).toEqual(["license"]);
  });

  it("matches an extension (LICENSE.md) and a scoped package's folder", () => {
    expect(files(find(pkg("@scope/shipped-scoped"), "MIT"))).toEqual(["LICENSE.md"]);
  });

  it("keeps every part of a conjunction (AND)", () => {
    const found = find(crate("dual"), "MIT AND Apache-2.0", "MIT AND Apache-2.0");
    expect(found.failure).toBeNull();
    expect(files(found).sort()).toEqual(["LICENSE-APACHE", "LICENSE-MIT"]);
  });

  it("counts every file in a LICENSES/ or license/ folder", () => {
    expect(files(find(crate("license-folder"), "MIT"))).toEqual(["license/MIT.txt"]);
  });

  it("takes the font package's LICENSE with its copyright line", () => {
    const found = find(pkg("@fontsource-variable/example-font"), "OFL-1.1");
    expect(files(found)).toEqual(["LICENSE"]);
    expect(found.texts[0].text).toContain("Copyright 2020 The Example Font Project Authors");
  });
});

describe("010 research.md §5 - NOTICE and attribution files", () => {
  it("keeps a NOTICE after the license text, then COPYRIGHT and AUTHORS", () => {
    const found = find(crate("with-notice"), "Apache-2.0");
    expect(files(found)).toEqual(["LICENSE", "NOTICE", "AUTHORS"]);
    expect(found.texts[1].text).toBe(content(crate("with-notice"), "NOTICE"));
  });

  it("keeps a NOTICE.md of an npm package", () => {
    expect(files(find(pkg("with-notice-pkg"), "Apache-2.0"))).toEqual(["LICENSE", "NOTICE.md"]);
  });

  it("keeps a NOTICE in a crate with a choice too, whichever license is chosen", () => {
    const dotted = crate("dotted");
    expect(files(find(dotted, "MIT", "Apache-2.0 OR MIT"))).toEqual(["LICENSE.MIT", "NOTICE.txt"]);
    expect(files(find(dotted, "Apache-2.0", "Apache-2.0 OR MIT"))).toEqual([
      "LICENSE.Apache-2.0",
      "NOTICE.txt",
    ]);
  });

  it("doesn't count AUTHORS without a license file beside it", () => {
    const found = find(crate("authors-only"), "MIT", "MIT", "crate authors-only 1.0.0");
    expect(found.texts).toEqual([]);
    expect(found.failure).toMatch(/^crate authors-only 1\.0\.0: no license text for MIT/);
  });
});

describe("010 research.md §5 - a crate with a choice gets only the chosen license's files", () => {
  it("places a file by its name (LICENSE-MIT, LICENSE-APACHE)", () => {
    const dual = crate("dual");
    expect(files(find(dual, "MIT", "MIT OR Apache-2.0"))).toEqual(["LICENSE-MIT"]);
    expect(files(find(dual, "Apache-2.0", "MIT OR Apache-2.0"))).toEqual(["LICENSE-APACHE"]);
  });

  it("places LICENSE.MIT and LICENSE.Apache-2.0", () => {
    const dotted = crate("dotted");
    const declared = "Apache-2.0 OR MIT";
    expect(files(find(dotted, "MIT", declared))[0]).toBe("LICENSE.MIT");
    expect(files(find(dotted, "Apache-2.0", declared))[0]).toBe("LICENSE.Apache-2.0");
  });

  it("places LICENSE-ZLIB among three", () => {
    expect(files(find(crate("zlib-named"), "Zlib", "Zlib OR Apache-2.0 OR MIT"))).toEqual([
      "LICENSE-ZLIB",
    ]);
  });

  it("places an npm package's LICENSE_MIT and LICENSE_APACHE-2.0", () => {
    const declared = "Apache-2.0 OR MIT";
    expect(files(find(pkg("choice-pkg"), "MIT", declared))).toEqual(["LICENSE_MIT"]);
    expect(files(find(pkg("choice-pkg"), "Apache-2.0", declared))).toEqual(["LICENSE_APACHE-2.0"]);
  });

  it("places the files under LICENSES/ by their names", () => {
    const folder = crate("licenses-dir");
    expect(files(find(folder, "MIT", "MIT OR Apache-2.0"))).toEqual(["LICENSES/MIT.txt"]);
    expect(files(find(folder, "Apache-2.0", "MIT OR Apache-2.0"))).toEqual([
      "LICENSES/Apache-2.0.txt",
    ]);
  });

  it("places a file whose name says nothing by its content", () => {
    const folder = crate("dual-by-content");
    expect(files(find(folder, "MIT", "MIT OR Apache-2.0"))).toEqual(["LICENSE"]);
    expect(files(find(folder, "Apache-2.0", "MIT OR Apache-2.0"))).toEqual(["COPYING"]);
  });

  // The opening phrases of 010 research.md §5
  it.each([
    ["Unicode-3.0", "LICENSE-1", "UNICODE LICENSE"],
    ["Zlib", "LICENSE-2", "This software is provided 'as-is'"],
    ["BSD-3-Clause", "LICENSE-3", "Redistribution and use in source and binary forms"],
  ])("places %s by its opening phrase", (chosen, file, phrase) => {
    const found = find(crate("content-matching"), chosen, "Unicode-3.0 OR Zlib OR BSD-3-Clause");
    expect(files(found)).toEqual([file]);
    expect(found.texts[0].text).toContain(phrase);
  });

  it("counts a file that holds two licenses for both", () => {
    const folder = crate("both-in-one", "0.4.42");
    expect(files(find(folder, "MIT", "MIT OR Apache-2.0"))).toEqual(["LICENSE.txt"]);
    expect(files(find(folder, "Apache-2.0", "MIT OR Apache-2.0"))).toEqual(["LICENSE.txt"]);
  });
});

describe("010 research.md §5 - a file that only points elsewhere isn't a text", () => {
  it("fails a crate whose only file is a pointer (siphasher's COPYING)", () => {
    const found = find(
      crate("pointer-only"),
      "MIT",
      "MIT OR Apache-2.0",
      "crate pointer-only 1.0.0",
    );
    expect(found.texts).toEqual([]);
    expect(found.failure).toMatch(/^crate pointer-only 1\.0\.0: no license text for MIT/);
  });

  it("leaves the pointer out beside a real license file, and isn't confused by it", () => {
    const folder = crate("pointer-and-license");
    const declared = "MIT OR Apache-2.0";
    const mit = find(folder, "MIT", declared);
    expect(mit.failure).toBeNull();
    expect(files(mit)).toEqual(["LICENSE-MIT"]);
  });

  it("doesn't take the pointer for the Apache text it names", () => {
    const found = find(
      crate("pointer-and-license"),
      "Apache-2.0",
      "MIT OR Apache-2.0",
      "crate pointer-and-license 1.0.0",
    );
    expect(found.texts).toEqual([]);
    expect(found.failure).toMatch(/no license text for Apache-2\.0/);
  });
});

describe("010 FR-010 - fail closed: no text, no guess, no generic fallback", () => {
  it("names the crate, the license and what to do when there is no text", () => {
    const found = find(
      crate("no-license"),
      "MIT",
      "MIT",
      "crate no-license 1.0.0 (x86_64-unknown-linux-gnu)",
    );
    expect(found.texts).toEqual([]);
    expect(found.failure).toBe(
      'crate no-license 1.0.0 (x86_64-unknown-linux-gnu): no license text for MIT in its package. Add an exception (licenses/exceptions.json) with the text from its source repository; see DEVELOPMENT.md, "License notices".',
    );
  });

  it("fails the same way for an npm package with no license file", () => {
    const found = find(pkg("no-license-file"), "MIT", "MIT", "npm no-license-file 1.0.0");
    expect(found.texts).toEqual([]);
    expect(found.failure).toBe(
      'npm no-license-file 1.0.0: no license text for MIT in its package. Add an exception (licenses/exceptions.json) with the text from its source repository; see DEVELOPMENT.md, "License notices".',
    );
  });

  it("doesn't take LICENSE.spdx, which is metadata with no license text", () => {
    const found = find(pkg("spdx-only"), "MIT", "MIT", "npm spdx-only 1.0.0");
    expect(found.texts).toEqual([]);
    expect(found.failure).toMatch(/^npm spdx-only 1\.0\.0: no license text for MIT/);
  });

  it("names the chosen license when the choice's text is missing", () => {
    const found = find(
      crate("pointer-and-license"),
      "Apache-2.0",
      "MIT OR Apache-2.0",
      "crate foo 1.2.0",
    );
    expect(found.failure).toBe(
      'crate foo 1.2.0: no license text for Apache-2.0 in its package. Add an exception (licenses/exceptions.json) with the text from its source repository; see DEVELOPMENT.md, "License notices".',
    );
  });

  it("fails a file it can't place in a crate with a choice, naming the files", () => {
    const found = find(crate("unplaceable"), "MIT", "MIT OR Apache-2.0", "crate foo 1.2.0");
    expect(found.texts).toEqual([]);
    expect(found.failure).toBe(
      "crate foo 1.2.0: can't tell which of LICENSE, LICENSE.txt is the MIT text. Add a clarification (licenses/clarify.json).",
    );
  });

  it("doesn't fail an unplaceable file in a crate with one license (all files count)", () => {
    // unplaceable's declared license, read as a single one
    const found = find(crate("unplaceable"), "MIT", "MIT");
    expect(found.failure).toBeNull();
    expect(files(found).sort()).toEqual(["LICENSE", "LICENSE.txt"]);
  });
});

describe("010 research.md §5 - a choice uses the alternative whose text the package ships", () => {
  const declared = "MIT OR Apache-2.0";
  const among = (folder: string, licenses: string[]) =>
    findTextsAmong(folder, licenses, { declared, label: "crate fixture 1.0.0" }) as Found & {
      license: string;
    };

  it("takes the first license that has a text, with that license named", () => {
    // with-notice ships only Apache-2.0's LICENSE and a NOTICE
    const found = among(crate("with-notice"), ["MIT", "Apache-2.0"]);
    expect(found.failure).toBeNull();
    expect(found.license).toBe("Apache-2.0");
    expect(files(found)).toEqual(["LICENSE", "NOTICE", "AUTHORS"]);
  });

  it("keeps the preferred license when the package ships its text", () => {
    const found = among(crate("dual"), ["MIT", "Apache-2.0"]);
    expect(found.license).toBe("MIT");
    expect(files(found)).toEqual(["LICENSE-MIT"]);
  });

  it("fails with the preferred license's message when no alternative has a text", () => {
    const found = among(crate("authors-only"), ["MIT", "Apache-2.0"]);
    expect(found.failure).toMatch(/^crate fixture 1\.0\.0: no license text for MIT/);
    expect(found.texts).toEqual([]);
  });
});
