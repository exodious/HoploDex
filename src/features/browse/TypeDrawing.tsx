import "./TypeDrawing.css";

/*
 * Generic per-type thumbnails (FR-009) drawn as technical line art, like a
 * patent drawing: outlined parts over a dash-dot bore axis. Parts are
 * filled with the frame's paper color and painted back to front, so nearer
 * parts occlude the outlines behind them. Vector, theme-aware, and needing
 * no IPC round trip, unlike the bundled placeholder PNGs they replace.
 */

type Part =
  | { d: string; role: "part" | "open" | "detail" }
  | { circle: [number, number, number]; role: "part" };

interface Drawing {
  parts: Part[];
  /** Bore axis: [x start, y, x end]. */
  axis: [number, number, number];
}

const DRAWINGS: Record<string, Drawing> = {
  handgun: {
    parts: [
      {
        d: "M84 78C80 82 78 86 77 92L64 154 66 158 116 162 118 158C120 138 126 120 134 106L138 96 140 78Z",
        role: "part",
      },
      { d: "M66 157 117 161 116 168 65 164Z", role: "part" },
      { d: "M132 100h5v6h-5Z", role: "detail" },
      { d: "M120 76H244L246 80V92L240 96H120Z", role: "part" },
      { d: "M214 88H232", role: "detail" },
      { d: "M140 96V112Q140 121 149 121H184Q193 121 195 112L198 96", role: "open" },
      { d: "M163 96C161 103 158 107 159 114", role: "open" },
      { d: "M74 48H240L248 55V71L243 77H74Q71 77 71 74V51Q71 48 74 48Z", role: "part" },
      { d: "M82 44H91V48H82ZM235 44H240V48H235Z", role: "part" },
      { d: "M86 53V72M91 53V72M96 53V72M101 53V72", role: "detail" },
      { d: "M150 77H176", role: "detail" },
    ],
    axis: [52, 62, 266],
  },
  rifle: {
    parts: [
      {
        d: "M262 97 118 96C110 96 100 93 88 91L17 88 12 134 64 121C82 116 93 118 99 126L108 128C112 118 120 108 136 105L190 105 260 102C267 102 267 97 262 97Z",
        role: "part",
      },
      { d: "M24 89 19 132", role: "detail" },
      { d: "M60 122l1 4M242 102.6v4", role: "detail" },
      { d: "M186 86 310 89V95L186 98Z", role: "part" },
      {
        d: "M122 84H186Q190 84 190 88V96Q190 100 186 100H122Q118 100 118 96V88Q118 84 122 84Z",
        role: "part",
      },
      { d: "M134 95 125 106", role: "open" },
      { circle: [122, 109, 4.5], role: "part" },
      { d: "M140 100V108Q140 116 148 116H160Q168 116 170 108L172 100", role: "open" },
      { d: "M154 100C153 106 151 108 152 112", role: "open" },
      { d: "M133 70H140V84H133ZM172 70H179V84H172Z", role: "part" },
      {
        d: "M96 58 112 58 124 62 184 62 198 56 212 56 212 76 198 76 184 70 124 70 112 74 96 74Z",
        role: "part",
      },
      { d: "M150 55H160V62H150Z", role: "part" },
      { d: "M101 59V73M207 57V75", role: "detail" },
    ],
    axis: [4, 92, 316],
  },
  shotgun: {
    parts: [
      { d: "M102 80 18 87 12 130 70 116Q88 111 102 108Z", role: "part" },
      { d: "M25 87 20 128", role: "detail" },
      { d: "M150 95H284V103H150Z", role: "part" },
      { d: "M283 93H293V105H283Z", role: "part" },
      { d: "M150 85H312V95H150Z", role: "part" },
      {
        d: "M156 81H312M166 81V85M178 81V85M190 81V85M202 81V85M214 81V85M226 81V85M238 81V85M250 81V85M262 81V85M274 81V85M286 81V85M298 81V85",
        role: "detail",
      },
      { circle: [309, 79, 1.8], role: "part" },
      {
        d: "M184 91H248Q253 91 253 96V104Q253 109 248 109H184Q179 109 179 104V96Q179 91 184 91Z",
        role: "part",
      },
      {
        d: "M189 95V105M195 95V105M201 95V105M207 95V105M213 95V105M219 95V105M225 95V105M231 95V105M237 95V105M243 95V105",
        role: "detail",
      },
      { d: "M108 78H156V108H100V86Q100 78 108 78Z", role: "part" },
      { d: "M120 84H146V93H120Z", role: "detail" },
      { d: "M112 108V116Q112 124 120 124H134Q142 124 144 116L146 108", role: "open" },
      { d: "M127 108C126 114 124 116 125 120", role: "open" },
    ],
    axis: [4, 90, 316],
  },
  // "Other" has no single silhouette, so it gets a cartridge instead.
  other: {
    parts: [
      { d: "M70 76 196 78 212 87 240 87 240 113 212 113 196 122 70 124Z", role: "part" },
      { d: "M240 88C268 88 290 95 298 100 290 105 268 112 240 112", role: "part" },
      { d: "M63 79H70V121H63Z", role: "part" },
      { d: "M56 74H63V126H56Z", role: "part" },
      { d: "M248 89V111", role: "detail" },
    ],
    axis: [40, 100, 312],
  },
};

export interface TypeDrawingProps {
  /** A `FirearmType.generic_thumbnail_key` ("handgun", "rifle", …);
   * unknown keys fall back to the "other" drawing. */
  typeKey: string;
  className?: string;
  /** Draws the lines in, once — reserved for the empty-collection state. */
  animate?: boolean;
}

export function TypeDrawing({ typeKey, className, animate }: TypeDrawingProps) {
  const drawing = DRAWINGS[typeKey] ?? DRAWINGS.other;
  const [x0, y, x1] = drawing.axis;
  return (
    <svg
      viewBox="0 0 320 200"
      className={["hd-drawing", animate && "hd-drawing--animate", className]
        .filter(Boolean)
        .join(" ")}
      aria-hidden
      focusable={false}
    >
      {drawing.parts.map((part, i) =>
        "circle" in part ? (
          <circle
            key={i}
            cx={part.circle[0]}
            cy={part.circle[1]}
            r={part.circle[2]}
            className="hd-drawing__part"
            pathLength={1}
          />
        ) : (
          <path key={i} d={part.d} className={`hd-drawing__${part.role}`} pathLength={1} />
        ),
      )}
      <path d={`M${x0} ${y}H${x1}`} className="hd-drawing__axis" />
    </svg>
  );
}
