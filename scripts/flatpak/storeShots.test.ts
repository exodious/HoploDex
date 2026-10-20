// @vitest-environment node
// The Flatpak's store shots (011 FR-017; 011 research.md §13): the script's
// record of what each shows (e2e/screenshots/store-shots.json, which
// store.e2e.ts checks the app's view against as it takes them) agrees with
// research.md's recorded list, with the photos' licences as the seed-photos
// README records them, and with the files the metainfo points at. The shots
// are retaken only on purpose, by `npm run screenshots:store`.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { readStoreShots } from "../../e2e/screenshots/storeShots";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const RESEARCH = path.join(ROOT, "specs/011-flatpak/research.md");
const SEED_PHOTOS = path.join(ROOT, "src-tauri/examples/seed-photos");
const SEED_SOURCE = path.join(ROOT, "src-tauri/examples/support/main_collection.rs");
const METAINFO = path.join(ROOT, "flatpak/io.github.exodious.HoploDex.metainfo.xml");
const STORE_DIR = path.join(ROOT, "flatpak/screenshots");

const { window, shots } = readStoreShots();

/** research.md §13's bullets, one per shot: `chooser.png` to its text. */
function researchBullets(): Map<string, string> {
  const section = fs.readFileSync(RESEARCH, "utf8");
  const start = section.indexOf("What each\n    shot shows, with the photos' licences:");
  if (start < 0) throw new Error("research.md §13 no longer has the list of what each shot shows");
  const end = section.indexOf("A retake of any of them must keep to this list", start);
  const list = section.slice(start, end);
  const bullets = new Map<string, string>();
  for (const match of list.matchAll(
    /^ {4}- `([a-z-]+)\.png`:([\s\S]*?)(?=^ {4}- `|$(?![\s\S]))/gm,
  )) {
    bullets.set(match[1], match[2].replace(/\s+/g, " ").trim());
  }
  return bullets;
}

/** The seed-photos README's "Which firearm uses which" table: file to
 * { firearm, licence }. */
function readmeTable(): Map<string, { firearm: string; licence: string }> {
  const readme = fs.readFileSync(path.join(SEED_PHOTOS, "README.md"), "utf8");
  const rows = new Map<string, { firearm: string; licence: string }>();
  for (const match of readme.matchAll(/^\| `([^`]+\.jpg)` \| ([^|]+) \| ([^|]+) \|$/gm)) {
    rows.set(match[1], { firearm: match[2].trim(), licence: match[3].trim() });
  }
  return rows;
}

const photoFiles = (shot: { photos: { file: string }[] }) => shot.photos.map((p) => p.file).sort();

describe("011 FR-017 - the store shots' recorded list", () => {
  it("names the five shots the metainfo points at, in its order, as files in flatpak/screenshots", () => {
    const metainfo = fs.readFileSync(METAINFO, "utf8");
    const named = [...metainfo.matchAll(/flatpak\/screenshots\/([a-z-]+\.png)</g)].map((m) => m[1]);
    expect(shots.map((s) => `${s.name}.png`)).toEqual(named);
    expect(named).toHaveLength(5);
    expect(fs.readdirSync(STORE_DIR).sort()).toEqual([...named].sort());
  });

  it("each shot is a PNG of the walk's window size", () => {
    for (const shot of shots) {
      const png = fs.readFileSync(path.join(STORE_DIR, `${shot.name}.png`));
      expect(png.subarray(1, 4).toString(), shot.name).toBe("PNG");
      expect([png.readUInt32BE(16), png.readUInt32BE(20)], shot.name).toEqual([
        window.width,
        window.height,
      ]);
    }
  });

  it("research.md §13 lists the same photos for each shot, and the same kind of licence", () => {
    const bullets = researchBullets();
    expect([...bullets.keys()]).toEqual(shots.map((s) => s.name));
    for (const shot of shots) {
      const text = bullets.get(shot.name)!;
      const files = [...text.matchAll(/`([a-z0-9-]+\.jpg)`/g)].map((m) => m[1]).sort();
      expect(files, `${shot.name}'s photos`).toEqual(photoFiles(shot));
      for (const photo of shot.photos) {
        // The licence the text gives the file: what follows its name, up to
        // the next photo's.
        const from = text.indexOf(`\`${photo.file}\``) + photo.file.length + 2;
        const next = text.slice(from).search(/`[a-z0-9-]+\.jpg`/);
        const given = next < 0 ? text.slice(from) : text.slice(from, from + next);
        const kind = photo.licence.startsWith("CC0") ? /CC0/ : /public domain/i;
        expect(given, `${photo.file}'s licence in research.md`).toMatch(kind);
      }
    }
  });

  it("every photo is the seed's, on the record the list says, under the licence its README records, and needs no credit line", () => {
    const table = readmeTable();
    const seed = fs.readFileSync(SEED_SOURCE, "utf8");
    for (const shot of shots) {
      for (const photo of shot.photos) {
        expect(fs.existsSync(path.join(SEED_PHOTOS, photo.file)), `${photo.file} exists`).toBe(
          true,
        );
        expect(seed, `${photo.file} is attached by the seed`).toContain(
          `seed_photo!("${photo.file}")`,
        );
        const row = table.get(photo.file);
        expect(row, `${photo.file} is in the README's table`).toBeDefined();
        expect(row!.firearm.startsWith(photo.record), `${photo.file}'s record`).toBe(true);
        expect(row!.licence, `${photo.file}'s licence`).toBe(photo.licence);
        // Public domain and CC0 ask for no credit line; CC BY and the OGL do,
        // and the constitution's licensing rule keeps them out of a shot.
        expect(photo.licence, `${photo.file} needs a credit line`).toMatch(/^(Public domain|CC0)/);
      }
    }
  });

  it("a shot that shows no photo from the seed names no record with one", () => {
    for (const name of ["chooser", "document-preview"]) {
      const shot = shots.find((s) => s.name === name)!;
      expect(shot.photos, name).toEqual([]);
    }
    // Research: the accessories shot's one picture is a generated gradient.
    expect(shots.find((s) => s.name === "accessories")!.photos).toEqual([]);
    expect(shots.find((s) => s.name === "accessories")!.generated).toEqual([
      "Leupold VX-5HD 3-15x44",
    ]);
  });

  it("is retaken only by `npm run screenshots:store`: no build, test or other script runs it", () => {
    const pkg = JSON.parse(fs.readFileSync(path.join(ROOT, "package.json"), "utf8")) as {
      scripts: Record<string, string>;
    };
    expect(pkg.scripts["screenshots:store"]).toBe("node e2e/run-e2e.mjs --store-shots");
    for (const [name, command] of Object.entries(pkg.scripts)) {
      if (name === "screenshots:store") continue;
      expect(command, `package.json's "${name}"`).not.toMatch(/store-shots|screenshots:store/);
    }
    // Not a spec `npm run test:e2e` finds, and not in the walk `npm run screenshots` runs.
    expect(fs.existsSync(path.join(ROOT, "e2e/specs/store.e2e.ts"))).toBe(false);
    const runner = fs.readFileSync(path.join(ROOT, "e2e/run-e2e.mjs"), "utf8");
    const walk = /const SCREENSHOT_SPECS = \[([^\]]*)\]/.exec(runner)?.[1] ?? "";
    expect(walk).not.toContain("store.e2e.ts");
    for (const file of fs.readdirSync(path.join(ROOT, "scripts"), {
      recursive: true,
    }) as string[]) {
      const full = path.join(ROOT, "scripts", file);
      if (!fs.statSync(full).isFile() || file.endsWith("storeShots.test.ts")) continue;
      expect(fs.readFileSync(full, "utf8"), file).not.toMatch(/screenshots:store|--store-shots/);
    }
  });
});
