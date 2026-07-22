import "./components.css";

export type InsuranceWarningKind = "uninsured" | "under_insured" | "expiring_soon" | "expired";

const LABELS: Record<InsuranceWarningKind, string> = {
  uninsured: "Uninsured",
  under_insured: "Under-insured",
  expiring_soon: "Policy expiring soon",
  expired: "Policy expired",
};

export interface InsuranceWarningBadgeProps {
  kind: InsuranceWarningKind;
}

/**
 * The single shared way every screen flags an insurance-related warning
 * (uninsured/under-insured firearms, expiring/expired policies) per
 * constitution Principle III — no screen renders its own ad-hoc warning
 * text (SC-004: warnings must always be visibly flagged).
 */
export function InsuranceWarningBadge({ kind }: InsuranceWarningBadgeProps) {
  return (
    <span className="hd-badge hd-badge--warning" role="status">
      ⚠ {LABELS[kind]}
    </span>
  );
}
