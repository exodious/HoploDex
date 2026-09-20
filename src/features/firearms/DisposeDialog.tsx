import { useState } from "react";
import type { FormEvent } from "react";
import { Button, ChoiceCards, DateField, Dialog, MoneyField, TextField } from "../../components";
import { dispositionOrderError, futureDateError, parseDateInput, todayIso } from "../../lib/dates";
import { parseDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { firearmName } from "../app/collectionStore";
import { DISPOSITION_TYPE_OPTIONS } from "./types";
import type { DisposeFirearmInput, DispositionType, Firearm } from "./types";
import "./forms.css";

const RECIPIENT_HINTS: Record<DispositionType, string> = {
  sold: "The buyer's name, or the dealer who took it.",
  traded: "Who you traded with.",
  gifted: "Who received it.",
  destroyed: "Who destroyed it, or how it was destroyed.",
  lost_stolen: "Where it was lost, or the police report number.",
};

export interface DisposeDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  firearm: Firearm;
  onDispose: (input: DisposeFirearmInput) => Promise<void>;
}

/** Marks a firearm disposed (US1 Scenario 4). The record keeps its full
 * history but leaves the active collection, its totals, and coverage
 * checks (FR-023, FR-025). */
export function DisposeDialog({ open, onOpenChange, firearm, onDispose }: DisposeDialogProps) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Mark as disposed"
      description={`${firearmName(firearm)} stays in your records with its full history, but leaves the active collection, its value totals, and coverage checks.`}
      bare
    >
      {/* Mounted only while open, so every opening starts from a blank form. */}
      <DisposeForm
        acquisitionDate={firearm.acquisitionDate}
        onDispose={onDispose}
        onCancel={() => onOpenChange(false)}
      />
    </Dialog>
  );
}

function DisposeForm({
  acquisitionDate,
  onDispose,
  onCancel,
}: {
  acquisitionDate: string | null;
  onDispose: (input: DisposeFirearmInput) => Promise<void>;
  onCancel: () => void;
}) {
  const [dispositionType, setDispositionType] = useState<DispositionType | "">("");
  const [recipient, setRecipient] = useState("");
  const [date, setDate] = useState(todayIso());
  const [price, setPrice] = useState("");
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
          dispositionOrderError(acquisitionDate, parsedDate.iso)),
    price: !parsedPrice.ok
      ? parsedPrice.error
      : parsedPrice.cents == null
        ? "Enter the price, or 0 if nothing was received."
        : undefined,
  };
  const shown = (error: string | undefined) => (submitted ? error : undefined);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setSubmitted(true);
    if (Object.values(errors).some(Boolean) || !parsedDate.ok || !parsedPrice.ok) return;

    setSubmitting(true);
    setServerError(null);
    try {
      await onDispose({
        dispositionType: dispositionType as DispositionType,
        recipient: recipient.trim(),
        date: parsedDate.iso as string,
        price: parsedPrice.cents as number,
      });
    } catch (e) {
      setServerError(
        e instanceof CommandFailure ? e.message : "The firearm couldn't be marked disposed.",
      );
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
          label="What happened?"
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
        <div className="hd-form-grid hd-form-grid--2">
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
