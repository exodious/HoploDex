import { useState } from "react";
import type { FormEvent } from "react";
import { Button, Dialog, MoneyField, Select } from "../../components";
import { dollarsToInput, formatDollars, parseDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { firearmName, useCollection } from "../app/collectionStore";
import { useNavigation } from "../app/navigation";
import type { Firearm } from "../firearms/types";
import { expiryLabel } from "./coverage";
import type { AssignCoverageInput } from "./types";
import "../firearms/forms.css";

const NOT_SCHEDULED = "none";

export interface CoverageDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  firearm: Firearm;
  onSave: (input: AssignCoverageInput) => Promise<void>;
}

/** Schedules a firearm under an insurance policy with its own coverage
 * amount, or leaves it unscheduled (US3, FR-014, FR-036). An unscheduled
 * firearm needs no assignment: the blanket policy in force covers it. */
export function CoverageDialog({ open, onOpenChange, firearm, onSave }: CoverageDialogProps) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Insurance coverage"
      description={`Whether ${firearmName(firearm)} is scheduled on its own policy, or covered by your blanket policy.`}
      bare
    >
      <CoverageForm firearm={firearm} onSave={onSave} onCancel={() => onOpenChange(false)} />
    </Dialog>
  );
}

function CoverageForm({
  firearm,
  onSave,
  onCancel,
}: {
  firearm: Firearm;
  onSave: (input: AssignCoverageInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { policies, summary } = useCollection();
  const { open: goTo } = useNavigation();
  const [policyId, setPolicyId] = useState(
    firearm.insurancePolicyId != null ? String(firearm.insurancePolicyId) : NOT_SCHEDULED,
  );
  const [amount, setAmount] = useState(dollarsToInput(firearm.scheduledCoverageAmount));
  const [submitted, setSubmitted] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [serverError, setServerError] = useState<string | null>(null);

  const scheduled = policyId !== NOT_SCHEDULED;
  const blanket = summary?.blanket ?? null;
  const parsedAmount = parseDollars(amount);
  const amountError = !scheduled
    ? undefined
    : !parsedAmount.ok
      ? parsedAmount.error
      : parsedAmount.dollars == null
        ? "Enter the amount scheduled on the policy."
        : undefined;

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setSubmitted(true);
    if (amountError) return;

    setSubmitting(true);
    setServerError(null);
    try {
      await onSave(
        scheduled && parsedAmount.ok && parsedAmount.dollars != null
          ? { policyId: Number(policyId), scheduledCoverageAmount: parsedAmount.dollars }
          : { policyId: null },
      );
    } catch (e) {
      setServerError(e instanceof CommandFailure ? e.message : "Coverage couldn't be saved.");
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
            back to schedule this firearm on it. A blanket policy covers every firearm you don’t
            schedule, with no assignment needed.
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
        />
        {!scheduled && (
          <p className="hd-form-note">
            {blanket
              ? `Not scheduled, so it's covered by ${blanket.policyName}, along with every other firearm that isn't scheduled. Its value counts toward that policy's ${formatDollars(blanket.limit)} limit.`
              : "No blanket policy is in force, so an unscheduled firearm is uninsured. Add a blanket policy on the Insurance page, or schedule this firearm on a policy."}
          </p>
        )}
        {scheduled && (
          <MoneyField
            label="Scheduled amount"
            required
            value={amount}
            onValueChange={setAmount}
            error={submitted ? amountError : undefined}
            fieldClassName="hd-field--third"
            hint={
              firearm.estimatedValue != null
                ? `Its estimated replacement value is ${formatDollars(firearm.estimatedValue)}.`
                : "It has no estimated value yet, so coverage can't be checked."
            }
          />
        )}
      </div>
      <footer className="hd-dialog__footer">
        <Button variant="secondary" onClick={onCancel} disabled={submitting}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" pending={submitting}>
          Save coverage
        </Button>
      </footer>
    </form>
  );
}
