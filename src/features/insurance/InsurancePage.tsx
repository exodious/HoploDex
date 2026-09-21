import { Button, Icon } from "../../components";
import { formatDollars } from "../../lib/money";
import { BackLink } from "../app/BackLink";
import { useCollection } from "../app/collectionStore";
import { useNavigation } from "../app/navigation";
import type { FirearmSummary } from "../browse/types";
import { coverageShortfall } from "./coverage";
import { FirearmLinkList, PolicyCard } from "./PolicyCard";
import { usePolicyEditors } from "./PolicyEditors";
import type { InsurancePolicy } from "./types";
import "./insurance.css";

/** Value and coverage across the collection (US3): the collection-wide
 * total, every policy with its blanket usage and scheduled firearms
 * (FR-015, FR-017), expiry warnings (FR-028), and whatever isn't covered.
 * Policies can be added, edited, and deleted here (FR-027). */
export function InsurancePage() {
  const { firearms, summary, policies } = useCollection();
  const { back, open } = useNavigation();

  const active = firearms.filter((f) => f.status === "active");
  const valued = active.filter((f) => (f.estimatedValue ?? 0) > 0);
  const sum = (list: FirearmSummary[]) => list.reduce((n, f) => n + (f.estimatedValue ?? 0), 0);
  const covered = valued.filter((f) => f.insuranceWarning === "none");
  const under = valued.filter((f) => f.insuranceWarning === "under_insured");
  const uninsured = valued.filter((f) => f.insuranceWarning === "uninsured");
  const unvalued = active.filter((f) => !f.estimatedValue);

  const { add, edit, remove, dialogs } = usePolicyEditors();
  const scheduledOn = (policy: InsurancePolicy) =>
    firearms.filter((f) => f.insurancePolicyId === policy.id && f.status === "active");

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
            <strong className="hd-num">{formatDollars(summary?.collectionTotal ?? 0)}</strong>{" "}
            estimated replacement value across <span className="hd-num">{active.length}</span>{" "}
            active {active.length === 1 ? "firearm" : "firearms"}
          </p>
        </div>
        <Button variant="primary" icon="plus" onClick={() => add()}>
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
              Add each policy that covers your firearms. A blanket policy, with a coverage limit,
              covers every firearm you haven't scheduled, with nothing to assign. A policy without a
              limit covers only the firearms you schedule on it from their records.
            </p>
            <Button variant="primary" icon="plus" onClick={() => add()}>
              Add policy
            </Button>
          </div>
        ) : (
          <div className="hd-policies">
            {policies.map((policy) => (
              <PolicyCard
                key={policy.id}
                policy={policy}
                blanket={summary?.blanket ?? null}
                summary={summary?.byPolicy.find((p) => p.policyId === policy.id)}
                firearms={scheduledOn(policy)}
                onOpen={() => open({ page: "policy", id: policy.id })}
                onEdit={() => edit(policy)}
                onDelete={() => remove(policy)}
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
                note="No blanket policy is in force for them, or the policy they're scheduled on has expired."
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

      {dialogs}
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
            <strong className="hd-num">{formatDollars(s.shown)}</strong>
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
