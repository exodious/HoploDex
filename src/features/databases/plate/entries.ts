import { DRAWINGS } from "../../browse/typeDrawings";

/*
 * Entry 2 of the catalogue plate: the firearm drawings it cycles through,
 * each with its caption and its real size for the scale bar. The drawings
 * are the app's own type drawings (typeDrawings.ts), so they are never
 * retraced here.
 *
 * The app shows them in an order shuffled once at startup
 * (STARTUP_ENTRIES), so someone who opens a database soon after starting
 * it still sees each drawing over time; the first in that order is the one
 * the draw-in draws. Each keeps its own catalogue number, from its place in
 * PLATE_ENTRIES, whatever the order.
 */

export interface PlateEntry {
  /** A key into DRAWINGS. */
  key: keyof typeof DRAWINGS;
  /** The caption's first word, in bold. */
  title: string;
  /** The rest of the caption. */
  caption: string;
  /** The real object's overall length, muzzle to butt (or base to tip), in
   * millimetres: what the drawing's width stands for. */
  lengthMm: number;
  /** The scale bar's length, in centimetres. */
  barCm: number;
}

export const PLATE_ENTRIES: PlateEntry[] = [
  // M16A1: 986 mm overall.
  {
    key: "rifle",
    title: "Rifle.",
    caption: "Steel, aluminium and plastic. 1967.",
    lengthMm: 986,
    barCm: 30,
  },
  // Glock 17: 202 mm overall.
  {
    key: "handgun",
    title: "Pistol.",
    caption: "Polymer frame, steel slide. 1982.",
    lengthMm: 202,
    barCm: 10,
  },
  // Remington 870 with a 20" barrel: about 1,010 mm overall.
  {
    key: "shotgun",
    title: "Shotgun.",
    caption: "Steel and walnut, pump action. 1950.",
    lengthMm: 1010,
    barCm: 30,
  },
  // A rifle suppressor: its 38 mm (1.5 in) tube at the drawing's
  // proportions makes it about 250 mm overall. The year is Hiram Percy
  // Maxim's patent, the first silencer sold commercially.
  {
    key: "suppressor",
    title: "Suppressor.",
    caption: "Steel tube, stacked baffles. 1909.",
    lengthMm: 250,
    barCm: 10,
  },
  // A .308-class rifle cartridge: 71 mm overall. The year is the 8 mm
  // Lebel's, the first military cartridge to pair a drawn brass case and
  // smokeless powder with a jacketed bullet.
  {
    key: "other",
    title: "Cartridge.",
    caption: "Brass case, jacketed bullet. About 1886.",
    lengthMm: 71,
    barCm: 2,
  },
];

/** An entry's catalogue number, the one beside its caption: its place in
 * PLATE_ENTRIES, counting on from the hoplon's 1. */
export function entryNumber(entry: PlateEntry): number {
  return PLATE_ENTRIES.findIndex((e) => e.key === entry.key) + 2;
}

/** `items` in a random order (Fisher–Yates), leaving `items` as it was. */
export function shuffled<T>(items: readonly T[], random: () => number = Math.random): T[] {
  const out = [...items];
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(random() * (i + 1));
    [out[i], out[j]] = [out[j], out[i]];
  }
  return out;
}

/** The order the app shows entry 2's drawings in, shuffled once each time
 * it starts. */
export const STARTUP_ENTRIES: readonly PlateEntry[] = shuffled(PLATE_ENTRIES);

/** Where entry 2's drawings go, in plate pixels: each is scaled to fit this
 * box (never above MAX_SCALE, the rifle's), against its left edge and
 * centred on its height. */
export const ENTRY_BOX = { x: 642, y: 548, width: 498, height: 170 };
const MAX_SCALE = 1.6;

/** A drawing's extent, in its 320×200 drawing units: [x0, y0, x1, y1]. The
 * control points of curves count, so it can be a little generous. */
export function drawingBounds(key: keyof typeof DRAWINGS): [number, number, number, number] {
  const xs: number[] = [];
  const ys: number[] = [];
  for (const part of DRAWINGS[key].parts) {
    if ("circle" in part) {
      const [cx, cy, r] = part.circle;
      xs.push(cx - r, cx + r);
      ys.push(cy - r, cy + r);
      continue;
    }
    const tokens = part.d.match(/[A-Za-z]|-?\d*\.?\d+/g) ?? [];
    let command = "M";
    let x = 0;
    let y = 0;
    let i = 0;
    const next = () => Number(tokens[i++]);
    while (i < tokens.length) {
      if (/[A-Za-z]/.test(tokens[i])) command = tokens[i++];
      switch (command) {
        case "Z":
        case "z":
          continue;
        case "H":
          x = next();
          break;
        case "V":
          y = next();
          break;
        case "M":
        case "L":
          x = next();
          y = next();
          break;
        case "Q":
          xs.push(next());
          ys.push(next());
          x = next();
          y = next();
          break;
        case "C":
          xs.push(next());
          ys.push(next());
          xs.push(next());
          ys.push(next());
          x = next();
          y = next();
          break;
        case "S":
          xs.push(next());
          ys.push(next());
          x = next();
          y = next();
          break;
        case "A":
        case "a": {
          const rx = next();
          next();
          next();
          next();
          next();
          const ex = next();
          const ey = next();
          x = command === "a" ? x + ex : ex;
          y = command === "a" ? y + ey : ey;
          // a small arc: its radius either side of the end point covers it
          xs.push(x - 2 * rx, x + 2 * rx);
          ys.push(y - 2 * rx, y + 2 * rx);
          break;
        }
        default:
          throw new Error(`drawingBounds: unexpected path command ${command}`);
      }
      xs.push(x);
      ys.push(y);
    }
  }
  return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
}

export interface EntryLayout {
  /** The drawing's transform into the plate. */
  transform: string;
  scale: number;
  /** The scale bar's width in plate pixels. */
  barPx: number;
}

/** Fits an entry's drawing into ENTRY_BOX, and sizes its scale bar. */
export function entryLayout(entry: PlateEntry): EntryLayout {
  const [x0, y0, x1, y1] = drawingBounds(entry.key);
  const box = ENTRY_BOX;
  const scale = Math.min(MAX_SCALE, box.width / (x1 - x0), box.height / (y1 - y0));
  const tx = box.x - x0 * scale;
  const ty = box.y + box.height / 2 - ((y0 + y1) / 2) * scale;
  const mmPerUnit = entry.lengthMm / (x1 - x0);
  const barPx = ((entry.barCm * 10) / mmPerUnit) * scale;
  return {
    transform: `translate(${tx.toFixed(1)} ${ty.toFixed(1)}) scale(${scale.toFixed(4)})`,
    scale,
    barPx,
  };
}
