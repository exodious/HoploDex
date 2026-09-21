import { useState } from "react";
import type { FormEvent } from "react";
import { Button, DateField, MoneyField, TextArea, TextField } from "../../components";
import { parseDateInput } from "../../lib/dates";
import { dollarsToInput, parseDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import type { InsurancePolicy, InsurancePolicyInput } from "./types";
import "../firearms/forms.css";

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
  notes: string;
  blanketCoverageLimit: string;
  effectiveStartDate: string;
  effectiveEndDate: string;
}

type Field = keyof FormState;

function toFormState(policy?: InsurancePolicy): FormState {
  return {
    name: policy?.name ?? "",
    policyNumber: policy?.policyNumber ?? "",
    insuranceCompany: policy?.insuranceCompany ?? "",
    companyContact: policy?.companyContact ?? "",
    agentName: policy?.agentName ?? "",
    agentContact: policy?.agentContact ?? "",
    notes: policy?.notes ?? "",
    blanketCoverageLimit: policy ? dollarsToInput(policy.blanketCoverageLimit) : "",
    effectiveStartDate: policy?.effectiveStartDate ?? "",
    effectiveEndDate: policy?.effectiveEndDate ?? "",
  };
}

function blankToNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed === "" ? null : trimmed;
}

function validate(form: FormState): Partial<Record<Field, string>> {
  const errors: Partial<Record<Field, string>> = {};
  if (form.name.trim() === "") errors.name = "Give the policy a name you'll recognize.";
  if (form.policyNumber.trim() === "") errors.policyNumber = "Enter the policy number.";
  if (form.insuranceCompany.trim() === "") errors.insuranceCompany = "Enter the insurance company.";
  const limit = parseDollars(form.blanketCoverageLimit);
  if (!limit.ok) errors.blanketCoverageLimit = limit.error;

  const start = parseDateInput(form.effectiveStartDate);
  const end = parseDateInput(form.effectiveEndDate);
  if (!start.ok) errors.effectiveStartDate = start.error;
  else if (!start.iso) errors.effectiveStartDate = "Enter the date coverage starts.";
  if (!end.ok) errors.effectiveEndDate = end.error;
  else if (!end.iso) errors.effectiveEndDate = "Enter the date coverage ends.";
  else if (start.ok && start.iso && end.iso <= start.iso) {
    errors.effectiveEndDate = "The end date must be after the start date.";
  }
  return errors;
}

const FIELD_ORDER: Field[] = [
  "name",
  "policyNumber",
  "insuranceCompany",
  "blanketCoverageLimit",
  "effectiveStartDate",
  "effectiveEndDate",
];

/** Create/edit form for an insurance policy (US3, FR-027). Renders its own
 * dialog body and footer (use inside `<Dialog bare>`). */
