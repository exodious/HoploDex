// @vitest-environment node
// The AppStream metadata and the desktop entry (011 US3-3, 011 US3-4,
// 011 US4-4, 011 FR-017; 011 data-model.md "AppStream metadata" and "The
// desktop entry", 011 research.md §13). These are the rules `appstreamcli
// validate` and the linter cannot know: the newest release is this build's
// version, the screenshots are files in the repository at that version's tag,
// the colours are the design tokens', and the desktop entry agrees with the
// metadata and the manifest. The files are written by 011 T020 and T021, so
// these tests are red until then.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import yaml from "js-yaml";
import { JSDOM } from "jsdom";
import { beforeAll, describe, expect, it } from "vitest";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, "../..");
const APP_ID = "io.github.exodious.HoploDex";
const METAINFO = path.join(ROOT, "flatpak", `${APP_ID}.metainfo.xml`);
const DESKTOP = path.join(ROOT, "flatpak", `${APP_ID}.desktop`);
const MANIFEST = path.join(ROOT, "flatpak", `${APP_ID}.yml`);
const SCREENSHOTS = path.join(ROOT, "flatpak/screenshots");

function readRequired(file: string, writtenBy: string): string {
  if (!fs.existsSync(file)) {
    throw new Error(`${path.relative(ROOT, file)} does not exist yet (${writtenBy} writes it)`);
  }
  return fs.readFileSync(file, "utf8");
}

/**
 * `APP_VERSION`: `src-tauri/src/lib.rs` takes it from `CARGO_PKG_VERSION`, so it
 * is `Cargo.toml`'s `[package] version`, read as text the way
 * `tests/app_version_test.rs` reads it (the first `version = "…"` after
 * `[package]` and before the next table).
 */
function appVersion(): string {
  const lib = fs.readFileSync(path.join(ROOT, "src-tauri/src/lib.rs"), "utf8");
  if (!/pub const APP_VERSION: &str = env!\("CARGO_PKG_VERSION"\);/.test(lib)) {
    throw new Error(
      "APP_VERSION in src-tauri/src/lib.rs is no longer CARGO_PKG_VERSION; read it from there",
    );
  }
  let inPackage = false;
  for (const raw of fs.readFileSync(path.join(ROOT, "src-tauri/Cargo.toml"), "utf8").split("\n")) {
    const line = raw.trim();
    if (line.startsWith("[")) {
      inPackage = line === "[package]";
    } else if (inPackage) {
      const match = /^version\s*=\s*"([^"]+)"/.exec(line);
      if (match) return match[1];
    }
  }
  throw new Error("src-tauri/Cargo.toml has no [package] version");
}

/** A design token's value in one block of `src/styles/tokens.css`. */
function token(name: string, block: ":root {" | ':root[data-theme="dark"] {'): string {
  const css = fs.readFileSync(path.join(ROOT, "src/styles/tokens.css"), "utf8");
  const start = css.indexOf(`\n${block}`);
  if (start < 0) throw new Error(`tokens.css has no ${block} block`);
  const end = css.indexOf("\n}", start);
  const match = new RegExp(`${name}:\\s*(#[0-9a-fA-F]{6})\\s*;`).exec(css.slice(start, end));
  if (!match) throw new Error(`tokens.css's ${block} block has no ${name}`);
  return match[1].toLowerCase();
}

/** The `[Desktop Entry]` group's keys. */
function desktopEntry(text: string): Map<string, string> {
  const keys = new Map<string, string>();
  let inEntry = false;
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (line === "" || line.startsWith("#")) continue;
    if (line.startsWith("[")) {
      inEntry = line === "[Desktop Entry]";
    } else if (inEntry) {
      const eq = line.indexOf("=");
      if (eq > 0) keys.set(line.slice(0, eq).trim(), line.slice(eq + 1).trim());
    }
  }
  return keys;
}

