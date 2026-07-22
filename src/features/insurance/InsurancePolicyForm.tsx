import { useState } from "react";
import type { FormEvent } from "react";
import { Button, TextField } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import type { InsurancePolicy, InsurancePolicyInput } from "./types";

export interface InsurancePolicyFormProps {
  initialValues?: InsurancePolicy;
  onSubmit: (input: InsurancePolicyInput) => Promise<void>;
  onCancel?: () => void;
}

interface FormState {
  name: string;
  policyNumber: string;
  insuranceCompany: string;
  companyContact: string;
  agentName: string;
  agentContact: string;
  blanketCoverageLimitDollars: string;
  effectiveStartDate: string;
  effectiveEndDate: string;
}

function toFormState(policy?: InsurancePolicy): FormState {
  return {
    name: policy?.name ?? "",
    policyNumber: policy?.policyNumber ?? "",
    insuranceCompany: policy?.insuranceCompany ?? "",
    companyContact: policy?.companyContact ?? "",
    agentName: policy?.agentName ?? "",
    agentContact: policy?.agentContact ?? "",
    blanketCoverageLimitDollars: policy ? (policy.blanketCoverageLimit / 100).toFixed(2) : "",
    effectiveStartDate: policy?.effectiveStartDate ?? "",
    effectiveEndDate: policy?.effectiveEndDate ?? "",
  };
}

function blankToNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed === "" ? null : trimmed;
}

function dollarsToCents(dollars: string): number {
  const parsed = Number.parseFloat(dollars.trim() || "0");
  return Number.isFinite(parsed) ? Math.round(parsed * 100) : 0;
}

function validate(form: FormState): Record<string, string> {
  const errors: Record<string, string> = {};
  if (form.name.trim() === "") errors.name = "Name is required.";
  if (form.policyNumber.trim() === "") errors.policyNumber = "Policy number is required.";
  if (form.insuranceCompany.trim() === "")
    errors.insuranceCompany = "Insurance company is required.";
  if (form.effectiveStartDate === "") errors.effectiveStartDate = "Start date is required.";
  if (form.effectiveEndDate === "") {
    errors.effectiveEndDate = "End date is required.";
  } else if (form.effectiveStartDate && form.effectiveEndDate <= form.effectiveStartDate) {
    errors.effectiveEndDate = "End date must be after the start date.";
  }
  return errors;
}

/** Create/edit form for an insurance policy (US3). */
export function InsurancePolicyForm({
  initialValues,
  onSubmit,
  onCancel,
}: InsurancePolicyFormProps) {
  const [form, setForm] = useState<FormState>(() => toFormState(initialValues));
  const [touched, setTouched] = useState<Record<string, boolean>>({});
  const [submitting, setSubmitting] = useState(false);
  const [serverError, setServerError] = useState<CommandFailure | null>(null);

  const clientErrors = validate(form);
  const fieldError = (field: string): string | undefined =>
    touched[field] ? (clientErrors[field] ?? serverError?.fieldErrors?.[field]) : undefined;

  function update<K extends keyof FormState>(key: K, value: FormState[K]) {
    setForm((prev) => ({ ...prev, [key]: value }));
  }

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setTouched({
      name: true,
      policyNumber: true,
      insuranceCompany: true,
      effectiveStartDate: true,
      effectiveEndDate: true,
    });
    if (Object.keys(clientErrors).length > 0) return;

    const input: InsurancePolicyInput = {
      name: form.name.trim(),
      policyNumber: form.policyNumber.trim(),
      insuranceCompany: form.insuranceCompany.trim(),
      companyContact: blankToNull(form.companyContact),
      agentName: blankToNull(form.agentName),
      agentContact: blankToNull(form.agentContact),
      blanketCoverageLimit: dollarsToCents(form.blanketCoverageLimitDollars),
      effectiveStartDate: form.effectiveStartDate,
      effectiveEndDate: form.effectiveEndDate,
    };

    setSubmitting(true);
    setServerError(null);
    try {
      await onSubmit(input);
    } catch (error) {
      if (error instanceof CommandFailure) {
        setServerError(error);
      } else {
        throw error;
      }
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} noValidate>
      {serverError && !serverError.fieldErrors && (
        <p className="hd-field__error" role="alert">
          {serverError.message}
        </p>
      )}

      <TextField
        label="Policy name"
        value={form.name}
        onChange={(e) => update("name", e.target.value)}
        onBlur={() => setTouched((t) => ({ ...t, name: true }))}
        error={fieldError("name")}
        required
      />
      <TextField
        label="Policy number"
        value={form.policyNumber}
        onChange={(e) => update("policyNumber", e.target.value)}
        onBlur={() => setTouched((t) => ({ ...t, policyNumber: true }))}
        error={fieldError("policyNumber")}
        required
      />
      <TextField
        label="Insurance company"
        value={form.insuranceCompany}
        onChange={(e) => update("insuranceCompany", e.target.value)}
        onBlur={() => setTouched((t) => ({ ...t, insuranceCompany: true }))}
        error={fieldError("insuranceCompany")}
        required
      />
      <TextField
        label="Company contact"
        value={form.companyContact}
        onChange={(e) => update("companyContact", e.target.value)}
      />
      <TextField
        label="Agent name"
        value={form.agentName}
        onChange={(e) => update("agentName", e.target.value)}
      />
      <TextField
        label="Agent contact"
        value={form.agentContact}
        onChange={(e) => update("agentContact", e.target.value)}
      />
      <TextField
        label="Blanket coverage limit ($)"
        inputMode="decimal"
        value={form.blanketCoverageLimitDollars}
        onChange={(e) => update("blanketCoverageLimitDollars", e.target.value)}
      />
      <TextField
        label="Effective start date"
        type="date"
        value={form.effectiveStartDate}
        onChange={(e) => update("effectiveStartDate", e.target.value)}
        onBlur={() => setTouched((t) => ({ ...t, effectiveStartDate: true }))}
        error={fieldError("effectiveStartDate")}
        required
      />
      <TextField
        label="Effective end date"
        type="date"
        value={form.effectiveEndDate}
        onChange={(e) => update("effectiveEndDate", e.target.value)}
        onBlur={() => setTouched((t) => ({ ...t, effectiveEndDate: true }))}
        error={fieldError("effectiveEndDate")}
        required
      />

      <div className="hd-dialog__actions">
        {onCancel && (
          <Button type="button" variant="secondary" onClick={onCancel} disabled={submitting}>
            Cancel
          </Button>
        )}
        <Button type="submit" variant="primary" disabled={submitting}>
          {initialValues ? "Save changes" : "Add policy"}
        </Button>
      </div>
    </form>
  );
}
