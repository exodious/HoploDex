import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import {
  Button,
  Dialog,
  formChanges,
  MoneyField,
  Select,
  UNREADABLE,
  useCloseGuard,
} from "../../components";
import { dollarsToInput, formatDollars, parseDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { useCollection } from "../app/collectionStore";
import { useNavigation } from "../app/navigation";
import { useSessionScope } from "../session/sessionScope";
import { resumedValues, useDirtyForm, useResumedDraftTaken } from "../session/usePendingDraft";
import type { DraftTarget } from "../session/usePendingDraft";
import { useRecordSubject } from "../firearms/recordSubject";
import type { RecordSubject, RecordSubjectProps } from "../firearms/recordSubject";
import { expiryLabel } from "./coverage";
import type { AssignCoverageInput } from "./types";
import "../firearms/forms.css";

const NOT_SCHEDULED = "none";

export type CoverageDialogProps = RecordSubjectProps & {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSave: (input: AssignCoverageInput) => Promise<void>;
};

/** Schedules a firearm or an accessory under an insurance policy with its own
 * coverage amount, or leaves it unscheduled (001 US3, FR-014, FR-036;
 * 006 FR-009). An unscheduled record needs no assignment: the blanket policy
 * in force covers it. */
export function CoverageDialog({ open, onOpenChange, onSave, ...record }: CoverageDialogProps) {
  const subject = useRecordSubject(record as RecordSubjectProps);
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Insurance coverage"
      description={`Whether ${subject.name} is scheduled on its own policy, or covered by your blanket policy.`}
      bare
    >
      <CoverageForm subject={subject} onSave={onSave} onCancel={() => onOpenChange(false)} />
    </Dialog>
  );
}

/** The version of this form's kept drafts (003 research.md §16). Raise it when
 * {@link CoverageValues} changes shape, so older drafts are only discarded. */
export const FORM_VERSION = 1;

/** The form's input, as a kept draft holds it. */
interface CoverageValues {
  policyId: string;
  amount: string;
}

