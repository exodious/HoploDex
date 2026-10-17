import { useRef, useState } from "react";
import type { FormEvent } from "react";
import { Button, DateField, MoneyField, TextArea, TextField } from "../../components";
import { parseDateInput } from "../../lib/dates";
import { dollarsToInput, parseDollars } from "../../lib/money";
import { MAX_LONG_TEXT_CHARS, MAX_SHORT_TEXT_CHARS, textLengthError } from "../../lib/textLimits";
import { CommandFailure } from "../../services/tauriClient";
import { useSessionScope } from "../session/sessionScope";
import { resumedValues, useDirtyForm, useResumedDraftTaken } from "../session/usePendingDraft";
import type { DraftTarget } from "../session/usePendingDraft";
import type { InsurancePolicy, InsurancePolicyInput } from "./types";
import "../firearms/forms.css";

export interface InsurancePolicyFormProps {
  initialValues?: InsurancePolicy;
  onSubmit: (input: InsurancePolicyInput) => Promise<void>;
  onCancel?: () => void;
}

/** The version of this form's kept drafts (003 research.md §16). Raise it when
 * `FormState` changes shape, so older drafts are only discarded. */
export const FORM_VERSION = 1;

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

/** 008 research.md §10: the free-text fields with a
 * maximum, by their label (the backend's, so the two messages match). */
const TEXT_LIMITS: readonly (readonly [Field, string, number])[] = [
  ["name", "Policy name", MAX_SHORT_TEXT_CHARS],
  ["policyNumber", "Policy number", MAX_SHORT_TEXT_CHARS],
  ["insuranceCompany", "Insurance company", MAX_SHORT_TEXT_CHARS],
  ["companyContact", "Company contact", MAX_SHORT_TEXT_CHARS],
  ["agentName", "Agent name", MAX_SHORT_TEXT_CHARS],
  ["agentContact", "Agent contact", MAX_SHORT_TEXT_CHARS],
  ["notes", "Notes", MAX_LONG_TEXT_CHARS],
];

/** The fields over their maximum. They show their message at once, not on
 * leaving the field, since text is usually pasted in. */
function lengthErrors(form: FormState): Partial<Record<Field, string>> {
  const errors: Partial<Record<Field, string>> = {};
  for (const [field, label, max] of TEXT_LIMITS) {
    const problem = textLengthError(label, form[field], max);
    if (problem) errors[field] = problem;
  }
  return errors;
}

function validate(form: FormState): Partial<Record<Field, string>> {
  const errors: Partial<Record<Field, string>> = {};
  if (form.name.trim() === "") errors.name = "Give the policy a name you'll recognize.";
  if (form.policyNumber.trim() === "") errors.policyNumber = "Enter the policy number.";
  if (form.insuranceCompany.trim() === "") errors.insuranceCompany = "Enter the insurance company.";
  Object.assign(errors, lengthErrors(form));
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
  "companyContact",
  "agentName",
  "agentContact",
  "notes",
];

/** Create/edit form for an insurance policy (001 US3, FR-027). Renders its own
 * dialog body and footer (use inside `<Dialog bare>`). */
