import { InsuranceWarningBadge } from "../../components";
import { formatCents } from "../../lib/money";
import type { ValueSummary } from "./types";

export interface ValueSummaryPanelProps {
  summary: ValueSummary;
  /** Maps firearm id -> "make model" label; falls back to "Firearm #id"
   * for any id missing from the map (e.g. loaded before this fetch). */
  firearmLabels?: Record<number, string>;
}

function labelFor(id: number, labels: Record<number, string> | undefined): string {
  return labels?.[id] ?? `Firearm #${id}`;
}

/** Always-current value summary: collection total, per-policy breakdown,
 * and unassigned group (US3, FR-015). The frontend re-invokes
 * `get_value_summary` after every mutating command — there is no
 * "refresh" action and no cached total that can go stale. */
export function ValueSummaryPanel({ summary, firearmLabels }: ValueSummaryPanelProps) {
  return (
    <section aria-label="Value summary">
      <h2>Collection value: {formatCents(summary.collectionTotal)}</h2>

      {summary.byPolicy.map((policy) => (
        <div key={policy.policyId}>
          <h3>
            {policy.policyName}
            {policy.isExpired && <InsuranceWarningBadge kind="expired" />}
            {!policy.isExpired && policy.isExpiringSoon && (
              <InsuranceWarningBadge kind="expiring_soon" />
            )}
          </h3>
          {policy.blanketTotal > 0 && (
            <p>
              Blanket coverage: {formatCents(policy.blanketTotal)} of{" "}
              {formatCents(policy.blanketLimit)}
              {policy.blanketUnderInsured && <InsuranceWarningBadge kind="under_insured" />}
            </p>
          )}
          {policy.individuallyScheduled.length > 0 && (
            <ul>
              {policy.individuallyScheduled.map((coverage) => (
                <li key={coverage.firearmId}>
                  {labelFor(coverage.firearmId, firearmLabels)}:{" "}
                  {formatCents(coverage.scheduledAmount)} scheduled for a{" "}
                  {formatCents(coverage.estimatedValue)} value
                  {coverage.underInsured && <InsuranceWarningBadge kind="under_insured" />}
                </li>
              ))}
            </ul>
          )}
        </div>
      ))}

      {summary.unassigned.length > 0 && (
        <div>
          <h3>Unassigned</h3>
          <ul>
            {summary.unassigned.map((firearm) => (
              <li key={firearm.firearmId}>
                {labelFor(firearm.firearmId, firearmLabels)}: {formatCents(firearm.estimatedValue)}
                {firearm.estimatedValue > 0 && <InsuranceWarningBadge kind="uninsured" />}
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
