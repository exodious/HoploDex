import { SolidOwl } from "./SolidOwl";

/** A hoplon — the round shield the name comes from — with Athena's owl as
 * its device: the program icon's small version (issue #22). */
export function BrandMark() {
  return (
    <svg className="hd-brand__mark" viewBox="-110 -110 220 220" aria-hidden focusable={false}>
      <circle r={102} className="hd-brand__rim" />
      <circle r={84} className="hd-brand__braid" />
      <g transform="scale(.66)">
        <SolidOwl fill="currentColor" ground="var(--vellum)" />
      </g>
    </svg>
  );
}
