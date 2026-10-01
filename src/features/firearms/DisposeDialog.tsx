import { useId, useState } from "react";
import type { FormEvent } from "react";
import {
  Button,
  ChoiceCards,
  DateField,
  Dialog,
  MoneyField,
  SegmentedControl,
  TextField,
} from "../../components";
import { dispositionOrderError, futureDateError, parseDateInput, todayIso } from "../../lib/dates";
import { parseDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { MountedList } from "../mounts/MountedList";
import { recordKey } from "../mounts/recordKey";
import { recordNameWithType } from "../mounts/recordNames";
import type { MountDetail, MountedEntry } from "../mounts/types";
import { resumedValues, useDirtyForm, useResumedDraftTaken } from "../session/usePendingDraft";
import type { DraftTarget } from "../session/usePendingDraft";
import { useRecordSubject } from "./recordSubject";
import type { RecordSubject, RecordSubjectProps } from "./recordSubject";
import { DISPOSITION_TYPE_OPTIONS } from "./types";
import type { DisposeInput, DispositionType } from "./types";
import "./forms.css";

const RECIPIENT_HINTS: Record<DispositionType, string> = {
  sold: "The buyer's name, or the dealer who took it.",
  traded: "Who you traded with.",
  gifted: "Who received it.",
  destroyed: "Who destroyed it, or how it was destroyed.",
  lost_stolen: "Where it was lost, or the police report number.",
};

export type DisposeDialogProps = RecordSubjectProps & {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onDispose: (input: DisposeInput) => Promise<void>;
  /** What the record is mounted on and what is mounted on it (FR-014); none
   * when the page has not loaded it. */
  mount?: MountDetail;
  /** Called when the backend reports that what is mounted has changed, so the
   * page reloads `mount`. */
  onMountChanged?: () => void | Promise<void>;
};

/** Marks a firearm or an accessory disposed (US1 Scenario 4; 006 FR-006). The
 * record keeps its full history but leaves the active collection, its totals,
 * and coverage checks (FR-023, FR-025). */
export function DisposeDialog({
  open,
  onOpenChange,
  onDispose,
  mount,
  onMountChanged,
  ...record
}: DisposeDialogProps) {
  const subject = useRecordSubject(record as RecordSubjectProps);
  const host = mount?.chain[0];
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Mark as disposed"
      description={
        <>
          {`${subject.name} stays in your records with its full history, but leaves the active collection, its value totals, and coverage checks.`}
          {host && (
            <span className="hd-dialog__note">
              {`${subject.name} will be unmounted from ${recordNameWithType(host)}.`}
            </span>
          )}
        </>
      }
      bare
    >
      {/* Mounted only while open, so every opening starts from a blank form. */}
      <DisposeForm
        subject={subject}
        mounted={mount?.mounted ?? []}
        onMountChanged={onMountChanged}
        onDispose={onDispose}
        onCancel={() => onOpenChange(false)}
      />
    </Dialog>
  );
}

/** The version of this form's kept drafts (research.md §16). Raise it when
 * {@link DisposeValues} changes shape, so older drafts are only discarded. */
export const FORM_VERSION = 2;

/** The form's input, as a kept draft holds it. */
interface DisposeValues {
  dispositionType: DispositionType | "";
  recipient: string;
  date: string;
  price: string;
  /** The records disposed with it, by `recordKey`, each with the price typed
   * for it (blank: none). A record not here is kept (FR-014). */
  disposeWith: Record<string, string>;
}

