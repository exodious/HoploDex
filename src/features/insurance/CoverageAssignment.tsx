import { useEffect, useState } from "react";
import { Button, Select, TextField } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import type { CoverageKind, Firearm } from "../firearms/types";
import * as insuranceService from "./insuranceService";
import type { InsurancePolicy } from "./types";

export interface CoverageAssignmentProps {
  firearm: Firearm;
  onAssigned: (firearm: Firearm) => void;
}

const NONE_VALUE = "__none__";

const COVERAGE_KIND_OPTIONS: { value: CoverageKind; label: string }[] = [
  { value: "individually_scheduled", label: "Individually scheduled" },
  { value: "blanket", label: "Blanket" },
];

function dollarsToCents(dollars: string): number {
  const parsed = Number.parseFloat(dollars.trim() || "0");
  return Number.isFinite(parsed) ? Math.round(parsed * 100) : 0;
}

/** Assigns a firearm to an insurance policy and coverage kind/amount (US3),
 * shown on the firearm detail view. */
export function CoverageAssignment({ firearm, onAssigned }: CoverageAssignmentProps) {
  const [policies, setPolicies] = useState<InsurancePolicy[]>([]);
  const [policyId, setPolicyId] = useState<string>(
    firearm.insurancePolicyId != null ? String(firearm.insurancePolicyId) : NONE_VALUE,
  );
  const [coverageKind, setCoverageKind] = useState<CoverageKind | "">(firearm.coverageKind ?? "");
  const [amountDollars, setAmountDollars] = useState(
    firearm.scheduledCoverageAmount != null
      ? (firearm.scheduledCoverageAmount / 100).toFixed(2)
      : "",
  );
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    insuranceService
      .listInsurancePolicies()
      .then(setPolicies)
      .catch(() => setPolicies([]));
  }, []);

  async function handleSave() {
    setError(null);
    if (policyId !== NONE_VALUE && !coverageKind) {
      setError("Select how this firearm is covered by the assigned policy.");
      return;
    }
    setSubmitting(true);
    try {
      const updated = await insuranceService.assignFirearmCoverage(firearm.id, {
        policyId: policyId === NONE_VALUE ? null : Number(policyId),
        coverageKind: policyId === NONE_VALUE ? undefined : (coverageKind as CoverageKind),
        scheduledCoverageAmount:
          policyId !== NONE_VALUE && coverageKind === "individually_scheduled"
            ? dollarsToCents(amountDollars)
            : undefined,
      });
      onAssigned(updated);
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Failed to update insurance coverage.");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div>
      <h3>Insurance coverage</h3>
      <Select
        label="Policy"
        value={policyId}
        onValueChange={(value) => {
          setPolicyId(value);
          if (value === NONE_VALUE) setCoverageKind("");
        }}
        options={[
          { value: NONE_VALUE, label: "None" },
          ...policies.map((p) => ({ value: String(p.id), label: p.name })),
        ]}
      />
      {policyId !== NONE_VALUE && (
        <>
          <Select
            label="Coverage kind"
            value={coverageKind}
            onValueChange={(value) => setCoverageKind(value as CoverageKind)}
            options={COVERAGE_KIND_OPTIONS}
          />
          {coverageKind === "individually_scheduled" && (
            <TextField
              label="Scheduled coverage amount ($)"
              inputMode="decimal"
              value={amountDollars}
              onChange={(e) => setAmountDollars(e.target.value)}
            />
          )}
        </>
      )}
      {error && (
        <p className="hd-field__error" role="alert">
          {error}
        </p>
      )}
      <div className="hd-dialog__actions">
        <Button variant="primary" onClick={handleSave} disabled={submitting}>
          Save coverage
        </Button>
      </div>
    </div>
  );
}
