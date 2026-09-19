import { Badge, Button, InsuranceWarningBadge } from "../../components";
import { formatDate } from "../../lib/dates";
import { formatCents } from "../../lib/money";
import { firearmName } from "../app/collectionStore";
import { useNavigation } from "../app/navigation";
import type { FirearmSummary } from "../browse/types";
import { policyExpiry } from "./coverage";
import type { InsurancePolicy, PolicySummary } from "./types";

/** One policy: its term, blanket usage, scheduled firearms, and contacts. */
export function PolicyCard({
  policy,
  summary,
  firearms,
  onEdit,
  onDelete,
}: {
  policy: InsurancePolicy;
  summary: PolicySummary | undefined;
  firearms: FirearmSummary[];
  onEdit: () => void;
  onDelete: () => void;
}) {
  const expiry = policyExpiry(policy.effectiveEndDate);
  const blanket = firearms.filter((f) => f.coverageKind === "blanket");
  const scheduled = firearms.filter((f) => f.coverageKind === "individually_scheduled");
  const scheduledAmounts = new Map(
    (summary?.individuallyScheduled ?? []).map((s) => [s.firearmId, s]),
  );
  const blanketTotal = summary?.blanketTotal ?? 0;
  const limit = policy.blanketCoverageLimit;
  const over = blanketTotal > limit;

  return (
    <article className="hd-policy" aria-labelledby={`policy-${policy.id}`}>
      <header className="hd-policy__head">
        <div className="hd-policy__title">
          <h3 className="hd-policy__name" id={`policy-${policy.id}`}>
            {policy.name}
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
        {expiry.expired ? (
          <InsuranceWarningBadge
            kind="expired"
            label={`Expired ${formatDate(policy.effectiveEndDate)}`}
          />
        ) : expiry.expiringSoon ? (
          <InsuranceWarningBadge
            kind="expiring_soon"
            label={
              expiry.daysLeft === 0
                ? "Expires today"
                : `Expires in ${expiry.daysLeft} ${expiry.daysLeft === 1 ? "day" : "days"}`
            }
          />
        ) : (
          <Badge tone="ok">In force</Badge>
        )}
      </div>
      {expiry.expired && (
        <p className="hd-policy__alert">
          Every firearm on an expired policy counts as uninsured. If it has been renewed, edit the
          end date.
        </p>
      )}

      {blanket.length > 0 ? (
        <div className="hd-policy__block">
          <div className="hd-policy__block-head">
            <span className="hd-eyebrow">Blanket coverage</span>
            <span className="hd-muted hd-num">
              {blanket.length} {blanket.length === 1 ? "firearm" : "firearms"}
            </span>
          </div>
          <Meter value={blanketTotal} limit={limit} />
          <p className="hd-policy__figures">
            <strong className="hd-num">{formatCents(blanketTotal, { whole: true })}</strong>
            <span className="hd-muted">
              {" "}
              of <span className="hd-num">{formatCents(limit, { whole: true })}</span> limit
            </span>
            {over ? (
              <InsuranceWarningBadge
                kind="under_insured"
                label={`Over by ${formatCents(blanketTotal - limit, { whole: true })}`}
              />
            ) : (
              <span className="hd-muted hd-num">
                {" "}
                · {formatCents(limit - blanketTotal, { whole: true })} to spare
              </span>
            )}
          </p>
          <FirearmLinkList firearms={blanket} inline />
        </div>
      ) : (
        limit > 0 && (
          <p className="hd-policy__block hd-muted">
            Blanket limit <span className="hd-num">{formatCents(limit, { whole: true })}</span>,
            with no firearms assigned to it yet.
          </p>
        )
      )}

      {scheduled.length > 0 && (
        <div className="hd-policy__block">
          <div className="hd-policy__block-head">
            <span className="hd-eyebrow">Scheduled individually</span>
          </div>
          <ScheduledTable
            firearms={scheduled}
            amounts={scheduledAmounts}
            expired={expiry.expired}
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
    </article>
  );
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
              <td className="hd-table__num hd-num">{formatCents(firearm.estimatedValue)}</td>
              <td className="hd-table__num hd-num">
                {formatCents(entry?.scheduledAmount ?? null)}
              </td>
              <td className="hd-mini-table__status">
                {expired ? (
                  <InsuranceWarningBadge kind="uninsured" />
                ) : firearm.insuranceWarning === "under_insured" ? (
                  <InsuranceWarningBadge
                    kind="under_insured"
                    label={`${formatCents(shortfall)} short`}
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
                {formatCents(firearm.estimatedValue)}
              </span>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
