import { OWL_BODY, OWL_EYES, OWL_HEAD, OWL_LEAVES, OWL_OFFSET } from "./owlShapes";

export interface SolidOwlProps {
  /** The owl's colour. */
  fill: string;
  /** The ground its eyes are cut out to. */
  ground: string;
}

/** The owl as a silhouette with its eyes cut out, for sizes where lines
 * disappear: the top bar's mark, and the program icon at 32 px and below. */
export function SolidOwl({ fill, ground }: SolidOwlProps) {
  return (
    <g transform={OWL_OFFSET}>
      <path fill={fill} d={OWL_LEAVES.join("")} />
      <path fill={fill} d={`${OWL_BODY}Z`} />
      <path fill={fill} d={OWL_HEAD} />
      {OWL_EYES.map(([x, y]) => (
        <g key={x}>
          <circle fill={ground} cx={x} cy={y} r={15} />
          <circle fill={fill} cx={x + 1} cy={y} r={9} />
        </g>
      ))}
    </g>
  );
}
