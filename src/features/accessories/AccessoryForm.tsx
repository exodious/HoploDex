import { useRef, useState } from "react";
import type { FormEvent } from "react";
import { flushSync } from "react-dom";
import { Button, DateField, MoneyField, Select, TextArea, TextField } from "../../components";
import { futureDateError, parseDateInput, todayIso } from "../../lib/dates";
import { dollarsToInput, parseDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { useAccessoryKinds } from "../app/collectionStore";
import { caliberHint, caliberReducer } from "../firearms/caliberDerivation";
import type { CaliberAction, CaliberMode, CaliberState } from "../firearms/caliberDerivation";
import { EntryField } from "../firearms/EntryField";
import { entryTextError, snapNote } from "../firearms/entryText";
import { settleEntry } from "../firearms/firearmsService";
import type { DerivedCaliber } from "../firearms/types";
import "../firearms/forms.css";
import { accessoryNameText } from "../mounts/recordNames";
import { resumedValues, useDirtyForm, useResumedDraftTaken } from "../session/usePendingDraft";
import type { DraftTarget } from "../session/usePendingDraft";
import type { Accessory, AccessoryInput } from "./types";

// specs/006-accessory-links contracts/ui-accessories.md §3 (FR-001 to FR-005,
// FR-027): the accessory form, laid out as the firearm form is. The Mounted
// on row is User Story 2's.

export interface AccessoryFormProps {
  /** Present in edit mode; omitted when creating a new record. */
  initialValues?: Accessory;
  onSubmit: (input: AccessoryInput) => Promise<void>;
  onCancel?: () => void;
}

/** The version of this form's kept drafts (specs/003 research.md §16). Raise
 * it when `FormState` changes shape, so older drafts are only discarded. */
export const FORM_VERSION = 1;

/** FR-002: shown under Kind. */
const KIND_HINT = "A suppressor is recorded as a firearm.";

/** FR-005: shown under Estimated value. */
const VALUE_HINT =
  "The value of this record as a whole, everything it describes included. Value each record on its own.";

/** Under Kind when `list_accessory_kinds` failed: it has nothing to offer,
 * and the user should know why rather than see an empty choice. */
const KIND_LIST_FAILED =
  "The list of kinds couldn't be loaded, so none can be chosen. Restart HoploDex to try again.";

/** The fields with suggestions and snapping (FR-003: the same entry commands
 * as the firearm form's). */
type AccessoryEntryField = "make" | "model" | "cartridge" | "caliber";

interface FormState {
  /** A kind id; "" is none chosen. */
  accessoryKindId: string;
  make: string;
  model: string;
  caliber: string;
  /** The caliber's state beside its text (specs/004 research.md §8), as
   * strings so a kept draft carries them; "" is none. */
  caliberMode: CaliberMode;
  caliberSource: "" | DerivedCaliber["source"];
  caliberSuggestion: string;
  caliberPrompt: string;
  cartridge: string;
  serialNumber: string;
  notes: string;
  estimatedValue: string;
  acquisitionSource: string;
  acquisitionDate: string;
  acquisitionPrice: string;
}

type Field = keyof FormState;

function toFormState(accessory?: Accessory): FormState {
  return {
    accessoryKindId: accessory ? String(accessory.accessoryKindId) : "",
    make: accessory?.make ?? "",
    model: accessory?.model ?? "",
    caliber: accessory?.caliber ?? "",
    // specs/004 FR-006: a saved record's caliber counts as already edited.
    caliberMode: accessory ? "edited" : "derived",
    caliberSource: "",
    caliberSuggestion: "",
    caliberPrompt: "",
    cartridge: accessory?.cartridge ?? "",
    serialNumber: accessory?.serialNumber ?? "",
    notes: accessory?.notes ?? "",
    estimatedValue: dollarsToInput(accessory?.estimatedValue ?? null),
    acquisitionSource: accessory?.acquisitionSource ?? "",
    acquisitionDate: accessory?.acquisitionDate ?? "",
    acquisitionPrice: dollarsToInput(accessory?.acquisitionPrice ?? null),
  };
}

function blankToNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed === "" ? null : trimmed;
}

