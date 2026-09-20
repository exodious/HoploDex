import { useRef, useState } from "react";
import type { FormEvent } from "react";
import {
  Button,
  Checkbox,
  ChoiceCards,
  DateField,
  MoneyField,
  Select,
  TextArea,
  TextField,
} from "../../components";
import { dispositionOrderError, futureDateError, parseDateInput, todayIso } from "../../lib/dates";
import { centsToInput, parseDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { TypeDrawing } from "../browse/TypeDrawing";
import { DISPOSITION_TYPE_OPTIONS, FIREARM_TYPE_OPTIONS } from "./types";
import type { DispositionType, Firearm, FirearmInput } from "./types";
import "./forms.css";

export interface FirearmFormProps {
  /** Present in edit mode; omitted when creating a new record. */
  initialValues?: Firearm;
  onSubmit: (input: FirearmInput) => Promise<void>;
  onCancel?: () => void;
}

interface FormState {
  make: string;
  model: string;
  nickname: string;
  caliber: string;
  firearmTypeId: string;
  serialNumber: string;
  noSerialAttested: boolean;
  notes: string;
  accessories: string;
  estimatedValue: string;
  acquisitionSource: string;
  acquisitionDate: string;
  acquisitionPrice: string;
  dispositionType: DispositionType | "";
  dispositionRecipient: string;
  dispositionDate: string;
  dispositionPrice: string;
}

type Field = keyof FormState;

function toFormState(firearm?: Firearm): FormState {
  return {
    make: firearm?.make ?? "",
    model: firearm?.model ?? "",
    nickname: firearm?.nickname ?? "",
    caliber: firearm?.caliber ?? "",
    firearmTypeId: firearm ? String(firearm.firearmTypeId) : "",
    serialNumber: firearm?.serialNumber ?? "",
    noSerialAttested: firearm?.noSerialAttested ?? false,
    notes: firearm?.notes ?? "",
    accessories: firearm?.accessories ?? "",
    estimatedValue: centsToInput(firearm?.estimatedValue ?? null),
    acquisitionSource: firearm?.acquisitionSource ?? "",
    acquisitionDate: firearm?.acquisitionDate ?? "",
    acquisitionPrice: centsToInput(firearm?.acquisitionPrice ?? null),
    dispositionType: firearm?.dispositionType ?? "",
    dispositionRecipient: firearm?.dispositionRecipient ?? "",
    dispositionDate: firearm?.dispositionDate ?? "",
    dispositionPrice: centsToInput(firearm?.dispositionPrice ?? null),
  };
}

function blankToNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed === "" ? null : trimmed;
}

/** Client-side mirror of the backend's validation (serial attestation,
 * FR-029; disposition completeness), plus amount and date formats, so the
 * user gets immediate feedback instead of a round-trip error. */
function validate(form: FormState, disposed: boolean): Partial<Record<Field, string>> {
  const errors: Partial<Record<Field, string>> = {};
  if (form.make.trim() === "") errors.make = "Enter the make.";
  if (form.model.trim() === "") errors.model = "Enter the model.";
  if (form.firearmTypeId === "") errors.firearmTypeId = "Choose a type.";
  if (form.caliber.trim() === "") errors.caliber = "Enter the caliber.";
  if (form.serialNumber.trim() === "" && !form.noSerialAttested) {
    errors.serialNumber = "Enter a serial number, or confirm this firearm has none.";
  }
  for (const field of ["estimatedValue", "acquisitionPrice"] as const) {
    const parsed = parseDollars(form[field]);
    if (!parsed.ok) errors[field] = parsed.error;
  }
  const acquired = parseDateInput(form.acquisitionDate);
  if (!acquired.ok) errors.acquisitionDate = acquired.error;
  else {
    const future = futureDateError(acquired.iso, "Acquisition date");
    if (future) errors.acquisitionDate = future;
  }

  if (disposed) {
    if (!form.dispositionType) errors.dispositionType = "Choose what happened to it.";
    if (form.dispositionRecipient.trim() === "")
      errors.dispositionRecipient = "Enter who received it.";
    const date = parseDateInput(form.dispositionDate);
    if (!date.ok) errors.dispositionDate = date.error;
    else if (!date.iso) errors.dispositionDate = "Enter the date.";
    else {
      const problem =
        futureDateError(date.iso, "Disposition date") ??
        dispositionOrderError(acquired.ok ? acquired.iso : null, date.iso);
      if (problem) errors.dispositionDate = problem;
    }
    const price = parseDollars(form.dispositionPrice);
    if (!price.ok) errors.dispositionPrice = price.error;
    else if (price.cents == null) errors.dispositionPrice = "Enter the price, or 0.";
  }
  return errors;
}

function cents(text: string): number | null {
  const parsed = parseDollars(text);
  return parsed.ok ? parsed.cents : null;
}

function isoDate(text: string): string | null {
  const parsed = parseDateInput(text);
  return parsed.ok ? parsed.iso : null;
}

