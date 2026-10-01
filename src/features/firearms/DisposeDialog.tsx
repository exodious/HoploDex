import { useState } from "react";
import type { FormEvent } from "react";
import { Button, ChoiceCards, DateField, Dialog, MoneyField, TextField } from "../../components";
import { dispositionOrderError, futureDateError, parseDateInput, todayIso } from "../../lib/dates";
import { parseDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { resumedValues, useDirtyForm, useResumedDraftTaken } from "../session/usePendingDraft";
import type { DraftTarget } from "../session/usePendingDraft";
import { useRecordSubject } from "./recordSubject";
import type { RecordSubject, RecordSubjectProps } from "./recordSubject";
import { DISPOSITION_TYPE_OPTIONS } from "./types";
import type { DisposeFirearmInput, DispositionType } from "./types";
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
  onDispose: (input: DisposeFirearmInput) => Promise<void>;
};

/** Marks a firearm or an accessory disposed (US1 Scenario 4; 006 FR-006). The
 * record keeps its full history but leaves the active collection, its totals,
 * and coverage checks (FR-023, FR-025). */
export function DisposeDialog({ open, onOpenChange, onDispose, ...record }: DisposeDialogProps) {
  const subject = useRecordSubject(record as RecordSubjectProps);
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Mark as disposed"
      description={`${subject.name} stays in your records with its full history, but leaves the active collection, its value totals, and coverage checks.`}
      bare
    >
      {/* Mounted only while open, so every opening starts from a blank form. */}
      <DisposeForm subject={subject} onDispose={onDispose} onCancel={() => onOpenChange(false)} />
    </Dialog>
  );
}

/** The version of this form's kept drafts (research.md §16). Raise it when
 * {@link DisposeValues} changes shape, so older drafts are only discarded. */
export const FORM_VERSION = 1;

/** The form's input, as a kept draft holds it. */
interface DisposeValues {
  dispositionType: DispositionType | "";
  recipient: string;
  date: string;
  price: string;
}

function DisposeForm({
  subject,
  onDispose,
  onCancel,
}: {
  subject: RecordSubject;
  onDispose: (input: DisposeFirearmInput) => Promise<void>;
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
    }),
  );
  useResumedDraftTaken(target);
  const [dispositionType, setDispositionType] = useState<DispositionType | "">(
    resumed.dispositionType,
  );
  const [recipient, setRecipient] = useState(resumed.recipient);
  const [date, setDate] = useState(resumed.date);
  const [price, setPrice] = useState(resumed.price);
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

  // Closing or quitting asks about unsaved input first (specs/003 FR-010),
  // and a lock keeps it (FR-039).
  const values: DisposeValues = { dispositionType, recipient, date, price };
  useDirtyForm({
    label,
    isDirty: dispositionType !== "" || recipient !== "" || date !== today || price !== "",
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
    if (Object.values(errors).some(Boolean) || !parsedDate.ok || !parsedPrice.ok) return false;

    setSubmitting(true);
    setServerError(null);
    try {
      await onDispose({
        dispositionType: dispositionType as DispositionType,
        recipient: recipient.trim(),
        date: parsedDate.iso as string,
        price: parsedPrice.dollars as number,
      });
      return true;
    } catch (e) {
      setServerError(
        e instanceof CommandFailure
          ? e.message
          : `The ${subject.noun} couldn't be marked disposed.`,
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
