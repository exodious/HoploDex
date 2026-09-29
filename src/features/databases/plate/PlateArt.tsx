import type { CSSProperties, ReactNode } from "react";
import { OWL_BODY, OWL_EYES, OWL_HEAD, OWL_LEAVES, OWL_OFFSET } from "../../app/owlShapes";
import { DRAWINGS } from "../../browse/typeDrawings";
import { ENTRY_BOX, entryLayout, entryNumber } from "./entries";
import type { PlateEntry } from "./entries";

/*
 * The catalogue plate's artwork (issue #23): a museum plate on the blueprint
 * grid. Entry 1 is a hoplon drawn head-on, with a braided rim, a band of
 * tongues and Athena's owl as its device; a Greek key rule divides it from
 * entry 2, the app's firearm drawings. It is drawn in plate pixels, in the
 * box PLATE_VIEWBOX names.
 *
 * Every line that draws in has pathLength="1" and a stroke of its own: a
 * dash-drawn path made of several pieces draws each in a sliver of the
 * time, so it pops in. The rim is cut into short pieces, each tagged with
 * where the sweep reaches it (data-sweep), because one long dashed line is
 * drawn differently from engine to engine. animation.ts gives them their
 * timing.
 */

export const PLATE_VIEWBOX = "600 40 580 756";

const HOPLON = { x: 880, y: 220, r: 140 };
const RIM_LINE_PIECES = 60; // each rim circle is drawn in this many arcs
const BRAID_WAVES = 30;
const BRAID_PIECES_PER_WAVE = 2; // the braid's two strands, in this many pieces per wave
export const TONGUES = 44;

const f = (n: number) => n.toFixed(2);
const unit = (scale: number) => ({ "--u": (1 / scale).toFixed(4) }) as CSSProperties;

/** Marks a piece of the rim from `f0` to `f1` of the way round. Line and
 * braid pieces keep pace with the sweep; tongues and beads take their own
 * time. */
function sweep(f0: number, f1: number, kind: "line" | "braid" | "tongue" | "bead") {
  return { "data-sweep": `${f0.toFixed(5)} ${f1.toFixed(5)} ${kind}` };
}

/** A rim circle as short arcs, each drawn as the sweep passes it. */
function rimCircle(r: number, cls: string, key: string): ReactNode[] {
  const n = RIM_LINE_PIECES;
  const at = (k: number) => [
    r * Math.cos((2 * Math.PI * k) / n),
    r * Math.sin((2 * Math.PI * k) / n),
  ];
  return Array.from({ length: n }, (_, k) => {
    const [x0, y0] = at(k);
    const [x1, y1] = at(k + 1);
    return (
      <path
        key={`${key}${k}`}
        className={`${cls} swept`}
        pathLength={1}
        {...sweep(k / n, (k + 1) / n, "line")}
        d={`M${f(x0)} ${f(y0)}A${r} ${r} 0 0 1 ${f(x1)} ${f(y1)}`}
      />
    );
  });
}

/** Two interlaced strands around a ring, with a bead in each eye. Each
 * strand is cut into pieces that the sweep draws one after another. */
function guilloche(r0: number, w: number, n: number): ReactNode[] {
  const out: ReactNode[] = [];
  const pieces = n * BRAID_PIECES_PER_WAVE;
  const steps = Math.max(2, Math.floor(24 / BRAID_PIECES_PER_WAVE)); // line segments per piece
  for (const phase of [0, Math.PI]) {
    for (let k = 0; k < pieces; k++) {
      const pts: string[] = [];
      for (let i = 0; i <= steps; i++) {
        const t = (2 * Math.PI * (k + i / steps)) / pieces;
        const r = r0 + (w / 2) * Math.sin(n * t + phase);
        pts.push(`${f(r * Math.cos(t))} ${f(r * Math.sin(t))}`);
      }
      out.push(
        <path
          key={`b${phase}-${k}`}
          className="orn swept"
          pathLength={1}
          {...sweep(k / pieces, (k + 1) / pieces, "braid")}
          d={`M${pts.join("L")}`}
        />,
      );
    }
  }
  for (let k = 0; k < n * 2; k++) {
    const t = ((k + 0.5) * Math.PI) / n;
    const frac = (k + 0.5) / (n * 2);
    out.push(
      <circle
        key={`d${k}`}
        className="dot"
        {...sweep(frac, frac, "bead")}
        cx={f(r0 * Math.cos(t))}
        cy={f(r0 * Math.sin(t))}
        r={f(w * 0.12)}
      />,
    );
  }
  return out;
}

