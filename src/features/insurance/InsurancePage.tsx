import { useState } from "react";
import {
  Badge,
  Button,
  ConfirmDialog,
  Dialog,
  Icon,
  InsuranceWarningBadge,
  useToast,
} from "../../components";
import { formatDate } from "../../lib/dates";
import { formatCents } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { BackLink } from "../app/BackLink";
import { firearmName, useCollection } from "../app/collectionStore";
import { useNavigation } from "../app/navigation";
import type { FirearmSummary } from "../browse/types";
import { coverageShortfall, policyExpiry } from "./coverage";
import * as insuranceService from "./insuranceService";
import { InsurancePolicyForm } from "./InsurancePolicyForm";
import { policyCardId } from "./policyCard";
import type { InsurancePolicy, InsurancePolicyInput, PolicySummary } from "./types";
import "./insurance.css";

/** Value and coverage across the collection (US3): the collection-wide
 * total, every policy with its blanket usage and scheduled firearms
 * (FR-015, FR-017), expiry warnings (FR-028), and whatever isn't covered.
 * Policies can be added, edited, and deleted here (FR-027). */
export function InsurancePage() {
  const { firearms, summary, policies, refresh } = useCollection();
  const notify = useToast();
  const { route, back } = useNavigation();
  const focusedPolicyId = route.page === "insurance" ? route.policyId : undefined;
  const [editing, setEditing] = useState<InsurancePolicy | "new" | null>(null);
  const [deleting, setDeleting] = useState<InsurancePolicy | null>(null);

  const active = firearms.filter((f) => f.status === "active");
  const valued = active.filter((f) => (f.estimatedValue ?? 0) > 0);
  const sum = (list: FirearmSummary[]) => list.reduce((n, f) => n + (f.estimatedValue ?? 0), 0);
  const covered = valued.filter((f) => f.insuranceWarning === "none");
  const under = valued.filter((f) => f.insuranceWarning === "under_insured");
  const uninsured = valued.filter((f) => f.insuranceWarning === "uninsured");
  const unvalued = active.filter((f) => !f.estimatedValue);

  async function handleSave(input: InsurancePolicyInput) {
    if (editing === "new") {
      const created = await insuranceService.createInsurancePolicy(input);
      notify(`Added ${created.name}.`);
    } else if (editing) {
      const updated = await insuranceService.updateInsurancePolicy(editing.id, input);
      notify(`Saved changes to ${updated.name}.`);
    }
    setEditing(null);
    await refresh();
  }

  async function handleDelete(policy: InsurancePolicy) {
    try {
      await insuranceService.deleteInsurancePolicy(policy.id, true);
      notify(`Deleted ${policy.name}.`);
      await refresh();
    } catch (e) {
      notify(
        e instanceof CommandFailure ? e.message : `${policy.name} couldn't be deleted.`,
        "error",
      );
    }
  }

  const assignedTo = (policy: InsurancePolicy) =>
    firearms.filter((f) => f.insurancePolicyId === policy.id);

  return (
    <>
      {back && (
        <div className="hd-page-back">
          <BackLink target={back} />
        </div>
      )}
      <header className="hd-page-head">
        <div>
          <h1 className="hd-page-title">Insurance</h1>
          <p className="hd-page-sub">
            <strong className="hd-num">
              {formatCents(summary?.collectionTotal ?? 0, { whole: true })}
            </strong>{" "}
            estimated replacement value across <span className="hd-num">{active.length}</span>{" "}
            active {active.length === 1 ? "firearm" : "firearms"}
          </p>
        </div>
        <Button variant="primary" icon="plus" onClick={() => setEditing("new")}>
          Add policy
        </Button>
      </header>

      <CoverageOverview
        covered={sum(covered)}
        under={sum(under)}
        underMissing={coverageShortfall(summary)}
        uninsured={sum(uninsured)}
        counts={{ covered: covered.length, under: under.length, uninsured: uninsured.length }}
        unvaluedCount={unvalued.length}
      />

      <section className="hd-insurance-section" aria-labelledby="policies-title">
        <h2 className="hd-section-title" id="policies-title">
          Policies
          <span className="hd-section-title__count hd-num">{policies.length}</span>
        </h2>
        {policies.length === 0 ? (
          <div className="hd-empty hd-empty--compact">
            <Icon name="shield" size={28} />
            <p className="hd-empty__text">
              Add each policy that covers your firearms, whether that's a rider on a homeowner's
              policy or a dedicated collection policy. Then assign firearms to it from their
              records.
            </p>
            <Button variant="primary" icon="plus" onClick={() => setEditing("new")}>
              Add policy
            </Button>
          </div>
        ) : (
          <div className="hd-policies">
            {policies.map((policy) => (
              <PolicyCard
                key={policy.id}
                policy={policy}
                focused={policy.id === focusedPolicyId}
                summary={summary?.byPolicy.find((p) => p.policyId === policy.id)}
                firearms={assignedTo(policy).filter((f) => f.status === "active")}
                onEdit={() => setEditing(policy)}
                onDelete={() => setDeleting(policy)}
              />
            ))}
          </div>
        )}
      </section>

      {(uninsured.length > 0 || unvalued.length > 0) && (
        <section className="hd-insurance-section" aria-labelledby="gaps-title">
          <h2 className="hd-section-title" id="gaps-title">
            Not covered
          </h2>
          <div className="hd-gaps">
            {uninsured.length > 0 && (
              <FirearmLinkList
                title="Uninsured"
                note="No policy, or the policy has expired."
                firearms={uninsured}
                showValue
              />
            )}
            {unvalued.length > 0 && (
              <FirearmLinkList
                title="No estimated value"
                note="Coverage can't be checked until a value is set."
                firearms={unvalued}
              />
            )}
          </div>
        </section>
      )}

      <Dialog
        open={editing != null}
        onOpenChange={(open) => !open && setEditing(null)}
        title={editing === "new" || editing == null ? "Add policy" : `Edit ${editing.name}`}
        size="lg"
        bare
      >
        <InsurancePolicyForm
          initialValues={editing === "new" || editing == null ? undefined : editing}
          onSubmit={handleSave}
          onCancel={() => setEditing(null)}
        />
      </Dialog>

      {deleting && assignedTo(deleting).length > 0 ? (
        <Dialog
          open
          onOpenChange={(open) => !open && setDeleting(null)}
          title={`${deleting.name} still covers firearms`}
          description="Assign these firearms to another policy, or mark them not insured, before deleting it. Deleting it first would silently leave them uninsured."
          footer={
            <Button variant="primary" onClick={() => setDeleting(null)}>
              OK
            </Button>
          }
        >
          <FirearmLinkList firearms={assignedTo(deleting)} onNavigate={() => setDeleting(null)} />
        </Dialog>
      ) : (
        <ConfirmDialog
          open={deleting != null}
          onOpenChange={(open) => !open && setDeleting(null)}
          title={`Delete ${deleting?.name ?? "this policy"}?`}
          description="The policy and its details will be permanently removed. No firearms are assigned to it."
          confirmLabel="Delete policy"
          onConfirm={() => (deleting ? handleDelete(deleting) : undefined)}
        />
      )}
    </>
  );
}

