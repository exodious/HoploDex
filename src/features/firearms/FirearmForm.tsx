import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import { flushSync } from "react-dom";
import {
  Button,
  Checkbox,
  ChoiceCards,
  ConfirmDialog,
  DateField,
  DecimalField,
  Disclosure,
  MoneyField,
  Select,
  TextArea,
  TextField,
} from "../../components";
import { dispositionOrderError, futureDateError, parseDateInput, todayIso } from "../../lib/dates";
import { inchesToInput, parseInches, parseWeight, weightToInputs } from "../../lib/measure";
import { dollarsToInput, parseDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { firearmName } from "../app/collectionStore";
import { TypeDrawing } from "../browse/TypeDrawing";
import { resumedValues, useDirtyForm, useResumedDraftTaken } from "../session/usePendingDraft";
import type { DraftTarget } from "../session/usePendingDraft";
import { OriginGuide } from "./OriginGuide";
import {
  CONDITION_OPTIONS,
  DISPOSITION_TYPE_OPTIONS,
  FIREARM_TYPE_OPTIONS,
  ORIGIN_OPTIONS,
  conditionLabel,
  originLabel,
} from "./types";
import type { Condition, DispositionType, Firearm, FirearmInput, Origin } from "./types";
import "./forms.css";

/** specs/002-firearm-identification research.md §7: not four digits, or
 * outside 1400 to the current local year, is the same error. */
function yearOfManufactureError(text: string): string | undefined {
  if (text === "") return undefined;
  const year = Number(text);
  const currentYear = new Date().getFullYear();
  if (text.length !== 4 || !Number.isInteger(year) || year < 1400 || year > currentYear) {
    return `Year of manufacture must be a four-digit year from 1400 to ${currentYear}.`;
  }
  return undefined;
}

/** specs/002-firearm-identification FR-010/research.md §8: what changing
 * origin away from `from` to `to` would discard, given the form's current
 * importer/country/original-marks values — `null` when nothing is lost. */
type OriginDiscard = "country" | "importerAndMarks";

function importMarked(origin: Origin | ""): boolean {
  return origin === "imported" || origin === "reimported";
}

type DiscardableFields = Pick<
  FormState,
  | "countryOfManufacture"
  | "importerName"
  | "originalMake"
  | "originalModel"
  | "originalSerialNumber"
>;

function originDiscard(
  from: Origin | "",
  to: Origin | "",
  form: DiscardableFields,
): OriginDiscard | null {
  if (from === "imported" && to === "reimported") {
    return form.countryOfManufacture.trim() !== "" ? "country" : null;
  }
  if (importMarked(from) && !importMarked(to)) {
    const hasImporter = form.importerName.trim() !== "";
    const hasCountry = form.countryOfManufacture.trim() !== "";
    // specs/002-firearm-identification US2: the "Original maker's marks"
    // fieldset only exists for an import-marked origin, so it is discarded
    // alongside importer and country (T024).
    const hasOriginalMarks =
      form.originalMake.trim() !== "" ||
      form.originalModel.trim() !== "" ||
      form.originalSerialNumber.trim() !== "";
    return hasImporter || hasCountry || hasOriginalMarks ? "importerAndMarks" : null;
  }
  return null;
}

/** specs/002-firearm-identification: the fields folded into the "Origin and
 * year of manufacture" group. Optional and unused by most records, so the
 * group starts closed unless one of them is recorded, and opens itself when
 * an error lands on one of them. */
const ORIGIN_GROUP_FIELDS = [
  "origin",
  "yearOfManufacture",
  "countryOfManufacture",
  "importerName",
  "originalMake",
  "originalModel",
  "originalSerialNumber",
] as const satisfies readonly (keyof FormState)[];

function hasOriginGroupValue(form: FormState): boolean {
  return ORIGIN_GROUP_FIELDS.some((field) => form[field].trim() !== "");
}

/** What the closed group says it holds: the recorded values read back as a
 * sentence or two, so closing it never hides one. */
function originGroupSummary(form: FormState): string {
  const sentences: string[] = [];
  const country = form.countryOfManufacture.trim();
  const importer = form.importerName.trim();
  if (form.origin !== "") {
    let origin = originLabel(form.origin);
    if (form.origin === "imported" && country) origin += ` from ${country}`;
    if (importMarked(form.origin) && importer) origin += ` by ${importer}`;
    sentences.push(origin);
  }
  if (form.yearOfManufacture !== "") sentences.push(`Made in ${form.yearOfManufacture}`);
  if (
    importMarked(form.origin) &&
    [form.originalMake, form.originalModel, form.originalSerialNumber].some((v) => v.trim())
  ) {
    sentences.push("Original maker's marks recorded");
  }
  if (sentences.length === 0) return "Optional: where and when it was made, and who imported it.";
  // An importer's name often ends in "Inc." or "Co.": don't double the stop.
  return sentences
    .map((sentence) => (sentence.endsWith(".") ? sentence : `${sentence}.`))
    .join(" ");
}

/** FR-039: the physical details, folded into their own group on the same
 * rules as the origin group (optional, closed unless recorded, opened by an
 * error inside it). */
const PHYSICAL_GROUP_FIELDS = [
  "barrelLength",
  "overallLength",
  "weightPounds",
  "weightOunces",
  "capacity",
  "finish",
  "condition",
] as const satisfies readonly (keyof FormState)[];

function hasPhysicalGroupValue(form: FormState): boolean {
  return PHYSICAL_GROUP_FIELDS.some((field) => form[field].trim() !== "");
}

/** The closed physical details group's read-back, in the record page's
 * units: "16.25 in barrel, 36 in overall. 2 lb 8.5 oz. 30 rounds." */
function physicalGroupSummary(form: FormState): string {
  const sentences: string[] = [];
  const lengths = [
    form.barrelLength && `${form.barrelLength} in barrel`,
    form.overallLength && `${form.overallLength} in overall`,
  ].filter(Boolean);
  if (lengths.length > 0) sentences.push(lengths.join(", "));
  const weight = [
    form.weightPounds && `${form.weightPounds} lb`,
    form.weightOunces && `${form.weightOunces} oz`,
  ].filter(Boolean);
  if (weight.length > 0) sentences.push(weight.join(" "));
  if (form.capacity !== "") {
    sentences.push(`${form.capacity} ${form.capacity === "1" ? "round" : "rounds"}`);
  }
  if (form.finish.trim() !== "") sentences.push(`Finish: ${form.finish.trim()}`);
  if (form.condition !== "") sentences.push(`Condition: ${conditionLabel(form.condition)}`);
  if (sentences.length === 0) {
    return "Optional: lengths, weight, capacity, finish and condition.";
  }
  return sentences
    .map((sentence) => (sentence.endsWith(".") ? sentence : `${sentence}.`))
    .join(" ");
}

/** A field the form can open on, for the record page's "Add" links. */
export type FocusField = "notes" | "accessories";

/** How long the section stays highlighted after the form opens on a field. */
const HIGHLIGHT_MS = 1800;

export interface FirearmFormProps {
  /** Present in edit mode; omitted when creating a new record. */
  initialValues?: Firearm;
  /** Opens with this field scrolled into view, focused, and its section
   * briefly highlighted (FR-038). */
  focusField?: FocusField;
  /** `confirmedWarnings` resends after an `ORIGINAL_MARKS_MATCH` (FR-009). */
  onSubmit: (input: FirearmInput, confirmedWarnings?: boolean) => Promise<void>;
  onCancel?: () => void;
}

/** The version of this form's kept drafts (research.md §16). Raise it when
 * `FormState` changes shape, so older drafts are only discarded. */
export const FORM_VERSION = 1;

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
  barrelLength: string;
  overallLength: string;
  weightPounds: string;
  weightOunces: string;
  capacity: string;
  finish: string;
  condition: Condition | "";
  estimatedValue: string;
  acquisitionSource: string;
  acquisitionDate: string;
  acquisitionPrice: string;
  dispositionType: DispositionType | "";
  dispositionRecipient: string;
  dispositionDate: string;
  dispositionPrice: string;
  origin: Origin | "";
  yearOfManufacture: string;
  countryOfManufacture: string;
  importerName: string;
  originalMake: string;
  originalModel: string;
  originalSerialNumber: string;
}

