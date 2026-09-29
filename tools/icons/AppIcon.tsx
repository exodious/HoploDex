import { SolidOwl } from "../../src/features/app/SolidOwl";
import { Hoplon } from "../../src/features/databases/plate/PlateArt";

/*
 * The program icon (issue #22): the chooser plate's shield, owl and all, in
 * bronze on a blued-steel tile. At 32 px and below its lines would vanish,
 * so there the owl is a silhouette with its eyes cut out, on a plain rim.
 *
 * It is rendered to SVG for tools/icons/make-icons.mjs, which rasterizes it
 * with resvg (`tauri icon`). resvg reads a <style> with class selectors but
 * not CSS variables, so the colours and line widths are written out here.
 */

const STEEL = "#16202b";
const BRONZE = "#e2b565";
const R = 196;

/** A line `px` device pixels wide at `size`, or `units` of the 512 box if
 * that's wider: thin lines thicken as the icon shrinks, so they stay. */
const line = (size: number, units: number, px: number) =>
  Math.max(units, (px * 512) / size).toFixed(2);

function styles(size: number): string {
  const owl = R * 0.0064; // the owl's own scale inside the shield
  return `
.ground,.device .paper{fill:${STEEL}}
.open{fill:none;stroke:${BRONZE};stroke-width:${line(size, 4, 1.2)}}
.thin{fill:none;stroke:${BRONZE};stroke-width:${line(size, 2.5, 0.9)}}
.orn{fill:none;stroke:${BRONZE};stroke-width:${line(size, 2.5, 0.9)};stroke-linejoin:round}
.dot,.device .pf,.device .pfi{fill:${BRONZE}}
.device .ol{fill:none;stroke:${BRONZE};stroke-width:${(Number(line(size, 4, 1.3)) / owl).toFixed(2)};stroke-linecap:round;stroke-linejoin:round}
.device .fe{fill:none;stroke:${BRONZE};stroke-width:${(Number(line(size, 2.5, 0.8)) / owl).toFixed(2)};stroke-linecap:round}
.device .wash{fill:${BRONZE};opacity:.15}`;
}

export function AppIcon({ size }: { size: number }) {
  const small = size <= 32;
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 512 512">
      <defs>
        <linearGradient id="steel" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#223041" />
          <stop offset="1" stopColor="#0f1419" />
        </linearGradient>
        <pattern id="grid" width="32" height="32" patternUnits="userSpaceOnUse">
          <path d="M32 0H0V32" fill="none" stroke="rgba(147,168,255,0.09)" strokeWidth="2" />
        </pattern>
        {!small && <style>{styles(size)}</style>}
      </defs>
      <rect x="16" y="16" width="480" height="480" rx="104" fill="url(#steel)" />
      {!small && <rect x="16" y="16" width="480" height="480" rx="104" fill="url(#grid)" />}
      <g transform="translate(256 256)">
        {small ? (
          <>
            <circle r={R} fill={STEEL} stroke={BRONZE} strokeWidth={size <= 16 ? 34 : 22} />
            <circle r={140} fill="none" stroke={BRONZE} strokeWidth={10} strokeDasharray="14 12" />
            <g transform="scale(1.25)">
              <SolidOwl fill={BRONZE} ground={STEEL} />
            </g>
          </>
        ) : (
          <Hoplon r={R} />
        )}
      </g>
    </svg>
  );
}