/** A ring of tongues, points inward: the bowl's border. */
function tongues(rOut: number, rIn: number, n: number): ReactNode[] {
  return Array.from({ length: n }, (_, k) => {
    const a0 = (2 * Math.PI * k) / n;
    const a1 = (2 * Math.PI * (k + 1)) / n;
    const am = (a0 + a1) / 2;
    const polar = (r: number, a: number) => `${f(r * Math.cos(a))} ${f(r * Math.sin(a))}`;
    return (
      <path
        key={`t${k}`}
        className="orn swept"
        pathLength={1}
        {...sweep(k / n, k / n, "tongue")}
        d={
          `M${polar(rOut, a0 + 0.02)}Q${polar(rIn * 1.02, a0 + 0.02)} ${polar(rIn, am)}` +
          `Q${polar(rIn * 1.02, a1 - 0.02)} ${polar(rOut, a1 - 0.02)}`
        }
      />
    );
  });
}

// ── Athena's owl ──────────────────────────────────────────────────────────
// As on the Athenian tetradrachm: a little owl (no ear tufts), body in
// profile facing right, head turned to face the viewer, and a small olive
// sprig. The coins' crescent is left out: alone it reads as a political or
// religious symbol. An engraving in fine bronze lines, drawn in a ±100 box:
// ol = outline, fe = feather and vein detail, pf = the pupils, which come in
// last; pfi = the beak's and berry's fill; wash = a faint bronze ground;
// paper = the head's ground, hiding the body's lines behind it.

const BEAK = "M-3-40H9C9-33 7-27 3-21C2-27-1-33-3-40Z";

/** Rows of small U-shaped feathers, engraving-style, each its own stroke. */
function scallops(rows: [number, number, number][], key: string, w = 6, h = 4): ReactNode[] {
  const out: ReactNode[] = [];
  rows.forEach(([y, x0, x1], i) => {
    let x = x0 + (i % 2 ? w * 0.66 : 0);
    while (x + w <= x1) {
      out.push(
        <path
          key={`${key}${out.length}`}
          className="fe"
          pathLength={1}
          d={`M${f(x)} ${y}q${w / 2} ${h} ${w} 0`}
        />,
      );
      x += w * 1.33;
    }
  });
  return out;
}

/** The facial disc: an arc over the eye from lower-inner to lower-outer. */
function disc(cx: number, cy: number, r: number) {
  const a0 = (160 * Math.PI) / 180;
  const a1 = (380 * Math.PI) / 180;
  return `M${(cx + r * Math.cos(a0)).toFixed(1)} ${(cy + r * Math.sin(a0)).toFixed(1)}A${r} ${r} 0 1 1 ${(cx + r * Math.cos(a1)).toFixed(1)} ${(cy + r * Math.sin(a1)).toFixed(1)}`;
}

/** Strokes with several pieces, each drawn as its own path. */
function lines(cls: string, d: string, key: string): ReactNode[] {
  return d
    .split(/(?=M)/)
    .filter((piece) => piece.trim())
    .map((piece, i) => <path key={`${key}${i}`} className={cls} pathLength={1} d={piece} />);
}