function CoverageOverview({
  covered,
  under,
  underMissing,
  uninsured,
  counts,
  unvaluedCount,
}: {
  covered: number;
  /** Combined value of the under-insured firearms — sizes the bar. */
  under: number;
  /** How much coverage those firearms are short by — what the legend shows. */
  underMissing: number;
  uninsured: number;
  counts: { covered: number; under: number; uninsured: number };
  unvaluedCount: number;
}) {
  const total = covered + under + uninsured;
  if (total === 0) {
    return (
      <p className="hd-overview-note">
        {unvaluedCount > 0
          ? "Set estimated replacement values on your firearms to see how much of the collection is covered."
          : "Add firearms with estimated values to see how much of the collection is covered."}
      </p>
    );
  }
  const pct = (n: number) => `${(n / total) * 100}%`;
  // The bar shows where the collection's value sits; the legend shows the
  // gap for under-insured firearms (the part of their value that isn't
  // covered) and the whole value for uninsured ones.
  const segments = [
    { key: "covered", label: "Covered", value: covered, shown: covered, count: counts.covered },
    {
      key: "under",
      label: "Under-insured",
      value: under,
      shown: underMissing,
      suffix: "short",
      count: counts.under,
    },
    {
      key: "uninsured",
      label: "Uninsured",
      value: uninsured,
      shown: uninsured,
      count: counts.uninsured,
    },
  ].filter((s) => s.value > 0);

  return (
    <section className="hd-overview" aria-label="Coverage of the collection's value">
      <div className="hd-overview__bar" aria-hidden>
        {segments.map((s) => (
          <span
            key={s.key}
            className={`hd-overview__seg hd-overview__seg--${s.key}`}
            style={{ width: pct(s.value) }}
          />
        ))}
      </div>
      <ul className="hd-overview__legend">
        {segments.map((s) => (
          <li key={s.key}>
            <span className={`hd-overview__swatch hd-overview__seg--${s.key}`} aria-hidden />
            <span className="hd-overview__label">{s.label}</span>
            <strong className="hd-num">{formatCents(s.shown, { whole: true })}</strong>
            {s.suffix && <span className="hd-muted">{s.suffix}</span>}
            <span className="hd-muted hd-num">
              {s.count} {s.count === 1 ? "firearm" : "firearms"}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}

function PolicyCard({
  policy,
  focused,
  summary,
  firearms,
  onEdit,
  onDelete,
}: {
  policy: InsurancePolicy;
  /** Arrived here from a link to this policy. */
  focused: boolean;
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
    <article
      className={focused ? "hd-policy hd-policy--focused" : "hd-policy"}
      id={policyCardId(policy.id)}
      aria-labelledby={`policy-${policy.id}`}
    >
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

function FirearmLinkList({
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
