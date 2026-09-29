import { useLayoutEffect, useRef, useState } from "react";
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
  /** Draws the lines in, once. */
  animate?: boolean;
  /** Crops the canvas to the drawing's own bounds, so long guns fill a
   * short, wide frame instead of keeping the shared 320×200 box. */
  crop?: boolean;
}

const FULL_BOX = "0 0 320 200";
/** Room for the outline strokes, in drawing units. */
const CROP_MARGIN = 3;

export function TypeDrawing({ typeKey, className, animate, crop }: TypeDrawingProps) {
  const drawing = DRAWINGS[typeKey] ?? DRAWINGS.other;
  const [x0, y, x1] = drawing.axis;
  const ref = useRef<SVGSVGElement>(null);
  const [viewBox, setViewBox] = useState(FULL_BOX);

  useLayoutEffect(() => {
    const svg = ref.current;
    // getBBox is absent outside a real layout engine (jsdom).
    if (!crop || !svg || typeof svg.getBBox !== "function") {
      setViewBox(FULL_BOX);
      return;
    }
    const box = svg.getBBox();
    if (box.width === 0 || box.height === 0) return;
    const m = CROP_MARGIN;
    setViewBox(`${box.x - m} ${box.y - m} ${box.width + 2 * m} ${box.height + 2 * m}`);
  }, [crop, drawing]);

  return (
    <svg
      ref={ref}
      viewBox={viewBox}
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