export function Owl() {
  return (
    <g className="device" transform={OWL_OFFSET}>
      {/* olive sprig: a short twig, two leaves with their midribs, a berry */}
      <path className="ol" pathLength={1} d="M-80-34C-72-48-60-60-46-70" />
      {OWL_LEAVES.map((d) => (
        <path key={d} className="ol" pathLength={1} d={d} />
      ))}
      {lines("fe", "M-52-66C-54-78-55-88-56-96M-60-58C-70-62-80-67-88-73", "rib")}
      <circle className="pfi" cx={-80} cy={-34} r={4} />
      <circle className="ol" pathLength={1} cx={-80} cy={-34} r={4} />
      {/* body: ground, outline, wing, feathering */}
      <path className="wash" d={`${OWL_BODY}Z`} />
      <path className="ol" pathLength={1} d={OWL_BODY} />
      <path className="ol" pathLength={1} d="M-24-8C-6 14-2 44-12 70L-40 88" />
      {scallops(
        [
          [8, -44, -18],
          [18, -48, -14],
          [28, -50, -12],
          [38, -50, -12],
        ],
        "wing",
      )}
      {lines(
        "fe",
        "M-18 48C-24 62-32 74-44 86M-28 48C-34 62-42 74-52 86M-40 48C-44 62-50 74-58 88",
        "prim",
      )}
      {scallops(
        [
          [4, 6, 38],
          [14, 2, 44],
          [24, 2, 46],
          [34, 2, 46],
          [44, 4, 44],
          [54, 6, 40],
          [64, 10, 32],
        ],
        "breast",
      )}
      {/* feet gripping the ground line */}
      {lines("ol", "M4 84V91M18 84V91", "leg")}
      {lines(
        "fe",
        "M4 91L-2 97M4 91V98M4 91L10 97M18 91L12 97M18 91V98M18 91L24 97M-54 98H40",
        "toe",
      )}
      {/* head: paper first, so the body's lines stop at it */}
      <path className="paper" d={OWL_HEAD} />
      <path className="wash" d={OWL_HEAD} />
      <path className="ol" pathLength={1} d={OWL_HEAD} />
      {scallops(
        [
          [-80, -18, 26],
          [-73, -26, 34],
        ],
        "crown",
        5,
        3,
      )}
      {OWL_EYES.map(([x, y]) => (
        <path key={`disc${x}`} className="fe" pathLength={1} d={disc(x, y, 20)} />
      ))}
      {OWL_EYES.map(([x, y]) => (
        <path
          key={`eye${x}`}
          className="ol"
          pathLength={1}
          d={`M${x + 13} ${y}A13 13 0 1 1 ${x - 13} ${y}A13 13 0 1 1 ${x + 13} ${y}`}
        />
      ))}
      {OWL_EYES.map(([x, y]) => (
        <circle key={`pupil${x}`} className="pf" cx={x + 1} cy={y} r={9} />
      ))}
      {/* the beak's fill takes a click, for the blink (animation.ts) */}
      <path className="pfi beak" d={BEAK} />
      <path className="ol" pathLength={1} d={BEAK} />
    </g>
  );
}

/** The shield head-on: rim with a braid, a band of tongues, the bowl, and
 * the owl in the field. The rim is turned a quarter back, so its lines,
 * braid, beads and tongues all start at 12 o'clock and run clockwise. */
export function Hoplon({ r }: { r: number }) {
  const owlScale = r * 0.0064;
  return (
    <>
      <g transform="rotate(-90)">
        <circle r={r} className="ground" />
        {rimCircle(r, "open", "r0")}
        {rimCircle(r * 0.965, "thin", "r1")}
        {guilloche(r * 0.9, r * 0.09, BRAID_WAVES)}
        {rimCircle(r * 0.835, "thin", "r2")}
        {tongues(r * 0.835, r * 0.77, TONGUES)}
        {rimCircle(r * 0.77, "open", "r3")}
      </g>
      <g transform={`scale(${owlScale.toFixed(4)})`} style={unit(owlScale)}>
        <Owl />
      </g>
    </>
  );
}

/** A running Greek key, `u` to the step, between border lines. It draws in
 * from both ends at once and meets in the middle: each key is its own line,
 * tagged with how far it is from its end (data-key). */
function Meander({ x, y, width, u }: { x: number; y: number; width: number; u: number }) {
  const n = Math.floor(width / (4 * u));
  const x1 = x + n * 4 * u;
  const mid = x + n * 2 * u;
  return (
    <>
      {Array.from({ length: n }, (_, i) => {
        const kx = x + i * 4 * u;
        return (
          <path
            key={`k${i}`}
            className="key unit"
            pathLength={1}
            data-key={Math.min(i, n - 1 - i)}
            d={`M${kx} ${y + 4 * u}V${y}H${kx + 3 * u}V${y + 3 * u}H${kx + u}V${y + u}H${kx + 2 * u}`}
          />
        );
      })}
      {(
        [
          [y + 4 * u, 1],
          [y - u, 0.6],
          [y + 5 * u, 0.6],
        ] as const
      ).map(([ly, opacity]) => (
        <g key={ly} opacity={opacity === 1 ? undefined : opacity}>
          <path className="key" pathLength={1} d={`M${x} ${ly}H${mid}`} />
          <path className="key" pathLength={1} d={`M${x1} ${ly}H${mid}`} />
        </g>
      ))}
    </>
  );
}