function CoverageForm({
  subject,
  onSave,
  onCancel,
}: {
  subject: RecordSubject;
  onSave: (input: AssignCoverageInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { policies, summary, loaded: listed, error } = useCollection();
  const loaded = listed && !error;
  const { open: goTo } = useNavigation();
  const [initialPolicyId] = useState(
    subject.insurancePolicyId != null ? String(subject.insurancePolicyId) : NOT_SCHEDULED,
  );
  const [initialAmount] = useState(() => dollarsToInput(subject.scheduledCoverageAmount));
  const scope = useSessionScope();
  const target: DraftTarget = {
    formVersion: FORM_VERSION,
    kind: subject.kind,
    mode: "coverage",
    targetId: subject.id,
  };
  // Pending changes the user resumed start as unsaved input (003 FR-039).
  const [resumed] = useState(() =>
    resumedValues<CoverageValues>(scope, target, {
      policyId: initialPolicyId,
      amount: initialAmount,
    }),
  );
  useResumedDraftTaken(target);
  const [policyId, setPolicyId] = useState(resumed.policyId);
  const [amount, setAmount] = useState(resumed.amount);
  const [submitted, setSubmitted] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [serverError, setServerError] = useState<string | null>(null);
  // A resumed draft's policy that no longer exists goes back to the saved
  // coverage, amount and all, once the policies have loaded (014 FR-020).
  useEffect(() => {
    if (!loaded || policyId === NOT_SCHEDULED) return;
    if (policies.some((policy) => String(policy.id) === policyId)) return;
    setPolicyId(initialPolicyId);
    setAmount(initialAmount);
    // Once, when the collection has loaded.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loaded]);

  const scheduled = policyId !== NOT_SCHEDULED;
  const valueNote =
    subject.estimatedValue != null
      ? `Its estimated replacement value is ${formatDollars(subject.estimatedValue)}.`
      : "It has no estimated value yet, so coverage can't be checked.";
  const blanket = summary?.blanket ?? null;
  const parsedAmount = parseDollars(amount);
  const amountError = !scheduled
    ? undefined
    : !parsedAmount.ok
      ? parsedAmount.error
      : parsedAmount.dollars == null
        ? "Enter the amount scheduled on the policy."
        : undefined;

  // Closing or quitting asks about unsaved input first (003 FR-010),
  // and a lock keeps it (003 FR-039).
  const values: CoverageValues = { policyId, amount };
  // 014 FR-002, FR-019: the amount is compared in whole dollars, and only
  // while scheduled, as a save stores it.
  const saved: CoverageValues = { policyId: initialPolicyId, amount: initialAmount };
  const { isDirty, changeFor } = formChanges({
    saved,
    current: values,
    fields: ["policyId", "amount"] as const,
    stored: (field, state) => {
      if (field === "policyId") return state.policyId;
      if (state.policyId === NOT_SCHEDULED) return "";
      const parsed = parseDollars(state.amount);
      return parsed.ok ? String(parsed.dollars ?? "") : UNREADABLE;
    },
    editing: true,
    display: (field) =>
      field === "amount"
        ? subject.scheduledCoverageAmount != null && initialPolicyId !== NOT_SCHEDULED
          ? formatDollars(subject.scheduledCoverageAmount)
          : null
        : (policies.find((p) => String(p.id) === initialPolicyId)?.name ?? "Not scheduled"),
    revert: (field) => {
      if (field === "amount") {
        setAmount(initialAmount);
        return;
      }
      setPolicyId(initialPolicyId);
      // 014 FR-012: back to unscheduled, the hidden amount leaves nothing the
      // record doesn't have; back to a schedule, an emptied amount is refilled.
      if (initialPolicyId === NOT_SCHEDULED || amount.trim() === "") setAmount(initialAmount);
    },
  });
  const requestClose = useCloseGuard(isDirty, onCancel);
  useDirtyForm({
    label: `${subject.name} (coverage)`,
    isDirty,
    submit: save,
    draft: { ...target, values },
  });

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    // Nothing differs from what is saved: Save says so and does nothing (014 FR-016).
    if (!isDirty) return;
    void save();
  }

  /** Validates and saves; resolves whether it was saved. */
  async function save(): Promise<boolean> {
    setSubmitted(true);
    if (amountError) return false;

    setSubmitting(true);
    setServerError(null);
    try {
      await onSave(
        scheduled && parsedAmount.ok && parsedAmount.dollars != null
          ? { policyId: Number(policyId), scheduledCoverageAmount: parsedAmount.dollars }
          : { policyId: null },
      );
      return true;
    } catch (e) {
      setServerError(e instanceof CommandFailure ? e.message : "Coverage couldn't be saved.");
      return false;
    } finally {
      setSubmitting(false);
    }
  }

  if (policies.length === 0) {
    return (
      <>
        <div className="hd-dialog__body">
          <p className="hd-form-note">
            You haven’t added any insurance policies yet. Add one on the Insurance page, then come
            back to schedule this {subject.noun} on it. A blanket policy covers every firearm and
            accessory you don’t schedule, with no assignment needed.
          </p>
        </div>
        <footer className="hd-dialog__footer">
          <Button variant="secondary" onClick={onCancel}>
            Cancel
          </Button>
          <Button variant="primary" onClick={() => goTo({ page: "insurance" })}>
            Go to Insurance
          </Button>
        </footer>
      </>
    );
  }

  return (
    <form className="hd-dialog__form" onSubmit={handleSubmit} noValidate>
      <div className="hd-dialog__body hd-form-section">
        {serverError && (
          <p className="hd-banner hd-banner--error" role="alert">
            {serverError}
          </p>
        )}
        <Select
          label="Policy"
          value={policyId}
          onValueChange={setPolicyId}
          options={[
            {
              value: NOT_SCHEDULED,
              label: "Not scheduled",
              detail: blanket ? `Covered by ${blanket.policyName}` : "Uninsured today",
            },
            ...policies.map((p) => ({
              value: String(p.id),
              label: p.name,
              detail: `${p.insuranceCompany} · ${expiryLabel(p)}`,
            })),
          ]}
          change={changeFor("policyId")}
        />
        {!scheduled && (
          <p className="hd-form-note">
            {blanket
              ? `Not scheduled, so it's covered by ${blanket.policyName}, along with ${subject.noun === "firearm" ? "every other firearm" : "everything else"} that isn't scheduled. Its value counts toward that policy's ${formatDollars(blanket.limit)} limit.${subject.estimatedValue != null ? ` ${valueNote}` : ""}`
              : `No blanket policy is in force, so an unscheduled ${subject.noun} is uninsured. Add a blanket policy on the Insurance page, or schedule this ${subject.noun} on a policy.`}
          </p>
        )}
        {scheduled && (
          <MoneyField
            label="Scheduled amount"
            required
            value={amount}
            onValueChange={setAmount}
            error={submitted ? amountError : undefined}
            change={changeFor("amount")}
            fieldClassName="hd-field--third"
            hint={valueNote}
          />
        )}
      </div>
      <footer className="hd-dialog__footer">
        <Button variant="secondary" onClick={requestClose} disabled={submitting}>
          Cancel
        </Button>
        <Button
          type="submit"
          variant="primary"
          pending={submitting}
          unavailable={isDirty ? undefined : "No changes to save"}
        >
          Save coverage
        </Button>
      </footer>
    </form>
  );
}
