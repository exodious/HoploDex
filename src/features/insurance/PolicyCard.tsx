import type { Ref } from "react";
import { Badge, Button, InsuranceWarningBadge } from "../../components";
import { formatDate } from "../../lib/dates";
import { formatDollars } from "../../lib/money";
import { firearmName } from "../app/collectionStore";
import { useNavigation } from "../app/navigation";
import type { FirearmSummary } from "../browse/types";
import { expiryLabel } from "./coverage";
import type { BlanketSummary, InsurancePolicy, PolicySummary } from "./types";

/** One policy: its term, its blanket usage when it is the blanket policy in
 * force, the firearms scheduled under it, and contacts (FR-015, FR-027,
 * FR-028, FR-036). */
export function PolicyCard({
  policy,
  blanket,
  summary,
  firearms,
  onOpen,
  onEdit,
  onDelete,
  headRef,
}: {
  policy: InsurancePolicy;
  /** The blanket policy in force today, if any. */
  blanket: BlanketSummary | null;
  /** Makes the name a link to the policy's own page; omitted on that page. */
  onOpen?: () => void;
  summary: PolicySummary | undefined;
  /** The active firearms scheduled under this policy. */
  firearms: FirearmSummary[];
  onEdit: () => void;
  onDelete: () => void;
  /** The card's header: the policy page pins a strip once it scrolls away. */
  headRef?: Ref<HTMLElement>;
}) {
  const scheduledAmounts = new Map(
    (summary?.individuallyScheduled ?? []).map((s) => [s.firearmId, s]),
  );
  const limit = policy.blanketCoverageLimit;
  const inForce = blanket?.policyId === policy.id ? blanket : null;

  return (
    <article className="hd-policy" aria-labelledby={`policy-${policy.id}`}>
      <header className="hd-policy__head" ref={headRef}>
        <div className="hd-policy__title">
          <h3
            className="hd-policy__name"
            id={`policy-${policy.id}`}
            // On its own page, the pinned strip's "back to top" focuses it.
            tabIndex={onOpen ? undefined : -1}
          >
            {onOpen ? (
              <button type="button" className="hd-policy__open" onClick={onOpen}>
                {policy.name}
              </button>
            ) : (
              policy.name
            )}
          </h3>
          <p className="hd-policy__meta">
            {policy.insuranceCompany} · Policy{" "}
            <span className="hd-serial">{policy.policyNumber}</span>
          </p>
        </div>
        <div className="hd-policy__actions">
          <Button size="sm" icon="pencil" onClick={onEdit}>
            Edit
          </Button>
          <Button
            size="sm"
            variant="ghost"
            icon="trash"
            aria-label={`Delete ${policy.name}`}
            onClick={onDelete}
          />
        </div>
      </header>

      <div className="hd-policy__term">
        <span>
          {formatDate(policy.effectiveStartDate)} – {formatDate(policy.effectiveEndDate)}
        </span>
        <TermBadge policy={policy} />
      </div>
      {policy.isExpired && policy.expiredWarning && (
        <p className="hd-policy__alert">
          {firearms.length > 0
            ? "Every firearm scheduled on an expired policy counts as uninsured. If it has been renewed, edit the end date."
            : "If it has been renewed, edit the end date."}
        </p>
      )}

      {limit != null &&
        (inForce ? (
          <div className="hd-policy__block">
            <div className="hd-policy__block-head">
              <span className="hd-eyebrow">Blanket coverage</span>
              <span className="hd-muted hd-num">
                {inForce.firearmCount} {inForce.firearmCount === 1 ? "firearm" : "firearms"}
              </span>
            </div>
            <Meter value={inForce.total} limit={limit} />
            <p className="hd-policy__figures">
              <strong className="hd-num">{formatDollars(inForce.total)}</strong>
              <span className="hd-muted">
                {" "}
                of <span className="hd-num">{formatDollars(limit)}</span> limit
              </span>
              {inForce.total > limit ? (
                <InsuranceWarningBadge
                  kind="under_insured"
                  label={`Over by ${formatDollars(inForce.total - limit)}`}
                />
              ) : (
                <span className="hd-muted hd-num">
                  {" "}
                  · {formatDollars(limit - inForce.total)} to spare
                </span>
              )}
            </p>
            <p className="hd-policy__note">Covers every firearm not scheduled individually.</p>
          </div>
        ) : (
          <p className="hd-policy__block hd-muted">
            Blanket limit <span className="hd-num">{formatDollars(limit)}</span> — not in force, so
            it covers no firearms.
          </p>
        ))}

      {firearms.length > 0 && (
        <div className="hd-policy__block">
          <div className="hd-policy__block-head">
            <span className="hd-eyebrow">Scheduled individually</span>
          </div>
          <ScheduledTable
            firearms={firearms}
            amounts={scheduledAmounts}
            expired={policy.isExpired}
          />
        </div>
      )}

      {(policy.companyContact || policy.agentName || policy.agentContact) && (
        <dl className="hd-policy__contacts">
          {policy.companyContact && (
            <div>
              <dt>Company</dt>
              <dd>{policy.companyContact}</dd>
            </div>
          )}
          {(policy.agentName || policy.agentContact) && (
            <div>
              <dt>Agent</dt>
              <dd>{[policy.agentName, policy.agentContact].filter(Boolean).join(" · ")}</dd>
            </div>
          )}
        </dl>
      )}

      {policy.notes && (
        <div className="hd-policy__notes">
          <span className="hd-eyebrow">Notes</span>
          <p>{policy.notes}</p>
        </div>
      )}
    </article>
  );
}