type Field = keyof FormState;

/** The name the backend gives a field when it rejects it. */
const SERVER_FIELD: Partial<Record<Field, string>> = {
  barrelLength: "barrelLengthHundredths",
  overallLength: "overallLengthHundredths",
  weightPounds: "weightTenthsOz",
};

/** Sent for the condition's "Not recorded" option: a Select item can't be "". */
const NOT_RECORDED = "none";

function weightFields(tenthsOz: number | null) {
  const { pounds, ounces } = weightToInputs(tenthsOz);
  return { weightPounds: pounds, weightOunces: ounces };
}

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
    barrelLength: inchesToInput(firearm?.barrelLengthHundredths ?? null),
    overallLength: inchesToInput(firearm?.overallLengthHundredths ?? null),
    ...weightFields(firearm?.weightTenthsOz ?? null),
    capacity: firearm?.capacity == null ? "" : String(firearm.capacity),
    finish: firearm?.finish ?? "",
    condition: firearm?.condition ?? "",
    estimatedValue: dollarsToInput(firearm?.estimatedValue ?? null),
    acquisitionSource: firearm?.acquisitionSource ?? "",
    acquisitionDate: firearm?.acquisitionDate ?? "",
    acquisitionPrice: dollarsToInput(firearm?.acquisitionPrice ?? null),
    dispositionType: firearm?.dispositionType ?? "",
    dispositionRecipient: firearm?.dispositionRecipient ?? "",
    dispositionDate: firearm?.dispositionDate ?? "",
    dispositionPrice: dollarsToInput(firearm?.dispositionPrice ?? null),
    origin: firearm?.origin ?? "",
    yearOfManufacture: firearm?.yearOfManufacture == null ? "" : String(firearm.yearOfManufacture),
    countryOfManufacture: firearm?.countryOfManufacture ?? "",
    importerName: firearm?.importerName ?? "",
    originalMake: firearm?.originalMake ?? "",
    originalModel: firearm?.originalModel ?? "",
    originalSerialNumber: firearm?.originalSerialNumber ?? "",
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
  for (const field of ["barrelLength", "overallLength"] as const) {
    const parsed = parseInches(form[field]);
    if (!parsed.ok) errors[field] = parsed.error;
  }
  const weight = parseWeight(form.weightPounds, form.weightOunces);
  if (!weight.ok) {
    if (weight.errors.pounds) errors.weightPounds = weight.errors.pounds;
    if (weight.errors.ounces) errors.weightOunces = weight.errors.ounces;
  }
  if (form.capacity !== "" && !(Number(form.capacity) >= 1)) {
    errors.capacity = "Capacity must be at least 1.";
  }
  const acquired = parseDateInput(form.acquisitionDate);
  if (!acquired.ok) errors.acquisitionDate = acquired.error;
  else {
    const future = futureDateError(acquired.iso, "Acquisition date");
    if (future) errors.acquisitionDate = future;
  }

  const yearError = yearOfManufactureError(form.yearOfManufacture);
  if (yearError) errors.yearOfManufacture = yearError;

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
    else if (price.dollars == null) errors.dispositionPrice = "Enter the price, or 0.";
  }
  return errors;
}

