import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

/**
 * The Flatpak's store shots (011 FR-017, research.md §13), as
 * `store-shots.json` records them: the five names, and for each what is in
 * view by record and photo. Read by `store.e2e.ts` (which takes them), by
 * `e2e/run-e2e.mjs` (which copies them into `flatpak/screenshots/`) and by
 * `scripts/flatpak/storeShots.test.ts`. Plain `fs`, no wdio import, so a
 * Vitest run can load it.
 */

export interface StorePhoto {
  /** The record the photo is on, as its listing names it (Make Model). */
  record: string;
  /** The file in `src-tauri/examples/seed-photos/`. */
  file: string;
  /** The licence, worded as that folder's README words it. */
  licence: string;
}

export interface StoreShot {
  /** The file's name in `flatpak/screenshots/`, without `.png`. */
  name: string;
  /** The photos the shot shows. */
  photos: StorePhoto[];
  /** The listed records that show their type's drawing, not a photo. */
  drawings: string[];
  /** The listed records whose picture is a generated gradient, not a seed
   * photo (no licence to keep to). */
  generated: string[];
  /** Text the page must show when the shot is taken. */
  shows: string[];
}

export interface StoreShots {
  window: { width: number; height: number };
  shots: StoreShot[];
}

const HERE = path.dirname(fileURLToPath(import.meta.url));

/** `store-shots.json`'s list. */
export function readStoreShots(): StoreShots {
  const parsed = JSON.parse(fs.readFileSync(path.join(HERE, "store-shots.json"), "utf8")) as {
    window: StoreShots["window"];
    shots: StoreShot[];
  };
  return { window: parsed.window, shots: parsed.shots };
}
