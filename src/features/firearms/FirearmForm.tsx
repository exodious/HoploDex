import { useState } from "react";
import type { FormEvent } from "react";
import { Button, Checkbox, Select, TextArea, TextField } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import { FIREARM_TYPE_OPTIONS } from "./types";
import type { Firearm, FirearmInput } from "./types";

export interface FirearmFormProps {
  /** Present in edit mode; omitted when creating a new record. */
  initialValues?: Firearm;
  onSubmit: (input: FirearmInput) => Promise<void>;
  onCancel?: () => void;
}

interface FormState {
  make: string;
  model: string;
  caliber: string;
  firearmTypeId: string;
  serialNumber: string;
  noSerialAttested: boolean;
  notes: string;
  accessories: string;
  estimatedValueDollars: string;
  acquisitionSource: string;
  acquisitionDate: string;
  acquisitionPriceDollars: string;
}

function toFormState(firearm?: Firearm): FormState {
  return {
    make: firearm?.make ?? "",
    model: firearm?.model ?? "",
    caliber: firearm?.caliber ?? "",
    firearmTypeId: firearm ? String(firearm.firearmTypeId) : "",
    serialNumber: firearm?.serialNumber ?? "",
    noSerialAttested: firearm?.noSerialAttested ?? false,
    notes: firearm?.notes ?? "",
    accessories: firearm?.accessories ?? "",
    estimatedValueDollars: centsToDollarsString(firearm?.estimatedValue ?? null),
    acquisitionSource: firearm?.acquisitionSource ?? "",
    acquisitionDate: firearm?.acquisitionDate ?? "",
    acquisitionPriceDollars: centsToDollarsString(firearm?.acquisitionPrice ?? null),
  };
}

function centsToDollarsString(cents: number | null): string {
  return cents == null ? "" : (cents / 100).toFixed(2);
}

function dollarsStringToCents(dollars: string): number | null {
  const trimmed = dollars.trim();
  if (trimmed === "") return null;
  const parsed = Number.parseFloat(trimmed);
  return Number.isFinite(parsed) ? Math.round(parsed * 100) : null;
}

function blankToNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed === "" ? null : trimmed;
}

/** Client-side mirror of the backend's serial-attestation rule (FR-029),
 * so the user gets immediate feedback instead of a round-trip error. */
function validate(form: FormState): Record<string, string> {
  const errors: Record<string, string> = {};
  if (form.make.trim() === "") errors.make = "Make is required.";
  if (form.model.trim() === "") errors.model = "Model is required.";
  if (form.caliber.trim() === "") errors.caliber = "Caliber is required.";
  if (form.firearmTypeId === "") errors.firearmTypeId = "Type is required.";
  if (form.serialNumber.trim() === "" && !form.noSerialAttested) {
    errors.serialNumber = "Enter a serial number, or confirm this firearm has none.";
  }
  return errors;
}

/** Create/edit form for a firearm's core record (US1). Disposition and
 * insurance coverage are set via their own dedicated actions elsewhere. */
export function FirearmForm({ initialValues, onSubmit, onCancel }: FirearmFormProps) {
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
      make: true,
      model: true,
      caliber: true,
      firearmTypeId: true,
      serialNumber: true,
    });
    if (Object.keys(clientErrors).length > 0) {
      return;
    }

    const input: FirearmInput = {
      make: form.make.trim(),
      model: form.model.trim(),
      caliber: form.caliber.trim(),
      firearmTypeId: Number(form.firearmTypeId),
      serialNumber: form.noSerialAttested
        ? blankToNull(form.serialNumber)
        : form.serialNumber.trim(),
      noSerialAttested: form.noSerialAttested,
      notes: blankToNull(form.notes),
      accessories: blankToNull(form.accessories),
      estimatedValue: dollarsStringToCents(form.estimatedValueDollars),
      acquisitionSource: blankToNull(form.acquisitionSource),
      acquisitionDate: blankToNull(form.acquisitionDate),
      acquisitionPrice: dollarsStringToCents(form.acquisitionPriceDollars),
      status: initialValues?.status ?? "active",
      dispositionType: initialValues?.dispositionType ?? null,
      dispositionRecipient: initialValues?.dispositionRecipient ?? null,
      dispositionDate: initialValues?.dispositionDate ?? null,
      dispositionPrice: initialValues?.dispositionPrice ?? null,
      insurancePolicyId: initialValues?.insurancePolicyId ?? null,
      coverageKind: initialValues?.coverageKind ?? null,
      scheduledCoverageAmount: initialValues?.scheduledCoverageAmount ?? null,
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
        label="Make"
        value={form.make}
        onChange={(e) => update("make", e.target.value)}
        onBlur={() => setTouched((t) => ({ ...t, make: true }))}
        error={fieldError("make")}
        required
      />
      <TextField
        label="Model"
        value={form.model}
        onChange={(e) => update("model", e.target.value)}
        onBlur={() => setTouched((t) => ({ ...t, model: true }))}
        error={fieldError("model")}
        required
      />
      <TextField
        label="Caliber"
        value={form.caliber}
        onChange={(e) => update("caliber", e.target.value)}
        onBlur={() => setTouched((t) => ({ ...t, caliber: true }))}
        error={fieldError("caliber")}
        required
      />
      <Select
        label="Type"
        value={form.firearmTypeId}
        onValueChange={(value) => {
          update("firearmTypeId", value);
          setTouched((t) => ({ ...t, firearmTypeId: true }));
        }}
        options={FIREARM_TYPE_OPTIONS}
        error={fieldError("firearmTypeId")}
      />

      <TextField
        label="Serial number"
        value={form.serialNumber}
        onChange={(e) => update("serialNumber", e.target.value)}
        onBlur={() => setTouched((t) => ({ ...t, serialNumber: true }))}
        error={fieldError("serialNumber")}
        disabled={form.noSerialAttested}
      />
      <Checkbox
        label="This firearm has no serial number"
        checked={form.noSerialAttested}
        onCheckedChange={(checked) => {
          update("noSerialAttested", checked);
          setTouched((t) => ({ ...t, serialNumber: true }));
        }}
      />

      <TextArea
        label="Notes"
        value={form.notes}
        onChange={(e) => update("notes", e.target.value)}
        hint="Condition notes and any other free-form details."
      />
      <TextArea
        label="Accessories"
        value={form.accessories}
        onChange={(e) => update("accessories", e.target.value)}
      />

      <TextField
        label="Estimated value ($)"
        inputMode="decimal"
        value={form.estimatedValueDollars}
        onChange={(e) => update("estimatedValueDollars", e.target.value)}
      />

      <TextField
        label="Acquisition source"
        value={form.acquisitionSource}
        onChange={(e) => update("acquisitionSource", e.target.value)}
      />
      <TextField
        label="Acquisition date"
        type="date"
        value={form.acquisitionDate}
        onChange={(e) => update("acquisitionDate", e.target.value)}
      />
      <TextField
        label="Acquisition price ($)"
        inputMode="decimal"
        value={form.acquisitionPriceDollars}
        onChange={(e) => update("acquisitionPriceDollars", e.target.value)}
      />

      <div className="hd-dialog__actions">
        {onCancel && (
          <Button type="button" variant="secondary" onClick={onCancel} disabled={submitting}>
            Cancel
          </Button>
        )}
        <Button type="submit" variant="primary" disabled={submitting}>
          {initialValues ? "Save changes" : "Add firearm"}
        </Button>
      </div>
    </form>
  );
}
