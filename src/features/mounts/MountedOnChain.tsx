import { Fragment } from "react";
import { RecordName } from "./RecordName";
import type { RecordLabel } from "./types";
import "./mounts.css";

// specs/006-accessory-links FR-013, contracts/ui-accessories.md §6: "Mounted
// on" followed by the chain, each record a link: "BCM upper · Upper receiver,
// on LaRue PredatAR · Rifle". The caller supplies the "Mounted on" label (a
// fact row's, or the firearm plate's).

export interface MountedOnChainProps {
  /** `MountDetail.chain`: the direct host first, then its host, and so on. */
  chain: RecordLabel[];
}

export function MountedOnChain({ chain }: MountedOnChainProps) {
  if (chain.length === 0) return null;
  return (
    <span className="hd-mounted-chain">
      {chain.map((label, index) => (
        <Fragment key={`${label.record.kind}:${label.record.id}`}>
          {index > 0 && ", on "}
          <RecordName label={label} link withType />
        </Fragment>
      ))}
    </span>
  );
}
