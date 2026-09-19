import { daysUntil, formatDate, todayIso } from "../../lib/dates";
import { formatCents } from "../../lib/money";
import type { InsuranceWarning } from "../browse/types";
import type { Firearm } from "../firearms/types";
import type { InsurancePolicy, ValueSummary } from "./types";

export interface PolicyExpiry {
  expired: boolean;
  expiringSoon: boolean;
  daysLeft: number;
}

/** Client-side mirror of `services::insurance_status::expiry_status` — for
 * policies the value summary doesn't cover (ones with no firearms yet). */
export function policyExpiry(effectiveEndDate: string, today: string = todayIso()): PolicyExpiry {
  const daysLeft = daysUntil(effectiveEndDate, today);
  const expired = daysLeft < 0;
  return { expired, expiringSoon: !expired && daysLeft <= 30, daysLeft };
}

/** "Expired Mar 1, 2026", "Expires today", "Expires in 12 days", or "In force until …". */
export function expiryLabel(effectiveEndDate: string, today: string = todayIso()): string {
  const { expired, expiringSoon, daysLeft } = policyExpiry(effectiveEndDate, today);
  if (expired) return `Expired ${formatDate(effectiveEndDate)}`;
  if (daysLeft === 0) return "Expires today";
  if (expiringSoon) return `Expires in ${daysLeft} ${daysLeft === 1 ? "day" : "days"}`;
  return `In force until ${formatDate(effectiveEndDate)}`;
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
 * explains it. */
export function coverageStatus(
  firearm: Firearm,
  warning: InsuranceWarning,
  policy: InsurancePolicy | undefined,
  today: string = todayIso(),
): CoverageStatus {
  if (firearm.status === "disposed") {
    return {
      tone: "neutral",
      label: "Not tracked",
      detail: "Disposed firearms are left out of totals and coverage checks.",
    };
  }

  if (warning === "uninsured") {
    if (policy && policyExpiry(policy.effectiveEndDate, today).expired) {
      return {
        tone: "warn",
        label: "Uninsured",
        detail: `${policy.name} expired ${formatDate(policy.effectiveEndDate)}.`,
      };
    }
    return { tone: "warn", label: "Uninsured", detail: "No policy assigned." };
  }

  if (warning === "under_insured") {
    if (firearm.coverageKind === "individually_scheduled") {
      const scheduled = firearm.scheduledCoverageAmount ?? 0;
      const shortfall = (firearm.estimatedValue ?? 0) - scheduled;
      return {
        tone: "warn",
        label: "Under-insured",
        detail: `Scheduled for ${formatCents(scheduled, { whole: true })} — ${formatCents(shortfall, { whole: true })} short of its value.`,
      };
    }
    return {
      tone: "warn",
      label: "Under-insured",
      detail: `Blanket-covered firearms on ${policy?.name ?? "this policy"} are worth more than its limit.`,
    };
  }

  if (!policy) {
    return {
      tone: "neutral",
      label: "Not insured",
      detail: "Add an estimated value to check coverage.",
    };
  }
  if (!firearm.estimatedValue) {
    return {
      tone: "neutral",
      label: `On ${policy.name}`,
      detail: "Add an estimated value to check coverage.",
    };
  }
  return {
    tone: "ok",
    label: "Covered",
    detail:
      firearm.coverageKind === "individually_scheduled"
        ? `Scheduled for ${formatCents(firearm.scheduledCoverageAmount, { whole: true })} on ${policy.name}.`
        : `Blanket coverage on ${policy.name}.`,
  };
}

/** How much coverage is missing on firearms flagged under-insured, in
 * cents: the amount a blanket policy's limit falls short of the firearms
 * assigned to it, plus, for each individually scheduled firearm, how far
 * its scheduled amount is below its value. Expired policies contribute
 * nothing here — their firearms count as uninsured instead, with their
 * whole value missing. Mirrors the rules in
 * `services::insurance_status::firearm_warning`. */
export function coverageShortfall(summary: ValueSummary | null): number {
  if (!summary) return 0;
  let missing = 0;
  for (const policy of summary.byPolicy) {
    if (policy.isExpired) continue;
    missing += Math.max(0, policy.blanketTotal - policy.blanketLimit);
    for (const entry of policy.individuallyScheduled) {
      if (entry.estimatedValue > 0) {
        missing += Math.max(0, entry.estimatedValue - entry.scheduledAmount);
      }
    }
  }
  return missing;
}
