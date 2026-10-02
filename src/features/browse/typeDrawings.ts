/*
 * The artwork behind TypeDrawing: one side elevation per firearm type,
 * muzzle to the right, in a 320×200 box. Each drawing is a list of parts
 * painted back to front.
 *
 * Each firearm is traced from a side-on photograph of a real model so its
 * proportions hold up. Muzzle-right puts the right side toward the viewer,
 * so only right-side features are drawn (ejection ports, not slide stops).
 *
 * The suppressor (specs/005-regulated-item-types FR-001, research.md §4) was
 * drawn for this project, traced from side-on photographs of rifle
 * suppressors for proportion (a tube about 6:1 in length to diameter), with
 * no brand marks. It is a partial section, cut away between two break lines
 * to show a generic cone-baffle stack. Like the rest of the source it is
 * GPL-3.0-only.
 *
 * The fourteen accessory kinds (specs/006-accessory-links FR-002, FR-007a,
 * research.md §15) are `optic`, `light`, `magazine`, `stock`, `upper`,
 * `barrel`, `trigger`, `muzzle`, `conversion`, `mount`, `bipod`, `sling`,
 * `case` and `accessory` (the kind "Other"). They were drawn for this
 * project from the published dimensions of common patterns (an AR-15
 * magazine and its 5.56 cartridge, a bolt-action rifle stock, a carbine
 * barrel's profile, the AR-15 trigger group, a 30 mm scope), each at a
 * stated scale so its parts keep their real proportions, with no brand
 * marks, in the same 320×200 box and line roles. Each is a side elevation,
 * muzzle or objective right, except the bipod, which is seen from the
 * front so both legs show, and the case, which lies flat. Like the rest of
 * the source they are GPL-3.0-only.
 */

export type Part =
  | { d: string; role: "part" | "open" | "detail" }
  | { circle: [number, number, number]; role: "part" };

