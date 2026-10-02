import { Badge, InsuranceWarningBadge } from "../../components";
import type { FirearmSummary } from "./types";

/** A firearm's coverage state in browse views — every uninsured or
 * under-insured firearm is visibly flagged wherever it's listed (SC-004).
 * An accessory's summary has the same three fields (specs/006). */
export function CoverageCell({
  firearm,
}: {
  firearm: Pick<FirearmSummary, "status" | "insuranceWarning" | "estimatedValue">;
}) {
  if (firearm.status === "disposed") {
    return (
      <Badge tone="neutral" icon="archive">
        Disposed
      </Badge>
    );
  }
  if (firearm.insuranceWarning !== "none") {
    return <InsuranceWarningBadge kind={firearm.insuranceWarning} />;
  }
  if (!firearm.estimatedValue) {
    return <span className="hd-muted hd-coverage-note">No value set</span>;
  }
  return <Badge tone="ok">Covered</Badge>;
}