function DisposeForm({
  subject,
  mounted,
  onMountChanged,
  onDispose,
  onCancel,
}: {
  subject: RecordSubject;
  mounted: MountedEntry[];
  onMountChanged: (() => void | Promise<void>) | undefined;
  onDispose: (input: DisposeInput) => Promise<void>;
  onCancel: () => void;
}) {
  const target: DraftTarget = {
    formVersion: FORM_VERSION,
    kind: subject.kind,
    mode: "dispose",
    targetId: subject.id,
  };
  const label = `${subject.name} (disposal)`;
  // Pending changes the user resumed start as unsaved input (FR-039).
  const [today] = useState(todayIso);
  const [resumed] = useState(() =>
    resumedValues<DisposeValues>(target, {
      dispositionType: "",
      recipient: "",
      date: today,
      price: "",
      disposeWith: {},
    }),
  );
  useResumedDraftTaken(target);
  const [dispositionType, setDispositionType] = useState<DispositionType | "">(
    resumed.dispositionType,
  );
  const [recipient, setRecipient] = useState(resumed.recipient);
  const [date, setDate] = useState(resumed.date);
  const [price, setPrice] = useState(resumed.price);
  // A resumed choice for a record no longer below this one is dropped.
  const [disposeWith, setDisposeWith] = useState<Record<string, string>>(() => {
    const here = new Set(mounted.map((entry) => recordKey(entry.label.record)));
    return Object.fromEntries(
      Object.entries(resumed.disposeWith).filter(
        ([key, text]) => here.has(key) && typeof text === "string",
      ),
    );
  });
  const [submitted, setSubmitted] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [serverError, setServerError] = useState<string | null>(null);

  const parsedDate = parseDateInput(date);
  const parsedPrice = parseDollars(price);
  const errors = {
    dispositionType: dispositionType ? undefined : "Choose what happened to it.",
    recipient: recipient.trim() ? undefined : "Enter who received it.",
    date: !parsedDate.ok
      ? parsedDate.error
      : !parsedDate.iso
        ? "Enter the date."
        : (futureDateError(parsedDate.iso, "Disposition date") ??
          dispositionOrderError(subject.acquisitionDate, parsedDate.iso)),
    price: !parsedPrice.ok
      ? parsedPrice.error
      : parsedPrice.dollars == null
        ? "Enter the price, or 0 if nothing was received."
        : undefined,
  };
  const shown = (error: string | undefined) => (submitted ? error : undefined);

  // What is disposed with it, in the list's order. Only what is still below
  // it counts, since a reload can take records away.
  const disposed = mounted.filter((entry) => recordKey(entry.label.record) in disposeWith);
  const parsedWith = disposed.map((entry) => ({
    entry,
    price: parseDollars(disposeWith[recordKey(entry.label.record)]),
  }));
  const withErrors = new Map(
    parsedWith.flatMap(({ entry, price: parsed }) =>
      parsed.ok ? [] : [[recordKey(entry.label.record), parsed.error] as const],
    ),
  );
  const kept = mounted.filter((entry) => !(recordKey(entry.label.record) in disposeWith));
  const disposedKeys = new Set(disposed.map((entry) => recordKey(entry.label.record)));
  const hostIsDisposed = (entry: MountedEntry) => disposedKeys.has(recordKey(entry.host));
  const statements = [
    kept.some((entry) => entry.depth <= 1) &&
      `Kept records mounted on ${subject.name} will be unmounted.`,
    kept.some((entry) => entry.depth > 1 && hostIsDisposed(entry)) &&
      "Kept records mounted on a record disposed with it will be unmounted.",
    kept.some((entry) => entry.depth > 1 && !hostIsDisposed(entry)) &&
      "Records kept with what they are mounted on stay mounted.",
  ].filter((text): text is string => Boolean(text));

  // Closing or quitting asks about unsaved input first (specs/003 FR-010),
  // and a lock keeps it (FR-039).
  const values: DisposeValues = { dispositionType, recipient, date, price, disposeWith };
  useDirtyForm({
    label,
    isDirty:
      dispositionType !== "" ||
      recipient !== "" ||
      date !== today ||
      price !== "" ||
      Object.keys(disposeWith).length > 0,
    submit: save,
    draft: { ...target, values },
  });

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    void save();
  }

  /** Validates and saves; resolves whether it was saved. */
  async function save(): Promise<boolean> {
    setSubmitted(true);
    if (
      Object.values(errors).some(Boolean) ||
      !parsedDate.ok ||
      !parsedPrice.ok ||
      withErrors.size > 0
    ) {
      return false;
    }

    setSubmitting(true);
    setServerError(null);
    try {
      await onDispose({
        dispositionType: dispositionType as DispositionType,
        recipient: recipient.trim(),
        date: parsedDate.iso as string,
        price: parsedPrice.dollars as number,
        withMounted: parsedWith.map(({ entry, price: parsed }) => ({
          record: entry.label.record,
          price: parsed.ok ? parsed.dollars : null,
        })),
      });
      return true;
    } catch (e) {
      // What is mounted changed under the dialog (FR-014): say so, and have the
      // page reload the list.
      const stale = e instanceof CommandFailure ? e.fieldErrors?.withMounted : undefined;
      if (stale) void onMountChanged?.();
      setServerError(
        stale ??
          (e instanceof CommandFailure
            ? e.message
            : `The ${subject.noun} couldn't be marked disposed.`),
      );
      return false;
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form className="hd-dialog__form" onSubmit={handleSubmit} noValidate>
      <div className="hd-dialog__body hd-form-section">
        {serverError && (
          <p className="hd-banner hd-banner--error" role="alert">
            {serverError}
          </p>
        )}
        <ChoiceCards
          label="What happened"
          required
          value={dispositionType}
          onChange={setDispositionType}
          options={DISPOSITION_TYPE_OPTIONS}
          error={shown(errors.dispositionType)}
          minCardWidth={96}
        />
        <TextField
          label="Transferred to"
          required
          value={recipient}
          onChange={(e) => setRecipient(e.target.value)}
          hint={dispositionType ? RECIPIENT_HINTS[dispositionType] : undefined}
          error={shown(errors.recipient)}
        />
        <div className="hd-form-grid hd-form-grid--3 hd-form-grid--short">
          <DateField
            label="Date"
            required
            value={date}
            max={todayIso()}
            onValueChange={setDate}
            error={shown(errors.date)}
          />
          <MoneyField
            label="Price received"
            required
            value={price}
            onValueChange={setPrice}
            hint="Enter 0 if nothing was received."
            error={shown(errors.price)}
          />
        </div>
        {mounted.length > 0 && (
          <MountedChoices
            mounted={mounted}
            disposeWith={disposeWith}
            onChange={setDisposeWith}
            errors={submitted ? withErrors : undefined}
            statements={statements}
          />
        )}
      </div>
      <footer className="hd-dialog__footer">
        <Button variant="secondary" onClick={onCancel} disabled={submitting}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" pending={submitting}>
          Mark as disposed
        </Button>
      </footer>
    </form>
  );
}