export function InsurancePolicyForm({
  initialValues,
  onSubmit,
  onCancel,
}: InsurancePolicyFormProps) {
  const scope = useSessionScope();
  const target: DraftTarget = {
    formVersion: FORM_VERSION,
    kind: "policy",
    mode: initialValues ? "edit" : "add",
    targetId: initialValues?.id ?? null,
  };
  // Pending changes the user resumed start as unsaved input (003 FR-039).
  const [form, setForm] = useState<FormState>(() =>
    resumedValues(scope, target, toFormState(initialValues)),
  );
  useResumedDraftTaken(target);
  const [pristine] = useState(() => toFormState(initialValues));
  const formRef = useRef<HTMLFormElement>(null);
  const [touched, setTouched] = useState<Partial<Record<Field, boolean>>>({});
  const [submitted, setSubmitted] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [serverError, setServerError] = useState<CommandFailure | null>(null);

  const clientErrors = validate(form);
  const overLength = lengthErrors(form);
  const errorFor = (field: Field): string | undefined =>
    overLength[field] ??
    (touched[field] || submitted
      ? (clientErrors[field] ?? serverError?.fieldErrors?.[field])
      : undefined);
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

  // Closing or quitting asks about unsaved input first (003 FR-010),
  // and a lock keeps it (003 FR-039).
  useDirtyForm({
    label: initialValues ? `${initialValues.name} (edit)` : "New insurance policy",
    isDirty: JSON.stringify(form) !== JSON.stringify(pristine),
    submit: save,
    draft: { ...target, values: form },
  });

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    void save();
  }

  function focusField(field: Field) {
    formRef.current
      ?.querySelector<HTMLElement>(`[data-field="${field}"] :is(input, textarea)`)
      ?.focus();
  }

  /** Validates and saves; resolves whether it was saved. */
  async function save(): Promise<boolean> {
    setSubmitted(true);
    const firstInvalid = FIELD_ORDER.find((field) => clientErrors[field]);
    if (firstInvalid) {
      focusField(firstInvalid);
      return false;
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
      // Blank means no blanket limit: a schedule-only policy (001 FR-036).
      blanketCoverageLimit: limit.ok ? limit.dollars : null,
      effectiveStartDate: (parseDateInput(form.effectiveStartDate) as { iso: string }).iso,
      effectiveEndDate: (parseDateInput(form.effectiveEndDate) as { iso: string }).iso,
    };

    setSubmitting(true);
    setServerError(null);
    try {
      await onSubmit(input);
      return true;
    } catch (error) {
      if (error instanceof CommandFailure) {
        setServerError(error);
        // A field the backend refused (009 FR-008: a duplicate name) takes the
        // focus, as a field the form's own check refused does.
        const refused = FIELD_ORDER.find((field) => error.fieldErrors?.[field]);
        if (refused) focusField(refused);
        return false;
      }
      throw error;
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form ref={formRef} className="hd-dialog__form" onSubmit={handleSubmit} noValidate>
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
                hint="Each policy needs its own name."
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
          </div>
          <div data-field="blanketCoverageLimit">
            <MoneyField
              label="Blanket coverage limit"
              value={form.blanketCoverageLimit}
              onValueChange={set("blanketCoverageLimit")}
              onBlur={touch("blanketCoverageLimit")}
              error={errorFor("blanketCoverageLimit")}
              fieldClassName="hd-field--quarter"
              hint="Makes this a blanket policy: its limit is shared by every firearm and accessory not scheduled individually while it is in force. Leave blank for a policy that only covers the firearms and accessories scheduled on it."
            />
          </div>
        </section>

        <section className="hd-form-section" aria-labelledby="pf-term">
          <h3 className="hd-form-section__title" id="pf-term">
            Term
          </h3>
          <div className="hd-form-grid hd-form-grid--4">
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
            <div data-field="companyContact">
              <TextField
                label="Company contact"
                placeholder="Claims phone, email, or address"
                {...bind("companyContact")}
              />
            </div>
          </div>
          <div className="hd-form-grid hd-form-grid--2">
            <div data-field="agentName">
              <TextField label="Agent name" {...bind("agentName")} />
            </div>
            <div data-field="agentContact">
              <TextField
                label="Agent contact"
                placeholder="Phone or email"
                {...bind("agentContact")}
              />
            </div>
          </div>
        </section>

        <section className="hd-form-section" aria-labelledby="pf-notes">
          <h3 className="hd-form-section__title" id="pf-notes">
            Other details
          </h3>
          <div data-field="notes">
            <TextArea
              label="Notes"
              hint="Renewal reminders, endorsements, where the paperwork is kept — anything worth recording."
              rows={4}
              value={form.notes}
              onChange={(e) => setForm((prev) => ({ ...prev, notes: e.target.value }))}
              error={errorFor("notes")}
              limit={MAX_LONG_TEXT_CHARS}
            />
          </div>
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
