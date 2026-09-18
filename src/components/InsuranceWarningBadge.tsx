import type { ReactNode } from "react";
import { Icon } from "./Icon";
import type { IconName } from "./Icon";
import "./components.css";

export type BadgeTone = "warn" | "ok" | "neutral" | "info";

const TONE_ICONS: Record<BadgeTone, IconName | undefined> = {
  warn: "alert",
  ok: "check",
  neutral: undefined,
  info: undefined,
};

export interface BadgeProps {
  tone: BadgeTone;
  children: ReactNode;
  icon?: IconName;
}

/** Small status label. Warnings always carry an icon as well as color, so
 * they never depend on color alone (WCAG 1.4.1). */
export function Badge({ tone, children, icon = TONE_ICONS[tone] }: BadgeProps) {
  return (
    <span className={`hd-badge hd-badge--${tone}`}>
      {icon && <Icon name={icon} size={14} strokeWidth={2} />}
      {children}
    </span>
  );
}

export type InsuranceWarningKind = "uninsured" | "under_insured" | "expiring_soon" | "expired";

const LABELS: Record<InsuranceWarningKind, string> = {
  uninsured: "Uninsured",
  under_insured: "Under-insured",
  expiring_soon: "Policy expiring soon",
  expired: "Policy expired",
};

export interface InsuranceWarningBadgeProps {
  kind: InsuranceWarningKind;
  /** Overrides the default label, e.g. "Expires in 12 days". */
  label?: string;
}

/**
 * The single shared way every screen flags an insurance-related warning
 * (uninsured/under-insured firearms, expiring/expired policies) per
 * constitution Principle III — no screen renders its own ad-hoc warning
 * text (SC-004: warnings must always be visibly flagged).
 */
export function InsuranceWarningBadge({ kind, label }: InsuranceWarningBadgeProps) {
  return <Badge tone="warn">{label ?? LABELS[kind]}</Badge>;
}