export interface Drawing {
  parts: Part[];
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

/** A point [x, y] in drawing units. */
type Point = [number, number];

/** A round part's side profile: [x, radius] stations along its axis. */
type Profile = [number, number][];

/** The outline of a round part lying on the horizontal line `cy`, from its
 * profile: along the top edge, then back along the bottom. */
function revolve(cy: number, profile: Profile): string {
  const top = profile.map(([x, rad]) => `${r(x)} ${r(cy - rad)}`);
  const bottom = [...profile].reverse().map(([x, rad]) => `${r(x)} ${r(cy + rad)}`);
  return `M${top.join("L")}L${bottom.join("L")}Z`;
}

/** Profile stations easing from radius r0 at x0 to r1 at x1. */
function ease(x0: number, r0: number, x1: number, r1: number, steps = 8): Profile {
  const out: Profile = [];
  for (let i = 1; i < steps; i++) {
    const t = i / steps;
    out.push([x0 + (x1 - x0) * t, r0 + (r1 - r0) * t * t * (3 - 2 * t)]);
  }
  return out;
}

/** The profile's radius at x, by linear interpolation between stations. */
function radiusAt(profile: Profile, x: number): number {
  for (let i = 1; i < profile.length; i++) {
    const [xa, ra] = profile[i - 1];
    const [xb, rb] = profile[i];
    if (x >= xa && x <= xb)
      return xb === xa ? Math.min(ra, rb) : ra + ((rb - ra) * (x - xa)) / (xb - xa);
  }
  return 0;
}

/** A line across a round part at each x, where one section meets the next. */
function rings(profile: Profile, cy: number, xs: number[]): string {
  return xs
    .map((x) => {
      const rad = radiusAt(profile, x);
      return `M${r(x)} ${r(cy - rad)}V${r(cy + rad)}`;
    })
    .join("");
}

/** A circle as a path, for a circle drawn in the detail role. */
function ring(cx: number, cy: number, rad: number): string {
  return `M${r(cx - rad)} ${cy}a${rad} ${rad} 0 1 0 ${2 * rad} 0a${rad} ${rad} 0 1 0 ${-2 * rad} 0`;
}

/** A rectangle with rounded corners. */
function roundRect(x: number, y: number, w: number, h: number, rad: number): string {
  return (
    `M${x + rad} ${y}H${x + w - rad}Q${x + w} ${y} ${x + w} ${y + rad}V${y + h - rad}` +
    `Q${x + w} ${y + h} ${x + w - rad} ${y + h}H${x + rad}Q${x} ${y + h} ${x} ${y + h - rad}` +
    `V${y + rad}Q${x} ${y} ${x + rad} ${y}Z`
  );
}

/** A slot with round ends, `w` long and `h` high, from x, centred on cy. */
function slot(x: number, cy: number, w: number, h: number): string {
  const k = h / 2;
  return `M${x + k} ${cy - k}H${x + w - k}A${k} ${k} 0 0 1 ${x + w - k} ${cy + k}H${x + k}A${k} ${k} 0 0 1 ${x + k} ${cy - k}Z`;
}

/** A hexagon with flats top and bottom: a nut seen end on. */
function hexagon(cx: number, cy: number, rad: number): string {
  const pts = [0, 60, 120, 180, 240, 300].map((deg) => {
    const a = (deg * Math.PI) / 180;
    return `${r(cx + rad * Math.cos(a))} ${r(cy + rad * Math.sin(a))}`;
  });
  return `M${pts.join("L")}Z`;
}

/** Thread crests across a threaded length: slanted strokes `pitch` apart. */
function threads(x0: number, x1: number, pitch: number, y0: number, y1: number): string {
  let d = "";
  for (let x = x0; x + pitch <= x1 + 0.01; x += pitch) d += `M${r(x)} ${y1}L${r(x + pitch)} ${y0}`;
  return d;
}

/** A point on a cubic Bézier curve. */
function bezier([p0, p1, p2, p3]: Point[], t: number): Point {
  const u = 1 - t;
  const [a, b, c, e] = [u * u * u, 3 * u * u * t, 3 * u * t * t, t * t * t];
  return [
    a * p0[0] + b * p1[0] + c * p2[0] + e * p3[0],
    a * p0[1] + b * p1[1] + c * p2[1] + e * p3[1],
  ];
}

/** A band of half-width `half` along a polyline: a strap's two edges. */
function ribbon(points: Point[], half: number): string {
  const normals = points.map((_, i) => {
    const [ax, ay] = points[Math.max(0, i - 1)];
    const [bx, by] = points[Math.min(points.length - 1, i + 1)];
    const length = Math.hypot(bx - ax, by - ay);
    return [(-(by - ay) / length) * half, ((bx - ax) / length) * half];
  });
  const left = points.map(([x, y], i) => `${r(x + normals[i][0])} ${r(y + normals[i][1])}`);
  const right = points.map(([x, y], i) => `${r(x - normals[i][0])} ${r(y - normals[i][1])}`);
  return `M${left.join("L")}L${right.reverse().join("L")}Z`;
}

/** Stitching inset `inset` from each edge of a strap along a polyline. */
function stitching(points: Point[], inset: number): string {
  let d = "";
  for (let i = 0; i + 1 < points.length; i++) {
    const [ax, ay] = points[i];
    const [bx, by] = points[i + 1];
    const length = Math.hypot(bx - ax, by - ay);
    const [nx, ny] = [(-(by - ay) / length) * inset, ((bx - ax) / length) * inset];
    const [ex, ey] = [ax + (bx - ax) * 0.55, ay + (by - ay) * 0.55];
    for (const k of [1, -1]) {
      d += `M${r(ax + k * nx)} ${r(ay + k * ny)}L${r(ex + k * nx)} ${r(ey + k * ny)}`;
    }
  }
  return d;
}

/** The Glock 17's sights, behind its slide (shared by the handgun and the
 * conversion kit). */
const GLOCK_SIGHTS: Part[] = [
  { d: "M67.8 24L71 19Q71.5 18.3 72.4 18.3H75.2L80.8 24Z", role: "part" },
  { d: "M262.8 24.3L264.3 21.2H267.4L270.6 24.3Z", role: "part" },
];

/** The Glock 17's slide, right side: rear serrations, the ejection port
 * with the barrel's hood in it, and the extractor. */
const GLOCK_SLIDE: Part[] = [
  { d: "M58.5 49.5L58.3 28Q58.5 24 62.5 23.6L271 24.3Q277.8 24.6 278 30V49.5Z", role: "part" },
  {
    d: "M65.5 27V47M71 27V47M76.5 27V47M82 27V47M87.5 27V47M93 27V47M98.5 27V47M104 27V47",
    role: "detail",
  },
  { d: "M141 24.1V36.5H184V24.4", role: "open" },
  { d: "M145.5 24.2V36.5M145.5 33H184", role: "detail" },
  { d: "M127 33H141M127 33V37.5H141", role: "detail" },
];

/** Knurling: a diamond lattice across a horizontal band, as rows of
 * strokes at 45° each way, `pitch` apart. */
function knurl(x0: number, x1: number, top: number, bottom: number, pitch = 4): string {
  let d = "";
  for (let y = top; y + pitch <= bottom + 0.01; y += pitch) {
    for (let x = x0; x + pitch <= x1 + 0.01; x += pitch) {
      d += `M${r(x)} ${r(y + pitch)}L${r(x + pitch)} ${r(y)}M${r(x)} ${r(y)}L${r(x + pitch)} ${r(y + pitch)}`;
    }
  }
  return d;
}

/** A coil spring seen from the side, from (x0, y0) to (x1, y1): the wire
 * zigzagging between its two edges, `pitch` apart along its length. */
function spring(
  x0: number,
  y0: number,
  x1: number,
  y1: number,
  half: number,
  pitch: number,
): string {
  const length = Math.hypot(x1 - x0, y1 - y0);
  const [ux, uy] = [(x1 - x0) / length, (y1 - y0) / length];
  const [nx, ny] = [-uy * half, ux * half];
  const pts: string[] = [];
  for (let i = 0, a = 0; a <= length + 0.01; i++, a += pitch / 2) {
    const k = i % 2 === 0 ? 1 : -1;
    pts.push(`${r(x0 + ux * a + k * nx)} ${r(y0 + uy * a + k * ny)}`);
  }
  return `M${pts.join("L")}`;
}

/** The weapon light's side profile: tail cap, body, the head's flare and
 * the bezel, at 1.73 units to the millimetre. */
const LIGHT_PROFILE: Profile = [
  [31, 21],
  [33, 23],
  [58, 23],
  [58, 21],
  [196, 21],
  ...ease(196, 21, 214, 33),
  [214, 33],
  [270, 33],
  [270, 34.5],
  [284, 34.5],
  [286, 33],
];

const LIGHT_BODY = revolve(112, LIGHT_PROFILE);

/** The soft case's padded carry handle, arched over its two patches. */
const CASE_HANDLE = ribbon(
  Array.from({ length: 21 }, (_, i) =>
    bezier(
      [
        [131, 62],
        [134, 36],
        [184, 36],
        [187, 62],
      ],
      i / 20,
    ),
  ),
  3.5,
);

/** The scope's x in drawing units from millimetres along it, eyecup at 0. */
function sx(mm: number): number {
  return r(20 + mm * 0.85);
}

/** The scope's side profile, from millimetre stations at 0.85 units/mm. */
const SCOPE_PROFILE: Profile = (
  [
    [0, 19.5],
    [1.5, 21.5],
    [6, 21.5],
    [6, 22],
    [62, 22],
    [62, 20.5],
    [66, 20.5],
    [66, 21.5],
    [96, 21.5],
    [96, 20],
    ...ease(96, 20, 118, 15),
    [118, 15],
    [146, 15],
    ...ease(146, 15, 152, 19, 4),
    [152, 19],
    [198, 19],
    ...ease(198, 19, 204, 15, 4),
    [204, 15],
    [252, 15],
    ...ease(252, 15, 284, 26),
    [284, 26],
    [322, 26],
    [322, 26.5],
    [329, 26.5],
    [330, 25.5],
  ] as Profile
).map(([mm, rad]) => [sx(mm), rad * 0.85]);

const SCOPE_BODY = revolve(104, SCOPE_PROFILE);

/** The elevation turret: its skirt on the saddle, then the knurled cap. */
const SCOPE_ELEVATION =
  `M${sx(162)} 90V83.6H${sx(160)}V73.4Q${sx(160)} 70.9 ${r(sx(160) + 2.5)} 70.9` +
  `H${r(sx(190) - 2.5)}Q${sx(190)} 70.9 ${sx(190)} 73.4V83.6H${sx(188)}V90Z`;

/** The 30-round magazine's geometry: the walls run straight down from the
 * feed lips to `bend`, then on arcs about one centre through `sweep`
 * radians, so the floorplate lies along a radius. */
const MAG = { rear: 124, front: 180, lips: 28, bend: 70, outer: 290, sweep: 0.372 };
const MAG_CENTRE: Point = [MAG.rear + MAG.outer, MAG.bend];
const MAG_INNER = MAG.outer - (MAG.front - MAG.rear);

/** A point on the magazine's arcs at radius `rad` and angle `a`. */
function magPoint(rad: number, a: number): Point {
  return [r(MAG_CENTRE[0] - rad * Math.cos(a)), r(MAG_CENTRE[1] + rad * Math.sin(a))];
}

const MAG_BODY = (() => {
  const [rx, ry] = magPoint(MAG.outer, MAG.sweep);
  const [fx, fy] = magPoint(MAG_INNER, MAG.sweep);
  const { rear, front, lips, bend, outer } = MAG;
  return (
    `M${rear} ${bend}A${outer} ${outer} 0 0 0 ${rx} ${ry}L${fx} ${fy}` +
    `A${MAG_INNER} ${MAG_INNER} 0 0 1 ${front} ${bend}V34H${front - 8}` +
    `Q${front - 14} 34 ${front - 18} 30L${front - 20} ${lips}H${rear + 2}Q${rear} ${lips} ${rear} ${lips + 2}Z`
  );
})();

/** Two stamped ribs along the magazine's side, following its curve. */
const MAG_RIBS = [272, 269, 252, 249]
  .map((rad) => {
    const [x, y] = magPoint(rad, MAG.sweep - 0.035);
    return `M${MAG_CENTRE[0] - rad} 44V${MAG.bend}A${rad} ${rad} 0 0 0 ${x} ${y}`;
  })
  .join("");

/** The floorplate, square to the curve and lapping both walls. */
const MAG_FLOORPLATE = (() => {
  const a = MAG.sweep;
  const [tx, ty] = [Math.sin(a) * 6, Math.cos(a) * 6];
  const [ax, ay] = magPoint(MAG_INNER - 2.5, a);
  const [bx, by] = magPoint(MAG.outer + 2.5, a);
  return `M${ax} ${ay}L${bx} ${by}L${r(bx + tx)} ${r(by + ty)}L${r(ax + tx)} ${r(ay + ty)}Z`;
})();

/** A 5.56×45 mm cartridge at the magazine's scale, head at the rear wall:
 * rim, extractor groove, tapered body, shoulder, neck and spitzer bullet. */
const CARTRIDGE_X = MAG.rear + 2.5;
const CARTRIDGE_556 = revolve(
  27,
  (
    [
      [0, 4.2],
      [1, 4.2],
      [1, 3.7],
      [2.6, 3.7],
      [2.6, 4.2],
      [32.1, 4.0],
      [34.8, 2.8],
      [39.3, 2.8],
      [39.3, 2.5],
      ...[0.15, 0.3, 0.45, 0.6, 0.75, 0.9].map((s): [number, number] => [
        39.3 + 11.2 * s,
        2.5 * (1 - s) ** 0.6,
      ]),
      [50.5, 0.4],
    ] as Profile
  ).map(([x, rad]) => [CARTRIDGE_X + x, rad]),
);

/** The barrel's x in drawing units from millimetres along it. */
function bx(mm: number): number {
  return r(16 + mm * 0.68);
}

/** An AR-15 carbine barrel's profile, from millimetre stations. */
const BARREL_PROFILE: Profile = (
  [
    [0, 12.7],
    [28, 12.7],
    [28, 14.5],
    [33, 14.5],
    [33, 12.45],
    [60, 12.45],
    ...ease(60, 12.45, 80, 7.95),
    [80, 7.95],
    [188, 7.95],
    [188, 9.5],
    [260, 9.5],
    [260, 8.7],
    [275, 8.7],
    [275, 9.5],
    [410, 9.5],
    [410, 6.35],
    [424, 6.35],
    [425, 5.6],
  ] as Profile
).map(([mm, rad]) => [bx(mm), rad * 0.68]);

const BARREL = revolve(100, BARREL_PROFILE);

/** The sling's centre line: down from the rear hook, in a shallow hang,
 * and up to the front hook. */
const SLING_REAR: Point[] = [
  [62, 64],
  [62, 108],
  [110, 126],
  [160, 126],
];
const SLING_FRONT: Point[] = [
  [160, 126],
  [210, 126],
  [258, 108],
  [258, 64],
];
const SLING_PATH: Point[] = [
  ...Array.from({ length: 24 }, (_, i) => bezier(SLING_REAR, i / 24)),
  ...Array.from({ length: 25 }, (_, i) => bezier(SLING_FRONT, i / 24)),
];

/** An HK-style snap hook at x, its hook opening toward `side` (-1 left,
 * 1 right): a flat steel body with a slot the webbing folds round, a
 * tongue rising to the hook, and the wire gate closing it. The webbing's
 * folded end, box-stitched, lies over the hook's bar. */
function snapHook(x: number, side: number): Part[] {
  const h = (dx: number) => r(x + side * dx);
  return [
    {
      d:
        `M${h(-11)} 72V60Q${h(-11)} 55 ${h(-6)} 53L${h(-3)} 52V33Q${h(-3)} 23 ${h(6)} 23` +
        `Q${h(15)} 23 ${h(15)} 32V42H${h(10)}V33Q${h(10)} 28 ${h(6)} 28Q${h(2)} 28 ${h(2)} 33` +
        `V52L${h(6)} 53Q${h(11)} 55 ${h(11)} 60V72Z`,
      role: "part",
    },
    { d: `M${h(-7.5)} 60H${h(7.5)}V67H${h(-7.5)}Z`, role: "open" },
    { d: `M${h(12.5)} 42L${h(2)} 50`, role: "open" },
    { d: `M${x - 7} 70H${x + 7}V92H${x - 7}Z`, role: "part" },
    {
      d: `M${x - 4} 76H${x + 4}V88H${x - 4}ZM${x - 4} 76L${x + 4} 88M${x + 4} 76L${x - 4} 88`,
      role: "detail",
    },
  ];
}

/** The adjuster on the rear run: a buckle across the strap, and the pull
 * tab looping out of it to the outside of the hang. */
const [SLING_BUCKLE, SLING_TAB] = (() => {
  const t = 0.45;
  const [px, py] = bezier(SLING_REAR, t);
  const [qx, qy] = bezier(SLING_REAR, t + 0.01);
  const length = Math.hypot(qx - px, qy - py);
  const [ux, uy] = [(qx - px) / length, (qy - py) / length];
  const [nx, ny] = [uy, -ux];
  const at = (a: number, b: number) => `${r(px + ux * a + nx * b)} ${r(py + uy * a + ny * b)}`;
  return [
    `M${at(-3.5, -10)}L${at(3.5, -10)}L${at(3.5, 10)}L${at(-3.5, 10)}Z`,
    `M${at(-2, -9)}L${at(18, -11)}Q${at(22, -11)} ${at(22, -15)}L${at(22, -16)}Q${at(22, -20)} ${at(18, -20)}L${at(-2, -18)}Z`,
  ];
})();

/** Where each baffle's cone meets its skirt; the first is the blast baffle. */
const BAFFLES = [130, 145, 160, 175, 190, 205, 220, 235];

export const DRAWINGS: Record<string, Drawing> = {
  // Glock 17, right side: ejection port and extractor show; the slide stop
  // lever and magazine catch are on the left side only.
  handgun: {
    parts: [
      ...GLOCK_SIGHTS,
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
      ...GLOCK_SLIDE,
    ],
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
  },
  // A 3-15×44 rifle scope on its own, eyepiece left, at 0.85 units to the
  // millimetre: the eyecup, the ocular housing with its diopter ring, the
  // power ring and its throw lever, a 30 mm tube, the turret saddle with
  // the elevation turret on top and the windage cap facing the viewer, and
  // the objective bell.
  optic: {
    parts: [
      { d: SCOPE_ELEVATION, role: "part" },
      { d: ticks(sx(162), sx(188), 2.2, 72.5, 82.5), role: "detail" },
      { d: SCOPE_BODY, role: "part" },
      { d: rings(SCOPE_PROFILE, 104, [6, 24, 62, 66, 96, 284, 322].map(sx)), role: "detail" },
      { d: ticks(sx(9), sx(21), 3, 104 - 17, 104 + 17), role: "detail" },
      { d: ticks(sx(70), sx(92), 3.2, 104 - 16, 104 + 16), role: "detail" },
      {
        d: `M${sx(76)} 85.7L${sx(77.5)} 80.5H${sx(82.5)}L${sx(84)} 85.7Z`,
        role: "part",
      },
      { circle: [sx(175), 104, 12.8], role: "part" },
      { d: ring(sx(175), 104, 9.6), role: "detail" },
      { d: `M${sx(175)} 96V100.5`, role: "detail" },
    ],
  },
  // A weapon light hanging from its rail mount, lens right, at 1.73 units
  // to the millimetre: the tail cap's pressure button and grip rings, a
  // knurled 1-inch body, the head flaring out to a bezel round the lens,
  // and the mount's ring round the body, with its rail clamp and thumb
  // screw above.
  light: {
    parts: [
      { d: "M31 101Q24 101 24 112Q24 123 31 123Z", role: "part" },
      { d: "M120 89V70H146V89Z", role: "part" },
      { d: LIGHT_BODY, role: "part" },
      { d: rings(LIGHT_PROFILE, 112, [58, 214, 270]), role: "detail" },
      { d: ticks(37, 53, 4, 112 - 19, 112 + 19), role: "detail" },
      { d: knurl(70, 184, 94, 130, 6), role: "detail" },
      { d: ticks(228, 256, 14, 112 - 33, 112 + 33), role: "detail" },
      { d: "M282 81V143", role: "detail" },
      { d: roundRect(118, 86, 30, 52, 3), role: "part" },
      { d: "M118 99H148M118 125H148", role: "detail" },
      { d: "M106 72V57Q106 54 109 54H113V62H153V54H157Q160 54 160 57V72Z", role: "part" },
      { circle: [133, 66, 4.5], role: "part" },
      { d: "M130 66H136", role: "detail" },
    ],
  },
  // An AR-15 30-round magazine, upright, at 0.88 units to the millimetre:
  // straight below the feed lips, then curved forward on concentric arcs,
  // so the floorplate sits square to the curve. The top round is a 5.56
  // cartridge drawn to the same scale, held by the lips with its bullet
  // just short of the front wall.
  magazine: {
    parts: [
      { d: CARTRIDGE_556, role: "part" },
      { d: `M${CARTRIDGE_X + 2.6} 22.8V31.2M${CARTRIDGE_X + 39.3} 24.2V29.8`, role: "detail" },
      { d: MAG_BODY, role: "part" },
      { d: MAG_RIBS, role: "detail" },
      { d: `M${MAG.rear + 2} 32H${MAG.rear + 38}`, role: "detail" },
      { d: MAG_FLOORPLATE, role: "part" },
    ],
  },
  // A bolt-action rifle stock, butt left, at 8.13 units to the inch (0.32
  // to the millimetre): the recoil pad and its spacer, a straight comb
  // falling to the wrist, the pistol grip and its cap, the notch the bolt
  // handle sits in, the bottom metal's inletting, the barrel channel along
  // a flat-bottomed forend, and a sling swivel stud at each end.
  stock: {
    parts: [
      { d: "M41 115.6V120H43.8V115.2Z", role: "part" },
      { circle: [42.4, 121.6, 2], role: "part" },
      { d: "M272.7 96.8V101H275.5V96.6Z", role: "part" },
      { circle: [274.1, 102.6, 2], role: "part" },
      {
        d: "M26.1 77.8L91.2 77C100.9 77 102.6 84.3 109.1 85.1L120.4 85.1L123.7 81.5L131.8 81.5Q132.6 87.2 136.7 87.2Q140.8 87.2 141.6 81.5L297.7 81.9Q302.6 81.9 302.6 86.8V90.8Q302.6 95.3 297.7 95.3L192.8 101L126.9 101C118.8 101.4 114.7 107.1 113.9 114L101.7 115.6C99.3 109.5 94.4 106.3 86.3 105.9L26.1 119.3Z",
        role: "part",
      },
      { d: "M26.1 77.8H20Q18 77.8 18 80.7V116.4Q18 119.3 20.4 119.3H26.1Z", role: "part" },
      { d: "M24.5 77.8V119.3", role: "detail" },
      { d: "M114.3 112L100.9 113.6", role: "detail" },
      { d: "M128.6 101V98.3H190.4V101", role: "detail" },
      { d: "M195.2 84.3L294.4 84.7", role: "detail" },
    ],
  },
  // A flat-top upper receiver with a 15-inch free-float handguard and a
  // 16-inch barrel, at 0.46 units to the millimetre: the charging handle's
  // latch, the forward assist and brass deflector, the closed ejection
  // port cover, the pivot and takedown lugs, the top rail running the
  // length of receiver and handguard, M-LOK slots, and an A2 flash hider.
  upper: {
    parts: [
      { d: "M23 106V112Q23 114 25 114H31Q33 114 33 112V106Z", role: "part" },
      { d: "M93 106V113Q93 116 96 116H100Q103 116 103 113V106Z", role: "part" },
      { circle: [28, 110.5, 1.4], role: "part" },
      { circle: [98, 112, 1.4], role: "part" },
      { d: "M21 89H15Q13 89 13 91V93Q13 95 15 95H21Z", role: "part" },
      { d: "M20 89H103V104Q103 108 99 108H24Q20 108 20 104Z", role: "part" },
      { d: slottedRail(20, 276, 85, 89, 23, 4.6, 2.4, 1.6), role: "part" },
      { d: "M38 89L48 89V99Q44 99 41 95Z", role: "part" },
      { d: "M48 92.5H71V100.5H48Z", role: "part" },
      { d: "M48 102H72M50 96.5H69", role: "detail" },
      { d: "M24 96Q24 92 28 92H34L40 100L34 106H28Q24 106 24 102Z", role: "part" },
      { circle: [30.5, 99, 3.6], role: "part" },
      { d: "M76 96H90Q92 96 92 98V100Q92 102 90 102H76Z", role: "open" },
      { d: "M105 89H272Q276 89 276 93V107Q276 111 272 111H105Z", role: "part" },
      { d: "M110 89V111", role: "detail" },
      ...[116, 138, 160, 182, 204, 226, 248].map<Part>((x) => ({
        d: slot(x, 98.3, 16, 3.4),
        role: "open",
      })),
      ...[127, 149, 171, 193, 215, 237].map<Part>((x) => ({
        d: slot(x, 105.6, 13, 2.2),
        role: "open",
      })),
      { d: "M276 95.6H283V104.4H276Z", role: "part" },
      {
        d: "M283 94.5H307Q309 94.5 309 96.5V103.5Q309 105.5 307 105.5H283Z",
        role: "part",
      },
      { d: "M289 96.4H302M289 98.2H302M289 101.6H302M289 103.4H302", role: "detail" },
    ],
  },
  // An AR-15 carbine barrel, bare, breech left, at 0.68 units to the
  // millimetre: the barrel extension, the flange and its index pin, the
  // chamber section tapering to the thin profile, the gas block journal,
  // the heavier profile forward of it with the M4 cut, and the threaded
  // muzzle.
  barrel: {
    parts: [
      { d: `M${bx(29.5)} 88V91H${bx(31.5)}V88Z`, role: "part" },
      { d: BARREL, role: "part" },
      {
        d: rings(BARREL_PROFILE, 100, [28.01, 33, 60, 188.01, 226, 260.01, 275, 410].map(bx)),
        role: "detail",
      },
      { d: threads(bx(411), bx(423), 1.4, 95.7, 104.3), role: "detail" },
    ],
  },
  // An AR-15 trigger group, assembled and out of the receiver, at 2.6
  // units to the millimetre: the hammer standing (fired) on the front pin,
  // its hook at the back of its head; the trigger on the rear pin, its
  // tail reaching back to where the safety selector stops it, its nose
  // forward under the hammer, and a curved blade; the disconnector riding
  // on top of the trigger; and the hammer spring round the front pin with
  // its leg resting on the trigger pin.
  trigger: {
    parts: [
      {
        d: "M112 106V98Q112 94 116 94H166L172 87Q174 85 177 87L176 94L170 106Z",
        role: "part",
      },
      {
        d: "M98 104L128 101L176 104L182 99L186 101L184 112L164 118C158 134 154 150 160 166Q164 174 158 175C148 170 142 146 144 120L104 116Q96 115 96 110Q96 105 98 104Z",
        role: "part",
      },
      { d: "M150 120C149 138 151 154 157 168", role: "detail" },
      {
        d: "M181 108Q180 96 185 92L183 50H188L187.5 45H182.5L182 38Q183 30 192 29.5L206 30Q212 30.5 212 36.5L211 52L204 70L203 96Q205 104 202 112Q194 118 186 115Q181 112 181 108Z",
        role: "part",
      },
      { d: "M211 37L210.2 52", role: "detail" },
      { d: ring(192, 104, 6.5) + ring(137, 108, 6), role: "detail" },
      { d: "M186 101Q164 97 138 104", role: "open" },
      { circle: [192, 104, 3.2], role: "part" },
      { circle: [137, 108, 3.2], role: "part" },
    ],
  },
  // A ported muzzle brake on a barrel's threaded end, at 3.6 units to the
  // millimetre: the barrel broken off at the left, the jam nut that times
  // the brake, wrench flats, three side ports with the largest first, and
  // a chamfered front face.
  muzzle: {
    parts: [
      { d: "M22 65.8H64V134.2H22", role: "part" },
      { d: "M22 65.8C27 80 17 92 22 100S17 120 22 134.2", role: "part" },
      { d: "M62 56H86V144H62Z", role: "part" },
      { d: "M62 78H86M62 122H86", role: "detail" },
      {
        d: "M86 60.4H274L288 70Q291 72 291 76V124Q291 128 288 130L274 139.6H86Z",
        role: "part",
      },
      { d: "M88 76H122M88 124H122", role: "detail" },
      { d: "M274 60.4V139.6M281 65.2V134.8", role: "detail" },
      { d: roundRect(130, 70, 36, 60, 6), role: "open" },
      { d: roundRect(180, 72, 28, 56, 6), role: "open" },
      { d: roundRect(222, 74, 24, 52, 6), role: "open" },
      {
        d: "M134 94H162M134 106H162M184 95H204M184 105H204M226 96H242M226 104H242",
        role: "detail",
      },
    ],
  },
  // A .22 LR conversion kit for a Glock 17, laid out as its parts at the
  // handgun drawing's scale: the slide (the handgun's own), the barrel
  // with its chamber block and lug, the recoil spring on its guide rod,
  // and a single-stack magazine on its side, lips right, with the top
  // round and the follower button in its slot.
  conversion: {
    parts: [
      ...GLOCK_SIGHTS,
      ...GLOCK_SLIDE,
      { d: "M102 96L110 108H130Q134 108 134 104V96Z", role: "part" },
      { d: "M136 78.4H228Q231 78.4 231 81.4V90.6Q231 93.6 228 93.6H136Z", role: "part" },
      { d: "M96 74H136V97H96Z", role: "part" },
      { d: "M100 80H122M100 80V90H122", role: "detail" },
      { d: "M112 120H117V136H112Z", role: "part" },
      { d: tube(117, 128, 216, 128, 2.2), role: "part" },
      { d: coils(118, 204, 128, 6.5, 6), role: "open" },
      { d: "M204 121H209V135H204Z", role: "part" },
      {
        d: "M207 181.5H215V179.5H214.4V163H214.6Q214.6 154 211 151.5Q207.4 154 207.4 163H207.6V179.5H207Z",
        role: "part",
      },
      { d: "M78 145H206L209 149V179L206 183H78Z", role: "part" },
      { d: "M72 143H80V185H72Q70 185 70 183V145Q70 143 72 143Z", role: "part" },
      { d: slot(94, 164, 100, 4), role: "open" },
      { circle: [176, 164, 5], role: "part" },
    ],
  },
  // A one-piece 30 mm cantilever scope mount on a length of Picatinny
  // rail, at 1.6 units to the millimetre: the rail's slots and dovetail
  // line, the clamp over the rail with a cross-bolt nut at each recoil
  // lug, the spine carried forward of the clamp to the front ring with a
  // lightening cut through it, and each ring split at the scope's axis
  // with a cap ear and its screws.
  mount: {
    parts: [
      { d: slottedRail(18, 302, 160, 176, 26, 16, 8.4, 4.8), role: "part" },
      { d: "M18 167H302", role: "detail" },
      { d: "M56 146H200V169H56Z", role: "part" },
      { d: "M56 160H200", role: "detail" },
      {
        d: "M74 112H246V124Q246 128 242 129L200 146H74Z",
        role: "part",
      },
      { d: roundRect(114, 121, 72, 16, 8), role: "open" },
      { d: hexagon(90, 154, 6) + hexagon(166, 154, 6), role: "part" },
      { circle: [90, 154, 2.4], role: "part" },
      { circle: [166, 154, 2.4], role: "part" },
      ...[74, 214].flatMap<Part>((x) => [
        {
          d: `M${x} 114V55Q${x} 50 ${x + 5} 50H${x + 27}Q${x + 32} 50 ${x + 32} 55V114Z`,
          role: "part",
        },
        {
          d: `M${x + 4} 69H${x + 11}V74H${x + 4}ZM${x + 21} 69H${x + 28}V74H${x + 21}Z`,
          role: "part",
        },
        { d: `M${x - 3} 74H${x + 35}V90H${x - 3}Z`, role: "part" },
        { d: `M${x - 3} 82H${x + 35}`, role: "detail" },
      ]),
    ],
  },
  // A bipod seen from the front, the only view that shows both legs: the
  // clamp gripping the rail, the yoke and its leg pivots, then each leg
  // splayed outward, a square outer tube, the locking collar, the notched
  // inner leg and a rubber foot.
  bipod: {
    parts: [
      ...[-1, 1].flatMap<Part>((side) => {
        const leg = (t: number): [number, number] => [
          r(160 + side * 38 + side * 0.25 * t),
          r(58 + 0.968 * t),
        ];
        const [ox, oy] = leg(76);
        const [cx, cy] = leg(84);
        const [ix, iy] = leg(122);
        const [fx, fy] = leg(128);
        const dx = side * 0.25;
        return [
          { d: tube(...leg(66), ix, iy, 3.6), role: "part" },
          {
            d:
              across(...leg(94), dx, 0.968, 3.6) +
              across(...leg(102), dx, 0.968, 3.6) +
              across(...leg(110), dx, 0.968, 3.6),
            role: "detail",
          },
          { d: tube(...leg(0), ox, oy, 5.8), role: "part" },
          { d: tube(ox, oy, cx, cy, 7.2), role: "part" },
          { d: tube(ix, iy, fx, fy, 6.2), role: "part" },
          { d: tube(fx, fy, ...leg(140), 9), role: "part" },
        ];
      }),
      { d: "M110 52H210Q216 52 216 58V64H104V58Q104 52 110 52Z", role: "part" },
      { d: "M142 52V34H178V52Z", role: "part" },
      { d: "M146 22H174V27L170 31H150L146 27Z", role: "part" },
      { d: "M142 34H148V30H172V34H178", role: "detail" },
      { circle: [122, 58, 5.5], role: "part" },
      { circle: [198, 58, 5.5], role: "part" },
      { circle: [122, 58, 2], role: "part" },
      { circle: [198, 58, 2], role: "part" },
      { circle: [160, 43, 4.5], role: "part" },
    ],
  },
  // A two-point sling hanging from HK-style snap hooks: each hook's slot,
  // with the webbing folded round its bar and box-stitched, and its
  // spring gate; the strap's edge stitching; and the adjuster with its
  // pull tab.
  sling: {
    parts: [
      { d: ribbon(SLING_PATH, 7), role: "part" },
      { d: stitching(SLING_PATH.slice(3, -3), 4.4), role: "detail" },
      ...[
        [62, -1],
        [258, 1],
      ].flatMap<Part>(([x, side]) => snapHook(x, side)),
      { d: SLING_TAB, role: "part" },
      { d: SLING_BUCKLE, role: "part" },
    ],
  },
  // A soft rifle case, lying flat, muzzle end right: its outline tapering
  // from the butt end as the rifle inside does, the zipper along the top
  // and round the muzzle end with its slider and pull, a padded carry
  // handle stitched on at both ends, and a zipped pocket.
  case: {
    parts: [
      {
        d: "M40 62H288Q306 62 306 80V86Q306 102 290 104C230 110 180 140 132 140H40Q16 140 16 116V86Q16 62 40 62Z",
        role: "part",
      },
      {
        d: "M28 69H287Q299 69 299 81V85Q299 96 289 97M28 72H287Q296 72 296 81V85Q296 93 288 94",
        role: "detail",
      },
      { d: CASE_HANDLE, role: "part" },
      ...[124, 180].flatMap<Part>((x) => [
        { d: `M${x} 57H${x + 14}V71H${x} Z`, role: "part" },
        { d: `M${x + 3} 60L${x + 11} 68M${x + 11} 60L${x + 3} 68`, role: "detail" },
      ]),
      { d: "M226 67H238V74H226Z", role: "part" },
      { d: "M229 74H235V84Q235 87 232 87Q229 87 229 84Z", role: "part" },
      { d: roundRect(98, 92, 84, 34, 6), role: "part" },
      { d: "M104 99H176M104 101.5H176", role: "detail" },
      { d: "M160 97H168V100H160Z", role: "part" },
    ],
  },
  // The generic accessory, for the kind "Other": an open cardboard box of
  // spare parts, its back and side flaps up and the front flap folded
  // down, with a coil spring, a bolt and a punch standing out of it and an
  // inventory label on the front.
  accessory: {
    parts: [
      { d: "M102 100V70L218 66V100Z", role: "part" },
      { d: "M96 100L62 82L66 76L99 95Z", role: "part" },
      { d: "M224 100L258 82L254 76L221 95Z", role: "part" },
      { d: spring(128, 104, 112, 52, 7, 5), role: "open" },
      { d: tube(156, 104, 156, 76, 3.5), role: "part" },
      { d: "M152.5 80H159.5M152.5 83.5H159.5M152.5 87H159.5M152.5 90.5H159.5", role: "detail" },
      { d: "M147 68H165V76H147Z", role: "part" },
      { d: "M153 68V76M159 68V76", role: "detail" },
      { d: tube(178, 104, 196, 60, 3), role: "part" },
      { d: tube(196, 60, 198.6, 53.6, 1.6), role: "part" },
      { d: "M96 100H224V172H96Z", role: "part" },
      { d: "M96 100L90 128H230L224 100Z", role: "part" },
      { d: "M180 140H214V162H180Z", role: "part" },
      { d: "M185 146H209M185 151H209M185 156H201", role: "detail" },
    ],
  },
};