export function InsurancePolicyForm({
  initialValues,
  onSubmit,
  onCancel,
}: InsurancePolicyFormProps) {
  const [form, setForm] = useState<FormState>(() => toFormState(initialValues));
  const [touched, setTouched] = useState<Partial<Record<Field, boolean>>>({});
  const [submitted, setSubmitted] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [serverError, setServerError] = useState<CommandFailure | null>(null);

  const clientErrors = validate(form);
  const errorFor = (field: Field): string | undefined =>
    touched[field] || submitted
      ? (clientErrors[field] ?? serverError?.fieldErrors?.[field])
      : undefined;
  // Blurring an empty field doesn't flag it; "required" errors wait for a
  // submit attempt.
  const touch = (field: Field) => () => {
    if (form[field].trim() !== "") setTouched((t) => ({ ...t, [field]: true }));
  };
  const bind = (field: Field) => ({
    value: form[field],
    onChange: (e: { target: { value: string } }) =>
      setForm((prev) => ({ ...prev, [field]: e.target.value })),
    onBlur: touch(field),
    error: errorFor(field),
  });
  const set = (field: Field) => (text: string) => setForm((prev) => ({ ...prev, [field]: text }));

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSubmitted(true);
    const firstInvalid = FIELD_ORDER.find((field) => clientErrors[field]);
    if (firstInvalid) {
      event.currentTarget
        .querySelector<HTMLElement>(`[data-field="${firstInvalid}"] input`)
        ?.focus();
      return;
    }

    const limit = parseDollars(form.blanketCoverageLimit);
    const input: InsurancePolicyInput = {
      name: form.name.trim(),
      policyNumber: form.policyNumber.trim(),
      insuranceCompany: form.insuranceCompany.trim(),
      companyContact: blankToNull(form.companyContact),
      agentName: blankToNull(form.agentName),
      agentContact: blankToNull(form.agentContact),
      notes: blankToNull(form.notes),
      // Blank means no blanket limit: a schedule-only policy (FR-036).
      blanketCoverageLimit: limit.ok ? limit.dollars : null,
      effectiveStartDate: (parseDateInput(form.effectiveStartDate) as { iso: string }).iso,
      effectiveEndDate: (parseDateInput(form.effectiveEndDate) as { iso: string }).iso,
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
    <form className="hd-dialog__form" onSubmit={handleSubmit} noValidate>
      <div className="hd-dialog__body">
        {serverError && !serverError.fieldErrors && (
          <p className="hd-banner hd-banner--error hd-form-banner" role="alert">
            {serverError.message}
          </p>
        )}

        <section className="hd-form-section" aria-labelledby="pf-policy">
          <h3 className="hd-form-section__title" id="pf-policy">
            Policy
          </h3>
          <div className="hd-form-grid hd-form-grid--2">
            <div data-field="name">
              <TextField
                label="Policy name"
                required
                placeholder="e.g. Homeowner's firearms rider"
                autoFocus={!initialValues}
                {...bind("name")}
              />
            </div>
            <div data-field="policyNumber">
              <TextField
                label="Policy number"
                required
                className="hd-serial"
                {...bind("policyNumber")}
              />
            </div>
            <div data-field="insuranceCompany">
              <TextField label="Insurance company" required {...bind("insuranceCompany")} />
            </div>
            <div data-field="blanketCoverageLimit">
              <MoneyField
                label="Blanket coverage limit"
                value={form.blanketCoverageLimit}
                onValueChange={set("blanketCoverageLimit")}
                onBlur={touch("blanketCoverageLimit")}
                error={errorFor("blanketCoverageLimit")}
                hint="Makes this a blanket policy: its limit is shared by every firearm not scheduled individually while it is in force. Leave blank for a policy that only covers firearms scheduled on it."
              />
            </div>
          </div>
        </section>

        <section className="hd-form-section" aria-labelledby="pf-term">
          <h3 className="hd-form-section__title" id="pf-term">
            Term
          </h3>
          <div className="hd-form-grid hd-form-grid--2">
            <div data-field="effectiveStartDate">
              <DateField
                label="Coverage starts"
                required
                value={form.effectiveStartDate}
                onValueChange={set("effectiveStartDate")}
                onBlur={touch("effectiveStartDate")}
                error={errorFor("effectiveStartDate")}
              />
            </div>
            <div data-field="effectiveEndDate">
              <DateField
                label="Coverage ends"
                required
                value={form.effectiveEndDate}
                onValueChange={set("effectiveEndDate")}
                onBlur={touch("effectiveEndDate")}
                error={errorFor("effectiveEndDate")}
                hint="You'll be warned 30 days before this date."
              />
            </div>
          </div>
        </section>

        <section className="hd-form-section" aria-labelledby="pf-contacts">
          <h3 className="hd-form-section__title" id="pf-contacts">
            Contacts
          </h3>
          <div className="hd-form-grid hd-form-grid--2">
            <TextField
              label="Company contact"
              placeholder="Claims phone, email, or address"
              {...bind("companyContact")}
            />
            <TextField label="Agent name" {...bind("agentName")} />
            <TextField
              label="Agent contact"
              placeholder="Phone or email"
              {...bind("agentContact")}
            />
          </div>
        </section>

        <section className="hd-form-section" aria-labelledby="pf-notes">
          <h3 className="hd-form-section__title" id="pf-notes">
            Other details
          </h3>
          <TextArea
            label="Notes"
            hint="Renewal reminders, endorsements, where the paperwork is kept — anything worth recording."
            rows={4}
            value={form.notes}
            onChange={(e) => setForm((prev) => ({ ...prev, notes: e.target.value }))}
          />
        </section>
      </div>

      <footer className="hd-dialog__footer">
        <p className="hd-dialog__footer-note">
          <span aria-hidden className="hd-required-mark">
            *
          </span>{" "}
          Required
        </p>
        {onCancel && (
          <Button variant="secondary" onClick={onCancel} disabled={submitting}>
            Cancel
          </Button>
        )}
        <Button type="submit" variant="primary" pending={submitting}>
          {initialValues ? "Save changes" : "Add policy"}
        </Button>
      </footer>
    </form>
  );
}
