import { memo, useId, useLayoutEffect, useRef, useState } from "react";
import { applyPlateAnimation } from "./animation";
import { PLATE_VIEWBOX, PlateArt } from "./PlateArt";
import { STARTUP_ENTRIES } from "./entries";
import type { PlateEntry } from "./entries";
import { PLATE_TIMING } from "./timing";
import type { PlateTiming } from "./timing";
import "./plate.css";

export interface CataloguePlateProps {
  className?: string;
  /** The timing to play; the app always plays timing.ts's. */
  timing?: PlateTiming;
  /** Entry 2's drawings, in the order to show them; the app always shows
   * them in the order shuffled at startup. */
  entries?: readonly PlateEntry[];
}

/** Whether the viewer asked for less motion: then the plate shows finished
 * and stays still. */
function reducedMotion(): boolean {
  return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

/**
 * The database chooser's catalogue plate (issue #23): the hoplon with its
 * owl and the firearm drawings, drawn in stroke by stroke when the chooser
 * opens, then cycling through the drawings until a database is opened.
 * Decorative only: hidden from assistive technology, and from the pointer
 * all but the owl's beak, which makes the owl blink (animation.ts).
 */
export const CataloguePlate = memo(function CataloguePlate({
  className,
  timing = PLATE_TIMING,
  entries = STARTUP_ENTRIES,
}: CataloguePlateProps) {
  const ref = useRef<SVGSVGElement>(null);
  const [animate] = useState(() => !reducedMotion());
  const clipId = `plate${useId().replace(/[^a-zA-Z0-9]/g, "")}`;

  useLayoutEffect(() => {
    if (!animate || !ref.current) return;
    return applyPlateAnimation(ref.current, timing);
  }, [animate, timing]);

  return (
    <svg
      ref={ref}
      className={["hd-catalogue", animate && "hd-catalogue--animate", className]
        .filter(Boolean)
        .join(" ")}
      viewBox={PLATE_VIEWBOX}
      preserveAspectRatio="xMidYMin meet"
      aria-hidden
      focusable={false}
    >
      <PlateArt clipId={clipId} entries={entries} />
    </svg>
  );
});