/** Client-side mirror of the backend's validation, so the user gets
 * immediate feedback instead of a round-trip error. */
function validate(form: FormState): Partial<Record<Field, string>> {
  const errors: Partial<Record<Field, string>> = {};
  if (form.accessoryKindId === "") errors.accessoryKindId = "Choose a kind.";
  for (const [field, label] of [
    ["make", "Make"],
    ["model", "Model"],
    ["cartridge", "Cartridge"],
    ["caliber", "Caliber"],
  ] as const) {
    const problem = entryTextError(label, form[field]);
    if (problem) errors[field] = problem;
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
  return errors;
}

function dollars(text: string): number | null {
  const parsed = parseDollars(text);
  return parsed.ok ? parsed.dollars : null;
}

function isoDate(text: string): string | null {
  const parsed = parseDateInput(text);
  return parsed.ok ? parsed.iso : null;
}

/** Order used to focus the first invalid field after a failed submit. */
const FIELD_ORDER: Field[] = [
  "accessoryKindId",
  "make",
  "model",
  "cartridge",
  "caliber",
  "serialNumber",
  "estimatedValue",
  "acquisitionDate",
  "acquisitionPrice",
];

function isEntryField(name: string | undefined): name is AccessoryEntryField {
  return name === "make" || name === "model" || name === "cartridge" || name === "caliber";
}

/** Create/edit form for an accessory's record (US1). Insurance coverage and
 * the disposition are set from the record's own panel and dialogs, so a save
 * here carries them over unchanged. Renders its own dialog body and footer
 * (use inside `<Dialog bare>`). */
export function AccessoryForm({ initialValues, onSubmit, onCancel }: AccessoryFormProps) {
  const kinds = useAccessoryKinds();
  const target: DraftTarget = {
    formVersion: FORM_VERSION,
    kind: "accessory",
    mode: initialValues ? "edit" : "add",
    targetId: initialValues?.id ?? null,
  };
  // Pending changes the user resumed start as unsaved input (specs/003 FR-039).
  const [form, setForm] = useState<FormState>(() =>
    resumedValues(target, toFormState(initialValues)),
  );
  useResumedDraftTaken(target);
  const [pristine] = useState(() => toFormState(initialValues));
  // The latest form, for a settle response to check the field still holds
  // what was sent.
  const latestForm = useRef(form);
  latestForm.current = form;
  // What each entry field was last settled to (a saved value counts as
  // settled, specs/004 FR-014), the cartridge and caliber it derived, and
  // the settles still in flight.
  const lastSettled = useRef<Record<AccessoryEntryField, string>>({
    make: (initialValues?.make ?? "").trim(),
    model: (initialValues?.model ?? "").trim(),
    cartridge: (initialValues?.cartridge ?? "").trim(),
    caliber: (initialValues?.caliber ?? "").trim(),
  });
  const derivedFrom = useRef<CaliberState["derivedFrom"]>(null);
  const settling = useRef(new Set<Promise<boolean>>());
  // specs/004 contracts/ui-entry.md §2: the note under a field that was
  // snapped, until the field is edited again.
  const [entryNotes, setEntryNotes] = useState<Partial<Record<AccessoryEntryField, string>>>({});
  const [touched, setTouched] = useState<Partial<Record<Field, boolean>>>({});
  const [submitted, setSubmitted] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [serverError, setServerError] = useState<CommandFailure | null>(null);
  const formRef = useRef<HTMLFormElement>(null);

  const clientErrors = validate(form);
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

  /** Runs the caliber reducer (specs/004 research.md §8) over the form's
   * caliber fields; an accessory always derives its caliber from the
   * cartridge. `derivedFrom` is set before a cartridge action. */
  function withCaliber(prev: FormState, action: CaliberAction): FormState {
    const next = caliberReducer(
      {
        caliber: prev.caliber,
        mode: prev.caliberMode,
        source: prev.caliberSource || null,
        suggestion: prev.caliberSuggestion || null,
        prompt: prev.caliberPrompt || null,
        derivedFrom: derivedFrom.current,
      },
      action,
      true,
    );
    return {
      ...prev,
      caliber: next.caliber,
      caliberMode: next.mode,
      caliberSource: next.source ?? "",
      caliberSuggestion: next.suggestion ?? "",
      caliberPrompt: next.prompt ?? "",
    };
  }

  function applyCaliber(action: CaliberAction) {
    setForm((prev) => withCaliber(prev, action));
  }

  /** specs/004 contracts/ui-entry.md §2–§3: settles one of the four fields,
   * when it was left or a suggestion was picked. Only text that changed since
   * it was last settled and, editing, differs from the saved value is sent
   * (FR-014); the answer is applied only if the field still holds what was
   * sent. Resolves whether the value was changed by snapping. A cartridge
   * also gets the caliber it derives. */
  function settle(field: AccessoryEntryField): Promise<boolean> {
    const text = latestForm.current[field];
    const trimmed = text.trim();
    if (trimmed === lastSettled.current[field]) return Promise.resolve(false);
    const previous = lastSettled.current[field];
    lastSettled.current[field] = trimmed;
    if (trimmed === "" || (initialValues && trimmed === (initialValues[field] ?? "").trim())) {
      // Cleared, or typed back to the saved value (never settled): a derived
      // caliber empties, an edited one loses its suggestion.
      if (field === "cartridge") {
        derivedFrom.current = null;
        if (trimmed === "" || latestForm.current.caliberMode === "edited") {
          applyCaliber({ type: "cartridgeCleared" });
        }
      }
      return Promise.resolve(false);
    }
    const run = (async () => {
      let settled;
      try {
        settled = await settleEntry(field, text);
      } catch {
        // Only guidance: saving still checks the field. Leaving it again
        // tries once more.
        lastSettled.current[field] = previous;
        return false;
      }
      if (latestForm.current[field] !== text) return false;
      if (field === "cartridge") {
        derivedFrom.current = { cartridge: settled.value, derived: settled.derivedCaliber };
        applyCaliber({
          type: "cartridgeSettled",
          cartridge: settled.value,
          derived: settled.derivedCaliber,
        });
      }
      if (settled.changedBy === null) return false;
      lastSettled.current[field] = settled.value.trim();
      const note = snapNote(settled.changedBy, settled.value);
      flushSync(() => {
        setEntryNotes((notes) => ({ ...notes, [field]: note }));
        if (field === "caliber") {
          applyCaliber({ type: "caliberTyped", caliber: settled.value });
        } else {
          update(field, settled.value);
        }
      });
      return true;
    })();
    settling.current.add(run);
    void run.finally(() => settling.current.delete(run));
    return run;
  }

  /** The user typed in an entry field: its note goes. */
  function editEntry(field: AccessoryEntryField, text: string) {
    setEntryNotes((notes) => (notes[field] ? { ...notes, [field]: undefined } : notes));
    if (field === "caliber") applyCaliber({ type: "caliberTyped", caliber: text });
    else update(field, text);
  }

  /** The user picked a suggestion: the field takes it and settles at once. */
  function pickEntry(field: AccessoryEntryField, value: string) {
    flushSync(() => editEntry(field, value));
    void settle(field);
  }

  function leaveEntry(field: AccessoryEntryField) {
    touch(field)();
    if (field === "caliber") applyCaliber({ type: "caliberLeft" });
    void settle(field);
  }

  const caliberState: CaliberState = {
    caliber: form.caliber,
    mode: form.caliberMode,
    source: form.caliberSource || null,
    suggestion: form.caliberSuggestion || null,
    prompt: form.caliberPrompt || null,
    derivedFrom: null,
  };
  const caliberGuessed = form.caliberMode === "derived" && form.caliberSource === "guess";

  // FR-002: the offered kinds in list order, plus the record's own when it
  // is no longer offered.
  const ownKindId = initialValues?.accessoryKindId;
  const kindOptions = kinds.kinds
    .filter((kind) => kind.offered || kind.id === ownKindId)
    .map((kind) => ({ value: String(kind.id), label: kind.name }));
  const kindHint =
    kinds.failed && !errorFor("accessoryKindId") ? `${KIND_HINT} ${KIND_LIST_FAILED}` : KIND_HINT;

  // Closing or quitting asks about unsaved input first (specs/003 FR-010),
  // and a lock keeps it (FR-039), labelled with the accessory's name (FR-027).
  const ownKindName = kinds.kinds.find((kind) => kind.id === ownKindId)?.name ?? "Accessory";
  useDirtyForm({
    label: initialValues
      ? `${accessoryNameText(initialValues.make, initialValues.model, ownKindName)} (edit)`
      : "New accessory",
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
    // specs/004 contracts/ui-entry.md §2: wait for any settle in flight, then
    // settle the field that has focus (Enter pressed inside it). If that
    // changes its value the note shows and nothing is saved: the user reviews
    // the change, then saves again.
    if (settling.current.size > 0) await Promise.all([...settling.current]);
    const focused = formRef.current?.contains(document.activeElement)
      ? document.activeElement?.closest<HTMLElement>("[data-field]")?.dataset.field
      : undefined;
    if (isEntryField(focused) && (await settle(focused))) return false;

    // What the form holds now, after any settle above changed it.
    const form = latestForm.current;
    const clientErrors = validate(form);
    setSubmitted(true);
    const firstInvalid = FIELD_ORDER.find((field) => clientErrors[field]);
    if (firstInvalid) {
      formRef.current
        ?.querySelector<HTMLElement>(`[data-field="${firstInvalid}"] :is(input, textarea, button)`)
        ?.focus();
      return false;
    }

    // Status, disposition, coverage and the mount are the record page's and
    // its dialogs' to change, so a save here keeps what the record has.
    const input: AccessoryInput = {
      accessoryKindId: Number(form.accessoryKindId),
      make: blankToNull(form.make),
      model: blankToNull(form.model),
      serialNumber: blankToNull(form.serialNumber),
      caliber: blankToNull(form.caliber),
      cartridge: blankToNull(form.cartridge),
      notes: blankToNull(form.notes),
      status: initialValues?.status ?? "active",
      estimatedValue: dollars(form.estimatedValue),
      acquisitionSource: blankToNull(form.acquisitionSource),
      acquisitionDate: isoDate(form.acquisitionDate),
      acquisitionPrice: dollars(form.acquisitionPrice),
      dispositionType: initialValues?.dispositionType ?? null,
      dispositionRecipient: initialValues?.dispositionRecipient ?? null,
      dispositionDate: initialValues?.dispositionDate ?? null,
      dispositionPrice: initialValues?.dispositionPrice ?? null,
      insurancePolicyId: initialValues?.insurancePolicyId ?? null,
      scheduledCoverageAmount: initialValues?.scheduledCoverageAmount ?? null,
      mountedOn: initialValues?.mountedOn ?? null,
    };
    return submitInput(input);
  }

  async function submitInput(input: AccessoryInput): Promise<boolean> {
    setSubmitting(true);
    setServerError(null);
    try {
      await onSubmit(input);
      return true;
    } catch (error) {
      if (error instanceof CommandFailure) {
        flushSync(() => setServerError(error));
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

        <section className="hd-form-section" aria-labelledby="af-identification">
          <h3 className="hd-form-section__title" id="af-identification">
            Identification
          </h3>
          <div data-field="accessoryKindId">
            <Select
              label="Kind"
              required
              fieldClassName="hd-field--third"
              value={form.accessoryKindId === "" ? undefined : form.accessoryKindId}
              onValueChange={(value) => {
                update("accessoryKindId", value);
                touch("accessoryKindId")();
              }}
              options={kindOptions}
              error={errorFor("accessoryKindId")}
              hint={kindHint}
            />
          </div>

          <div className="hd-form-grid hd-form-grid--2">
            <div data-field="make">
              <EntryField
                field="make"
                label="Make"
                value={form.make}
                onValueChange={(text) => editEntry("make", text)}
                onPick={(value) => pickEntry("make", value)}
                onLeave={() => leaveEntry("make")}
                note={entryNotes.make}
                error={errorFor("make")}
                placeholder="e.g. Leupold"
              />
            </div>
            <div data-field="model">
              <EntryField
                field="model"
                label="Model"
                make={form.make}
                value={form.model}
                onValueChange={(text) => editEntry("model", text)}
                onPick={(value) => pickEntry("model", value)}
                onLeave={() => leaveEntry("model")}
                note={entryNotes.model}
                error={errorFor("model")}
                placeholder="e.g. VX-5HD 3-15x44"
              />
            </div>
          </div>

          {/* specs/004 contracts/ui-entry.md §3: the cartridge, then the
              caliber it fills in, to its right. */}
          <div className="hd-form-grid hd-form-grid--2">
            <div data-field="cartridge">
              <EntryField
                field="cartridge"
                label="Cartridge"
                value={form.cartridge}
                onValueChange={(text) => editEntry("cartridge", text)}
                onPick={(value) => pickEntry("cartridge", value)}
                onLeave={() => leaveEntry("cartridge")}
                note={entryNotes.cartridge}
                error={errorFor("cartridge")}
                hint="Optional. The round it takes, e.g. 5.56x45mm NATO."
                placeholder="e.g. 5.56x45mm NATO"
              />
            </div>
            <div data-field="caliber" className="hd-form-stack">
              <EntryField
                field="caliber"
                id="af-caliber"
                label="Caliber"
                value={form.caliber}
                onValueChange={(text) => editEntry("caliber", text)}
                onPick={(value) => pickEntry("caliber", value)}
                onLeave={() => leaveEntry("caliber")}
                note={entryNotes.caliber}
                error={errorFor("caliber")}
                hint={caliberHint(caliberState)}
                placeholder="e.g. 5.56mm"
                trailing={
                  caliberGuessed && (
                    // specs/004 FR-005: marked as a guess in words, not by
                    // color alone.
                    <span className="hd-guess-tag" aria-describedby="af-caliber-hint">
                      Guess
                    </span>
                  )
                }
              />
              {form.caliberSuggestion && (
                <p className="hd-caliber-suggestion">
                  <span>The cartridge suggests “{form.caliberSuggestion}”.</span>
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => applyCaliber({ type: "suggestionUsed" })}
                  >
                    Use {form.caliberSuggestion}
                  </Button>
                </p>
              )}
            </div>
          </div>

          {/* FR-004: free text, no uniqueness, and no "no serial number" box. */}
          <div data-field="serialNumber">
            <TextField
              label="Serial number"
              className="hd-serial"
              fieldClassName="hd-field--third"
              value={form.serialNumber}
              onChange={(e) => update("serialNumber", e.target.value)}
              onBlur={touch("serialNumber")}
              error={errorFor("serialNumber")}
              spellCheck={false}
            />
          </div>
        </section>

        <section className="hd-form-section" aria-labelledby="af-value">
          <h3 className="hd-form-section__title" id="af-value">
            Value
          </h3>
          <div data-field="estimatedValue">
            <MoneyField
              label="Estimated value"
              value={form.estimatedValue}
              onValueChange={(text) => update("estimatedValue", text)}
              onBlur={touch("estimatedValue")}
              error={errorFor("estimatedValue")}
              fieldClassName="hd-field--quarter"
              hint={VALUE_HINT}
            />
          </div>
        </section>

        <section className="hd-form-section" aria-labelledby="af-acquisition">
          <h3 className="hd-form-section__title" id="af-acquisition">
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

        <section className="hd-form-section" aria-labelledby="af-condition">
          <h3 className="hd-form-section__title" id="af-condition">
            Condition and notes
          </h3>
          <TextArea
            label="Notes"
            value={form.notes}
            onChange={(e) => update("notes", e.target.value)}
            hint="Condition, markings, settings — anything worth recording. All of it is searchable."
            rows={4}
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
          Save
        </Button>
      </footer>
    </form>
  );
}