/** A catalogue scale bar, alternately filled, with its right end at `right`. */
function ScaleBar({
  right,
  y,
  px,
  label,
}: {
  right: number;
  y: number;
  px: number;
  label: string;
}) {
  const x = right - px;
  const q = px / 4;
  return (
    <>
      {[0, 1, 2, 3].map((i) => (
        <rect
          key={i}
          x={f(x + i * q)}
          y={y}
          width={f(q)}
          height={5}
          className={i % 2 ? "sb-empty" : "sb-fill"}
        />
      ))}
      <text x={f(x)} y={y + 20} className="sb">
        0
      </text>
      <text x={right} y={y + 20} className="sb" textAnchor="end">
        {label}
      </text>
    </>
  );
}

/** One firearm drawing, its parts painted back to front, then its bore axis. */
function EntryDrawing({ entry }: { entry: PlateEntry }) {
  const drawing = DRAWINGS[entry.key];
  const { transform, scale } = entryLayout(entry);
  const [x0, y, x1] = drawing.axis;
  return (
    <g transform={transform} style={unit(scale)}>
      {drawing.parts.flatMap((part, i) =>
        "circle" in part ? (
          <circle
            key={i}
            className="part"
            pathLength={1}
            cx={part.circle[0]}
            cy={part.circle[1]}
            r={part.circle[2]}
          />
        ) : part.role === "part" ? (
          <path key={i} className="part" pathLength={1} d={part.d} />
        ) : (
          lines(part.role, part.d, `${i}-`)
        ),
      )}
      <path className="axis" d={`M${x0} ${y}H${x1}`} />
    </g>
  );
}

const ENTRY_RIGHT = ENTRY_BOX.x + ENTRY_BOX.width;

/** Entry 2: one layer per drawing it cycles through, in the order given.
 * The first shows; the others wait, hidden, for animation.ts to bring them
 * in. Each layer's art and caption (with its catalogue number) change
 * separately, and the art can be clipped by the straightedge (`clipId`). */
function Entry2({ clipId, entries }: { clipId: string; entries: readonly PlateEntry[] }) {
  const clipX = ENTRY_BOX.x - 20;
  const clipW = ENTRY_BOX.width + 60;
  return (
    <g className="entry">
      <defs>
        {entries.map((entry, i) => (
          <clipPath key={entry.key} id={`${clipId}-${i}`}>
            <rect
              className="clip"
              x={clipX}
              y={ENTRY_BOX.y - 40}
              width={clipW}
              height={ENTRY_BOX.height + 80}
            />
          </clipPath>
        ))}
      </defs>
      {entries.map((entry, i) => {
        const { barPx } = entryLayout(entry);
        return (
          <g key={entry.key} className="layer" data-layer={i} data-drawing={entry.key}>
            <g className="art" data-clip={`url(#${clipId}-${i})`}>
              <EntryDrawing entry={entry} />
            </g>
            <g className="caption">
              <ScaleBar right={ENTRY_RIGHT} y={724} px={barPx} label={`${entry.barCm} cm`} />
              <text x={640} y={768} className="no">
                {entryNumber(entry)}
              </text>
              <text x={668} y={761} className="cap">
                <tspan className="t">{entry.title}</tspan> {entry.caption}
              </text>
            </g>
          </g>
        );
      })}
      {/* the straightedge that wipes one drawing into the next */}
      <path
        className="edge"
        d={`M${clipX} ${ENTRY_BOX.y - 22}V${ENTRY_BOX.y + ENTRY_BOX.height + 22}`}
      />
    </g>
  );
}

/** The straightedge's travel, in plate pixels: from the clip's left edge
 * past the box's right. */
export const STRAIGHTEDGE_TRAVEL = ENTRY_BOX.width + 60;

/** The whole plate, with entry 2's drawings in the order `entries` gives. */
export function PlateArt({ clipId, entries }: { clipId: string; entries: readonly PlateEntry[] }) {
  return (
    <>
      <g transform={`translate(${HOPLON.x} ${HOPLON.y})`}>
        <Hoplon r={HOPLON.r} />
      </g>
      <ScaleBar right={1093} y={396} px={93} label="30 cm" />
      <text x={640} y={452} className="no">
        1
      </text>
      <text x={668} y={445} className="cap">
        <tspan className="t">Hoplon.</tspan> The Greek hoplite’s shield, bronze over wood.
      </text>
      <text x={668} y={465} className="cap">
        About 500 BC.
      </text>
      <Meander x={640} y={492} width={500} u={3.5} />
      <Entry2 clipId={clipId} entries={entries} />
    </>
  );
}