/** Order used to focus the first invalid field after a failed submit. */
const FIELD_ORDER: Field[] = [
  "make",
  "model",
  "nickname",
  "firearmTypeId",
  "caliber",
  "serialNumber",
  "estimatedValue",
  "acquisitionDate",
  "acquisitionPrice",
  "dispositionType",
  "dispositionRecipient",
  "dispositionDate",
  "dispositionPrice",
];

/** Create/edit form for a firearm's record (US1). Insurance coverage is
 * set from the record's own coverage panel; a disposed record's
 * disposition details can be corrected here. Renders its own dialog body
 * and footer (use inside `<Dialog bare>`). */
export function FirearmForm({ initialValues, onSubmit, onCancel }: FirearmFormProps) {
  const disposed = initialValues?.status === "disposed";
  const [form, setForm] = useState<FormState>(() => toFormState(initialValues));
  const [touched, setTouched] = useState<Partial<Record<Field, boolean>>>({});
  const [submitted, setSubmitted] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [serverError, setServerError] = useState<CommandFailure | null>(null);
  const formRef = useRef<HTMLFormElement>(null);

  const clientErrors = validate(form, disposed);
  const errorFor = (field: Field): string | undefined =>
    touched[field] || submitted
      ? (clientErrors[field] ?? serverError?.fieldErrors?.[field])
      : undefined;
  // Blurring an empty field doesn't flag it; "required" errors wait for a
  // submit attempt, so tabbing through the form isn't a wall of red.
  const touch = (field: Field) => () => {
    if (String(form[field]).trim() !== "") setTouched((t) => ({ ...t, [field]: true }));
  };

  function update<K extends Field>(key: K, value: FormState[K]) {
    setForm((prev) => ({ ...prev, [key]: value }));
  }

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setSubmitted(true);
    const firstInvalid = FIELD_ORDER.find((field) => clientErrors[field]);
    if (firstInvalid) {
      formRef.current
        ?.querySelector<HTMLElement>(`[data-field="${firstInvalid}"] :is(input, textarea, button)`)
        ?.focus();
      return;
    }

    const input: FirearmInput = {
      make: form.make.trim(),
      model: form.model.trim(),
      nickname: blankToNull(form.nickname),
      caliber: form.caliber.trim(),
      firearmTypeId: Number(form.firearmTypeId),
      serialNumber: blankToNull(form.serialNumber),
      noSerialAttested: form.noSerialAttested,
      notes: blankToNull(form.notes),
      accessories: blankToNull(form.accessories),
      estimatedValue: cents(form.estimatedValue),
      acquisitionSource: blankToNull(form.acquisitionSource),
      acquisitionDate: isoDate(form.acquisitionDate),
      acquisitionPrice: cents(form.acquisitionPrice),
      status: initialValues?.status ?? "active",
      dispositionType: disposed ? (form.dispositionType as DispositionType) : null,
      dispositionRecipient: disposed ? form.dispositionRecipient.trim() : null,
      dispositionDate: disposed ? isoDate(form.dispositionDate) : null,
      dispositionPrice: disposed ? cents(form.dispositionPrice) : null,
      insurancePolicyId: initialValues?.insurancePolicyId ?? null,
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
    <form ref={formRef} className="hd-dialog__form" onSubmit={handleSubmit} noValidate>
      <div className="hd-dialog__body">
        {serverError && !serverError.fieldErrors && (
          <p className="hd-banner hd-banner--error hd-form-banner" role="alert">
            {serverError.message}
          </p>
        )}

        <section className="hd-form-section" aria-labelledby="ff-identification">
          <h3 className="hd-form-section__title" id="ff-identification">
            Identification
          </h3>
          <div className="hd-form-grid hd-form-grid--2">
            <div data-field="make">
              <TextField
                label="Make"
                required
                value={form.make}
                onChange={(e) => update("make", e.target.value)}
                onBlur={touch("make")}
                error={errorFor("make")}
                placeholder="e.g. Smith & Wesson"
                autoFocus={!initialValues}
              />
            </div>
            <div data-field="model">
              <TextField
                label="Model"
                required
                value={form.model}
                onChange={(e) => update("model", e.target.value)}
                onBlur={touch("model")}
                error={errorFor("model")}
                placeholder="e.g. Model 29"
              />
            </div>
          </div>

          <div data-field="nickname">
            <TextField
              label="Nickname"
              value={form.nickname}
              onChange={(e) => update("nickname", e.target.value)}
              error={errorFor("nickname")}
              hint="Optional. Tells apart firearms with the same make and model; each active firearm needs its own."
              placeholder="e.g. Range gun"
            />
          </div>

          <div data-field="firearmTypeId">
            <ChoiceCards
              label="Type"
              required
              value={form.firearmTypeId}
              onChange={(value) => {
                update("firearmTypeId", value);
                touch("firearmTypeId")();
              }}
              error={errorFor("firearmTypeId")}
              minCardWidth={140}
              options={FIREARM_TYPE_OPTIONS.map((option) => ({
                value: option.value,
                label: option.label,
                art: <TypeDrawing typeKey={option.key} crop />,
              }))}
            />
          </div>

          <div className="hd-form-grid hd-form-grid--2">
            <div data-field="caliber">
              <TextField
                label="Caliber"
                required
                value={form.caliber}
                onChange={(e) => update("caliber", e.target.value)}
                onBlur={touch("caliber")}
                error={errorFor("caliber")}
                placeholder="e.g. .357 Magnum"
              />
            </div>
            <div data-field="serialNumber" className="hd-form-stack">
              <TextField
                label="Serial number"
                required={!form.noSerialAttested}
                className="hd-serial"
                value={form.serialNumber}
                onChange={(e) => update("serialNumber", e.target.value)}
                onBlur={touch("serialNumber")}
                error={errorFor("serialNumber")}
                placeholder={form.noSerialAttested ? "None" : undefined}
                spellCheck={false}
              />
              <Checkbox
                label="This firearm has no serial number"
                hint="Only for firearms not required to have one: made before October 22, 1968, or homemade. If it does have a serial number, you can still record it."
                checked={form.noSerialAttested}
                onCheckedChange={(checked) => {
                  update("noSerialAttested", checked);
                  touch("serialNumber")();
                }}
              />
            </div>
          </div>
        </section>

        <section className="hd-form-section" aria-labelledby="ff-value">
          <h3 className="hd-form-section__title" id="ff-value">
            Value
          </h3>
          <div className="hd-form-grid hd-form-grid--2">
            <div data-field="estimatedValue">
              <MoneyField
                label="Estimated replacement value"
                value={form.estimatedValue}
                onValueChange={(text) => update("estimatedValue", text)}
                onBlur={touch("estimatedValue")}
                error={errorFor("estimatedValue")}
                hint="What it would cost to replace today. Used to check insurance coverage."
              />
            </div>
          </div>
        </section>

        <section className="hd-form-section" aria-labelledby="ff-acquisition">
          <h3 className="hd-form-section__title" id="ff-acquisition">
            Acquisition
          </h3>
          <div className="hd-form-grid hd-form-grid--3">
            <TextField
              label="Acquired from"
              value={form.acquisitionSource}
              onChange={(e) => update("acquisitionSource", e.target.value)}
              placeholder="Seller, shop, or person"
            />
            <div data-field="acquisitionDate">
              <DateField
                label="Date acquired"
                value={form.acquisitionDate}
                max={todayIso()}
                onValueChange={(text) => update("acquisitionDate", text)}
                onBlur={touch("acquisitionDate")}
                error={errorFor("acquisitionDate")}
              />
            </div>
            <div data-field="acquisitionPrice">
              <MoneyField
                label="Price paid"
                value={form.acquisitionPrice}
                onValueChange={(text) => update("acquisitionPrice", text)}
                onBlur={touch("acquisitionPrice")}
                error={errorFor("acquisitionPrice")}
              />
            </div>
          </div>
        </section>

        <section className="hd-form-section" aria-labelledby="ff-condition">
          <h3 className="hd-form-section__title" id="ff-condition">
            Condition and accessories
          </h3>
          <div className="hd-form-grid hd-form-grid--2">
            <TextArea
              label="Notes"
              value={form.notes}
              onChange={(e) => update("notes", e.target.value)}
              hint="Condition, markings, repairs — anything worth recording. All of it is searchable."
              rows={4}
            />
            <TextArea
              label="Accessories"
              value={form.accessories}
              onChange={(e) => update("accessories", e.target.value)}
              hint="e.g. two magazines, original box, holster."
              rows={4}
            />
          </div>
        </section>

        {disposed && (
          <section className="hd-form-section" aria-labelledby="ff-disposition">
            <h3 className="hd-form-section__title" id="ff-disposition">
              Disposition
            </h3>
            <div className="hd-form-grid hd-form-grid--2">
              <div data-field="dispositionType">
                <Select
                  label="What happened"
                  required
                  value={form.dispositionType || undefined}
                  onValueChange={(value) => update("dispositionType", value as DispositionType)}
                  options={DISPOSITION_TYPE_OPTIONS}
                  error={errorFor("dispositionType")}
                />
              </div>
              <div data-field="dispositionRecipient">
                <TextField
                  label="Transferred to"
                  required
                  value={form.dispositionRecipient}
                  onChange={(e) => update("dispositionRecipient", e.target.value)}
                  onBlur={touch("dispositionRecipient")}
                  error={errorFor("dispositionRecipient")}
                />
              </div>
              <div data-field="dispositionDate">
                <DateField
                  label="Date"
                  required
                  value={form.dispositionDate}
                  max={todayIso()}
                  onValueChange={(text) => update("dispositionDate", text)}
                  onBlur={touch("dispositionDate")}
                  error={errorFor("dispositionDate")}
                />
              </div>
              <div data-field="dispositionPrice">
                <MoneyField
                  label="Price received"
                  required
                  value={form.dispositionPrice}
                  onValueChange={(text) => update("dispositionPrice", text)}
                  onBlur={touch("dispositionPrice")}
                  error={errorFor("dispositionPrice")}
                />
              </div>
            </div>
          </section>
        )}
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
          {initialValues ? "Save changes" : "Add firearm"}
        </Button>
      </footer>
    </form>
  );
}