function dollars(text: string): number | null {
  const parsed = parseDollars(text);
  return parsed.ok ? parsed.dollars : null;
}

/** The scaled integer for a physical-detail field already validated. */
function measure(parse: typeof parseInches, text: string): number | null {
  const parsed = parse(text);
  return parsed.ok ? parsed.value : null;
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
  "yearOfManufacture",
  "barrelLength",
  "overallLength",
  "weightPounds",
  "weightOunces",
  "capacity",
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
export function FirearmForm({ initialValues, focusField, onSubmit, onCancel }: FirearmFormProps) {
  const disposed = initialValues?.status === "disposed";
  const target: DraftTarget = {
    formVersion: FORM_VERSION,
    kind: "firearm",
    mode: initialValues ? "edit" : "add",
    targetId: initialValues?.id ?? null,
  };
  // Pending changes the user resumed start as unsaved input (FR-039).
  const [form, setForm] = useState<FormState>(() =>
    resumedValues(target, toFormState(initialValues)),
  );
  useResumedDraftTaken(target);
  const [pristine] = useState(() => toFormState(initialValues));
  const [touched, setTouched] = useState<Partial<Record<Field, boolean>>>({});
  const [submitted, setSubmitted] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [serverError, setServerError] = useState<CommandFailure | null>(null);
  const formRef = useRef<HTMLFormElement>(null);
  const notesRef = useRef<HTMLTextAreaElement>(null);
  const accessoriesRef = useRef<HTMLTextAreaElement>(null);
  // Motion-free under prefers-reduced-motion: still marked, so the user can
  // see where to type, but neither animated nor smooth-scrolled.
  const [highlight, setHighlight] = useState<"animated" | "static" | null>(null);
  const [showOriginGuide, setShowOriginGuide] = useState(false);
  const [originGroupOpen, setOriginGroupOpen] = useState(() => hasOriginGroupValue(form));
  const [physicalGroupOpen, setPhysicalGroupOpen] = useState(() => hasPhysicalGroupValue(form));
  const originGuideButtonRef = useRef<HTMLButtonElement>(null);
  // specs/002-firearm-identification FR-010: set while the discard
  // confirmation is open, holding the origin the user picked and what it
  // would discard.
  const [pendingOrigin, setPendingOrigin] = useState<{
    next: Origin | "";
    discard: OriginDiscard;
  } | null>(null);
  // specs/002-firearm-identification FR-009: set when a save returns
  // ORIGINAL_MARKS_MATCH, holding the input to resend if confirmed.
  const [pendingWarning, setPendingWarning] = useState<{
    input: FirearmInput;
    message: string;
  } | null>(null);

  useEffect(() => {
    const field = focusField === "notes" ? notesRef.current : accessoriesRef.current;
    if (!focusField || !field) return;
    const reduceMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    field.focus({ preventScroll: true });
    field.scrollIntoView?.({ block: "center", behavior: reduceMotion ? "auto" : "smooth" });
    setHighlight(reduceMotion ? "static" : "animated");
    const timer = setTimeout(() => setHighlight(null), HIGHLIGHT_MS);
    return () => clearTimeout(timer);
  }, [focusField]);

  const clientErrors = validate(form, disposed);
  const errorFor = (field: Field): string | undefined =>
    touched[field] || submitted
      ? (clientErrors[field] ?? serverError?.fieldErrors?.[SERVER_FIELD[field] ?? field])
      : undefined;
  // Blurring an empty field doesn't flag it; "required" errors wait for a
  // submit attempt, so tabbing through the form isn't a wall of red.
  const touch = (field: Field) => () => {
    if (String(form[field]).trim() !== "") setTouched((t) => ({ ...t, [field]: true }));
  };

  function update<K extends Field>(key: K, value: FormState[K]) {
    setForm((prev) => ({ ...prev, [key]: value }));
  }

  function handleOriginChange(next: Origin | "") {
    const discard = originDiscard(form.origin, next, form);
    if (discard) {
      setPendingOrigin({ next, discard });
    } else {
      update("origin", next);
    }
  }

  /** specs/002-firearm-identification contracts/ui-identification.md §3:
   * lists what will be discarded, by name and recorded value. */
  function discardedValuesDescription(): string {
    const items: string[] = [];
    if (pendingOrigin?.discard === "importerAndMarks" && form.importerName.trim() !== "") {
      items.push(`the importer (${form.importerName.trim()})`);
    }
    if (
      (pendingOrigin?.discard === "country" || pendingOrigin?.discard === "importerAndMarks") &&
      form.countryOfManufacture.trim() !== ""
    ) {
      items.push(`the country of manufacture (${form.countryOfManufacture.trim()})`);
    }
    if (pendingOrigin?.discard === "importerAndMarks") {
      const marks = [form.originalMake, form.originalModel, form.originalSerialNumber]
        .map((v) => v.trim())
        .filter((v) => v !== "");
      if (marks.length > 0) {
        items.push(`the original maker's marks (${marks.join(", ")})`);
      }
    }
    return `Changing the origin will discard ${items.join(" and ")}. It can't be recovered once saved.`;
  }

  function confirmOriginChange() {
    if (!pendingOrigin) return;
    const { next, discard } = pendingOrigin;
    setForm((prev) => ({
      ...prev,
      origin: next,
      countryOfManufacture: "",
      importerName: discard === "importerAndMarks" ? "" : prev.importerName,
      originalMake: discard === "importerAndMarks" ? "" : prev.originalMake,
      originalModel: discard === "importerAndMarks" ? "" : prev.originalModel,
      originalSerialNumber: discard === "importerAndMarks" ? "" : prev.originalSerialNumber,
    }));
    setPendingOrigin(null);
  }

  // Closing or quitting asks about unsaved input first (specs/003 FR-010),
  // and a lock keeps it (FR-039).
  useDirtyForm({
    label: initialValues ? `${firearmName(initialValues)} (edit)` : "New firearm",
    isDirty: JSON.stringify(form) !== JSON.stringify(pristine),
    submit: save,
    draft: { ...target, values: form },
  });

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    void save();
  }

  /** Validates and saves; resolves whether it was saved. */
  async function save(): Promise<boolean> {
    setSubmitted(true);
    const firstInvalid = FIELD_ORDER.find((field) => clientErrors[field]);
    if (firstInvalid) {
      if ((ORIGIN_GROUP_FIELDS as readonly Field[]).includes(firstInvalid)) {
        flushSync(() => setOriginGroupOpen(true));
      }
      if ((PHYSICAL_GROUP_FIELDS as readonly Field[]).includes(firstInvalid)) {
        flushSync(() => setPhysicalGroupOpen(true));
      }
      formRef.current
        ?.querySelector<HTMLElement>(`[data-field="${firstInvalid}"] :is(input, textarea, button)`)
        ?.focus();
      return false;
    }

    const weight = parseWeight(form.weightPounds, form.weightOunces);
    const weightTenths = weight.ok ? weight.value : null;

    const input: FirearmInput = {
      make: form.make.trim(),
      model: form.model.trim(),
      nickname: blankToNull(form.nickname),
      caliber: form.caliber.trim(),
      // specs/004-cartridges-action-types: the form has no cartridge or
      // action field yet (T030, T059), so an edit keeps the saved ones.
      cartridge: initialValues?.cartridge ?? null,
      firearmTypeId: Number(form.firearmTypeId),
      actionTypeId: initialValues?.actionTypeId ?? null,
      serialNumber: form.noSerialAttested ? null : form.serialNumber.trim(),
      noSerialAttested: form.noSerialAttested,
      notes: blankToNull(form.notes),
      accessories: blankToNull(form.accessories),
      barrelLengthHundredths: measure(parseInches, form.barrelLength),
      overallLengthHundredths: measure(parseInches, form.overallLength),
      weightTenthsOz: weightTenths,
      capacity: form.capacity === "" ? null : Number(form.capacity),
      finish: blankToNull(form.finish),
      condition: form.condition === "" ? null : form.condition,
      estimatedValue: dollars(form.estimatedValue),
      acquisitionSource: blankToNull(form.acquisitionSource),
      acquisitionDate: isoDate(form.acquisitionDate),
      acquisitionPrice: dollars(form.acquisitionPrice),
      status: initialValues?.status ?? "active",
      dispositionType: disposed ? (form.dispositionType as DispositionType) : null,
      dispositionRecipient: disposed ? form.dispositionRecipient.trim() : null,
      dispositionDate: disposed ? isoDate(form.dispositionDate) : null,
      dispositionPrice: disposed ? dollars(form.dispositionPrice) : null,
      insurancePolicyId: initialValues?.insurancePolicyId ?? null,
      scheduledCoverageAmount: initialValues?.scheduledCoverageAmount ?? null,
      origin: form.origin === "" ? null : form.origin,
      yearOfManufacture: form.yearOfManufacture === "" ? null : Number(form.yearOfManufacture),
      countryOfManufacture: blankToNull(form.countryOfManufacture),
      importerName: blankToNull(form.importerName),
      originalMake: blankToNull(form.originalMake),
      originalModel: blankToNull(form.originalModel),
      originalSerialNumber: blankToNull(form.originalSerialNumber),
    };

    return submitInput(input);
  }

  /** A save rejected on a physical detail opens that group, so the error is
   * never hidden. The backend names the scaled fields (`SERVER_FIELD`). */
  function revealPhysicalGroupFor(error: CommandFailure) {
    const fieldErrors = error.fieldErrors;
    if (!fieldErrors) return;
    if (PHYSICAL_GROUP_FIELDS.some((field) => fieldErrors[SERVER_FIELD[field] ?? field])) {
      flushSync(() => setPhysicalGroupOpen(true));
    }
  }

  /** A save rejected on a field inside the origin group opens it, so the
   * error is never hidden. An identity clash on the serial number points at
   * Year of manufacture (contracts/ui-identification.md §4): when no year is
   * recorded, open the group and bring the year into view. */
  function revealOriginGroupFor(error: CommandFailure) {
    const fieldErrors = error.fieldErrors;
    if (!fieldErrors) return;
    const clashNeedsYear = fieldErrors.serialNumber !== undefined && form.yearOfManufacture === "";
    if (!clashNeedsYear && !ORIGIN_GROUP_FIELDS.some((field) => fieldErrors[field])) return;
    flushSync(() => setOriginGroupOpen(true));
    if (clashNeedsYear) {
      const reduceMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
      formRef.current
        ?.querySelector('[data-field="yearOfManufacture"]')
        ?.scrollIntoView?.({ block: "nearest", behavior: reduceMotion ? "auto" : "smooth" });
    }
  }

  /** specs/002-firearm-identification FR-009, contracts/ui-identification.md
   * §4: an `ORIGINAL_MARKS_MATCH` opens a confirm-to-save dialog instead of
   * the usual error banner; every other failure behaves as before. */
  async function submitInput(input: FirearmInput, confirmedWarnings?: boolean): Promise<boolean> {
    setSubmitting(true);
    setServerError(null);
    try {
      await onSubmit(input, confirmedWarnings);
      return true;
    } catch (error) {
      if (error instanceof CommandFailure) {
        if (error.code === "ORIGINAL_MARKS_MATCH" && !confirmedWarnings) {
          setPendingWarning({ input, message: error.message });
        } else {
          flushSync(() => setServerError(error));
          revealOriginGroupFor(error);
          revealPhysicalGroupFor(error);
        }
        return false;
      }
      throw error;
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <>
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
                fieldClassName="hd-field--half"
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
                  value={form.noSerialAttested ? "" : form.serialNumber}
                  onChange={(e) => update("serialNumber", e.target.value)}
                  onBlur={touch("serialNumber")}
                  error={errorFor("serialNumber")}
                  disabled={form.noSerialAttested}
                  placeholder={form.noSerialAttested ? "None" : undefined}
                  spellCheck={false}
                />
                <Checkbox
                  label="This firearm has no serial number"
                  hint="Only for firearms not required to have one: made before October 22, 1968, or homemade."
                  checked={form.noSerialAttested}
                  onCheckedChange={(checked) => {
                    update("noSerialAttested", checked);
                    touch("serialNumber")();
                  }}
                />
              </div>
            </div>

            <Disclosure
              title="Origin and year of manufacture"
              summary={originGroupOpen ? undefined : originGroupSummary(form)}
              open={originGroupOpen}
              onOpenChange={setOriginGroupOpen}
            >
              <div data-field="yearOfManufacture">
                <TextField
                  label="Year of manufacture"
                  inputMode="numeric"
                  autoComplete="off"
                  value={form.yearOfManufacture}
                  onChange={(e) =>
                    update("yearOfManufacture", e.target.value.replace(/\D/g, "").slice(0, 4))
                  }
                  onBlur={touch("yearOfManufacture")}
                  error={errorFor("yearOfManufacture")}
                  fieldClassName="hd-field--quarter"
                  hint="A single year, e.g. 1943. Put anything uncertain in Notes."
                  placeholder="e.g. 1943"
                />
              </div>
              <div className="hd-form-grid hd-form-grid--origin">
                <div data-field="origin" className="hd-form-stack">
                  <ChoiceCards
                    label="Origin"
                    value={form.origin}
                    onChange={handleOriginChange}
                    minCardWidth={150}
                    options={ORIGIN_OPTIONS}
                  />
                  <Button
                    ref={originGuideButtonRef}
                    className="hd-origin-guide__trigger"
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => setShowOriginGuide(true)}
                  >
                    How do I record this?
                  </Button>
                  {form.origin === "domestic" && (
                    <p className="hd-field__hint">
                      Made in the U.S. but stamped with an importer's name? Choose Re-imported.
                    </p>
                  )}
                </div>

                {form.origin === "imported" && (
                  <>
                    <div data-field="countryOfManufacture">
                      <TextField
                        label="Country of manufacture"
                        value={form.countryOfManufacture}
                        onChange={(e) => update("countryOfManufacture", e.target.value)}
                        onBlur={touch("countryOfManufacture")}
                        error={errorFor("countryOfManufacture")}
                      />
                    </div>
                    <div data-field="importerName">
                      <TextField
                        label="Importer"
                        value={form.importerName}
                        onChange={(e) => update("importerName", e.target.value)}
                        onBlur={touch("importerName")}
                        error={errorFor("importerName")}
                      />
                    </div>
                  </>
                )}
                {form.origin === "reimported" && (
                  <>
                    <p className="hd-field hd-static-line" data-field="countryOfManufactureDisplay">
                      Country of manufacture: United States
                    </p>
                    <div data-field="importerName">
                      <TextField
                        label="Importer"
                        value={form.importerName}
                        onChange={(e) => update("importerName", e.target.value)}
                        onBlur={touch("importerName")}
                        error={errorFor("importerName")}
                      />
                    </div>
                  </>
                )}
              </div>

              {importMarked(form.origin) && (
                <div className="hd-form-subgroup">
                  <fieldset className="hd-form-section hd-form-fieldset">
                    <legend className="hd-form-subgroup__title">Original maker's marks</legend>
                    <p className="hd-field__hint">
                      Only if the original maker's marks differ from the make, model and serial
                      number above, or you want both.
                    </p>
                    <div className="hd-form-grid hd-form-grid--3">
                      <div data-field="originalMake">
                        <TextField
                          label="Original maker"
                          value={form.originalMake}
                          onChange={(e) => update("originalMake", e.target.value)}
                          onBlur={touch("originalMake")}
                          error={errorFor("originalMake")}
                        />
                      </div>
                      <div data-field="originalModel">
                        <TextField
                          label="Original model"
                          value={form.originalModel}
                          onChange={(e) => update("originalModel", e.target.value)}
                          onBlur={touch("originalModel")}
                          error={errorFor("originalModel")}
                        />
                      </div>
                      <div data-field="originalSerialNumber">
                        <TextField
                          label="Original serial number"
                          value={form.originalSerialNumber}
                          onChange={(e) => update("originalSerialNumber", e.target.value)}
                          onBlur={touch("originalSerialNumber")}
                          error={errorFor("originalSerialNumber")}
                        />
                      </div>
                    </div>
                  </fieldset>
                </div>
              )}
            </Disclosure>
          </section>

          <section className="hd-form-section hd-form-section--folded">
            <Disclosure
              title="Physical details"
              headingLevel={3}
              summary={physicalGroupOpen ? undefined : physicalGroupSummary(form)}
              open={physicalGroupOpen}
              onOpenChange={setPhysicalGroupOpen}
            >
              <div className="hd-form-grid hd-form-grid--2">
                <div className="hd-form-pair">
                  <div data-field="barrelLength">
                    <DecimalField
                      label="Barrel length (in)"
                      value={form.barrelLength}
                      onValueChange={(text) => update("barrelLength", text)}
                      onBlur={touch("barrelLength")}
                      error={errorFor("barrelLength")}
                      placeholder="e.g. 4.25"
                    />
                  </div>
                  <div data-field="overallLength">
                    <DecimalField
                      label="Overall length (in)"
                      value={form.overallLength}
                      onValueChange={(text) => update("overallLength", text)}
                      onBlur={touch("overallLength")}
                      error={errorFor("overallLength")}
                      placeholder="e.g. 7.4"
                    />
                  </div>
                  <p className="hd-form-pair__hint">Saved to the nearest 0.01 in.</p>
                </div>
                <div className="hd-form-pair">
                  <div data-field="weightPounds">
                    <DecimalField
                      label="Weight (lb)"
                      value={form.weightPounds}
                      onValueChange={(text) => update("weightPounds", text)}
                      onBlur={touch("weightPounds")}
                      error={errorFor("weightPounds")}
                      placeholder="e.g. 6.5"
                    />
                  </div>
                  <div data-field="weightOunces">
                    <DecimalField
                      label="Weight (oz)"
                      value={form.weightOunces}
                      onValueChange={(text) => update("weightOunces", text)}
                      onBlur={touch("weightOunces")}
                      error={errorFor("weightOunces")}
                      placeholder="e.g. 8"
                    />
                  </div>
                  <p className="hd-form-pair__hint">
                    Fill in either or both. Saved to the nearest 0.1 oz.
                  </p>
                </div>
              </div>
              <div className="hd-form-grid hd-form-grid--4">
                <div data-field="capacity">
                  <TextField
                    label="Capacity"
                    inputMode="numeric"
                    autoComplete="off"
                    value={form.capacity}
                    onChange={(e) => update("capacity", e.target.value.replace(/\D/g, ""))}
                    onBlur={touch("capacity")}
                    error={errorFor("capacity")}
                    hint="Rounds in the magazine, cylinder or tube."
                  />
                </div>
                <div className="hd-form-span-2">
                  <TextField
                    label="Finish"
                    value={form.finish}
                    onChange={(e) => update("finish", e.target.value)}
                    placeholder="e.g. Blued, Cerakote"
                    hint="Searchable."
                  />
                </div>
                <Select
                  label="Condition"
                  value={form.condition || NOT_RECORDED}
                  onValueChange={(value) =>
                    update("condition", value === NOT_RECORDED ? "" : (value as Condition))
                  }
                  options={[{ value: NOT_RECORDED, label: "Not recorded" }, ...CONDITION_OPTIONS]}
                />
              </div>
            </Disclosure>
          </section>

          <section className="hd-form-section" aria-labelledby="ff-value">
            <h3 className="hd-form-section__title" id="ff-value">
              Value
            </h3>
            <div data-field="estimatedValue">
              <MoneyField
                label="Estimated replacement value"
                value={form.estimatedValue}
                onValueChange={(text) => update("estimatedValue", text)}
                onBlur={touch("estimatedValue")}
                error={errorFor("estimatedValue")}
                fieldClassName="hd-field--quarter"
                hint="What it would cost to replace today. Used to check insurance coverage."
              />
            </div>
          </section>

          <section className="hd-form-section" aria-labelledby="ff-acquisition">
            <h3 className="hd-form-section__title" id="ff-acquisition">
              Acquisition
            </h3>
            <div className="hd-form-grid hd-form-grid--4">
              <div className="hd-form-span-2">
                <TextField
                  label="Acquired from"
                  value={form.acquisitionSource}
                  onChange={(e) => update("acquisitionSource", e.target.value)}
                  placeholder="Seller, shop, or person"
                />
              </div>
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

          <section
            className="hd-form-section"
            aria-labelledby="ff-condition"
            data-highlight={highlight ?? undefined}
          >
            <h3 className="hd-form-section__title" id="ff-condition">
              Condition and accessories
            </h3>
            <div className="hd-form-grid hd-form-grid--2">
              <TextArea
                ref={notesRef}
                label="Notes"
                value={form.notes}
                onChange={(e) => update("notes", e.target.value)}
                hint="Condition, markings, repairs — anything worth recording. All of it is searchable."
                rows={4}
              />
              <TextArea
                ref={accessoriesRef}
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
              <div className="hd-form-grid hd-form-grid--4">
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
                <div data-field="dispositionRecipient" className="hd-form-span-3">
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

      <OriginGuide open={showOriginGuide} onOpenChange={setShowOriginGuide} />

      <ConfirmDialog
        open={pendingOrigin !== null}
        onOpenChange={(open) => !open && setPendingOrigin(null)}
        title={
          pendingOrigin?.discard === "country"
            ? "Discard the country of manufacture?"
            : "Discard importer and original marks?"
        }
        description={pendingOrigin && discardedValuesDescription()}
        confirmLabel="Discard and change"
        onConfirm={confirmOriginChange}
      />

      <ConfirmDialog
        open={pendingWarning !== null}
        onOpenChange={(open) => !open && setPendingWarning(null)}
        title="Another firearm has the same original marks"
        description={pendingWarning?.message}
        confirmLabel="Save anyway"
        destructive={false}
        onConfirm={async () => {
          if (!pendingWarning) return;
          await submitInput(pendingWarning.input, true);
          setPendingWarning(null);
        }}
      />
    </>
  );
}
