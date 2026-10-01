/*
 * The artwork behind TypeDrawing: one side elevation per firearm type,
 * muzzle to the right, in a 320×200 box. Each drawing is a list of parts
 * painted back to front, then a bore axis.
 *
 * Each firearm is traced from a side-on photograph of a real model so its
 * proportions hold up. Muzzle-right puts the right side toward the viewer,
 * so only right-side features are drawn (ejection ports, not slide stops).
 *
 * The suppressor (specs/005-regulated-item-types FR-001, research.md §4) was
 * drawn for this project, traced from side-on photographs of rifle
 * suppressors for proportion (a tube about 6:1 in length to diameter,
 * centred on the bore axis), with no brand marks. It is a partial section,
 * cut away between two break lines to show a generic cone-baffle stack.
 * Like the rest of the source it is GPL-3.0-only.
 *
 * The twelve accessory kinds (specs/006-accessory-links FR-007a, research.md
 * §15) are `optic`, `light`, `magazine`, `stock`, `upper`, `barrel`,
 * `muzzle`, `conversion`, `mount`, `sling`, `case` and `accessory` (the
 * kind "Other"). They were drawn for this project, traced from side-on
 * photographs for proportion, with no brand marks, in the same 320×200 box,
 * muzzle-right convention and part, open and detail line roles. Each has
 * separate subpaths per stroke, and its axis is the line the item sits on:
 * the bore, the optical axis, or the rail it clamps to. Like the rest of
 * the source they are GPL-3.0-only.
 */

export type Part =
  | { d: string; role: "part" | "open" | "detail" }
  | { circle: [number, number, number]; role: "part" };

export interface Drawing {
  parts: Part[];
  /** Bore axis: [x start, y, x end]. */
  axis: [number, number, number];
}

/** Section hatching at 45° across a horizontal band of a cut wall. */
function sectionHatch(x0: number, x1: number, top: number, bottom: number): string {
  const rise = bottom - top;
  let d = "";
  for (let x = x0; x + rise <= x1; x += 4) d += `M${x} ${bottom}L${x + rise} ${top}`;
  return d;
}

/** Round to a tenth, so computed coordinates stay short. */
function r(n: number): number {
  return Math.round(n * 10) / 10;
}

/** Evenly spaced vertical strokes from x0 to x1, each from y0 to y1. */
function ticks(x0: number, x1: number, step: number, y0: number, y1: number): string {
  let d = "";
  for (let x = x0; x <= x1 + 0.01; x += step) d += `M${r(x)} ${y0}V${y1}`;
  return d;
}

/** Evenly spaced horizontal dashes of length `len` along y. */
function dashes(x0: number, x1: number, y: number, len: number, gap: number): string {
  let d = "";
  for (let x = x0; x + len <= x1 + 0.01; x += len + gap) d += `M${r(x)} ${y}h${len}`;
  return d;
}

/** A rail's outline: a body with a transverse slot cut from the top edge
 * every `pitch`, each `width` wide and `depth` deep. */
function slottedRail(
  x0: number,
  x1: number,
  top: number,
  bottom: number,
  first: number,
  pitch: number,
  width: number,
  depth: number,
): string {
  let d = `M${x0} ${bottom}V${top}`;
  for (let x = first; x + width <= x1 - 4; x += pitch) {
    d += `H${x}V${top + depth}H${x + width}V${top}`;
  }
  return `${d}H${x1}V${bottom}Z`;
}

/** A straight tube of half-width `half` from (x0, y0) to (x1, y1). */
function tube(x0: number, y0: number, x1: number, y1: number, half: number): string {
  const length = Math.hypot(x1 - x0, y1 - y0);
  const nx = (-(y1 - y0) / length) * half;
  const ny = ((x1 - x0) / length) * half;
  return (
    `M${r(x0 + nx)} ${r(y0 + ny)}L${r(x1 + nx)} ${r(y1 + ny)}` +
    `L${r(x1 - nx)} ${r(y1 - ny)}L${r(x0 - nx)} ${r(y0 - ny)}Z`
  );
}

/** A stroke across a tube at (x, y), perpendicular to its direction. */
function across(x: number, y: number, dx: number, dy: number, half: number): string {
  const length = Math.hypot(dx, dy);
  const nx = (-dy / length) * half;
  const ny = (dx / length) * half;
  return `M${r(x + nx)} ${r(y + ny)}L${r(x - nx)} ${r(y - ny)}`;
}

/** A spring's coils: slanted strokes between the wire's two edges. */
function coils(x0: number, x1: number, y: number, half: number, pitch: number): string {
  let d = "";
  for (let x = x0; x + pitch <= x1 + 0.01; x += pitch) {
    d += `M${r(x)} ${y + half}L${r(x + pitch * 0.6)} ${y - half}`;
  }
  return d;
}

