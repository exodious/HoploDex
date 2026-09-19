import "./TypeDrawing.css";
import { DRAWINGS } from "./typeDrawings";

/*
 * Generic per-type thumbnails (FR-009) drawn as technical line art, like a
 * patent drawing: outlined parts over a dash-dot bore axis. Parts are
 * filled with the frame's paper color and painted back to front, so nearer
 * parts occlude the outlines behind them. Vector, theme-aware, and needing
 * no IPC round trip, unlike the bundled placeholder PNGs they replace.
 */

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