describe("011 US3-3, 011 US4-4 - the AppStream metadata", () => {
  let doc: Document;
  let version: string;
  const text = (selector: string) => doc.querySelector(selector)?.textContent?.trim() ?? null;

  beforeAll(() => {
    version = appVersion();
    doc = new JSDOM(readRequired(METAINFO, "011 T021"), { contentType: "text/xml" }).window
      .document;
  });

  it("is a desktop-application with the bundle's identifier and name", () => {
    expect(doc.documentElement.tagName).toBe("component");
    expect(doc.documentElement.getAttribute("type")).toBe("desktop-application");
    expect(text("component > id")).toBe(APP_ID);
    expect(text("component > name")).toBe("HoploDex");
    expect(text("component > summary")).toBeTruthy();
  });

  it("011 FR-017: describes what HoploDex is, and that everything stays on the computer with no account and no network", () => {
    const description = text("component > description") ?? "";
    expect(description).toMatch(/computer/i);
    expect(description).toMatch(/account/i);
    expect(description).toMatch(/network/i);
  });

  it("is licensed CC0-1.0 as metadata and GPL-3.0-only as a project", () => {
    expect(text("metadata_license")).toBe("CC0-1.0");
    expect(text("project_license")).toBe("GPL-3.0-only");
  });

  it("names its developer, homepage, source and bug tracker", () => {
    expect(doc.querySelector("developer")?.getAttribute("id")).toBe("io.github.exodious");
    for (const type of ["homepage", "vcs-browser", "bugtracker"]) {
      expect(text(`url[type="${type}"]`), `url type="${type}"`).toContain(
        "github.com/exodious/HoploDex",
      );
    }
  });

  it("launches the desktop entry of the same name", () => {
    expect(doc.querySelector("launchable")?.getAttribute("type")).toBe("desktop-id");
    expect(text("launchable")).toBe(path.basename(DESKTOP));
  });

  it("declares an empty OARS 1.1 content rating", () => {
    const rating = doc.querySelector("content_rating");
    expect(rating?.getAttribute("type")).toBe("oars-1.1");
    expect(rating?.children).toHaveLength(0);
  });

  it("branding colours are the design tokens' --vellum, light and dark", () => {
    const light = text('branding > color[type="primary"][scheme_preference="light"]');
    const dark = text('branding > color[type="primary"][scheme_preference="dark"]');
    expect(light?.toLowerCase()).toBe(token("--vellum", ":root {"));
    expect(dark?.toLowerCase()).toBe(token("--vellum", ':root[data-theme="dark"] {'));
  });

  describe("releases", () => {
    it("011 FR-017: the newest release's version equals APP_VERSION (src-tauri/src/lib.rs)", () => {
      const newest = doc.querySelector("releases > release");
      expect(
        newest?.getAttribute("version"),
        `the newest release must be APP_VERSION ${version}, the version in src-tauri/Cargo.toml`,
      ).toBe(version);
    });

    it("lists them newest first, each with a version, a date and notes", () => {
      const releases = [...doc.querySelectorAll("releases > release")];
      expect(releases.length).toBeGreaterThan(0);
      for (const release of releases) {
        const version = release.getAttribute("version");
        expect(version, "a release version").toBeTruthy();
        expect(release.getAttribute("date"), `release ${version}'s date`).toMatch(
          /^\d{4}-\d{2}-\d{2}$/,
        );
        expect(
          release.querySelector("description")?.textContent?.trim(),
          `release ${version}'s notes`,
        ).toBeTruthy();
      }
      const dates = releases.map((r) => r.getAttribute("date") ?? "");
      expect(dates, "releases newest first").toEqual([...dates].sort().reverse());
    });
  });

  describe("screenshots", () => {
    it("011 US4-4: there are four to six, each with a caption", () => {
      const shots = [...doc.querySelectorAll("screenshots > screenshot")];
      expect(shots.length).toBeGreaterThanOrEqual(4);
      expect(shots.length).toBeLessThanOrEqual(6);
      for (const shot of shots) {
        expect(
          shot.querySelector("caption")?.textContent?.trim(),
          "a screenshot's caption",
        ).toBeTruthy();
      }
    });

    it("011 US4-4: every address names a file under flatpak/screenshots/ at v<version>", () => {
      const prefix = `https://raw.githubusercontent.com/exodious/HoploDex/v${version}/flatpak/screenshots/`;
      const images = [...doc.querySelectorAll("screenshots > screenshot > image")];
      expect(images.length).toBeGreaterThan(0);
      for (const image of images) {
        const address = image.textContent?.trim() ?? "";
        expect(address.startsWith(prefix), `${address} must start with ${prefix}`).toBe(true);
        const file = address.slice(prefix.length);
        expect(file, `${address} must name a file, not a folder`).toMatch(/^[^/]+$/);
        expect(
          fs.existsSync(path.join(SCREENSHOTS, file)),
          `flatpak/screenshots/${file} must exist`,
        ).toBe(true);
      }
    });
  });
});

describe("011 US3-4, 011 FR-017 - the desktop entry", () => {
  let entry: Map<string, string>;

  beforeAll(() => {
    entry = desktopEntry(readRequired(DESKTOP, "011 T020"));
  });

  it("is an application named HoploDex that runs without a terminal", () => {
    expect(entry.get("Type")).toBe("Application");
    expect(entry.get("Name")).toBe("HoploDex");
    expect(entry.get("Terminal")).toBe("false");
    expect(entry.get("Categories")).toBe("Office;Database;");
  });

  it("uses the bundle's icon and the main window's class", () => {
    expect(entry.get("Icon")).toBe(APP_ID);
    expect(entry.get("StartupWMClass"), "StartupWMClass").toBeTruthy();
  });

  it("runs the manifest's command", () => {
    const manifest = yaml.load(readRequired(MANIFEST, "011 T019")) as { command: string };
    expect(entry.get("Exec")).toBe("hoplodex");
    expect(entry.get("Exec")).toBe(manifest.command);
  });

  it("has the metadata's summary as its comment", () => {
    const doc = new JSDOM(readRequired(METAINFO, "011 T021"), { contentType: "text/xml" }).window
      .document;
    expect(entry.get("Comment")).toBe(
      doc.querySelector("component > summary")?.textContent?.trim(),
    );
  });
});
