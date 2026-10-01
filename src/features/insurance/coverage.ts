import { daysUntil, formatDate, todayIso } from "../../lib/dates";
import { formatDollars } from "../../lib/money";
import type { InsuranceWarning } from "../browse/types";
import type { Firearm } from "../firearms/types";
import type { BlanketSummary, InsurancePolicy, ValueSummary } from "./types";

/** "Expired Mar 1, 2026", "Expires today", "Expires in 12 days", "Starts
 * Jan 1, 2027", or "In force until …". Which of these applies comes from the
 * backend's flags (the 30-day rule lives there, once); only the day count
 * is worked out here. */
export function expiryLabel(
  policy: Pick<
    InsurancePolicy,
    "effectiveStartDate" | "effectiveEndDate" | "isInForce" | "isExpired" | "isExpiringSoon"
  >,
  today: string = todayIso(),
): string {
  if (policy.isExpired) return `Expired ${formatDate(policy.effectiveEndDate)}`;
  if (!policy.isInForce) return `Starts ${formatDate(policy.effectiveStartDate)}`;
  if (policy.isExpiringSoon) {
    const daysLeft = daysUntil(policy.effectiveEndDate, today);
    if (daysLeft === 0) return "Expires today";
    return `Expires in ${daysLeft} ${daysLeft === 1 ? "day" : "days"}`;
  }
  return `In force until ${formatDate(policy.effectiveEndDate)}`;
}

export type CoverageTone = "ok" | "warn" | "neutral";

export interface CoverageStatus {
  tone: CoverageTone;
  label: string;
  detail: string;
}

/** Plain-language coverage status for one firearm. The warning itself
 * always comes from the backend (`insuranceWarning` on its browse summary,
 * the single source of truth shared with the value summary); this only
 * explains it. `policy` is the policy the firearm is scheduled on, if any;
 * `blanket` is the blanket policy in force, which covers it otherwise. */
export function coverageStatus(
  firearm: Firearm,
  warning: InsuranceWarning,
  policy: InsurancePolicy | undefined,
  blanket: BlanketSummary | null,
): CoverageStatus {
  if (firearm.status === "disposed") {
    return {
      tone: "neutral",
      label: "Not tracked",
      detail: "Disposed firearms are left out of totals and coverage checks.",
    };
  }

  const scheduled = firearm.insurancePolicyId != null;

  if (warning === "uninsured") {
    if (scheduled && policy?.isExpired) {
      return {
        tone: "warn",
        label: "Uninsured",
        detail: `${policy.name} expired ${formatDate(policy.effectiveEndDate)}.`,
      };
    }
    return {
      tone: "warn",
      label: "Uninsured",
      detail: "No blanket policy is in force, and it isn't scheduled on a policy.",
    };
  }

  if (warning === "under_insured") {
    if (scheduled) {
      const amount = firearm.scheduledCoverageAmount ?? 0;
      const shortfall = (firearm.estimatedValue ?? 0) - amount;
      return {
        tone: "warn",
        label: "Under-insured",
        detail: `Scheduled for ${formatDollars(amount)} — ${formatDollars(shortfall)} short of its value.`,
      };
    }
    return {
      tone: "warn",
      label: "Under-insured",
      detail: `The firearms covered by ${blanket?.policyName ?? "the blanket policy"} are worth more than its limit.`,
    };
  }

  if (!firearm.estimatedValue) {
    return {
      tone: "neutral",
      label: scheduled
        ? `On ${policy?.name ?? "a policy"}`
        : blanket
          ? "Blanket-covered"
          : "Not insured",
      detail: "Add an estimated value to check coverage.",
    };
  }
  return {
    tone: "ok",
    label: "Covered",
    detail: scheduled
      ? `Scheduled for ${formatDollars(firearm.scheduledCoverageAmount)} on ${policy?.name ?? "a policy"}.`
      : `Covered by ${blanket?.policyName ?? "the blanket policy"}.`,
  };
}

/** How much coverage is missing on firearms flagged under-insured, in
 * dollars: the amount the blanket policy's limit falls short of the firearms
 * it covers, plus, for each individually scheduled firearm, how far its
 * scheduled amount is below its value. Expired policies contribute
 * nothing here — their firearms count as uninsured instead, with their
 * whole value missing. Mirrors the rules in
 * `services::insurance_status::record_warning`. */
export function coverageShortfall(summary: ValueSummary | null): number {
  if (!summary) return 0;
  let missing = summary.blanket ? Math.max(0, summary.blanket.total - summary.blanket.limit) : 0;
  for (const policy of summary.byPolicy) {
    if (policy.isExpired) continue;
    for (const entry of policy.individuallyScheduled) {
      if (entry.estimatedValue > 0) {
        missing += Math.max(0, entry.estimatedValue - entry.scheduledAmount);
      }
    }
  }
  return missing;
}
