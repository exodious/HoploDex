import { useState } from "react";
import type { FormEvent } from "react";
import { Button, ChoiceCards, Dialog, MoneyField, Select } from "../../components";
import { centsToInput, formatCents, parseDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { firearmName, useCollection } from "../app/collectionStore";
import { useNavigation } from "../app/navigation";
import type { CoverageKind, Firearm } from "../firearms/types";
import { expiryLabel } from "./coverage";
import type { AssignCoverageInput } from "./types";
import "../firearms/forms.css";

const NOT_INSURED = "none";

export interface CoverageDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  firearm: Firearm;
  onSave: (input: AssignCoverageInput) => Promise<void>;
}

/** Assigns a firearm to an insurance policy, as individually scheduled or
 * blanket-covered (US3, FR-014). */
export function CoverageDialog({ open, onOpenChange, firearm, onSave }: CoverageDialogProps) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Insurance coverage"
      description={`Which policy covers ${firearmName(firearm)}, and how.`}
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
  const { policies, policiesById, firearms } = useCollection();
  const { navigate } = useNavigation();
  const [policyId, setPolicyId] = useState(
    firearm.insurancePolicyId != null ? String(firearm.insurancePolicyId) : NOT_INSURED,
  );
  const [kind, setKind] = useState<CoverageKind | "">(firearm.coverageKind ?? "");
  const [amount, setAmount] = useState(centsToInput(firearm.scheduledCoverageAmount));
  const [submitted, setSubmitted] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [serverError, setServerError] = useState<string | null>(null);

  const insured = policyId !== NOT_INSURED;
  const policy = insured ? policiesById.get(Number(policyId)) : undefined;
  const blanketMates = firearms.filter(
    (f) =>
      f.status === "active" &&
      f.id !== firearm.id &&
      f.insurancePolicyId === policy?.id &&
      f.coverageKind === "blanket",
  ).length;
  const parsedAmount = parseDollars(amount);

  const errors = {
    kind: insured && !kind ? "Choose how the policy covers it." : undefined,
    amount:
      insured && kind === "individually_scheduled"
        ? !parsedAmount.ok
          ? parsedAmount.error
          : parsedAmount.cents == null
            ? "Enter the amount scheduled on the policy."
            : undefined
        : undefined,
  };

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setSubmitted(true);
    if (errors.kind || errors.amount) return;

    setSubmitting(true);
    setServerError(null);
    try {
      await onSave(
        insured
          ? {
              policyId: Number(policyId),
              coverageKind: kind as CoverageKind,
              scheduledCoverageAmount:
                kind === "individually_scheduled" && parsedAmount.ok
                  ? (parsedAmount.cents ?? undefined)
                  : undefined,
            }
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
            back to assign this firearm to it.
          </p>
        </div>
        <footer className="hd-dialog__footer">
          <Button variant="secondary" onClick={onCancel}>
            Cancel
          </Button>
          <Button variant="primary" onClick={() => navigate({ page: "insurance" })}>
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
          onValueChange={(value) => {
            setPolicyId(value);
            if (value === NOT_INSURED) setKind("");
          }}
          options={[
            { value: NOT_INSURED, label: "Not insured" },
            ...policies.map((p) => ({
              value: String(p.id),
              label: p.name,
              detail: `${p.insuranceCompany} · ${expiryLabel(p.effectiveEndDate)}`,
            })),
          ]}
        />
        {insured && (
          <ChoiceCards<CoverageKind>
            label="How it’s covered"
            required
            value={kind}
            onChange={setKind}
            error={submitted ? errors.kind : undefined}
            minCardWidth={200}
            options={[
              {
                value: "individually_scheduled",
                label: "Scheduled individually",
                description: "Listed on the policy with its own coverage amount.",
              },
              {
                value: "blanket",
                label: "Blanket",
                description: !policy
                  ? "Shares the policy's blanket limit."
                  : policy.blanketCoverageLimit === 0
                    ? "This policy has no blanket limit, so blanket coverage would leave it under-insured."
                    : `Shares the ${formatCents(policy.blanketCoverageLimit, { whole: true })} blanket limit${blanketMates > 0 ? ` with ${blanketMates} other ${blanketMates === 1 ? "firearm" : "firearms"}` : ""}.`,
              },
            ]}
          />
        )}
        {insured && kind === "individually_scheduled" && (
          <MoneyField
            label="Scheduled amount"
            required
            value={amount}
            onValueChange={setAmount}
            error={submitted ? errors.amount : undefined}
            hint={
              firearm.estimatedValue != null
                ? `Its estimated replacement value is ${formatCents(firearm.estimatedValue)}.`
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