const CHOICES = [
  { value: "keep", label: "Keep" },
  { value: "dispose", label: "Dispose with it" },
] as const;

/** The **Mounted** group (contracts/ui-accessories.md §7): everything below
 * the record, each with a Keep | Dispose with it choice and, for a record
 * disposed with it, an optional price. Below it, what happens to the rest. */
function MountedChoices({
  mounted,
  disposeWith,
  onChange,
  errors,
  statements,
}: {
  mounted: MountedEntry[];
  disposeWith: Record<string, string>;
  onChange: (next: Record<string, string>) => void;
  /** Price errors by `recordKey`, once the form has been submitted. */
  errors: Map<string, string> | undefined;
  statements: string[];
}) {
  const titleId = useId();
  return (
    <div className="hd-dispose-mounted" role="group" aria-labelledby={titleId}>
      <h3 className="hd-form-section__title" id={titleId}>
        Mounted
      </h3>
      <MountedList
        entries={mounted}
        links={false}
        detail={(entry) => {
          const key = recordKey(entry.label.record);
          const name = recordNameWithType(entry.label);
          const disposing = key in disposeWith;
          return (
            <div className="hd-mounted__choice">
              <SegmentedControl
                label={name}
                hideLabel
                size="sm"
                value={disposing ? "dispose" : "keep"}
                options={[...CHOICES]}
                onChange={(choice) => {
                  const next = { ...disposeWith };
                  if (choice === "dispose") next[key] = disposeWith[key] ?? "";
                  else delete next[key];
                  onChange(next);
                }}
              />
              {disposing && (
                <MoneyField
                  label={`Price for ${name}`}
                  fieldClassName="hd-field--third"
                  value={disposeWith[key]}
                  onValueChange={(text) => onChange({ ...disposeWith, [key]: text })}
                  hint="Leave blank if none was received separately."
                  error={errors?.get(key)}
                />
              )}
            </div>
          );
        }}
      />
      {statements.map((text) => (
        <p className="hd-dispose-mounted__note" key={text}>
          {text}
        </p>
      ))}
    </div>
  );
}