/** The policy's state today. Which warnings show is the backend's call, so a
 * blanket policy that a successor has taken over from reads as history
 * rather than a lapse (FR-028). */
function TermBadge({ policy }: { policy: InsurancePolicy }) {
  if (policy.expiredWarning) {
    return <InsuranceWarningBadge kind="expired" label={expiryLabel(policy)} />;
  }
  if (policy.isExpired) {
    return <Badge tone="neutral">Ended {formatDate(policy.effectiveEndDate)}</Badge>;
  }
  if (policy.expiringWarning) {
    return <InsuranceWarningBadge kind="expiring_soon" label={expiryLabel(policy)} />;
  }
  if (!policy.isInForce) {
    return <Badge tone="info">Starts {formatDate(policy.effectiveStartDate)}</Badge>;
  }
  return <Badge tone="ok">In force</Badge>;
}

function Meter({ value, limit }: { value: number; limit: number }) {
  const over = value > limit;
  const fill = limit > 0 ? Math.min(value / limit, 1) : value > 0 ? 1 : 0;
  return (
    <div className={over ? "hd-meter hd-meter--over" : "hd-meter"} aria-hidden>
      <span className="hd-meter__fill" style={{ width: `${fill * 100}%` }} />
    </div>
  );
}

function ScheduledTable({
  firearms,
  amounts,
  expired,
}: {
  firearms: FirearmSummary[];
  amounts: Map<number, { scheduledAmount: number; estimatedValue: number; underInsured: boolean }>;
  expired: boolean;
}) {
  const { open } = useNavigation();
  return (
    <table className="hd-mini-table">
      <thead>
        <tr>
          <th scope="col">Firearm</th>
          <th scope="col" className="hd-table__num">
            Value
          </th>
          <th scope="col" className="hd-table__num">
            Scheduled
          </th>
          <th scope="col">
            <span className="hd-sr-only">Status</span>
          </th>
        </tr>
      </thead>
      <tbody>
        {firearms.map((firearm) => {
          const entry = amounts.get(firearm.id);
          const shortfall = entry ? entry.estimatedValue - entry.scheduledAmount : 0;
          return (
            <tr key={firearm.id}>
              <td>
                <button
                  type="button"
                  className="hd-link"
                  onClick={() => open({ page: "firearm", id: firearm.id, from: "insurance" })}
                >
                  {firearmName(firearm)}
                </button>
                {firearm.serialNumber && (
                  <span className="hd-serial hd-mini-table__serial">{firearm.serialNumber}</span>
                )}
              </td>
              <td className="hd-table__num hd-num">{formatDollars(firearm.estimatedValue)}</td>
              <td className="hd-table__num hd-num">
                {formatDollars(entry?.scheduledAmount ?? null)}
              </td>
              <td className="hd-mini-table__status">
                {expired ? (
                  <InsuranceWarningBadge kind="uninsured" />
                ) : firearm.insuranceWarning === "under_insured" ? (
                  <InsuranceWarningBadge
                    kind="under_insured"
                    label={`${formatDollars(shortfall)} short`}
                  />
                ) : firearm.estimatedValue ? (
                  <Badge tone="ok">Covered</Badge>
                ) : (
                  <span className="hd-muted">No value set</span>
                )}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

export function FirearmLinkList({
  firearms,
  title,
  note,
  showValue,
  inline,
  onNavigate,
}: {
  firearms: FirearmSummary[];
  title?: string;
  note?: string;
  showValue?: boolean;
  inline?: boolean;
  onNavigate?: () => void;
}) {
  const { open } = useNavigation();
  const openRecord = (id: number) => {
    onNavigate?.();
    open({ page: "firearm", id, from: "insurance" });
  };
  return (
    <div className={inline ? "hd-linklist hd-linklist--inline" : "hd-linklist"}>
      {title && (
        <h3 className="hd-linklist__title">
          {title} <span className="hd-muted hd-num">{firearms.length}</span>
        </h3>
      )}
      {note && <p className="hd-linklist__note">{note}</p>}
      <ul>
        {firearms.map((firearm) => (
          <li key={firearm.id}>
            <button type="button" className="hd-link" onClick={() => openRecord(firearm.id)}>
              {firearmName(firearm)}
            </button>
            {!inline && firearm.serialNumber && (
              <span className="hd-serial hd-linklist__serial">{firearm.serialNumber}</span>
            )}
            {showValue && (
              <span className="hd-linklist__value hd-num">
                {formatDollars(firearm.estimatedValue)}
              </span>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