/** One cone baffle in section, apex toward the mount: the cone's wall above
 * and below the bore, from the skirt at `x` down to the bore hole. */
function coneBaffle(x: number, thickness: number): Part {
  const [depth, t] = [10, thickness];
  return {
    d:
      `M${x} 86.5L${x - depth} 95.5H${x - depth + t}L${x + t} 86.5Z` +
      `M${x} 113.5L${x - depth} 104.5H${x - depth + t}L${x + t} 113.5Z`,
    role: "part",
  };
}

/** Where each baffle's cone meets its skirt; the first is the blast baffle. */
const BAFFLES = [130, 145, 160, 175, 190, 205, 220, 235];

export const DRAWINGS: Record<string, Drawing> = {
  // Glock 17, right side: ejection port and extractor show; the slide stop
  // lever and magazine catch are on the left side only.
  handgun: {
    parts: [
      { d: "M67.8 24L71 19Q71.5 18.3 72.4 18.3H75.2L80.8 24Z", role: "part" },
      { d: "M262.8 24.3L264.3 21.2H267.4L270.6 24.3Z", role: "part" },
      {
        d: "M54.8 168H101.4V175.8L98.5 178.8Q96.5 181.3 92.6 181.4L53 181.1Q51.6 181 51.6 179.8L51.8 175.8L54.8 173Z",
        role: "part",
      },
      {
        d: "M58.5 49.5L57.2 50.7Q56.2 55.6 59.5 57.8C66 58.6 72.5 60.5 75.4 66.5Q77.2 70.5 76.4 76C74.8 84 70 100 64.7 109.1C57.5 121.5 46 142 43.2 154Q42 161 42.8 165Q44 170 49 171L101.4 171.3L108.6 164.9C108.4 157 110.5 148.5 115 143.5Q118.6 140.5 117.4 136.5Q116.4 132 119.5 127Q122 123.5 125.5 121.5Q126.6 116 126.5 111Q127 105.5 131 101.5Q135 98.6 140 99.8Q144 102.4 149 103L194.7 101.9Q196 101 195.4 99.8L193.4 93Q192.9 86 193.3 79Q194 74.5 197 71.6Q199.5 69.4 203 69L265.7 67.3Q272 66.6 275.4 62.5Q277.3 59.5 277.2 55L277 49.5Z",
        role: "part",
      },
      {
        d: "M150 71.8Q146 72.5 142 76Q139.5 78.8 139.6 83V90Q140 96 146 97.5H181Q186.2 97.5 186.2 92V75Q186.2 69.6 181 69.6Z",
        role: "open",
      },
      {
        d: "M143 73.5Q149 71.5 152.5 72.2C149.5 78 148.5 86 152.4 95.2Q151.8 96.6 150.2 96C145 90 142.5 85 140.6 79Q140.5 75.5 143 73.5Z",
        role: "part",
      },
      { d: "M149.4 73.6C147.2 80.5 147.2 88 150.8 95.4", role: "detail" },
      { d: "M208.5 51H213.5V59.5H208.5ZM214 60H268M214 62.4H266", role: "detail" },
      { circle: [151.3, 52.9, 1.5], role: "part" },
      { circle: [153, 61.8, 1.9], role: "part" },
      { d: "M113.5 49.5V56.5H127.5V49.5", role: "part" },
      { d: "M117 50.5V55.5M120.5 50.5V55.5M124 50.5V55.5", role: "detail" },
      { d: "M58.5 49.5L58.3 28Q58.5 24 62.5 23.6L271 24.3Q277.8 24.6 278 30V49.5Z", role: "part" },
      {
        d: "M65.5 27V47M71 27V47M76.5 27V47M82 27V47M87.5 27V47M93 27V47M98.5 27V47M104 27V47",
        role: "detail",
      },
      { d: "M141 24.1V36.5H184V24.4", role: "open" },
      { d: "M145.5 24.2V36.5M145.5 33H184", role: "detail" },
      { d: "M127 33H141M127 33V37.5H141", role: "detail" },
    ],
    axis: [46, 37, 300],
  },
  // M16A1, right side: carry handle with the A1 rear sight's windage drum,
  // teardrop forward assist, plain slip ring, tapered handguard seated in
  // the front sight's cap, triangular front sight, A1 grip and a straight
  // 20-round magazine.
  rifle: {
    parts: [
      { d: "M259.1 87.1H295.7V91.1H259.1Z", role: "part" },
      { d: "M295.3 85.9H309.6Q310 85.9 310 86.4V91.7Q310 92.2 309.6 92.2H295.3Z", role: "part" },
      { d: "M297.3 85.9V92.2M299.3 85.9V92.2M302 87.4H308.4M302 90.7H308.4", role: "detail" },
      {
        d: "M256.9 91.1H262.5Q263.2 91.1 263.2 91.7V94.6Q263.2 95.3 262.5 95.3H258.5L256.9 93.8Z",
        role: "part",
      },
      {
        d: "M247.5 91.1L247.1 94.2Q247.1 95.5 248.4 95.5H249.6Q250.7 95.5 250.4 94.2L250 91.1",
        role: "open",
      },
      {
        d: "M245.5 83.3L254 69.7Q254.7 68.6 256 68.6H258Q259.6 68.6 259.6 70.3V83.3L260.3 84.2V91.7H245.5Z",
        role: "part",
      },
      { d: "M248.2 82.8L252.9 74.8Q253.3 74.1 254.2 74.1H257.6V82.8", role: "open" },
      { d: "M255.4 82.8V75.9Q255.4 75.2 256 75.2Q256.7 75.2 256.7 75.9V82.8", role: "part" },
      { d: "M245.5 83.3H260.3", role: "detail" },
      { d: "M146.7 82.2H149.2V94.2H146.7Z", role: "part" },
      {
        d: "M149.2 81H153.9Q154.3 81 154.3 81.5V94.9Q154.3 95.3 153.9 95.3H149.2Q148.7 95.3 148.7 94.9V81.5Q148.7 81 149.2 81Z",
        role: "part",
      },
      { d: "M153.6 81V95.3", role: "detail" },
      {
        d: "M157 78.9L243.8 82.4Q246 88.6 243.8 95.1L157.2 99.7Q154.5 99.7 154.5 97.1V81.5Q154.5 78.9 157 78.9Z",
        role: "part",
      },
      {
        d: "M198.5 96.3H202.9M206.1 96.1H211.2M213.9 95.9H218.6M221.7 95.5H226.4M229 95.2H233.7",
        role: "detail",
      },
      { d: "M88.3 83.3H10.9V121.2L61.5 104.9Q74 100.9 88.3 98.9Z", role: "part" },
      { d: "M10.9 83.3H9.6Q8.4 83.3 8.4 84.4L9.8 120.1Q10 121.2 10.9 121.2Z", role: "part" },
      {
        d: "M122.9 107.4H142.7V123.9Q142.7 124.5 142 124.6L123.5 127.4Q122.9 127.5 122.9 126.8Z",
        role: "part",
      },
      {
        d: "M122.9 125.8L142.7 123M125.5 113.4V125.6M128.2 113.4V125.2M137.4 112.9V123.6M140 112.9V123.2",
        role: "detail",
      },
      {
        d: "M87.4 90.2H147V96.2Q147 97.8 145.4 97.8H144.3V109.4L123.8 112.9H103V102.2H93.4Q89.6 102 87.4 98.9Z",
        role: "part",
      },
      {
        d: "M107.5 102.7H120Q121.3 102.7 121.3 104V110.3Q121.3 111.6 120 111.6H107.5Q106.1 111.6 106.1 110.3V104Q106.1 102.7 107.5 102.7Z",
        role: "open",
      },
      { d: "M122.4 98.7V112.5", role: "detail" },
      {
        d: "M111.3 102.7L113.5 102.7Q112.4 105.1 112.4 107.8Q112.6 109.8 113.3 110.9L112.6 111.2Q111.3 109.6 111 107.4Q110.8 104.7 111.3 102.7Z",
        role: "part",
      },
      { circle: [95.4, 93.3, 0.8], role: "part" },
      { circle: [101.9, 96.9, 0.7], role: "part" },
      { circle: [110.8, 98.9, 0.4], role: "part" },
      { circle: [116.8, 96.7, 0.4], role: "part" },
      { circle: [121.3, 97.5, 1], role: "part" },
      { circle: [146.1, 93.8, 0.5], role: "part" },
      {
        d: "M92.5 102.2H104.4V109.1Q104.4 111.8 103 113.4Q96.3 120.7 91.9 131.4Q91 133.5 89.2 133L78 130.1Q76 129.4 76.9 127.9L89 111.8Q92.5 107.6 92.5 102.2Z",
        role: "part",
      },
      {
        d: "M87.4 82.6L94.1 80.1L95.7 78.4L101 67.7Q101.7 66.3 103.2 66.3L120.9 67.9L142.3 69.9Q145.8 70.3 147 73.5V90.6H87.4Z",
        role: "part",
      },
      {
        d: "M105.5 78.4Q105.5 79.3 106.6 79.3H143.4Q145.8 79.3 145.8 76.8V74.8Q145.8 72.1 143.2 72.1H110.4Q107.5 72.3 105.7 75.7Z",
        role: "open",
      },
      { d: "M105.5 73.7H99Q97.7 73.7 97 75.5L96.3 77.9Q96.3 78.8 97.2 78.8H105.5", role: "detail" },
      { circle: [103.9, 70.3, 2.8], role: "part" },
      { d: "M103.1 70.3a0.8 0.8 0 1 0 1.6 0a0.8 0.8 0 1 0 -1.6 0", role: "detail" },
      {
        d: "M88.5 82.6V78.8Q88.5 77.5 90.1 77.5Q91.6 77.5 92.1 78.8L92.5 79.5H95.4Q96.5 79.5 96.5 80.7Q96.5 81.9 95.4 81.9H92.1L91.6 82.6Z",
        role: "part",
      },
      { d: "M117.1 90.2V83.3Q117.1 81.7 118.6 81.7Q120.2 81.7 120.2 83.3V90.2", role: "part" },
      {
        d: "M121.3 83.5H142.7Q143.4 83.5 143.4 84.2V89.1Q143.4 89.7 142.7 89.7H121.3Q120.6 89.7 120.6 89.1V84.2Q120.6 83.5 121.3 83.5Z",
        role: "part",
      },
      { d: "M128.9 83.5V85.3H133.8V83.5M128.2 88H134.2", role: "detail" },
    ],
    axis: [4, 89.1, 316],
  },
  // Remington 870 with a 20" barrel, right side: the ejection port is on
  // this side; ribbed forend, magazine cap and vented recoil pad.
  shotgun: {
    parts: [
      {
        d: "M18.5 88L13.2 88.3Q11.3 88.5 11 90.3Q8.6 104 8.8 122Q9 124.6 11.2 124.8L16.8 125.3Z",
        role: "part",
      },
      {
        d: "M14.6 90.5L12.8 93.1L14.8 95.7L12.8 98.3L14.8 100.9L12.8 103.5L14.8 106.1L12.8 108.7L14.8 111.3L12.8 113.9L14.8 116.5L12.8 119.1L14.8 121.7",
        role: "detail",
      },
      {
        d: "M112 73.2L107.5 73.6C99 75.8 90.5 81 84 82.1Q81.5 82.5 78 82.6L18.3 88.2L16.6 125.2Q16.6 125.6 17.2 125.3L70 101.6C77 99.8 82.5 101.3 86.3 103.2Q89 104.5 90.9 102.2C94 97 101 89.5 110 85.4H112Z",
        role: "part",
      },
      { d: "M163 71.2H308.4Q309.8 71.2 309.8 72.6V76.2Q309.8 77.6 308.4 77.6H163Z", role: "part" },
      { circle: [302.3, 70.2, 0.9], role: "part" },
      { d: "M163 78.6H247V87H163Z", role: "part" },
      { d: "M140 86.6H191V88.2H140Z", role: "part" },
      {
        d: "M246.2 79.2Q246.2 78.3 247.1 78.3H250Q251 78.3 251 79.3V86.3Q251 87.3 250 87.3H247.1Q246.2 87.3 246.2 86.4Z",
        role: "part",
      },
      { d: "M247.8 78.8V86.8M249.4 78.8V86.8", role: "detail" },
      {
        d: "M193 75.2H240Q245.8 75.2 245.8 80V86Q245.8 90.6 240.5 90.6H193Q190.4 90.6 190.4 88V77.8Q190.4 75.2 193 75.2Z",
        role: "part",
      },
      {
        d: "M197 78.6V87.4M200.6 78.6V87.4M204.2 78.6V87.4M207.8 78.6V87.4M211.4 78.6V87.4M215 78.6V87.4M218.6 78.6V87.4M222.2 78.6V87.4M225.8 78.6V87.4M229.4 78.6V87.4M233 78.6V87.4M236.6 78.6V87.4",
        role: "detail",
      },
      { d: "M111 89.5Q111 95.8 116 95.8H121.5Q125.8 95.6 126.2 90.5V88.5Z", role: "part" },
      { d: "M113.3 90.2Q113.5 94.3 116.5 94.4H121.3Q124.3 94.2 124.4 90.2", role: "open" },
      {
        d: "M114.4 89.4H115.8C115.6 91.6 116.2 93 118 93.8L117.7 94.5C115.3 93.9 114.3 92.2 114.4 89.4Z",
        role: "part",
      },
      { d: "M109.6 85.4L110.2 90.3L139.5 87.8L140.5 85.4Z", role: "part" },
      { d: "M107.9 75.6Q108.2 72.4 111.2 72.3L163.3 71.4V85.4H109.6Z", role: "part" },
      {
        d: "M141 73.2H157.2Q159.7 73.2 159.7 75.7Q159.7 78.2 157.2 78.2H141Q138.4 78.2 138.4 75.7Q138.4 73.2 141 73.2Z",
        role: "open",
      },
      { circle: [112.5, 82.7, 0.9], role: "part" },
      { circle: [127.1, 82.7, 0.9], role: "part" },
    ],
    axis: [2, 74.4, 318],
  },
  // "Other" has no single silhouette, so it gets a cartridge instead.
  other: {
    parts: [
      {
        d: "M34.5 81.4H38L40 78.6H179L196 83.5H216V116.5H196L179 121.4H40L38 118.6H34.5Z",
        role: "part",
      },
      { d: "M179 79.2V120.8M196 83.5V116.5", role: "detail" },
      { d: "M40 78.6V121.4", role: "detail" },
      { d: "M30.4 78H34.5V122H30.4Q29 122 29 120.6V79.4Q29 78 30.4 78Z", role: "part" },
      { d: "M216 85.7H253C271 85.7 282 92 292 100C282 108 271 114.3 253 114.3H216Z", role: "part" },
      { d: "M225 86.6V113.4M228.5 86.6V113.4", role: "detail" },
      { d: "M216 83.5V116.5", role: "detail" },
    ],
    axis: [23, 100, 313],
  },
  // A rifle-caliber suppressor, mount end left: a threaded mount collar,
  // wrench flats near the mount, and a seam where the end cap seats, flush
  // with the tube and rounded at the muzzle. Between two break lines the
  // tube is cut away on the bore axis: a hatched wall, an empty blast
  // chamber, then the baffle stack, a sleeve of skirts with a cone at each
  // joint, the blast baffle heavier than the rest.
  suppressor: {
    parts: [
      { d: "M29 88.5H47V111.5H29Q27 111.5 27 109.5V90.5Q27 88.5 29 88.5Z", role: "part" },
      { d: "M32 89V111M35 89V111M38 89V111M41 89V111", role: "detail" },
      { d: "M47 80.5H283Q287 80.5 287 84.5V115.5Q287 119.5 283 119.5H47Z", role: "part" },
      { d: "M52 80.8V119.2M82 80.8V119.2", role: "detail" },
      { d: "M52 87H82M52 113H82", role: "detail" },
      { d: "M268 80.8V119.2", role: "detail" },
      { d: "M92 80.5C95 87 89 93 92 100S89 113 92 119.5", role: "open" },
      { d: "M258 80.5C261 87 255 93 258 100S255 113 258 119.5", role: "open" },
      { d: "M92.9 84H258.9M91.1 116H257.1", role: "open" },
      { d: sectionHatch(96, 256, 80.5, 84) + sectionHatch(96, 256, 116, 119.5), role: "detail" },
      { d: "M130 86.5H250V84M130 113.5H250V116", role: "open" },
      { d: BAFFLES.map((x) => `M${x} 84V86.5M${x} 116V113.5`).join(""), role: "detail" },
      ...BAFFLES.map((x, i) => coneBaffle(x, i === 0 ? 3.5 : 2)),
    ],
    axis: [23, 100, 313],
  },
  // A variable-power rifle scope, right side, on a rail with two rings: the
  // ocular bell and its diopter ring at the rear, the magnification ring,
  // the windage turret facing the viewer over the elevation turret, and the
  // objective bell with its lens ring at the muzzle end.
  optic: {
    parts: [
      { d: "M100 120H216V128H100Z", role: "part" },
      { d: ticks(106, 210, 8, 120, 128), role: "detail" },
      { d: "M139 86V70Q139 68 141 68H159Q161 68 161 70V86Z", role: "part" },
      { d: "M139 73.5H161", role: "detail" },
      { d: "M32 82L66 85.5V114.5L32 118Z", role: "part" },
      { d: "M26 80H32V120H26Q24 120 24 118V82Q24 80 26 80Z", role: "part" },
      { d: "M66 85H212V115H66Z", role: "part" },
      { d: "M212 85L244 76H292V124H244L212 115Z", role: "part" },
      { d: "M292 76.5H298Q300 76.5 300 78.5V121.5Q300 123.5 298 123.5H292Z", role: "part" },
      { d: "M244 76V124M284 76V124M44 82.5V117.5", role: "detail" },
      { d: "M76 85V115M108 85V115" + ticks(80, 104, 4, 88, 112), role: "detail" },
      { d: "M113.5 80H128.5Q130 80 130 81.5V120H112V81.5Q112 80 113.5 80Z", role: "part" },
      { d: "M187.5 80H200.5Q202 80 202 81.5V120H184V81.5Q184 80 185.5 80Z", role: "part" },
      { circle: [121, 89, 2], role: "part" },
      { circle: [121, 111, 2], role: "part" },
      { circle: [193, 89, 2], role: "part" },
      { circle: [193, 111, 2], role: "part" },
      { circle: [150, 100, 13], role: "part" },
      { d: "M141 100a9 9 0 1 0 18 0a9 9 0 1 0 -18 0", role: "detail" },
      { d: "M150 88.5V92M150 108V111.5M138.5 100H142M158 100H161.5", role: "detail" },
    ],
    axis: [14, 100, 308],
  },
  // A weapon light on a rail clamp, lens end right: a tail cap with its
  // pressure button, a knurled body and a finned head behind the bezel.
  light: {
    parts: [
      { d: "M114 112H186V126Q186 130 182 130H118Q114 130 114 126Z", role: "part" },
      { d: "M122 130H178V135H122Z", role: "part" },
      { d: ticks(130, 170, 10, 130, 135), role: "detail" },
      { circle: [150, 121, 3.5], role: "part" },
      { d: "M31 94.5H37V105.5H31Q29 105.5 29 103.5V96.5Q29 94.5 31 94.5Z", role: "part" },
      { d: "M42 90H66V110H42Q37 110 37 105V95Q37 90 42 90Z", role: "part" },
      { d: "M66 88H232V112H66Z", role: "part" },
      { d: "M98 88V112M104 88V112" + ticks(72, 92, 4, 91, 109), role: "detail" },
      { d: "M232 88L244 78H288V122H244L232 112Z", role: "part" },
      { d: ticks(252, 280, 7, 80, 120) + "M244 78V122", role: "detail" },
      { d: "M288 75H297Q300 75 300 78V122Q300 125 297 125H288Z", role: "part" },
      { d: "M293 79V121", role: "detail" },
    ],
    axis: [20, 100, 308],
  },
  // A 30-round box magazine, upright and curved forward, with the top
  // round seated between the feed lips and its bullet pointing at the
  // muzzle; the axis runs through that round, where the bore would be.
  magazine: {
    parts: [
      { d: "M112 42.5H141L147 44.5V51.5L141 53.5H112Z", role: "part" },
      { d: "M147 44.5H160Q169 44.5 178 48Q169 51.5 160 51.5H147Z", role: "part" },
      { d: "M113 42.5V53.5M141 42.5V53.5", role: "detail" },
      {
        d: "M112 58C109 100 120 138 136 168H172C156 138 145 100 148 58V53H142L140 58H122L120 53H112Z",
        role: "part",
      },
      { d: "M130 64C128 102 138 136 154 164", role: "detail" },
      { circle: [141, 96, 2.2], role: "part" },
      { circle: [145, 116, 2.2], role: "part" },
      { circle: [151, 136, 2.2], role: "part" },
      { d: "M133 166H175L178 172Q178 175 175 175H131Q128 175 128.5 172Z", role: "part" },
      { d: "M133.5 170.5H174", role: "detail" },
    ],
    axis: [70, 48, 232],
  },
  // A collapsible carbine stock on its buffer tube, right side: butt pad,
  // cheek slot, adjustment lever and the notched tube, ending in the
  // castle-nut flange.
  stock: {
    parts: [
      { d: "M96 91H272V109H96Z", role: "part" },
      { d: ticks(188, 250, 12, 101, 109), role: "detail" },
      { d: "M132 112H162V122Q162 126 158 126H136Q132 126 132 122Z", role: "part" },
      { d: "M140 112V126", role: "detail" },
      {
        d: "M40 70H88C120 70 146 80 172 91V109C152 112 130 118 108 124L48 134Q40 135 40 127Q35 100 40 70Z",
        role: "part",
      },
      { d: "M50 72Q45 100 50 131", role: "detail" },
      {
        d: "M64 88H98Q104 88 104 94V104Q104 110 98 110H64Q58 110 58 104V94Q58 88 64 88Z",
        role: "open",
      },
      { circle: [126, 104, 3], role: "part" },
      { d: "M272 86H280Q283 86 283 89V111Q283 114 280 114H272Z", role: "part" },
      { d: "M277 86V114", role: "detail" },
    ],
    axis: [18, 100, 306],
  },
  // A flat-top upper receiver with its free-float handguard, right side:
  // brass deflector, ejection port cover, forward assist, the top rail all
  // the way along, M-LOK slots, gas block with a front sight post and a
  // birdcage flash hider.
  upper: {
    parts: [
      { d: "M30 84H48V96H30Q28 96 28 94V86Q28 84 30 84Z", role: "part" },
      { d: "M254 94.5H284V105.5H254Z", role: "part" },
      { d: "M48 76H252V84H48Z", role: "part" },
      { d: ticks(54, 246, 8, 76, 84), role: "detail" },
      { d: "M48 84H138V113H48Z", role: "part" },
      { d: "M60 104V96Q60 93 63 93H72V104Z", role: "part" },
      { d: "M78 89H112Q114 89 114 91V101Q114 103 112 103H78Z", role: "part" },
      { d: "M80 104H112", role: "detail" },
      { d: "M120 98Q120 94 124 94H134V102H124Q120 102 120 98Z", role: "part" },
      { d: "M138 84H250Q254 84 254 88V112Q254 116 250 116H138Z", role: "part" },
      { d: "M136 82H144V118H136Z", role: "part" },
      ...[148, 174, 200, 226].flatMap<Part>((x) => [
        {
          d: `M${x + 2} 92H${x + 16}Q${x + 18} 92 ${x + 18} 94Q${x + 18} 96 ${x + 16} 96H${x + 2}Q${x} 96 ${x} 94Q${x} 92 ${x + 2} 92Z`,
          role: "open",
        },
        {
          d: `M${x + 2} 105H${x + 16}Q${x + 18} 105 ${x + 18} 107Q${x + 18} 109 ${x + 16} 109H${x + 2}Q${x} 109 ${x} 107Q${x} 105 ${x + 2} 105Z`,
          role: "open",
        },
      ]),
      { d: "M262 88H274V112H262Z", role: "part" },
      { d: "M265 88L266.5 72H269.5L271 88Z", role: "part" },
      { d: "M280 90H284V110H280Z", role: "part" },
      { d: "M284 92H304Q307 92 307 95V105Q307 108 304 108H284Z", role: "part" },
      {
        d: "M292 92V100M296 92V100M300 92V100M292 100V108M296 100V108M300 100V108",
        role: "detail",
      },
    ],
    axis: [20, 100, 314],
  },
  // A rifle barrel, bare: the barrel extension with its feed ramp, a heavy
  // chamber end tapering to a lighter profile, the gas block journal and
  // the threaded muzzle.
  barrel: {
    parts: [
      {
        d: "M76 89H120L148 92H196V90H214V93H276V94.5H302L304.5 96.5V103.5L302 105.5H276V107H214V110H196V108H148L120 111H76Z",
        role: "part",
      },
      { d: "M120 89V111M205 90V110", role: "detail" },
      {
        d: "M280 105.5L283 94.5M285 105.5L288 94.5M290 105.5L293 94.5M295 105.5L298 94.5",
        role: "detail",
      },
      {
        d: "M38 85.5H70Q74 85.5 76 88V112Q74 114.5 70 114.5H38Q36 114.5 36 112.5V87.5Q36 85.5 38 85.5Z",
        role: "part",
      },
      { d: "M70 85.5V114.5M44 85.5L56 93H66", role: "detail" },
    ],
    axis: [18, 100, 310],
  },
  // A ported muzzle brake: crush washer, wrench flats, swept ports through
  // the side and a rounded nose.
  muzzle: {
    parts: [
      { d: "M78 82H90V118H78Z", role: "part" },
      { d: "M90 74H224Q238 74 244 84V116Q238 126 224 126H90Z", role: "part" },
      { d: "M98 74V126M130 74V126M98 88H130M98 112H130", role: "detail" },
      ...[146, 168, 190].map<Part>((x) => ({
        d: `M${x} 84H${x + 9}L${x + 14} 116H${x + 5}Z`,
        role: "part",
      })),
      { d: "M214 74V126", role: "detail" },
      { d: "M228 90Q234 100 228 110", role: "detail" },
    ],
    axis: [50, 100, 290],
  },
  // A rimfire conversion kit for a pistol laid out as its parts: the slide
  // with its sights, serrations and ejection port, the barrel with its
  // hood, and the recoil spring on its guide rod.
  conversion: {
    parts: [
      { d: "M56 46H66V40H80V46H232V40H246V46H258V78H56Z", role: "part" },
      { d: "M64 52H80M64 56H80M64 60H80M64 64H80", role: "detail" },
      { d: ticks(212, 244, 6, 54, 70), role: "detail" },
      {
        d: "M118 52H176Q180 52 180 56V64Q180 68 176 68H118Q114 68 114 64V56Q114 52 118 52Z",
        role: "open",
      },
      { d: "M62 73H252", role: "detail" },
      { d: "M70 95H120V113H70Z", role: "part" },
      { d: "M120 99.5H258Q262 99.5 262 104Q262 108.5 258 108.5H120Z", role: "part" },
      { d: "M96 95V113", role: "detail" },
      { d: "M62 138H214V142H62Z", role: "part" },
      { d: "M52 132H62V148H52Z", role: "part" },
      { d: "M214 135H224V145H214Z", role: "part" },
      { d: coils(70, 206, 140, 8, 8), role: "open" },
    ],
    axis: [26, 104, 298],
  },
  // A one-piece scope mount on a rail section: two ring bodies over a
  // bar that clamps to the slotted rail with two cross bolts, a ring screw
  // at each end of the split. The axis is the rings' bore.
  mount: {
    parts: [
      { d: slottedRail(34, 286, 118, 138, 46, 16, 8, 7), role: "part" },
      { d: "M34 131H286", role: "detail" },
      { d: "M84 118H106V136H84Z", role: "part" },
      { d: "M204 118H226V136H204Z", role: "part" },
      { circle: [95, 127, 3.4], role: "part" },
      { circle: [215, 127, 3.4], role: "part" },
      { d: "M74 98H236V118H74Z", role: "part" },
      { d: "M96 98V66Q96 56 106 56H122Q132 56 132 66V98Z", role: "part" },
      { d: "M180 98V66Q180 56 190 56H206Q216 56 216 66V98Z", role: "part" },
      { d: "M96 84H132M180 84H216", role: "detail" },
      { circle: [103, 84, 2.4], role: "part" },
      { circle: [125, 84, 2.4], role: "part" },
      { circle: [187, 84, 2.4], role: "part" },
      { circle: [209, 84, 2.4], role: "part" },
    ],
    axis: [20, 72, 300],
  },
  // A two-point sling laid flat: a swivel at each end, the strap with its
  // stitching, the adjuster and the shoulder pad.
  sling: {
    parts: [
      { d: "M48 93H272V107H48Z", role: "part" },
      { d: dashes(60, 106, 96, 4, 3) + dashes(60, 106, 104, 4, 3), role: "detail" },
      { d: dashes(222, 262, 96, 4, 3) + dashes(222, 262, 104, 4, 3), role: "detail" },
      { d: "M76 89H92V111H76Z", role: "part" },
      { d: "M84 89V111", role: "detail" },
      {
        d: "M116 84H204Q212 84 212 92V108Q212 116 204 116H116Q108 116 108 108V92Q108 84 116 84Z",
        role: "part",
      },
      { d: ticks(120, 200, 8, 90, 110), role: "detail" },
      { d: "M226 89H242V111H226Z", role: "part" },
      { d: "M234 89V111", role: "detail" },
      {
        d: "M34 84H46Q56 84 56 94V106Q56 116 46 116H34Q22 116 22 106V94Q22 84 34 84Z",
        role: "part",
      },
      {
        d: "M36 92H44Q48 92 48 96V104Q48 108 44 108H36Q32 108 32 104V96Q32 92 36 92Z",
        role: "open",
      },
      {
        d: "M274 84H286Q298 84 298 94V106Q298 116 286 116H274Q264 116 264 106V94Q264 84 274 84Z",
        role: "part",
      },
      {
        d: "M276 92H284Q288 92 288 96V104Q288 108 284 108H276Q272 108 272 104V96Q272 92 276 92Z",
        role: "open",
      },
    ],
    axis: [14, 100, 306],
  },
  // A hard rifle case, closed: carry handle, lid seam, three latches,
  // end bands, a pressure valve and feet.
  case: {
    parts: [
      { d: "M122 64V57Q122 52 127 52H193Q198 52 198 57V64Z", role: "part" },
      { d: "M132 64V60Q132 58 134 58H186Q188 58 188 60V64", role: "open" },
      { d: "M52 134H72V140Q72 142 70 142H54Q52 142 52 140Z", role: "part" },
      { d: "M248 134H268V140Q268 142 266 142H250Q248 142 248 140Z", role: "part" },
      {
        d: "M30 70Q30 64 36 64H284Q290 64 290 70V128Q290 134 284 134H36Q30 134 30 128Z",
        role: "part",
      },
      { d: "M44 64V134M276 64V134", role: "detail" },
      { d: "M30 90H290", role: "open" },
      { d: "M30 94H290", role: "detail" },
      ...[70, 160, 250].flatMap<Part>((x) => [
        { d: `M${x - 8} 80H${x + 8}V92H${x - 8}Z`, role: "part" },
        {
          d: `M${x - 9} 92H${x + 9}V104Q${x + 9} 108 ${x + 5} 108H${x - 5}Q${x - 9} 108 ${x - 9} 104Z`,
          role: "part",
        },
        { d: `M${x - 4} 98H${x + 4}`, role: "detail" },
      ]),
      { circle: [273, 76, 3], role: "part" },
    ],
    axis: [14, 118, 306],
  },
  // The generic accessory, a rail-clamp bipod with one leg deployed
  // forward: clamp body and thumb knob, pivot housing, a telescoping leg
  // with detents, and the rubber foot.
  accessory: {
    parts: [
      { d: tube(167, 116, 184, 164, 5.5), role: "part" },
      {
        d:
          across(172, 130, 0.342, 0.94, 5.5) +
          across(176, 142, 0.342, 0.94, 5.5) +
          across(181, 154, 0.342, 0.94, 5.5),
        role: "detail",
      },
      { d: tube(184, 164, 188, 174, 9), role: "part" },
      { d: tube(150, 72, 168.5, 123, 8.5), role: "part" },
      { d: "M110 28H206Q210 28 210 32V56H106V32Q106 28 110 28Z", role: "part" },
      { d: "M210 33H230Q234 33 234 37V47Q234 51 230 51H210Z", role: "part" },
      { d: ticks(214, 228, 5, 34, 50), role: "detail" },
      { circle: [128, 42, 6], role: "part" },
      { d: "M124 42H132", role: "detail" },
      { d: "M132 56H168V66Q168 70 164 70H136Q132 70 132 66Z", role: "part" },
      { circle: [150, 72, 9], role: "part" },
      { circle: [150, 72, 3.2], role: "part" },
    ],
    axis: [40, 43, 280],
  },
};
